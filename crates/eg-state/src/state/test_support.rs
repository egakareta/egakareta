/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Test harness for the eg-state `App`.
//!
//! [`TestApp`] is the Bevy-native equivalent of `State::new_test()` — it
//! builds a [`bevy_app::App`] with every eg-state plugin registered and
//! the per-frame [`StateSchedule`] wired in, without touching the heavy
//! GPU/surface resources. Tests that want to assert on the World, queries,
//! or resources use [`TestApp`] instead of constructing a full [`State`].
//!
//! The harness is intentionally minimal: no headless GPU, no window, no
//! surface. It exposes only the resources, events, schedules, and systems
//! that the plugin graph registers. Tests that need the heavy render
//! path (e.g. the wgpu buffers, the surface, the egui renderer) still
//! use [`State::new_test`].
//!
//! # Examples
//!
//! ```ignore
//! use eg_state::state::test_support::TestApp;
//! use eg_state::state::AppPhaseRes;
//! use eg_types::AppPhase;
//!
//! let mut app = TestApp::minimal();
//! app.world_mut().resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
//! app.tick();
//! assert_eq!(app.world().resource::<AppPhaseRes>().get(), AppPhase::Editor);
//! ```
//!
//! REFACTOR POINT(full-bevy): in the full-Bevy migration, the harness
//! becomes a thin wrapper around `bevy_app::App::new()` + the eg-state
//! plugin graph; the `State` god-object disappears entirely and the
//! `TestApp` is the only test entry point.
//!
//! [State]: crate::state::State
//! [StateSchedule]: crate::state::StateSchedule

use bevy_app::App;
use bevy_ecs::prelude::*;

use super::StateSchedule;
use super::plugins::{register_state_command_pipeline, setup_state_app};

/// Test harness for the eg-state plugin graph.
///
/// Wraps a [`bevy_app::App`] built with [`setup_state_app`] + the
/// per-frame [`StateSchedule`]. Provides ergonomic accessors for the
/// underlying [`World`] + [`App`] so tests can assert on resources,
/// events, and queries without going through the full [`State`] god
/// object.
///
/// Construct via [`TestApp::minimal`] (no GPU) or [`TestApp::with_app`]
/// (custom pre-wired [`App`]).
///
/// [`State`]: crate::state::State
pub struct TestApp {
    app: App,
}

impl TestApp {
    /// Build a `TestApp` with every eg-state plugin + the `StateSchedule`
    /// registered. No GPU, no surface, no window.
    ///
    /// This is the canonical entry point for headless eg-state tests. It
    /// mirrors what `State::new_test()` does for the legacy `State` —
    /// same plugin graph, same schedule wiring, no heavy render
    /// resources.
    pub fn minimal() -> Self {
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_command_pipeline(app.world_mut());
        Self { app }
    }

    /// Build a `TestApp` from a pre-wired `App`. Use this when the test
    /// needs a custom plugin set (e.g. an additional test-only plugin)
    /// before the harness is constructed.
    pub fn with_app(app: App) -> Self {
        Self { app }
    }

    /// Read-only access to the underlying `bevy_app::App`.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Mutable access to the underlying `bevy_app::App`.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Read-only access to the `World`.
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// Mutable access to the `World`.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// Run a single frame of the `App` via `app.update()`.
    ///
    /// In Phase 7-scaffold the App's default schedules are empty (no
    /// systems have been registered into `Update` yet), so this call
    /// is a no-op. The actual frame driver is the per-frame
    /// [`StateSchedule`]; use [`TestApp::tick_state`] to invoke the
    /// eg-state pipeline or [`TestApp::run_state_schedule`] to run
    /// only the schedule.
    pub fn tick(&mut self) {
        self.app.update();
    }

    /// Run a single frame of the eg-state pipeline.
    ///
    /// Run the same state, gameplay, and render-extraction schedules as the
    /// production Runtime frame driver.
    pub fn tick_state(&mut self) {
        super::plugins::run_state_schedule_in_world(self.world_mut());
    }

    /// Run the [`StateSchedule`] once on the World.
    ///
    /// Equivalent to [`TestApp::tick_state`], returning whether the
    /// schedule completed without panicking (Bevy 0.19's
    /// `try_run_schedule` is the underlying API).
    pub fn run_state_schedule(&mut self) -> bool {
        self.world_mut().try_run_schedule(StateSchedule).is_ok()
    }

