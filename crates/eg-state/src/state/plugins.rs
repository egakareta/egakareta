/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Plugin shells for the eg-state `App`.
//!
//! Each plugin owns the resources + systems for one logical domain. Today
//! (Phase 2a) the plugins only register the resources from
//! [`super::state_resources`]; the systems that use those resources land
//! in Phase 2c/3+. The plugin skeleton establishes the boundaries so
//! subsystems can be moved into systems one at a time.
//!
//! Plugin graph:
//! - [`GameplayPlugin`] — wraps `eg_game::GamePlugin` + owns
//!   `GameplaySubsystem`-related resources (`SessionStateRes`-style
//!   fields that affect gameplay).
//! - [`EditorPlugin`] — owns the `Editor*Res` family (input, config,
//!   camera, triggers, timeline, timing, runtime, history, perf).
//! - [`SessionPlugin`] — owns `SessionStateRes` (level/music/creator
//!   metadata, settings, practice mode).
//! - [`MenuPlugin`] — owns `MenuStateRes` + `AppPhaseRes` (top-level
//!   navigation state).
//! - [`StatePlugins`] — aggregator that registers all of the above in
//!   dependency order.
//!
//! REFACTOR POINT(full-bevy): the full-Bevy migration promotes these
//! plugin shells to the actual plugin graph (with `RenderPlugin` /
//! `AudioPlugin` / `AuthPlugin` / `UiPlugin` added), the systems they
//! register move into `bevy_render` / `bevy_egui` / `bevy_audio`, and
//! the schedule labels here are replaced by Bevy's standard
//! `Update` / `FixedUpdate` / `OnEnter` schedules.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

use super::render::plugin::RenderPlugin;
use super::state_resources::{
    AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
    EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, PerfRes,
    SessionStateRes,
};
use super::{StateSchedule, transitions};

/// Registers the headless gameplay systems. Currently a thin shim around
/// [`eg_game::GamePlugin`]; in later phases the gameplay plugin will
/// also add `SessionStateRes`-style resources and domain-specific event
/// handlers.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(eg_game::GamePlugin);
    }
}

/// Registers the editor-domain resources. No systems yet — the editor
/// mutation loop stays on `State` until Phase 2c, then moves into systems
/// here. The resources are registered so the resource mirror is
/// observable from the start.
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInputRes>()
            .init_resource::<EditorConfigRes>()
            .init_resource::<EditorCameraRes>()
            .init_resource::<EditorTriggerRes>()
            .init_resource::<EditorTimelineRes>()
            .init_resource::<EditorTimingRes>()
            .init_resource::<EditorRuntimeRes>()
            .init_resource::<EditorHistoryRes>()
            .init_resource::<PerfRes>();
        super::input_bridge::register_platform_input_resources(app.world_mut());
    }
}

/// Registers session-related resources (level/music/creator metadata,
/// settings, practice mode).
pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionStateRes>();
    }
}

/// Registers menu + app-phase resources. Owns the [`AppPhaseRes`] so
/// the top-level navigation state is the first thing initialized.
///
/// `MenuStateRes` doesn't have a `Default` impl (`MenuState` itself does
/// not derive `Default`), so it must be inserted explicitly by the
/// `State` constructor. [`AppPhaseRes`] is auto-initialized to
/// `AppPhase::Menu`.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppPhaseRes>();
        // `MenuStateRes` is added by the `State` constructor because it
        // needs level names from `eg-level`.
    }
}

/// Aggregator plugin. Add this to an `App` to register every eg-state
/// plugin in dependency order. Equivalent to a hand-written
/// `app.add_plugins((MenuPlugin, SessionPlugin, EditorPlugin, GameplayPlugin))`
/// — kept as a single plugin so the order is centralized and the full
/// eg-state `App` is built with one call.
pub struct StatePlugins;

impl Plugin for StatePlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MenuPlugin,
            SessionPlugin,
            EditorPlugin,
            GameplayPlugin,
            // Rendering extraction plugin. Registers the POD
            // resources (RenderUniformsRes, RenderFrameInputRes,
            // RenderResizeRequest) and the system sets (RenderSet).
            // State construction inserts the authoritative heavy
            // resources (RenderGpuRes, RenderMeshesRes,
            // RenderSurfaceHostRes) after wgpu initialization.
            RenderPlugin,
        ));
    }
}

/// One-shot setup: add all eg-state plugins + register the system set
/// ordering. Use this when constructing a `bevy_app::App` for eg-state
/// (full-ECS harness) or for tests.
///
/// In Bevy 0.19, per-schedule `configure_sets(...)` must be called on the
/// `Schedule` itself, not on `App`. The set ordering is therefore
/// configured per-schedule by callers (see [`super::sets::StateSet`] for
/// the per-set semantics). This entry point exists as the canonical
/// "give me a fully wired eg-state `App`" helper and is where
/// `app.add_schedule(StateUpdate::default())` + the schedule's
/// `configure_sets` will be wired in Phase 2c.
pub fn setup_state_app(app: &mut App) {
    app.add_plugins(StatePlugins);
}

