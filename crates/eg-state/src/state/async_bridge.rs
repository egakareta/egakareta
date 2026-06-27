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
//! ## Phase 6b: result handler systems
//!
//! Phase 6a established the `poll_channels_system` that drains the
//! 5 receivers into 5 distinct result events. Phase 6b adds **5
//! handler systems** (one per channel) that consume the result
//! events and apply them to the legacy `State`.
//!
//! The handler systems can't take `&mut State` directly because
//! `State` owns `Arc<Mutex<Receiver<T>>>` (and previously
//! `mpsc::Receiver<T>`) which makes it `!Send`. We use the same
//! thread-local bridge pattern as Phase 4a (`state::render::mirror`):
//! each handler system installs a `FnOnce(&mut State)` closure
//! into a thread-local slot; `State::tick_state` drains the slot
//! and applies each closure after the schedule finishes.
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
//!     waveform_handler_system          // WaveformResultEvent
//!     audio_preload_handler_system     // AudioPreloadResultEvent
//!     level_import_handler_system      // LevelImportResultEvent
//!   render::mirror_pod_state_via_bridge
//!   State::tick_state drains pending async result closures → mutates State
//! ```
//!
//! REFACTOR POINT(full-bevy): the `mpsc` channels disappear in the
//! full-Bevy migration; background tasks write directly into
//! `Messages<T>` or `AssetEvent<T>`. The `poll_channels_system` is
//! replaced by `bevy_tasks` async event streams. The thread-local
//! bridge is replaced by Bevy `Resource`s that the handler systems
//! mutate directly (no closures needed).

use std::cell::RefCell;
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
use crate::state::State;

// ---------------------------------------------------------------------------
// Type aliases for the shared `Arc<Mutex<Receiver<T>>>` channels
// ---------------------------------------------------------------------------

/// Phase 6b: shared `Arc<Mutex<Receiver<AuthServiceMessage>>>`
/// held by both the legacy `State::auth.channel.1` field and the
/// World's `AuthChannelRes` resource.
pub type SharedAuthReceiver = std::sync::Arc<Mutex<Receiver<AuthServiceMessage>>>;

/// Phase 6b: shared `Arc<Mutex<Receiver<(String, Vec<u8>)>>>`.
pub type SharedAudioImportReceiver = std::sync::Arc<Mutex<Receiver<(String, Vec<u8>)>>>;

/// Phase 6b: shared `Arc<Mutex<Receiver<WaveformResult>>>`.
pub type SharedWaveformReceiver = std::sync::Arc<Mutex<Receiver<WaveformResult>>>;

/// Phase 6b: shared `Arc<Mutex<Receiver<AudioPreloadResult>>>`.
pub type SharedAudioPreloadReceiver = std::sync::Arc<Mutex<Receiver<AudioPreloadResult>>>;

/// Phase 6b: shared `Arc<Mutex<Receiver<Vec<u8>>>>`.
pub type SharedLevelImportReceiver = std::sync::Arc<Mutex<Receiver<Vec<u8>>>>;

// ---------------------------------------------------------------------------
// Channel wrapper resources
// ---------------------------------------------------------------------------

/// Bevy `Resource` wrapper around the auth-service `mpsc::Receiver`.
///
/// Phase 6b: holds the **shared** `Arc<Mutex<Receiver<T>>>` so both
/// the World's `poll_channels_system` and the legacy
/// `State::update_auth_results` can race for messages. The
/// `Arc<Mutex<>>` is constructed by `State::lifecycle` and shared
/// with the World via `install_async_bridge_from_state_arc`.
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

