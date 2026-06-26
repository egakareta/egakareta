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

use super::state_resources::{
    AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
    EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, PerfRes,
    SessionStateRes,
};

/// Registers the headless gameplay systems. Currently a thin shim around
/// [`eg_game::GamePlugin`]; in later phases the gameplay plugin will
/// also add `SessionStateRes`-style resources and domain-specific event
/// handlers.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, _app: &mut App) {
        // The eg-game plugin registers its own resources and systems.
        // We don't currently re-expose them here; once eg-state reads
        // them via the world (Phase 2b) we will add
        // `app.add_plugins(eg_game::GamePlugin)` here.
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
    fn build(&self, _app: &mut App) {
        // `AppPhaseRes` is auto-registered here; `MenuStateRes` is added
        // by the `State` constructor (it needs level names from
        // `eg-level`).
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
        app.add_plugins((MenuPlugin, SessionPlugin, EditorPlugin, GameplayPlugin));
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
}