/// Drive the eg-state schedule on a World.
///
/// Phase 7 keeps the legacy `State` object as the compatibility owner for
/// rendering and input, but Runtime now controls the frame order explicitly:
/// `app.update()` runs App systems, then Runtime invokes this helper before
/// draining App-world `PendingCommands` into the legacy State.
pub fn run_state_schedule_in_world(world: &mut World) {
    transitions::phase_transition_system(world.resource_mut::<AppPhaseRes>());
    let _ = world.try_run_schedule(StateSchedule);
}

/// Register all default-constructible eg-state resources on a `World`.
///
/// `State` carries a `World` directly (Phase 2b: see
/// [`crate::state::State::world`]). Use this helper to bring the world
/// up to the same state as [`setup_state_app`] without going through an
/// `App` first. Note: this only handles default-constructible resources.
/// `MenuStateRes` (which needs level names) is not registered here and
/// must be inserted by the caller with [`MenuStateRes::new`].
pub fn register_state_resources(world: &mut World) {
    world.init_resource::<AppPhaseRes>();
    world.init_resource::<SessionStateRes>();
    world.init_resource::<EditorInputRes>();
    world.init_resource::<EditorConfigRes>();
    world.init_resource::<EditorCameraRes>();
    world.init_resource::<EditorTriggerRes>();
    world.init_resource::<EditorTimelineRes>();
    world.init_resource::<EditorTimingRes>();
    world.init_resource::<EditorRuntimeRes>();
    world.init_resource::<EditorHistoryRes>();
    world.init_resource::<PerfRes>();
    super::input_bridge::register_platform_input_resources(world);
    // Phase 4a: rendering extraction plugin's POD resources. The
    // heavy resources (`RenderGpuRes`, `RenderMeshesRes`,
    // `RenderSurfaceHostRes`) are not registered here — they are
    // defined in `state::render::resources` and will be inserted in
    // Phase 4b/4c when the legacy `State::render` field ownership
    // moves into the world.
    super::render::plugin::register_render_pod_resources(world);
}

/// Register the per-message command-pipeline resources + the
/// [`StateSchedule`] on a `World`.
///
/// Phase 3: alongside the eg-state resources, this also initializes
/// the `Messages<T>` buffers for every event type and the
/// [`crate::events::PendingCommands`] resource, then adds the
/// [`StateSchedule`] to the world. The schedule contains the
/// per-message drain systems that convert events to legacy commands
/// and stage them in `PendingCommands` for Runtime to forward to
/// `State::dispatch` / `State::dispatch_editor` during the compatibility
/// window.
///
/// Phase 4a: the schedule also contains the
/// `mirror_pod_state_via_bridge` system from
/// `state::render::plugin` that drains the per-frame mirror closure
/// installed by `State::tick_state` into the POD render resources.
///
/// Phase 6a: the schedule also contains the
/// `poll_channels_system` from `state::async_bridge` that drains
/// the five legacy `mpsc` receivers (auth, audio import,
/// waveform, audio preload, level import) and emits the
/// corresponding async-result events into the World. The channel
/// wrappers themselves are inserted lazily by
/// `install_async_bridge_from_state` (when the legacy `State`
/// fields are wired into the world).
///
/// `State` constructs its world via
/// [`crate::state::lifecycle::build_state_world`] which calls this
/// helper, so callers don't need to invoke it directly.
pub fn register_state_command_pipeline(world: &mut World) {
    // Message resources for every command event type. The
    // `register_command_events` helper takes an `App`; we use the
    // `World`-flavoured helper to keep this function working on a
    // bare world (no `App`).
    crate::events::register_command_events_world(world);

    // Phase 6a: register the async-bridge `Messages<T>` resources
    // (the channel wrappers are inserted lazily by
    // `install_async_bridge_from_state`).
    crate::state::async_bridge::register_state_async_bridge_world(world);

    // Build the per-frame schedule that drains the per-event
    // message resources into `PendingCommands`, then runs the
    // Phase 4a render mirror bridge, the Phase 6a channel poll
    // systems, and the Phase 6b result handler systems. The
    // schedule is added to the world's scheduler so
    // `world.run_schedule(...)` (used by `State::tick_state`) can
    // invoke it.
    let mut schedule = bevy_ecs::schedule::Schedule::new(crate::state::StateSchedule);
    crate::events::add_command_systems(&mut schedule);
    crate::state::async_bridge::add_poll_channels_systems(&mut schedule);
    crate::state::async_bridge::add_apply_async_results_systems(&mut schedule);
    crate::state::render::plugin::add_mirror_render_systems(&mut schedule);
    world.add_schedule(schedule);
}