/// Phase 6b: same as [`install_async_bridge_from_state`] but the
/// receivers are passed as `Arc<Mutex<Receiver<T>>>` (the
/// wrapper used by the new `State.auth.channel.1` /
/// `State.audio.state.editor.audio_import_channel.1` / etc.
/// fields). The function unwraps the `Arc` and stores a clone
/// of the inner `Mutex<Receiver<T>>` in the resource. The
/// `Arc<Mutex<>>` is shared between the `State` and the World,
/// so both consumers race for messages via `try_recv`.
///
/// Why two `Mutex` layers (one in the resource, one in the
/// `Arc`)? `mpsc::Receiver<T>` is `Send` but not `Sync`; the
/// outer `Mutex<Receiver<T>>` resource satisfies Bevy's
/// `Send + Sync + 'static` bound for `Resource`. The inner
/// `Arc<Mutex<Receiver<T>>>` is the legacy field's storage.
/// The two `Mutex`es are uncontended in practice (the World
/// has at most one consumer of each).
#[allow(dead_code)]
pub fn install_async_bridge_from_state_arc(
    world: &mut World,
    auth_receiver: SharedAuthReceiver,
    audio_import_receiver: SharedAudioImportReceiver,
    waveform_receiver: SharedWaveformReceiver,
    audio_preload_receiver: SharedAudioPreloadReceiver,
    level_import_receiver: SharedLevelImportReceiver,
) {
    // Phase 6b: take the inner `Mutex<Receiver<T>>` from the
    // `Arc<Mutex<Receiver<T>>>` and install it in the resource.
    // The `Arc` is `Clone`, so we can clone the inner `Mutex`
    // (which doesn't move the receiver — just shares it) and
    // install the clone in the resource. Both the State and
    // the World now point at the same `Mutex<Receiver<T>>`,
    // and the outer `Mutex<Receiver<T>>` resource is the
    // second `Mutex` layer. The cost is one extra
    // `lock()` per `try_recv`, which is fine.
    if !world.contains_resource::<AuthChannelRes>() {
        // The State holds an `Arc<Mutex<Receiver<T>>>`. The
        // resource wants a `Mutex<Receiver<T>>`. We can clone
        // the `Arc` (cheap) and unwrap the `Mutex` (also cheap
        // because `Mutex::lock` returns the same inner value
        // for shared ownership). Actually `Mutex` doesn't
        // expose the inner by `Clone`, so we have to lock +
        // use the guard as the resource directly... but the
        // resource is `'static`. So we install a new
        // `Mutex<Receiver<T>>` that shares the receiver
        // through the original `Arc`'s inner `Mutex`.
        //
        // Simpler: install a new `Mutex` that wraps a fresh
        // channel endpoint... no, the channel is the same.
        // The clean approach: install a `Mutex<Receiver<T>>`
        // that holds the **shared** `Arc<Mutex<Receiver<T>>>`,
        // and `try_recv` calls go through both layers.
        //
        // But that means changing the resource type to
        // `Arc<Mutex<Receiver<T>>>`. Easier path: just install
        // a `Mutex::new(Receiver::new())` placeholder; the
        // real receiver is accessed via the `Arc`. No, that
        // would lose messages.
        //
        // Final approach: the resource holds the **shared**
        // `Arc<Mutex<Receiver<T>>>` directly (one fewer layer
        // of indirection). The `poll_channels_system` locks
        // the inner `Mutex` via the `Arc`. This is what the
        // implementation actually does. See the resource
        // definition above — the `AuthChannelRes` already
        // takes a `Mutex<Receiver<T>>`. We can change it to
        // take an `Arc<Mutex<Receiver<T>>>` to share with the
        // legacy field. For Phase 6b, we keep the existing
        // resource type and just install a clone of the inner
        // `Mutex` (which is possible because `Mutex` is
        // `Sync` so the State and the World can each hold a
        // `Mutex<Receiver<T>>` constructed by locking the
        // shared `Arc<Mutex<Receiver<T>>>` and cloning the
        // resulting `MutexGuard`'s `Mutex`). Actually no —
        // `MutexGuard` is not `Mutex`.
        //
        // The truly correct path: change `AuthChannelRes` to
        // hold the shared `Arc<Mutex<Receiver<T>>>`. The
        // `poll_channels_system` accesses it via
        // `res.0.lock().try_recv()`. The legacy
        // `update_auth_results` accesses it via
        // `self.auth.channel.1.lock().try_recv()` — both
        // lock the same `Mutex`. Done.
        //
        // For Phase 6b, we change the resource type from
        // `Mutex<Receiver<T>>` to `Arc<Mutex<Receiver<T>>>`
        // (see the resource definitions below). This helper
        // installs clones of the `Arc` directly.
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
// Phase 6b: result handler systems + thread-local bridge
// ---------------------------------------------------------------------------

/// A closure that mutates the legacy `State` to apply a single
/// async result. The handler systems (below) install one closure
/// per result event into a thread-local slot; [`State::tick_state`]
/// drains the slot after the schedule finishes and applies each
/// closure to the State.
///
/// The closure must be `'static` (no borrowed State) because the
/// schedule outlives a single call: handler systems run during
/// `State::tick_state`, but the closure is invoked after
/// `try_run_schedule` returns, by `tick_state` itself.
pub type AsyncResultFn = Box<dyn FnOnce(&mut State)>;

thread_local! {
    /// Thread-local queue of pending async-result closures. Each
    /// handler system pushes a closure per consumed event;
    /// `State::tick_state` drains the slot after the schedule
    /// finishes.
    ///
    /// Why a thread-local? `State` is `!Send` (it owns the
    /// `Arc<Mutex<Receiver<T>>>` wrappers from Phase 6b which
    /// contain `mpsc::Receiver<T>`). The schedule is invoked on
    /// the main thread today; the closure runs on the same
    /// thread, so the thread-local slot is the simplest correct
    /// mechanism. In the full-Bevy migration the slot disappears
    /// (handler systems mutate Bevy `Resource`s directly).
    static PENDING_ASYNC_RESULT_FNS: RefCell<Vec<AsyncResultFn>> =
        const { RefCell::new(Vec::new()) };
}

/// Install a closure that will be applied to the legacy `State`
/// after the schedule finishes. Called by the handler systems
/// below; `State::tick_state` drains the queue via
/// [`drain_pending_async_result_fns`].
#[allow(dead_code)]
pub fn install_async_result_fn<F>(f: F)
where
    F: FnOnce(&mut State) + 'static,
{
    PENDING_ASYNC_RESULT_FNS.with(|cell| cell.borrow_mut().push(Box::new(f)));
}

/// Drain every pending async-result closure and apply it to
/// `state`. Called by [`State::tick_state`] after the schedule
/// finishes. Idempotent: a second call with no pending closures
/// is a no-op.
#[allow(dead_code)]
pub fn drain_pending_async_result_fns(state: &mut State) {
    let pending: Vec<AsyncResultFn> =
        PENDING_ASYNC_RESULT_FNS.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
    for f in pending {
        f(state);
    }
}

/// System set for the result handler systems. Runs after
/// `PollChannelsSet` (the events must be emitted first) and after
/// `CommandHandlerSet` (the per-message drain systems must finish
/// first, so any per-frame command ordering is preserved). The
/// handler systems only install closures into the thread-local
/// slot; the actual State mutation happens in
/// `State::tick_state` after the schedule.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApplyAsyncResultsSet;

/// Consume `AuthServiceResultEvent`s and install one closure per
/// event into the thread-local async-result slot. The closure
/// routes the `AuthServiceMessage` to the matching `complete_*`
/// method on `State`.
#[allow(dead_code)]
pub fn auth_handler_system(mut events: MessageReader<AuthServiceResultEvent>) {
    for event in events.read() {
        let message = event.message.clone();
        install_async_result_fn(move |state: &mut State| match message {
            AuthServiceMessage::SignedIn(result) => state.complete_auth_sign_in(result),
            AuthServiceMessage::Refreshed(result) => state.complete_auth_refresh(result),
            AuthServiceMessage::SignedOut(result) => state.complete_auth_sign_out(result),
        });
    }
}

/// Consume `AudioImportResultEvent`s and install one closure per
/// event. Each closure mirrors the body of
/// `State::update_audio_imports` (Phase 6a: copy filename + bytes
/// into the editor audio cache, then reload the waveform).
#[allow(dead_code)]
pub fn audio_import_handler_system(mut events: MessageReader<AudioImportResultEvent>) {
    for event in events.read() {
        let filename = event.filename.clone();
        let bytes = event.bytes.clone();
        install_async_result_fn(move |state: &mut State| {
            // Phase 6b: replay the body of `update_audio_imports`
            // for a single (filename, bytes) result. The original
            // helper is left in place (drains the same shared
            // receiver) — when the schedule runs first, this
            // closure does the work and the helper becomes a
            // no-op for the same frame.
            let _ = filename;
            let level_name = state
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let bytes = match bytes {
                Some(b) => b,
                None => return,
            };
            let source_key = state
                .audio
                .state
                .editor
                .cache
                .insert_level_audio(&level_name, bytes);
            state.audio.state.editor.cache.remove_waveform(&source_key);
            state.audio.state.editor.cache.set_loading_source(None);
            state.load_waveform_for_current_audio();
        });
    }
}

/// Consume `WaveformResultEvent`s and install one closure per
/// event. Each closure mirrors the body of
/// `State::update_waveform_loading` for one `WaveformLoadMessage`.
///
/// Phase 6b note: `WaveformResultEvent` is not `Clone` (its
/// payload is a non-`Clone` `WaveformLoadMessage`), so we can't
/// use the standard `MessageReader<T>` + `for event in events.read()`
/// pattern (which iterates over `&T`). Instead we take `&mut
/// World` and `drain` the buffer to get owned `T` values.
#[allow(dead_code)]
pub fn waveform_handler_system(world: &mut World) {
    use bevy_ecs::message::Messages;
    let drained: Vec<WaveformResultEvent> = {
        let mut messages = world.resource_mut::<Messages<WaveformResultEvent>>();
        messages.drain().collect()
    };
    for event in drained {
        let source = event.source;
        let level_name = event.level_name;
        let message = event.message;
        install_async_result_fn(move |state: &mut State| {
            apply_waveform_message(state, source, level_name, message);
        });
    }
}

/// Apply a single `(source, level_name, WaveformLoadMessage)` to
/// the legacy `State`. Mirrors the body of the `Started` /
/// `Chunk` / `Cached` / `Finished` / `Failed` arms of
/// `State::update_waveform_loading`. Lives in the schedule
/// helper module so the handler system can stay a one-liner.
#[allow(dead_code)]
fn apply_waveform_message(
    state: &mut State,
    source: String,
    level_name: String,
    message: WaveformLoadMessage,
) {
    use eg_audio::audio::runtime_asset_source_key;
    use eg_audio::service::WaveformCacheEntry;

    let source_key = runtime_asset_source_key(&level_name, &source);
    let is_current = state.audio.state.editor.cache.loading_source() == Some(source_key.as_str());

    /// Mirror of the private `apply_waveform_chunk` helper in
    /// `state::audio_state` — inlined here so the handler system
    /// doesn't depend on a private function. Same semantics.
    fn apply_chunk(samples: &mut Vec<f32>, start_peak: usize, peaks: Vec<f32>) {
        if samples.len() < start_peak {
            samples.resize(start_peak, 0.0);
        }
        let end_peak = start_peak + peaks.len();
        if samples.len() < end_peak {
            samples.resize(end_peak, 0.0);
        }
        samples[start_peak..end_peak].copy_from_slice(&peaks);
    }

    match message {
        WaveformLoadMessage::Started { window_size } => {
            state.audio.state.editor.cache.insert_waveform(
                source_key.clone(),
                WaveformCacheEntry {
                    window_size,
                    ..WaveformCacheEntry::default()
                },
            );
            if is_current {
                state.editor.timing.waveform_samples.clear();
                state.editor.timing.waveform_sample_rate = 0;
                state.editor.timing.waveform_window_size = window_size;
                state.editor.timing.waveform_loading = true;
                state.editor.timing.waveform_complete = false;
            }
        }
        WaveformLoadMessage::Chunk {
            start_peak,
            peaks,
            sample_rate,
            window_size,
        } => {
            let entry = state
                .audio
                .state
                .editor
                .cache
                .waveform_mut_or_default(source_key.clone());
            entry.sample_rate = sample_rate;
            entry.window_size = window_size;
            entry.complete = false;
            apply_chunk(&mut entry.samples, start_peak, peaks);
            let entry = entry.clone();
            if is_current {
                state.editor.timing.waveform_samples = entry.samples.clone();
                state.editor.timing.waveform_sample_rate = sample_rate;
                state.editor.timing.waveform_window_size = window_size;
                state.editor.timing.waveform_loading = true;
                state.editor.timing.waveform_complete = false;
            }
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
            state
                .audio
                .state
                .editor
                .cache
                .insert_waveform(source_key.clone(), entry.clone());
            if is_current {
                state.editor.timing.waveform_samples = entry.samples.clone();
                state.editor.timing.waveform_sample_rate = entry.sample_rate;
                state.editor.timing.waveform_window_size = entry.window_size;
                state.editor.timing.waveform_loading = false;
                state.editor.timing.waveform_complete = true;
                state
                    .audio
                    .state
                    .editor
                    .cache
                    .clear_loading_source_if(&source_key);
            }
            if let Some(bytes) = bytes {
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .insert(source_key.clone(), bytes);
            }
        }
        WaveformLoadMessage::Finished {
            sample_rate,
            window_size,
            total_peaks,
            cache_key,
            bytes,
        } => {
            let entry = {
                let entry = state
                    .audio
                    .state
                    .editor
                    .cache
                    .waveform_mut_or_default(source_key.clone());
                entry.sample_rate = sample_rate;
                entry.window_size = window_size;
                entry.complete = true;
                entry.samples.truncate(total_peaks);
                entry.clone()
            };
            if let Some(cache_key) = cache_key {
                use crate::state::audio_state::persist_waveform_cache_entry;
                persist_waveform_cache_entry(cache_key, &entry);
            }
            if is_current {
                state.editor.timing.waveform_samples = entry.samples.clone();
                state.editor.timing.waveform_sample_rate = sample_rate;
                state.editor.timing.waveform_window_size = window_size;
                state.editor.timing.waveform_loading = false;
                state.editor.timing.waveform_complete = true;
            }
            if let Some(bytes) = bytes {
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .insert(source_key.clone(), bytes);
            }
            if is_current {
                state
                    .audio
                    .state
                    .editor
                    .cache
                    .clear_loading_source_if(&source_key);
            }
        }
        WaveformLoadMessage::Failed { bytes } => {
            state.audio.state.editor.cache.remove_waveform(&source_key);
            if is_current {
                state.editor.timing.waveform_samples.clear();
                state.editor.timing.waveform_sample_rate = 0;
                state.editor.timing.waveform_loading = false;
                state.editor.timing.waveform_complete = false;
            }
            if let Some(bytes) = bytes {
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .insert(source_key.clone(), bytes);
            }
            if is_current {
                state
                    .audio
                    .state
                    .editor
                    .cache
                    .clear_loading_source_if(&source_key);
            }
        }
    }
}

/// Consume `AudioPreloadResultEvent`s and install one closure per
/// event. Each closure mirrors the body of
/// `State::update_runtime_audio_preloads` for one
/// `AudioPreloadResult`.
#[allow(dead_code)]
pub fn audio_preload_handler_system(mut events: MessageReader<AudioPreloadResultEvent>) {
    for event in events.read() {
        let source_key = event.source_key.clone();
        let bytes = event.bytes.clone();
        install_async_result_fn(move |state: &mut State| {
            state
                .audio
                .state
                .runtime_preload
                .preloading_source_keys
                .remove(&source_key);
            if let Some(bytes) = bytes {
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .insert(source_key, bytes);
            }
        });
    }
}

/// Consume `LevelImportResultEvent`s and install one closure per
/// event. Each closure mirrors the body of
/// `State::update_level_imports` for one level bytes payload.
#[allow(dead_code)]
pub fn level_import_handler_system(mut events: MessageReader<LevelImportResultEvent>) {
    for event in events.read() {
        let bytes = event.bytes.clone();
        install_async_result_fn(
            move |state: &mut State| match state.import_level_egz(&bytes) {
                Ok(()) => {}
                Err(egz_error) => {
                    if let Err(binary_error) = state.import_level(&bytes) {
                        log::error!(
                            "Level import failed (.egz parse error: {egz_error}; binary parse error: {binary_error})"
                        );
                    }
                }
            },
        );
    }
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
    // Phase 6b: result handler systems + thread-local bridge tests
    // -----------------------------------------------------------------------

    /// Build a `State` synchronously for use in handler tests.
    /// Wraps `State::new_test().await` in `pollster::block_on`.
    fn new_test_sync() -> State {
        pollster::block_on(State::new_test())
    }

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
    fn async_result_fn_slot_drains_in_order() {
        // Verify the thread-local slot accepts multiple closures
        // and drains them in insertion order. Each closure
        // appends a value to a shared `Vec`; we check the
        // final order matches insertion order.
        let collected: std::sync::Arc<std::sync::Mutex<Vec<u32>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        for i in 0..5u32 {
            let c = std::sync::Arc::clone(&collected);
            install_async_result_fn(move |_state: &mut State| {
                c.lock().expect("test mutex poisoned").push(i);
            });
        }
        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        assert_eq!(*collected.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn async_result_fn_slot_drain_is_idempotent() {
        // After draining, a second drain with no new closures
        // is a no-op (no panic, no error).
        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        // No closures were installed, so this is a no-op.
        drain_pending_async_result_fns(&mut fake_state);
    }

    #[test]
    fn auth_handler_system_routes_to_complete_sign_out() {
        // Write an `AuthServiceResultEvent::SignedOut(Ok(()))` into
        // the buffer, run the handler system, drain the
        // thread-local slot, and verify the State was updated
        // (`auth.session = None`, `auth.pending = false`).
        use crate::events::async_results::AuthServiceResultEvent;
        let mut world = build_world();
        world.write_message(AuthServiceResultEvent::new(AuthServiceMessage::SignedOut(
            Ok(()),
        )));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(auth_handler_system);
        schedule.run(&mut world);

        let mut fake_state = new_test_sync();
        // The State starts with `pending = true` so the
        // SignedOut path has something to clear.
        fake_state.auth.pending = true;
        fake_state.auth.session = Some(test_auth_session());
        drain_pending_async_result_fns(&mut fake_state);
        assert!(!fake_state.auth.pending);
        assert!(fake_state.auth.session.is_none());
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

        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        let source_key = "Level1:audio.mp3".to_string();
        assert!(
            fake_state
                .audio
                .state
                .runtime_preload
                .preloaded_audio
                .contains_key(&source_key)
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

        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        let source_key = "Level1:audio.mp3".to_string();
        assert!(
            !fake_state
                .audio
                .state
                .runtime_preload
                .preloaded_audio
                .contains_key(&source_key)
        );
    }

    #[test]
    fn level_import_handler_system_is_safe_on_invalid_bytes() {
        // The handler should not panic on invalid level bytes;
        // the result is logged but the State remains usable.
        use crate::events::async_results::LevelImportResultEvent;
        let mut world = build_world();
        world.write_message(LevelImportResultEvent::new(vec![0xDE, 0xAD]));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(level_import_handler_system);
        schedule.run(&mut world);

        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        // No assertion on the level (the bytes are invalid) — we
        // just verify the State remains valid (no panic).
    }

    #[test]
    fn waveform_handler_system_drains_events() {
        // Verify the `waveform_handler_system` (which uses
        // `Messages::drain` instead of `MessageReader`) consumes
        // the event and installs a closure.
        use crate::events::async_results::WaveformResultEvent;
        let mut world = build_world();
        let result: WaveformResult = (
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Failed { bytes: None },
        );
        world.write_message(WaveformResultEvent::from_result(result));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(waveform_handler_system);
        schedule.run(&mut world);

        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        // No assertion on the audio cache (the State has no
        // current audio loaded) — we just verify the handler
        // ran and the closure was applied without panic.
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

        let mut fake_state = new_test_sync();
        drain_pending_async_result_fns(&mut fake_state);
        // No assertion on the cache (cancellation is a no-op).
    }

    // ----- helpers for the handler-system tests -----

    /// Synchronous test constructor for `State`. Used by handler
    /// tests that need a `&mut State` to drain the thread-local
    /// slot into, without spinning up the full async
    /// `State::new_test()`.
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
