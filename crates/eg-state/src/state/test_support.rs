/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Test harness for the eg-state `App`.
//!
//! [`TestApp`] is the canonical headless Bevy harness for eg-state -- it
//! builds a [`bevy_app::App`] with every eg-state plugin registered and
//! the per-frame [`StateSchedule`] wired in, without touching the heavy
//! GPU/surface resources. Tests that want to assert on the World, queries,
//! or resources use [`TestApp`] directly.
//!
//! The harness is intentionally minimal: no headless GPU, no window, no
//! surface. It exposes only the resources, events, schedules, and systems
//! that the plugin graph registers. Tests that need the heavy render
//! path (e.g. the wgpu buffers, the surface, the egui renderer) use
//! App-owned render resources and focused render fixtures.
//!
//! # Examples
//!
//! ```ignore
//! use eg_state::state::test_support::TestApp;
//! use bevy_state::prelude::State;
//! use eg_types::AppPhase;
//!
//! let mut app = TestApp::minimal();
//! app.world_mut().insert_resource(State::new(AppPhase::Editor));
//! app.tick();
//! assert_eq!(*app.world().resource::<State<AppPhase>>().get(), AppPhase::Editor);
//! ```
//!
//! REFACTOR POINT(full-bevy): in the full-Bevy migration, the harness
//! remains a thin wrapper around `bevy_app::App::new()` plus the eg-state
//! plugin graph; App-owned resources are the only state authority.
//!
//! [StateSchedule]: crate::state::StateSchedule

use bevy_app::App;
use bevy_ecs::prelude::*;

use super::StateSchedule;
use super::plugins::{register_state_schedule, setup_state_app};

/// Test harness for the eg-state plugin graph.
///
/// Wraps a [`bevy_app::App`] built with [`setup_state_app`] + the
/// per-frame [`StateSchedule`]. Provides ergonomic accessors for the
/// underlying [`World`] + [`App`] so tests can assert on resources,
/// events, and queries without constructing a monolithic compatibility
/// object.
///
/// Construct via [`TestApp::minimal`] (no GPU) or [`TestApp::with_app`]
/// (custom pre-wired [`App`]).
///
pub struct TestApp {
    app: App,
}

impl TestApp {
    /// Build a `TestApp` with every eg-state plugin + the `StateSchedule`
    /// registered. No GPU, no surface, no window.
    ///
    /// This is the canonical entry point for headless eg-state tests. It
    /// installs the plugin graph and schedule wiring used by App tests,
    /// without heavy render resources.
    pub fn minimal() -> Self {
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_schedule(app.world_mut());
        app.insert_resource(super::gameplay_world::ManualGameplayDriverRes);

        app.insert_resource(super::MenuStateRes::new(eg_types::MenuState {
            selected_level: 0,
            levels: eg_level::builtin_level_names(),
            preview_level_index: None,
            preview_sky_color: eg_types::LevelMetadata::default().sky_color,
            preview_camera_position: eg_types::DEFAULT_MENU_PREVIEW_CAMERA_POSITION,
            preview_camera_target: eg_types::DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
        }));

        super::gameplay_world::seed_app_gameplay_world(app.world_mut());

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

    /// Consume the harness and return its configured Bevy App.
    pub fn into_app(self) -> App {
        self.app
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
        super::plugins::run_state_schedule_system(self.world_mut());
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
        EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorObjectsRes,
        EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
        PerfRes, RenderExtractSchedule, RenderTrailVerticesRes, SessionStateRes, UiStateData,
    };
    use bevy_app::FixedUpdate;
    use bevy_state::prelude::State;
    use eg_game::TimelineSimulationRuntime;
    use eg_types::{AppPhase, Direction, EditorMode};

    #[test]
    fn test_app_minimal_seeds_state_plugin_resources() {
        let app = TestApp::minimal();
        app.assert_resource_exists::<eg_game::GameClock>();
        app.assert_resource_exists::<State<AppPhase>>();
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
    fn ui_state_data_reads_authoritative_editor_resources() {
        let mut app = TestApp::minimal();
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [4.0, 5.0, 6.0];
        app.world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 2.5;

        let data = UiStateData::from_world(app.world());
        let view = data;

        assert_eq!(view.place_preview_position, [4.0, 5.0, 6.0]);
        assert_eq!(view.timeline_time_seconds, 2.5);
    }

    #[test]
    fn test_app_minimal_defaults_app_phase_to_menu() {
        let app = TestApp::minimal();
        assert_eq!(
            *app.world().resource::<State<AppPhase>>().get(),
            AppPhase::Menu,
        );
    }

    #[test]
    fn test_app_default_constructor_matches_minimal() {
        let a = TestApp::default();
        let b = TestApp::minimal();
        assert_eq!(
            a.world().resource::<State<AppPhase>>().get(),
            b.world().resource::<State<AppPhase>>().get(),
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
        assert!(app.world_mut().try_run_schedule(FixedUpdate).is_ok());
    }

    #[test]
    fn test_app_phase_transition_via_tick_state() {
        let mut app = TestApp::minimal();

        app.tick_state();
        assert_eq!(
            *app.world().resource::<State<AppPhase>>().get(),
            AppPhase::Menu
        );

        // Change the phase manually.
        app.world_mut()
            .insert_resource(State::new(AppPhase::Playing));

        app.tick_state();
        assert_eq!(
            *app.world().resource::<State<AppPhase>>().get(),
            AppPhase::Playing
        );

        app.tick_state();
        assert_eq!(
            *app.world().resource::<State<AppPhase>>().get(),
            AppPhase::Playing
        );
    }
    #[test]
    fn test_app_state_schedule_stops_editor_playback_without_audio() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Null;
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .last_mode = Some(EditorMode::Place);

        let playback_runtime =
            TimelineSimulationRuntime::new([0.0, 2.0, 0.0], Direction::Forward, &[], &[]);
        {
            let mut timeline = app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.time_seconds = 2.0;
            timeline.0.clock.duration_seconds = 10.0;
            timeline.0.playback.playing = true;
            timeline.0.playback.runtime = Some(playback_runtime);
            timeline.0.playback.pending_seek_time_seconds = Some(2.0);
            timeline.0.playback.seek_resync_cooldown_seconds = 0.0;
        }

        assert!(app.run_state_schedule());

        let timeline = &app.world().resource::<EditorTimelineRes>().0;
        assert!(!timeline.playback.playing);
        assert!(timeline.playback.runtime.is_none());
        assert!(timeline.playback.pending_seek_time_seconds.is_none());
        assert_eq!(
            app.world().resource::<EditorInputRes>().0.mode,
            EditorMode::Place
        );
    }

    #[test]
    fn test_app_render_extract_builds_editor_playback_trail() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Place;

        let mut playback_runtime =
            TimelineSimulationRuntime::new([0.0, 2.0, 0.0], Direction::Forward, &[], &[]);
        playback_runtime.advance_to(0.65);
        {
            let mut timeline = app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.time_seconds = 0.65;
            timeline.0.clock.duration_seconds = 4.0;
            timeline.0.playback.playing = true;
            timeline.0.playback.runtime = Some(playback_runtime);
        }

        assert!(
            app.world_mut()
                .try_run_schedule(RenderExtractSchedule)
                .is_ok()
        );
        assert!(
            !app.world()
                .resource::<RenderTrailVerticesRes>()
                .0
                .is_empty()
        );
    }
}
