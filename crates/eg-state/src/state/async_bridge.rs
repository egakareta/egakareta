/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Async channel bridge (Phase 6a).
//!
//! The legacy `State` struct holds five `mpsc::Receiver`s that the
//! platform layer (auth service, audio service, level importer) and
//! background tasks write to. The production frame now drains those
//! receivers through `StateSchedule`.
//!
//! Phase 6a introduces **Bevy resources** that wrap each receiver
//! (`AuthChannelRes`, `AudioImportChannelRes`, `WaveformLoadChannelRes`,
//! `AudioPreloadChannelRes`, `LevelImportChannelRes`) plus a
//! `PollChannelsSet` system set with a `poll_channels_system` that
//! drains every receiver once per frame and emits the corresponding
//! result event into the World (Phase 6a's five new
//! [`async_results`](crate::events::async_results) message types).
//!
//! `StateSchedule` is the sole owner for these drains.
//!
//! ## Why a `Mutex<Receiver<T>>` wrapper?
//!
//! `mpsc::Receiver<T>` is `Send` but not `Sync` (its API requires
//! `&mut self`). Bevy `Resource` requires `Send + Sync + 'static`.
//! The wrapper wraps the receiver in a `Mutex` (which is `Sync`)
//! so the receiver can sit in the World and be accessed via
//! `ResMut<T>` from a single system. The `Mutex` is uncontended in
//! practice — only the `poll_channels_system` ever locks it, and
//! it runs at the top of the frame before any handler system.
//!
//! ## Phase 6b: result handler systems
//!
//! Phase 6a established the `poll_channels_system` that drains the
//! 5 receivers into 5 distinct result events. Phase 6b added one
//! handler system per channel. All five handlers mutate App-world
//! resources directly. Shared editor and runtime audio-cache resources
//! let remaining compatibility playback code read the same authoritative
//! cache without copying it back into `State`.
//!
//! ## Schedule graph (post Phase 6b)
//!
//! ```text
//!   PollChannelsSet
//!     poll_channels_system             // drain receivers → emit result events
//!   CommandHandlerSet
//!     events::handle_* (per-message)   // Phase 3 message drain
//!   ApplyAsyncResultsSet
//!     auth_handler_system              // AuthServiceResultEvent
//!     audio_import_handler_system      // AudioImportResultEvent
//!     waveform_handler_system          // WaveformResultEvent → timing + audio caches
//!     audio_preload_handler_system     // AudioPreloadResultEvent
//!     level_import_handler_system      // LevelImportResultEvent → editor/session resources
//! ```
//!
//! REFACTOR POINT(full-bevy): the `mpsc` channels disappear in the
//! full-Bevy migration; background tasks write directly into
//! `Messages<T>` or `AssetEvent<T>`. The `poll_channels_system` is
//! replaced by `bevy_tasks` async event streams. Handler systems already
//! mutate Bevy resources directly.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;

use eg_audio::service::{AudioPreloadResult, WaveformLoadMessage, WaveformResult};
use eg_platform::services::AuthServiceMessage;

use crate::events::async_results::{
    AudioImportResultEvent, AudioPreloadResultEvent, AuthServiceResultEvent,
    LevelImportResultEvent, WaveformResultEvent,
};
use crate::state::{
    AuthStateRes, EditorAudioCacheRes, EditorCameraRes, EditorHistoryRes, EditorInputRes,
    EditorObjectsRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTimingState,
    EditorTriggerRes, RuntimeAudioPreloadRes, SessionStateRes, WaveformLoadSenderRes,
};

// ---------------------------------------------------------------------------
// Type aliases for the shared `Arc<Mutex<Receiver<T>>>` channels
// ---------------------------------------------------------------------------

/// Shared `Arc<Mutex<Receiver<AuthServiceMessage>>>` held by both the
/// legacy `State::auth.channel.1` field and the World's
/// `AuthChannelRes` resource.
pub type SharedAuthReceiver = std::sync::Arc<Mutex<Receiver<AuthServiceMessage>>>;

/// Shared `Arc<Mutex<Receiver<(String, Vec<u8>)>>>`.
pub type SharedAudioImportReceiver = std::sync::Arc<Mutex<Receiver<(String, Vec<u8>)>>>;

/// Shared `Arc<Mutex<Receiver<WaveformResult>>>`.
pub type SharedWaveformReceiver = std::sync::Arc<Mutex<Receiver<WaveformResult>>>;

/// Shared `Arc<Mutex<Receiver<AudioPreloadResult>>>`.
pub type SharedAudioPreloadReceiver = std::sync::Arc<Mutex<Receiver<AudioPreloadResult>>>;

/// Shared `Arc<Mutex<Receiver<Vec<u8>>>>`.
pub type SharedLevelImportReceiver = std::sync::Arc<Mutex<Receiver<Vec<u8>>>>;

// ---------------------------------------------------------------------------
// Channel wrapper resources
// ---------------------------------------------------------------------------

/// Bevy `Resource` wrapper around the auth-service `mpsc::Receiver`.
///
/// Holds the shared `Arc<Mutex<Receiver<T>>>` drained by the World's
/// `poll_channels_system` in production. The `Arc<Mutex<>>` is
/// constructed by `State::lifecycle` and shared with the World via
/// `install_async_bridge_from_state_arc`; direct legacy helper calls can
/// still use the same receiver in focused tests.
#[allow(dead_code)]
#[derive(Resource)]
pub struct AuthChannelRes(pub SharedAuthReceiver);

/// Bevy `Resource` wrapper around the audio-import `mpsc::Receiver`.
///
/// Phase 6b: shared `Arc<Mutex<Receiver<T>>>` (see
/// [`AuthChannelRes`] for the rationale).
#[allow(dead_code)]
#[derive(Resource)]
pub struct AudioImportChannelRes(pub SharedAudioImportReceiver);

