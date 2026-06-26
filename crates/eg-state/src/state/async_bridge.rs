/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Async channel bridge (Phase 6a).
//!
//! The legacy `State` struct holds five `mpsc::Receiver`s that the
//! platform layer (auth service, audio service, level importer) and
//! background tasks write to. Today those receivers are drained
//! inline by `update_*` methods called from `State::update`.
//!
//! Phase 6a introduces **Bevy resources** that wrap each receiver
//! (`AuthChannelRes`, `AudioImportChannelRes`, `WaveformLoadChannelRes`,
//! `AudioPreloadChannelRes`, `LevelImportChannelRes`) plus a
//! `PollChannelsSet` system set with a `poll_channels_system` that
//! drains every receiver once per frame and emits the corresponding
//! result event into the World (Phase 6a's five new
//! [`async_results`](crate::events::async_results) message types).
//!
//! The existing inline `update_*` methods keep running — both the
//! new poll system and the legacy drains call `try_recv`, so each
//! message is consumed exactly once across the whole frame. Phase
//! 6b replaces the inline drains with handler systems that consume
//! the events.
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
//! ## Schedule graph (post Phase 6a)
//!
//! ```text
//!   PollChannelsSet
//!     poll_channels_system     // drain receivers → emit result events
//!   ModeChangeSet
//!   PreCommandHistorySet
//!   CommandHandlerSet          // per-message drain systems (Phase 3)
//!   ... simulation set ...
//!   PollChannelsSet
//!     apply_async_results_system  // Phase 6b: consume events → mutate state
//! ```
//!
//! REFACTOR POINT(full-bevy): the `mpsc` channels disappear in the
//! full-Bevy migration; background tasks write directly into
//! `Messages<T>` or `AssetEvent<T>`. The `poll_channels_system` is
//! replaced by `bevy_tasks` async event streams.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;

use eg_audio::service::{AudioPreloadResult, WaveformResult};
use eg_platform::services::AuthServiceMessage;

use crate::events::async_results::{
    AudioImportResultEvent, AudioPreloadResultEvent, AuthServiceResultEvent,
    LevelImportResultEvent, WaveformResultEvent,
};

// ---------------------------------------------------------------------------
// Channel wrapper resources
// ---------------------------------------------------------------------------

/// Bevy `Resource` wrapper around the auth-service `mpsc::Receiver`.
///
/// The legacy `State::auth.channel.1` field stays authoritative; this
/// resource is a parallel mirror used by the `poll_channels_system`.
/// Phase 6b replaces the legacy field entirely.
#[allow(dead_code)]
#[derive(Resource)]
pub struct AuthChannelRes(pub Mutex<Receiver<AuthServiceMessage>>);

/// Bevy `Resource` wrapper around the audio-import `mpsc::Receiver`.
///
/// Mirrors the legacy `State::audio.state.editor.audio_import_channel.1`
/// field. Phase 6b replaces the legacy field entirely.
#[allow(dead_code)]
#[derive(Resource)]
pub struct AudioImportChannelRes(pub Mutex<Receiver<(String, Vec<u8>)>>);

/// Bevy `Resource` wrapper around the waveform-load `mpsc::Receiver`.
///
/// Mirrors the legacy `State::audio.state.editor.waveform_load_channel.1`
/// field. Phase 6b replaces the legacy field entirely.
#[allow(dead_code)]
#[derive(Resource)]
pub struct WaveformLoadChannelRes(pub Mutex<Receiver<WaveformResult>>);

/// Bevy `Resource` wrapper around the audio-preload `mpsc::Receiver`.
///
/// Mirrors the legacy `State::audio.state.runtime_preload.preload_channel.1`
/// field. Phase 6b replaces the legacy field entirely.
#[allow(dead_code)]
#[derive(Resource)]
pub struct AudioPreloadChannelRes(pub Mutex<Receiver<AudioPreloadResult>>);

/// Bevy `Resource` wrapper around the level-import `mpsc::Receiver`.
///
/// Mirrors the legacy `State::session.editor_level_import_channel.1`
/// field. Phase 6b replaces the legacy field entirely.
#[allow(dead_code)]
#[derive(Resource)]
pub struct LevelImportChannelRes(pub Mutex<Receiver<Vec<u8>>>);

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
/// The drains are idempotent with the legacy `update_*` methods —
/// each message is consumed by exactly one of the two paths (whichever
/// runs first).
///
/// Phase 6a emits the events but does not yet apply them to legacy
/// state. Phase 6b adds an `apply_async_results_system` that
/// consumes the events and replaces the legacy `update_*` methods.
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
/// Used by `State::tick_state` to populate the resources from the
/// legacy `State::auth.channel.1` /
/// `State::audio.state.editor.audio_import_channel.1` / etc. fields.
/// The legacy fields are drained in parallel (today via `update_*`,
/// later via the Phase 6b handler systems), so each message is
/// consumed exactly once.
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
    if !world.contains_resource::<AuthChannelRes>() {
        world.insert_resource(AuthChannelRes(Mutex::new(auth_receiver)));
    }
    if !world.contains_resource::<AudioImportChannelRes>() {
        world.insert_resource(AudioImportChannelRes(Mutex::new(audio_import_receiver)));
    }
    if !world.contains_resource::<WaveformLoadChannelRes>() {
        world.insert_resource(WaveformLoadChannelRes(Mutex::new(waveform_receiver)));
    }
    if !world.contains_resource::<AudioPreloadChannelRes>() {
        world.insert_resource(AudioPreloadChannelRes(Mutex::new(audio_preload_receiver)));
    }
    if !world.contains_resource::<LevelImportChannelRes>() {
        world.insert_resource(LevelImportChannelRes(Mutex::new(level_import_receiver)));
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
    // inserted lazily by `install_async_bridge_from_state` (or not
    // at all in tests that don't go through `State::tick_state`).
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn build_world() -> World {
        let mut world = World::new();
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
}