    /// Assert the World contains a resource of type `T`. Panics with
    /// a readable message if the resource is missing.
    ///
    /// Convenience for the common "the plugin registered this resource"
    /// assertion. Equivalent to:
    /// ```ignore
    /// assert!(app.world().contains_resource::<T>());
    /// ```
    pub fn assert_resource_exists<T: Resource>(&self) {
        assert!(
            self.world().contains_resource::<T>(),
            "expected `World` to contain resource `{}`",
            std::any::type_name::<T>(),
        );
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::minimal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
        EditorObjectsRes, EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
        FrameTimeRes, MenuStateRes, PerfRes, SessionStateRes, UiStateRes,
    };
    use eg_types::{AppPhase, MenuState};

    #[test]
    fn test_app_minimal_seeds_state_plugin_resources() {
        let app = TestApp::minimal();
        app.assert_resource_exists::<eg_game::GameClock>();
        app.assert_resource_exists::<AppPhaseRes>();
        app.assert_resource_exists::<SessionStateRes>();
        app.assert_resource_exists::<EditorInputRes>();
        app.assert_resource_exists::<EditorObjectsRes>();
        app.assert_resource_exists::<EditorConfigRes>();
        app.assert_resource_exists::<EditorCameraRes>();
        app.assert_resource_exists::<EditorTriggerRes>();
        app.assert_resource_exists::<EditorTimelineRes>();
        app.assert_resource_exists::<EditorTimingRes>();
        app.assert_resource_exists::<EditorRuntimeRes>();
        app.assert_resource_exists::<EditorHistoryRes>();
        app.assert_resource_exists::<FrameTimeRes>();
        app.assert_resource_exists::<PerfRes>();
    }

    #[test]
    fn ui_state_resource_reads_authoritative_app_world_resources() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(MenuStateRes::new(MenuState {
                selected_level: 1,
                levels: vec!["First".to_string(), "Second".to_string()],
                preview_level_index: None,
                preview_sky_color: [0.1, 0.2, 0.3],
                preview_camera_position: [1.0, 2.0, 3.0],
                preview_camera_target: [0.0, 0.0, 0.0],
            }));
        app.world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(AppPhase::Editor);
        app.world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_level_name = Some("World-owned level".to_string());
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [4.0, 5.0, 6.0];
        app.world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 2.5;

        let resource = UiStateRes::from_world(app.world());
        let view = resource.view();

        assert!(view.is_editor);
        assert_eq!(
            view.available_levels,
            ["First".to_string(), "Second".to_string()]
        );
        assert_eq!(view.menu_level_name, Some("Second"));
        assert_eq!(view.level_name, Some("World-owned level"));
        assert_eq!(view.place_preview_position, [4.0, 5.0, 6.0]);
        assert_eq!(view.timeline_time_seconds, 2.5);
    }

    #[test]
    fn test_app_minimal_defaults_app_phase_to_menu() {
        let app = TestApp::minimal();
        assert_eq!(app.world().resource::<AppPhaseRes>().get(), AppPhase::Menu,);
    }

    #[test]
    fn test_app_default_constructor_matches_minimal() {
        let a = TestApp::default();
        let b = TestApp::minimal();
        assert_eq!(
            a.world().resource::<AppPhaseRes>().get(),
            b.world().resource::<AppPhaseRes>().get(),
        );
    }

    #[test]
    fn test_app_tick_is_a_safe_no_op() {
        let mut app = TestApp::minimal();
        app.tick();
        app.tick();
        app.tick();
    }

    #[test]
    fn test_app_run_state_schedule_succeeds_on_empty_world() {
        let mut app = TestApp::minimal();
        assert!(app.run_state_schedule());
    }

    #[test]
    fn test_app_registers_gameplay_schedule() {
        let mut app = TestApp::minimal();
        assert!(
            app.world_mut()
                .try_run_schedule(eg_game::GameplaySchedule)
                .is_ok()
        );
    }

    #[test]
    fn test_app_phase_transition_via_tick_state() {
        let mut app = TestApp::minimal();

        // Initial tick: no transition.
        app.tick_state();
        assert_eq!(
            app.world().resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Menu,
        );

        // Change the phase manually.
        app.world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(AppPhase::Playing);

        // StateSchedule detects the change and updates `previous_app_phase`.
        app.tick_state();
        assert_eq!(
            app.world().resource::<AppPhaseRes>().get(),
            AppPhase::Playing
        );
        assert_eq!(
            app.world().resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Playing,
        );

        // Subsequent tick with no change: no-op.
        app.tick_state();
        assert_eq!(
            app.world().resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Playing,
        );
    }
}