/// Bevy `Resource` wrapper around the waveform-load `mpsc::Receiver`.
///
/// Phase 6b: shared `Arc<Mutex<Receiver<T>>>`.
#[allow(dead_code)]
#[derive(Resource)]
pub struct WaveformLoadChannelRes(pub SharedWaveformReceiver);

/// Bevy `Resource` wrapper around the audio-preload `mpsc::Receiver`.
///
/// Phase 6b: shared `Arc<Mutex<Receiver<T>>>`.
#[allow(dead_code)]
#[derive(Resource)]
pub struct AudioPreloadChannelRes(pub SharedAudioPreloadReceiver);

/// Bevy `Resource` wrapper around the level-import `mpsc::Receiver`.
///
/// Phase 6b: shared `Arc<Mutex<Receiver<T>>>`.
#[allow(dead_code)]
#[derive(Resource)]
pub struct LevelImportChannelRes(pub SharedLevelImportReceiver);

// ---------------------------------------------------------------------------
// System set
// ---------------------------------------------------------------------------

/// System set that contains the per-frame channel poll systems.
///
/// Runs at the top of the frame so the result events are available
/// to all handler systems that follow. The same set runs at the
/// bottom of the frame too (in `apply_async_results_system`,
/// Phase 6b) so the events have a chance to be consumed by the
/// `CommandHandlerSet` / `EditorLogicSet` systems before the frame
/// ends.
///
/// In a future full-Bevy frame the order is:
/// `PollChannelsSet → ModeChangeSet → PreCommandHistorySet →
/// CommandHandlerSet → SimulationSet → ApplyAsyncResultsSet →
/// RebuildMeshSet → ExtractSet → RenderSet` (see the per-frame
/// schedule graph in `state/sets.rs`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PollChannelsSet;

// ---------------------------------------------------------------------------
// Poll system
// ---------------------------------------------------------------------------

/// Poll every `mpsc::Receiver` once and emit the corresponding
/// result event into the World.
///
/// Drains each channel in a `while let Ok(...) = try_recv()` loop so
/// a single frame consumes every message that's currently buffered.
/// Production frames drain channels here. Phase 6a emitted the events;
/// Phase 6b added handler systems, which now apply every result directly
/// to App-world resources.
#[allow(clippy::too_many_arguments)]
pub fn poll_channels_system(
    auth: Option<Res<AuthChannelRes>>,
    audio_import: Option<Res<AudioImportChannelRes>>,
    waveform: Option<Res<WaveformLoadChannelRes>>,
    audio_preload: Option<Res<AudioPreloadChannelRes>>,
    level_import: Option<Res<LevelImportChannelRes>>,
    mut auth_writer: MessageWriter<AuthServiceResultEvent>,
    mut audio_import_writer: MessageWriter<AudioImportResultEvent>,
    mut waveform_writer: MessageWriter<WaveformResultEvent>,
    mut audio_preload_writer: MessageWriter<AudioPreloadResultEvent>,
    mut level_import_writer: MessageWriter<LevelImportResultEvent>,
) {
    if let Some(res) = auth {
        drain_mutex(&res.0, |msg| {
            auth_writer.write(AuthServiceResultEvent::new(msg));
        });
    }
    if let Some(res) = audio_import {
        drain_mutex(&res.0, |msg| {
            let (filename, bytes) = msg;
            audio_import_writer.write(AudioImportResultEvent::new(filename, Some(bytes)));
        });
    }
    if let Some(res) = waveform {
        drain_mutex(&res.0, |msg| {
            waveform_writer.write(WaveformResultEvent::from_result(msg));
        });
    }
    if let Some(res) = audio_preload {
        drain_mutex(&res.0, |msg| {
            audio_preload_writer.write(AudioPreloadResultEvent::from_result(&msg));
        });
    }
    if let Some(res) = level_import {
        drain_mutex(&res.0, |msg| {
            level_import_writer.write(LevelImportResultEvent::new(msg));
        });
    }
}

/// Drain a `Receiver<T>` and forward each message through `emit`.
///
/// Stops on the first `Empty` error (no more messages buffered) and
/// ignores `Disconnected` (the sender was dropped — typical at
/// shutdown).
fn drain_one<T, F>(receiver: &mut Receiver<T>, mut emit: F)
where
    F: FnMut(T),
{
    loop {
        match receiver.try_recv() {
            Ok(message) => emit(message),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Drain a `Mutex<Receiver<T>>` (the wrapper we put in the World)
/// and forward each message through `emit`. The `MutexGuard` is
/// `DerefMut`-derefable to `Receiver<T>`, but `drain_one` takes
/// `&mut Receiver<T>` explicitly so the call site stays
/// type-checked.
fn drain_mutex<T, F>(mutex: &std::sync::Mutex<Receiver<T>>, mut emit: F)
where
    F: FnMut(T),
{
    let mut guard = mutex.lock().expect("async channel mutex poisoned");
    drain_one(&mut guard, &mut emit);
}

// ---------------------------------------------------------------------------
// Helpers used by `State` to wire the legacy channels into the resources
// ---------------------------------------------------------------------------

/// Install every async channel wrapper resource on a `World`.
///
/// Used by the runtime/test harness to populate the resources from the
/// legacy `State::auth.channel.1` /
/// `State::audio.state.editor.audio_import_channel.1` / etc. fields.
/// Production frames drain these resources through the schedule. The
/// legacy helper methods still share the same receiver when called
/// directly by tests, so each message is consumed at most once.
///
/// This function is a no-op when a resource is already installed
/// (e.g. by `register_state_async_bridge` during world construction).
#[allow(dead_code)]
pub fn install_async_bridge_from_state(
    world: &mut World,
    auth_receiver: Receiver<AuthServiceMessage>,
    audio_import_receiver: Receiver<(String, Vec<u8>)>,
    waveform_receiver: Receiver<WaveformResult>,
    audio_preload_receiver: Receiver<AudioPreloadResult>,
    level_import_receiver: Receiver<Vec<u8>>,
) {
    use std::sync::Arc;
    if !world.contains_resource::<AuthChannelRes>() {
        world.insert_resource(AuthChannelRes(Arc::new(Mutex::new(auth_receiver))));
    }
    if !world.contains_resource::<AudioImportChannelRes>() {
        world.insert_resource(AudioImportChannelRes(Arc::new(Mutex::new(
            audio_import_receiver,
        ))));
    }
    if !world.contains_resource::<WaveformLoadChannelRes>() {
        world.insert_resource(WaveformLoadChannelRes(Arc::new(Mutex::new(
            waveform_receiver,
        ))));
    }
    if !world.contains_resource::<AudioPreloadChannelRes>() {
        world.insert_resource(AudioPreloadChannelRes(Arc::new(Mutex::new(
            audio_preload_receiver,
        ))));
    }
    if !world.contains_resource::<LevelImportChannelRes>() {
        world.insert_resource(LevelImportChannelRes(Arc::new(Mutex::new(
            level_import_receiver,
        ))));
    }
}

/// Same as [`install_async_bridge_from_state`] but the receivers are
/// already wrapped in `Arc<Mutex<Receiver<T>>>`, which is the storage
/// used by `State.auth.channel.1`,
/// `State.audio.state.editor.audio_import_channel.1`, and the other
/// legacy channel fields. The resource stores a clone of the same
/// `Arc`, so the schedule drains the actual legacy receiver.
#[allow(dead_code)]
pub fn install_async_bridge_from_state_arc(
    world: &mut World,
    auth_receiver: SharedAuthReceiver,
    audio_import_receiver: SharedAudioImportReceiver,
    waveform_receiver: SharedWaveformReceiver,
    audio_preload_receiver: SharedAudioPreloadReceiver,
    level_import_receiver: SharedLevelImportReceiver,
) {
    if !world.contains_resource::<AuthChannelRes>() {
        world.insert_resource(AuthChannelRes(auth_receiver));
    }
    if !world.contains_resource::<AudioImportChannelRes>() {
        world.insert_resource(AudioImportChannelRes(audio_import_receiver));
    }
    if !world.contains_resource::<WaveformLoadChannelRes>() {
        world.insert_resource(WaveformLoadChannelRes(waveform_receiver));
    }
    if !world.contains_resource::<AudioPreloadChannelRes>() {
        world.insert_resource(AudioPreloadChannelRes(audio_preload_receiver));
    }
    if !world.contains_resource::<LevelImportChannelRes>() {
        world.insert_resource(LevelImportChannelRes(level_import_receiver));
    }
}

/// Register the async-bridge `Message<T>` resources + the
/// `PollChannelsSet` on an `App`.
///
/// This is the `App`-flavoured registration helper. The
/// `World`-flavoured equivalent is
/// [`register_state_async_bridge_world`]. See the
/// [`crate::state::plugins`] module for the schedule wiring.
#[allow(dead_code)]
pub fn register_state_async_bridge(app: &mut bevy_app::App) {
    use bevy_ecs::message::Messages;

    app.add_message::<AuthServiceResultEvent>()
        .add_message::<AudioImportResultEvent>()
        .add_message::<WaveformResultEvent>()
        .add_message::<AudioPreloadResultEvent>()
        .add_message::<LevelImportResultEvent>();

    // Channel wrappers are inserted in Phase 6b when the legacy
    // fields are torn out. For Phase 6a, the world has the
    // `Messages<T>` resources but the channel wrappers are
    // inserted lazily by `install_async_bridge_from_state_arc` (or
    // not at all in tests that don't install legacy channel resources).
    //
    // The `_ = Messages::<...>::default()` lines below silence the
    // "unused import" lint without forcing the messages to be
    // initialized (Bevy `add_message` does that).
    let _ = Messages::<AuthServiceResultEvent>::default;
    let _ = Messages::<AudioImportResultEvent>::default;
    let _ = Messages::<WaveformResultEvent>::default;
    let _ = Messages::<AudioPreloadResultEvent>::default;
    let _ = Messages::<LevelImportResultEvent>::default;
}

/// World-flavoured registration for callers that build a `World`
/// directly (the `State` constructor). Mirrors
/// [`register_state_async_bridge`] without going through an `App`.
#[allow(dead_code)]
pub fn register_state_async_bridge_world(world: &mut World) {
    use bevy_ecs::message::Messages;

    world.init_resource::<Messages<AuthServiceResultEvent>>();
    world.init_resource::<Messages<AudioImportResultEvent>>();
    world.init_resource::<Messages<WaveformResultEvent>>();
    world.init_resource::<Messages<AudioPreloadResultEvent>>();
    world.init_resource::<Messages<LevelImportResultEvent>>();
}

/// Add the `poll_channels_system` to a `Schedule`.
///
/// Callers (today: the `register_state_command_pipeline` helper) add
/// the system in a `.add_systems((...))` tuple inside the schedule.
/// The system is assigned to the [`PollChannelsSet`] so cross-set
/// ordering can be expressed via `.in_set(PollChannelsSet)`.
#[allow(dead_code)]
pub fn add_poll_channels_systems(schedule: &mut bevy_ecs::schedule::Schedule) {
    schedule.add_systems(poll_channels_system.in_set(PollChannelsSet));
}

// ---------------------------------------------------------------------------
// Phase 6b: result handler systems
// ---------------------------------------------------------------------------

/// System set for the result handler systems. Runs after
/// `PollChannelsSet` (the events must be emitted first) and after
/// `CommandHandlerSet` (the per-message drain systems must finish
/// first, so any per-frame command ordering is preserved).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApplyAsyncResultsSet;

/// Consume `AuthServiceResultEvent`s and apply them to App-world auth state.
/// Auth results never install a compatibility closure.
#[allow(dead_code)]
pub fn auth_handler_system(
    mut events: MessageReader<AuthServiceResultEvent>,
    mut auth: ResMut<AuthStateRes>,
) {
    for event in events.read() {
        let message = event.message.clone();
        match message {
            AuthServiceMessage::SignedIn(result) => auth.complete_auth_sign_in(result),
            AuthServiceMessage::Refreshed(result) => auth.complete_auth_refresh(result),
            AuthServiceMessage::SignedOut(result) => auth.complete_auth_sign_out(result),
        }
    }
}

/// Consume `AudioImportResultEvent`s, update the App-owned editor audio cache,
/// reset waveform presentation, and start decoding through the App-owned sender.
#[allow(dead_code)]
pub fn audio_import_handler_system(
    mut events: MessageReader<AudioImportResultEvent>,
    session: Res<SessionStateRes>,
    cache: Res<EditorAudioCacheRes>,
    waveform_sender: Res<WaveformLoadSenderRes>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        let Some(bytes) = event.bytes.clone() else {
            continue;
        };
        let level_name = session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let source_key = cache.insert_level_audio(&level_name, bytes.clone());
        cache.remove_waveform(&source_key);
        cache.set_loading_source(Some(source_key));
        timing.0.waveform_samples.clear();
        timing.0.waveform_sample_rate = 0;
        timing.0.waveform_window_size = eg_audio::service::WAVEFORM_WINDOW;
        timing.0.waveform_loading = true;
        timing.0.waveform_complete = false;
        eg_audio::service::start_waveform_loading(
            level_name,
            Some(bytes),
            waveform_sender.0.clone(),
        );
    }
}

/// Consume `WaveformResultEvent`s and update App-owned waveform presentation,
/// editor cache, persistent cache, and runtime preload resources directly.
///
/// Phase 6b note: `WaveformResultEvent` is not `Clone` (its
/// payload is a non-`Clone` `WaveformLoadMessage`), so we can't
/// use the standard `MessageReader<T>` + `for event in events.read()`
/// pattern (which iterates over `&T`). Instead we take `&mut
/// World` and `drain` the buffer to get owned `T` values.
#[allow(dead_code)]
pub fn waveform_handler_system(world: &mut World) {
    use bevy_ecs::message::Messages;
    use eg_audio::audio::runtime_asset_source_key;

    let current_level_name = world
        .resource::<SessionStateRes>()
        .editor_level_name
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());
    let current_source_key = runtime_asset_source_key(&current_level_name, "audio.mp3");
    let editor_cache = world.resource::<EditorAudioCacheRes>().clone();
    let runtime_preload = world.resource::<RuntimeAudioPreloadRes>().clone();
    let drained: Vec<WaveformResultEvent> = {
        let mut messages = world.resource_mut::<Messages<WaveformResultEvent>>();
        messages.drain().collect()
    };
    for event in drained {
        let source = event.source;
        let level_name = event.level_name;
        let message = event.message;
        let source_key = runtime_asset_source_key(&level_name, &source);
        if source_key == current_source_key {
            apply_waveform_timing_message(&mut world.resource_mut::<EditorTimingRes>().0, &message);
        }
        apply_waveform_cache_message(&editor_cache, &runtime_preload, source, level_name, message);
    }
}

fn apply_waveform_chunk(samples: &mut Vec<f32>, start_peak: usize, peaks: &[f32]) {
    if samples.len() < start_peak {
        samples.resize(start_peak, 0.0);
    }
    let end_peak = start_peak + peaks.len();
    if samples.len() < end_peak {
        samples.resize(end_peak, 0.0);
    }
    samples[start_peak..end_peak].copy_from_slice(peaks);
}

fn apply_waveform_timing_message(timing: &mut EditorTimingState, message: &WaveformLoadMessage) {
    match message {
        WaveformLoadMessage::Started { window_size } => {
            timing.waveform_samples.clear();
            timing.waveform_sample_rate = 0;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = true;
            timing.waveform_complete = false;
        }
        WaveformLoadMessage::Chunk {
            start_peak,
            peaks,
            sample_rate,
            window_size,
        } => {
            apply_waveform_chunk(&mut timing.waveform_samples, *start_peak, peaks);
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = true;
            timing.waveform_complete = false;
        }
        WaveformLoadMessage::Cached {
            samples,
            sample_rate,
            window_size,
            ..
        } => {
            timing.waveform_samples = samples.clone();
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = false;
            timing.waveform_complete = true;
        }
        WaveformLoadMessage::Finished {
            sample_rate,
            window_size,
            total_peaks,
            ..
        } => {
            timing.waveform_samples.truncate(*total_peaks);
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = false;
            timing.waveform_complete = true;
        }
        WaveformLoadMessage::Failed { .. } => {
            timing.waveform_samples.clear();
            timing.waveform_sample_rate = 0;
            timing.waveform_loading = false;
            timing.waveform_complete = false;
        }
    }
}

fn apply_waveform_cache_message(
    editor_cache: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
    source: String,
    level_name: String,
    message: WaveformLoadMessage,
) {
    use eg_audio::audio::runtime_asset_source_key;
    use eg_audio::service::WaveformCacheEntry;

    let source_key = runtime_asset_source_key(&level_name, &source);
    match message {
        WaveformLoadMessage::Started { window_size } => {
            editor_cache.insert_waveform(
                source_key.clone(),
                WaveformCacheEntry {
                    window_size,
                    ..WaveformCacheEntry::default()
                },
            );
        }
        WaveformLoadMessage::Chunk {
            start_peak,
            peaks,
            sample_rate,
            window_size,
        } => {
            editor_cache.with_mut(|cache| {
                let entry = cache.waveform_mut_or_default(source_key.clone());
                entry.sample_rate = sample_rate;
                entry.window_size = window_size;
                entry.complete = false;
                apply_waveform_chunk(&mut entry.samples, start_peak, &peaks);
            });
        }
        WaveformLoadMessage::Cached {
            samples,
            sample_rate,
            window_size,
            bytes,
        } => {
            let entry = WaveformCacheEntry {
                samples,
                sample_rate,
                window_size,
                complete: true,
            };
            editor_cache.insert_waveform(source_key.clone(), entry);
            editor_cache.clear_loading_source_if(&source_key);
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
        }
        WaveformLoadMessage::Finished {
            sample_rate,
            window_size,
            total_peaks,
            cache_key,
            bytes,
        } => {
            let entry = editor_cache.with_mut(|cache| {
                let entry = cache.waveform_mut_or_default(source_key.clone());
                entry.sample_rate = sample_rate;
                entry.window_size = window_size;
                entry.complete = true;
                entry.samples.truncate(total_peaks);
                entry.clone()
            });
            if let Some(cache_key) = cache_key {
                use crate::state::audio_state::persist_waveform_cache_entry;
                persist_waveform_cache_entry(cache_key, &entry);
            }
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
            editor_cache.clear_loading_source_if(&source_key);
        }
        WaveformLoadMessage::Failed { bytes } => {
            editor_cache.remove_waveform(&source_key);
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
            editor_cache.clear_loading_source_if(&source_key);
        }
    }
}

/// Consume `AudioPreloadResultEvent`s into the shared App-owned preload cache.
#[allow(dead_code)]
pub fn audio_preload_handler_system(
    mut events: MessageReader<AudioPreloadResultEvent>,
    preload: Res<RuntimeAudioPreloadRes>,
) {
    for event in events.read() {
        preload.apply_result(event.source_key.clone(), event.bytes.clone());
    }
}

/// Consume `LevelImportResultEvent`s and replace the App-owned editor session.
/// Bundled `.egz` audio bytes are inserted into the shared editor cache directly.
#[allow(dead_code)]
pub fn level_import_handler_system(world: &mut World) {
    use bevy_ecs::message::Messages;

    let events: Vec<LevelImportResultEvent> = world
        .resource_mut::<Messages<LevelImportResultEvent>>()
        .drain()
        .collect();
    for event in events {
        let (metadata, audio_bytes) = match eg_level::read_metadata_from_egz(&event.bytes) {
            Ok(import) => import,
            Err(egz_error) => match eg_codec::decode_level_metadata_binary(&event.bytes) {
                Ok(metadata) => (metadata, None),
                Err(binary_error) => {
                    log::error!(
                        "Level import failed (.egz parse error: {egz_error}; binary parse error: {binary_error})"
                    );
                    continue;
                }
            },
        };

        let level_name = apply_imported_level_to_world(world, metadata);
        if let Some(bytes) = audio_bytes {
            world
                .resource::<EditorAudioCacheRes>()
                .insert_level_audio(&level_name, bytes);
        }
    }
}

pub(crate) fn apply_imported_level_to_world(
    world: &mut World,
    metadata: eg_types::LevelMetadata,
) -> String {
    let level_name = metadata.basic.name.clone();
    let mut init = eg_editor_domain::editor_session_init_from_metadata(Some(metadata));
    init.timing_points
        .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    let tap_indicator_positions = eg_editor_domain::derive_tap_indicator_positions(
        init.spawn.position,
        init.spawn.direction,
        &init.tap_times,
        &init.objects,
    );

    {
        let mut objects = world.resource_mut::<EditorObjectsRes>();
        objects.objects = init.objects;
    }
    {
        let mut input = world.resource_mut::<EditorInputRes>();
        input.0.cursor = init.cursor;
        input.0.selected_block_index = None;
        input.0.selected_block_indices.clear();
        input.0.hovered_block_index = None;
    }
    {
        let mut timeline = world.resource_mut::<EditorTimelineRes>();
        timeline.0.taps.tap_times = init.tap_times;
        timeline.0.taps.tap_indicator_positions = tap_indicator_positions;
        timeline.0.taps.selected_index = None;
        timeline.0.tap_cursor_override = None;
        timeline.0.clock.time_seconds = init.timeline_time_seconds;
        timeline.0.clock.duration_seconds = init.timeline_duration_seconds;
    }
    {
        let mut timing = world.resource_mut::<EditorTimingRes>();
        timing.0.timing_points = init.timing_points;
        timing.0.timing_selected_index = None;
        timing.0.mark_timing_points_changed();
    }
    world.resource_mut::<EditorSpawnRes>().0 = init.spawn;
    {
        let mut camera = world.resource_mut::<EditorCameraRes>();
        camera.0.editor_pan = init.camera_pan;
        camera.0.editor_target_z = init.cursor[1];
    }
    world.resource_mut::<EditorTriggerRes>().0.selected_index = None;
    {
        let mut history = world.resource_mut::<EditorHistoryRes>();
        history.0.undo.clear();
        history.0.redo.clear();
    }
    {
        let mut session = world.resource_mut::<SessionStateRes>();
        session.editor_level_name = Some(level_name.clone());
        session.editor_music_metadata = init.music;
        session.editor_creator_metadata = init.creator_metadata;
        session.editor_sky_color = init.sky_color;
        session.editor_menu_preview_camera = init.menu_preview_camera;
    }

    level_name
}

/// Add the 5 result handler systems to a `Schedule`. Each system
/// is assigned to [`ApplyAsyncResultsSet`] so cross-set ordering
/// can be expressed via `.in_set(ApplyAsyncResultsSet)` / `.before()`.
#[allow(dead_code)]
pub fn add_apply_async_results_systems(schedule: &mut bevy_ecs::schedule::Schedule) {
    schedule.add_systems(
        (
            auth_handler_system,
            audio_import_handler_system,
            waveform_handler_system,
            audio_preload_handler_system,
            level_import_handler_system,
        )
            .in_set(ApplyAsyncResultsSet),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn build_world() -> World {
        let mut world = World::new();
        crate::state::plugins::register_state_resources(&mut world);
        register_state_async_bridge_world(&mut world);
        world
    }

    #[test]
    fn drain_one_stops_on_empty() {
        let (tx, rx) = mpsc::channel::<u32>();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        let mut collected: Vec<u32> = Vec::new();
        let mut rx = rx;
        drain_one(&mut rx, |v| collected.push(v));
        assert_eq!(collected, vec![1, 2]);
        // The receiver is now empty; draining again is a no-op.
        let mut second: Vec<u32> = Vec::new();
        drain_one(&mut rx, |v| second.push(v));
        assert!(second.is_empty());
    }

    #[test]
    fn drain_one_handles_disconnected_sender() {
        let (tx, rx) = mpsc::channel::<u32>();
        drop(tx);
        let mut collected: Vec<u32> = Vec::new();
        let mut rx = rx;
        // Disconnected with no messages yields no items.
        drain_one(&mut rx, |v| collected.push(v));
        assert!(collected.is_empty());
    }

    #[test]
    fn register_async_bridge_world_initializes_message_resources() {
        let world = build_world();
        assert!(world.contains_resource::<Messages<AuthServiceResultEvent>>());
        assert!(world.contains_resource::<Messages<AudioImportResultEvent>>());
        assert!(world.contains_resource::<Messages<WaveformResultEvent>>());
        assert!(world.contains_resource::<Messages<AudioPreloadResultEvent>>());
        assert!(world.contains_resource::<Messages<LevelImportResultEvent>>());
    }

    #[test]
    fn install_async_bridge_from_state_inserts_wrappers() {
        let mut world = build_world();
        let (_auth_tx, auth_rx) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx, ai_rx) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx, wf_rx) = mpsc::channel::<WaveformResult>();
        let (_ap_tx, ap_rx) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx, li_rx) = mpsc::channel::<Vec<u8>>();

        install_async_bridge_from_state(&mut world, auth_rx, ai_rx, wf_rx, ap_rx, li_rx);

        assert!(world.contains_resource::<AuthChannelRes>());
        assert!(world.contains_resource::<AudioImportChannelRes>());
        assert!(world.contains_resource::<WaveformLoadChannelRes>());
        assert!(world.contains_resource::<AudioPreloadChannelRes>());
        assert!(world.contains_resource::<LevelImportChannelRes>());
    }

    #[test]
    fn install_async_bridge_from_state_is_idempotent() {
        // Calling install_async_bridge_from_state twice must not
        // overwrite an existing wrapper (the legacy receiver would
        // be lost otherwise). The second call is a no-op for
        // already-installed resources.
        let mut world = build_world();
        let (_auth_tx, auth_rx) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx, ai_rx) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx, wf_rx) = mpsc::channel::<WaveformResult>();
        let (_ap_tx, ap_rx) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx, li_rx) = mpsc::channel::<Vec<u8>>();

        install_async_bridge_from_state(&mut world, auth_rx, ai_rx, wf_rx, ap_rx, li_rx);

        // A second call with fresh receivers must not replace the
        // installed ones.
        let (_auth_tx2, auth_rx2) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx2, ai_rx2) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx2, wf_rx2) = mpsc::channel::<WaveformResult>();
        let (_ap_tx2, ap_rx2) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx2, li_rx2) = mpsc::channel::<Vec<u8>>();

        install_async_bridge_from_state(&mut world, auth_rx2, ai_rx2, wf_rx2, ap_rx2, li_rx2);

        // The original receivers are still installed (we can verify
        // by checking the wrappers exist).
        assert!(world.contains_resource::<AuthChannelRes>());
    }

    // -----------------------------------------------------------------------
    // Result handler system tests
    // -----------------------------------------------------------------------

    #[test]
    fn install_async_bridge_from_state_arc_shares_receivers() {
        // The new Arc-based installer stores `Arc<Mutex<Receiver<T>>>`
        // in the resources. We verify the resources are inserted
        // and that the Arc has a single reference (shared between
        // the install call's argument and the resource).
        let mut world = build_world();
        let (_auth_tx, auth_rx) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx, ai_rx) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx, wf_rx) = mpsc::channel::<WaveformResult>();
        let (_ap_tx, ap_rx) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx, li_rx) = mpsc::channel::<Vec<u8>>();

        let auth_arc = std::sync::Arc::new(std::sync::Mutex::new(auth_rx));
        let ai_arc = std::sync::Arc::new(std::sync::Mutex::new(ai_rx));
        let wf_arc = std::sync::Arc::new(std::sync::Mutex::new(wf_rx));
        let ap_arc = std::sync::Arc::new(std::sync::Mutex::new(ap_rx));
        let li_arc = std::sync::Arc::new(std::sync::Mutex::new(li_rx));

        install_async_bridge_from_state_arc(
            &mut world,
            std::sync::Arc::clone(&auth_arc),
            std::sync::Arc::clone(&ai_arc),
            std::sync::Arc::clone(&wf_arc),
            std::sync::Arc::clone(&ap_arc),
            std::sync::Arc::clone(&li_arc),
        );

        assert!(world.contains_resource::<AuthChannelRes>());
        assert!(world.contains_resource::<AudioImportChannelRes>());
        assert!(world.contains_resource::<WaveformLoadChannelRes>());
        assert!(world.contains_resource::<AudioPreloadChannelRes>());
        assert!(world.contains_resource::<LevelImportChannelRes>());

        // The Arc we passed in is the same one stored in the
        // resource (we can verify by `Arc::ptr_eq` after
        // extracting the inner Arc from the resource).
        let stored_arc = &world.resource::<AuthChannelRes>().0;
        assert!(std::sync::Arc::ptr_eq(stored_arc, &auth_arc));
    }

    #[test]
    fn install_async_bridge_from_state_arc_is_idempotent() {
        // Calling the Arc-based installer twice must not
        // overwrite an existing wrapper.
        let mut world = build_world();
        let (_auth_tx, auth_rx) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx, ai_rx) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx, wf_rx) = mpsc::channel::<WaveformResult>();
        let (_ap_tx, ap_rx) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx, li_rx) = mpsc::channel::<Vec<u8>>();

        install_async_bridge_from_state_arc(
            &mut world,
            std::sync::Arc::new(std::sync::Mutex::new(auth_rx)),
            std::sync::Arc::new(std::sync::Mutex::new(ai_rx)),
            std::sync::Arc::new(std::sync::Mutex::new(wf_rx)),
            std::sync::Arc::new(std::sync::Mutex::new(ap_rx)),
            std::sync::Arc::new(std::sync::Mutex::new(li_rx)),
        );

        // Second call: fresh receivers. The install must be a
        // no-op for already-installed resources.
        let (_auth_tx2, auth_rx2) = mpsc::channel::<AuthServiceMessage>();
        let (_ai_tx2, ai_rx2) = mpsc::channel::<(String, Vec<u8>)>();
        let (_wf_tx2, wf_rx2) = mpsc::channel::<WaveformResult>();
        let (_ap_tx2, ap_rx2) = mpsc::channel::<AudioPreloadResult>();
        let (_li_tx2, li_rx2) = mpsc::channel::<Vec<u8>>();

        install_async_bridge_from_state_arc(
            &mut world,
            std::sync::Arc::new(std::sync::Mutex::new(auth_rx2)),
            std::sync::Arc::new(std::sync::Mutex::new(ai_rx2)),
            std::sync::Arc::new(std::sync::Mutex::new(wf_rx2)),
            std::sync::Arc::new(std::sync::Mutex::new(ap_rx2)),
            std::sync::Arc::new(std::sync::Mutex::new(li_rx2)),
        );

        // Both resources are still present (the second install
        // didn't drop them).
        assert!(world.contains_resource::<AuthChannelRes>());
        assert!(world.contains_resource::<LevelImportChannelRes>());
    }

    #[test]
    fn auth_handler_system_updates_auth_resource_for_sign_out() {
        // Write an `AuthServiceResultEvent::SignedOut(Ok(()))` into
        // the buffer, run the handler system, and verify the
        // App-world auth resource was updated (`session = None`,
        // `pending = false`).
        use crate::events::async_results::AuthServiceResultEvent;
        let mut world = build_world();
        {
            let mut auth = world.resource_mut::<AuthStateRes>();
            auth.pending = true;
            auth.session = Some(test_auth_session());
        }
        world.write_message(AuthServiceResultEvent::new(AuthServiceMessage::SignedOut(
            Ok(()),
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(auth_handler_system);
        schedule.run(&mut world);

        let auth = world.resource::<AuthStateRes>();
        assert!(!auth.pending);
        assert!(auth.session.is_none());
    }

    #[test]
    fn audio_preload_handler_system_inserts_bytes() {
        // Write an `AudioPreloadResultEvent` into the buffer, run
        // the handler, drain the slot, and verify the State's
        // `preloaded_audio` map contains the entry.
        use crate::events::async_results::AudioPreloadResultEvent;
        let mut world = build_world();
        world.write_message(AudioPreloadResultEvent::from_result(&(
            "Level1:audio.mp3".to_string(),
            Some(vec![1, 2, 3, 4]),
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(audio_preload_handler_system);
        schedule.run(&mut world);

        let source_key = "Level1:audio.mp3".to_string();
        assert!(
            world
                .resource::<RuntimeAudioPreloadRes>()
                .contains_preloaded(&source_key)
        );
    }

    #[test]
    fn audio_preload_handler_system_handles_failure() {
        // Same as above but the result is `None` (failure). The
        // handler should not panic and should not insert
        // anything.
        use crate::events::async_results::AudioPreloadResultEvent;
        let mut world = build_world();
        world.write_message(AudioPreloadResultEvent::from_result(&(
            "Level1:audio.mp3".to_string(),
            None,
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(audio_preload_handler_system);
        schedule.run(&mut world);

        let source_key = "Level1:audio.mp3".to_string();
        assert!(
            !world
                .resource::<RuntimeAudioPreloadRes>()
                .contains_preloaded(&source_key)
        );
    }

    #[test]
    fn level_import_handler_system_is_safe_on_invalid_bytes() {
        // The handler should not panic or mutate App resources on invalid bytes.
        use crate::events::async_results::LevelImportResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name = Some("Original".to_string());
        world.write_message(LevelImportResultEvent::new(vec![0xDE, 0xAD]));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(level_import_handler_system);
        schedule.run(&mut world);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("Original")
        );
    }

    #[test]
    fn level_import_handler_system_replaces_app_editor_session_without_state_closure() {
        use crate::events::async_results::LevelImportResultEvent;
        use eg_types::{Direction, LevelMetadata, LevelObject, SpawnMetadata};

        let mut world = build_world();
        let metadata = LevelMetadata {
            basic: eg_types::BasicLevelMetadata {
                name: "Imported".to_string(),
                ..eg_types::BasicLevelMetadata::default()
            },
            spawn: SpawnMetadata {
                position: [2.0, 3.0, 4.0],
                direction: Direction::Right,
            },
            tap_times: vec![0.8, 0.2],
            timeline_duration_seconds: 9.0,
            objects: vec![LevelObject {
                position: [6.0, 2.0, 1.0],
                ..LevelObject::default()
            }],
            ..LevelMetadata::default()
        };
        let bytes = eg_codec::encode_level_metadata_binary(&metadata).expect("encode level");
        world.write_message(LevelImportResultEvent::new(bytes));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(level_import_handler_system);
        schedule.run(&mut world);

        let session = world.resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("Imported"));
        let objects = world.resource::<EditorObjectsRes>();
        assert_eq!(objects.objects.len(), 1);
        assert_eq!(objects.objects[0].position, [6.0, 2.0, 1.0]);
        assert_eq!(world.resource::<EditorSpawnRes>().0, metadata.spawn);
        let timeline = &world.resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.2, 0.8]);
        assert_eq!(timeline.taps.tap_indicator_positions.len(), 2);
        assert_eq!(timeline.clock.duration_seconds, 9.0);
        let input = &world.resource::<EditorInputRes>().0;
        assert_eq!(input.cursor, [6.0, 2.0, 1.0]);
        assert_eq!(input.selected_block_index, None);
    }

    #[test]
    fn waveform_handler_system_updates_current_app_timing_state() {
        use crate::events::async_results::WaveformResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name = Some("Level1".to_string());
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Started { window_size: 128 },
        )));
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Chunk {
                start_peak: 1,
                peaks: vec![0.25, 0.75],
                sample_rate: 44_100,
                window_size: 128,
            },
        )));
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Finished {
                sample_rate: 44_100,
                window_size: 128,
                total_peaks: 3,
                cache_key: None,
                bytes: None,
            },
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(waveform_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.waveform_samples, vec![0.0, 0.25, 0.75]);
        assert_eq!(timing.waveform_sample_rate, 44_100);
        assert_eq!(timing.waveform_window_size, 128);
        assert!(!timing.waveform_loading);
        assert!(timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("Level1", "audio.mp3");
        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .waveform(&source_key)
                .is_some_and(|entry| entry.complete)
        );
    }

    #[test]
    fn waveform_handler_system_keeps_stale_results_out_of_app_timing_state() {
        use crate::events::async_results::WaveformResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name =
            Some("CurrentLevel".to_string());
        {
            let mut timing = world.resource_mut::<EditorTimingRes>();
            timing.0.waveform_samples = vec![1.0];
            timing.0.waveform_sample_rate = 22_050;
            timing.0.waveform_complete = true;
        }
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "OldLevel".to_string(),
            WaveformLoadMessage::Cached {
                samples: vec![0.25, 0.75],
                sample_rate: 44_100,
                window_size: 256,
                bytes: None,
            },
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(waveform_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.waveform_samples, vec![1.0]);
        assert_eq!(timing.waveform_sample_rate, 22_050);
        assert!(timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("OldLevel", "audio.mp3");
        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .contains_waveform(&source_key)
        );
    }

    #[test]
    fn audio_import_handler_system_runs_with_cancellation() {
        // The audio-import handler should be a no-op when the
        // event's `bytes` is `None` (cancellation).
        use crate::events::async_results::AudioImportResultEvent;
        let mut world = build_world();
        world.write_message(AudioImportResultEvent::new("level1.ogg".to_string(), None));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(audio_import_handler_system);
        schedule.run(&mut world);

        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .bytes_for_source("level1.ogg")
                .is_none()
        );
    }

    #[test]
    fn audio_import_handler_system_resets_app_waveform_presentation() {
        use crate::events::async_results::AudioImportResultEvent;
        let mut world = build_world();
        {
            let mut timing = world.resource_mut::<EditorTimingRes>();
            timing.0.waveform_samples = vec![0.5];
            timing.0.waveform_sample_rate = 44_100;
            timing.0.waveform_complete = true;
        }
        world.write_message(AudioImportResultEvent::new(
            "imported.mp3".to_string(),
            Some(vec![1, 2, 3, 4]),
        ));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(audio_import_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert!(timing.waveform_samples.is_empty());
        assert_eq!(timing.waveform_sample_rate, 0);
        assert_eq!(
            timing.waveform_window_size,
            eg_audio::service::WAVEFORM_WINDOW
        );
        assert!(timing.waveform_loading);
        assert!(!timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("Untitled", "audio.mp3");
        assert_eq!(
            world
                .resource::<EditorAudioCacheRes>()
                .bytes_for_source(&source_key),
            Some(vec![1, 2, 3, 4])
        );
    }

    // ----- helpers for the handler-system tests -----

    fn test_auth_session() -> eg_types::AuthSession {
        use eg_types::{AuthSession, AuthSessionTokens, AuthUser};
        AuthSession {
            session: AuthSessionTokens {
                access_token: String::new(),
                refresh_token: String::new(),
                expires_at: Some(0),
                token_type: String::new(),
            },
            user: AuthUser {
                id: String::new(),
                email: None,
            },
            profile: None,
        }
    }
}
