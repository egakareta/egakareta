/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::load_menu_textures;
use crate::platform::pipeline::FramePipeline;
use bevy_app::App;
use bevy_ecs::world::World;
use eg_state::events::PendingCommands;
use eg_state::state::EditorConfigRes;
use eg_state::state::EditorInputRes;
use eg_state::state::EditorTimingRes;
use eg_state::state::SessionStateRes;
use eg_state::state::State;
use eg_state::state::enqueue_platform_input;
use eg_state::state::register_state_command_pipeline;
use eg_state::state::run_state_schedule_in_world;
use eg_state::state::setup_state_app;
use eg_ui::{
    drain_ui_command_queue, enqueue_ui_commands, register_ui_resources_world, sync_egui_context,
};

const SORA_TTF: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/Sora.ttf"
));

/// Runtime owns the winit-facing frame loop.
///
/// Phase 7: alongside the legacy `state: State` and
/// `pipeline: FramePipeline` fields, the runtime owns a
/// `bevy_app::App` wired with the eg-state plugin graph (see
/// [`setup_state_app`]). The winit redraw path now calls
/// [`Runtime::run_app_frame`], which owns the App update and command
/// drain before handing the frame back to the compatibility renderer.
///
/// The App world is the production command/schedule driver during the
/// compatibility window. Legacy `State` fields still own render/input data
/// until those resources move fully into the App.
///
/// REFACTOR POINT(full-bevy): when full Bevy lands, the `state`
/// and `pipeline` fields disappear; the `App` owns the egui
/// context + render resources and the winit loop calls
/// `runtime.app.update()` directly.
pub struct Runtime {
    pub state: State,
    pub pipeline: FramePipeline,
    pub app: App,
}

impl Runtime {
    pub fn new(state: State) -> Self {
        let egui_ctx = egui::Context::default();

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "sora".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(SORA_TTF)),
        );
        fonts.font_data.insert(
            "sora_thin".to_owned(),
            std::sync::Arc::new(
                egui::FontData::from_static(SORA_TTF).tweak(egui::FontTweak {
                    coords: egui::epaint::text::VariationCoords::new([(b"wght", 100.0)]),
                    ..Default::default()
                }),
            ),
        );
        fonts
            .families
            .entry(egui::FontFamily::Name("sora_thin".into()))
            .or_default()
            .insert(0, "sora_thin".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "sora".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "sora".to_owned());

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        egui_ctx.set_fonts(fonts);

        let egui_renderer = state.create_egui_renderer();
        let menu_textures = load_menu_textures(&egui_ctx);
        let pipeline = FramePipeline::new(egui_ctx, egui_renderer, menu_textures);

        // Build the Bevy `App` with the eg-state plugin graph. The
        // App owns the single ECS `World`; legacy `State` fields are
        // bridged into it each frame during the compatibility window.
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_command_pipeline(app.world_mut());
        register_ui_resources_world(app.world_mut());

        // Seed `EditorConfigRes` from the legacy `State` so the
        // resource starts in sync with user-loaded `app_settings`
        // (snap config, selected block id, recent blocks). Without
        // this, `EditorPlugin` Default-initializes the resource and
        // the change-guarded `sync_app_editor_config_state` below
        // would clobber user settings on the first frame: the
        // resource default differs from the legacy user value, the
        // setter fires, and the user value is overwritten and
        // re-persisted. Seeding once at construction makes the first
        // frame a true no-op for unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_config_resource());

        // Seed `EditorTimingRes` from the legacy `State` for the
        // bpm-tap carve-out (see `events::bpm`). The resource's
        // bpm_tap_times/bpm_tap_result would otherwise be
        // Default-initialized (empty list, no result) on the first
        // frame; the change-guarded `sync_app_editor_timing_state`
        // would then see a diff vs the legacy state, fire the
        // setter, and clobber any in-flight tap history. Seeding
        // once at construction makes the first frame a no-op for
        // unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_timing_resource());

        // Seed the carved-out `SessionStateRes` fields from the
        // legacy `State.session` (see `events::session`). The
        // carved-out fields (`editor_show_metadata`,
        // `editor_show_settings`, `editor_settings_section`,
        // `editor_keybind_capture_action`) are written to the
        // resource directly by their Bevy handlers; the
        // change-guarded `sync_app_session_state` would otherwise
        // see a default-vs-legacy diff on the first frame and
        // clobber any user-driven UI-window state. Seeding once
        // at construction makes the first frame a no-op for
        // unchanged fields.
        app.world_mut()
            .insert_resource(state.session_state_resource());

        Self {
            state,
            pipeline,
            app,
        }
    }

    /// Drive one frame through the compatibility `FramePipeline` while
    /// routing UI-originated messages through Bevy's `App` world.
    pub fn run_app_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let Runtime {
            state,
            pipeline,
            app,
        } = self;
        sync_egui_context(app.world_mut(), pipeline.ctx());

        pipeline.run_frame(state, raw_input, |state, ui_commands| {
            enqueue_ui_commands(app.world_mut(), ui_commands);
            drain_ui_command_queue(app.world_mut());
            app.update();
            state.prepare_app_world_for_state_schedule(app.world_mut());
            run_state_schedule_in_world(app.world_mut());
            Self::sync_app_editor_input_state(state, app.world());
            Self::sync_app_editor_config_state(state, app.world());
            Self::sync_app_editor_timing_state(state, app.world());
            Self::sync_app_session_state(state, app.world());
            Self::dispatch_app_pending_commands(state, app.world_mut());
            state.drain_pending_async_results();
        })
    }

    fn sync_app_editor_input_state(state: &mut State, world: &World) {
        let input = &world.resource::<EditorInputRes>().0;
        let shift_held = input.shift_held;
        let ctrl_held = input.ctrl_held;
        let alt_held = input.alt_held;
        let pan_up_held = input.pan_up_held;
        let pan_down_held = input.pan_down_held;
        let pan_left_held = input.pan_left_held;
        let pan_right_held = input.pan_right_held;

        state.set_editor_shift_held(shift_held);
        state.set_editor_ctrl_held(ctrl_held);
        state.set_editor_alt_held(alt_held);
        state.set_editor_pan_up_held(pan_up_held);
        state.set_editor_pan_down_held(pan_down_held);
        state.set_editor_pan_left_held(pan_left_held);
        state.set_editor_pan_right_held(pan_right_held);
    }

    /// Sync the legacy `State.editor.config` from the App-world
    /// `EditorConfigRes`. The `snap_*` fields (carved out in
    /// `events::snap`) and `selected_block_id` (carved out in
    /// `events::editor_mode::handle_set_block_id`) are written to the
    /// resource directly by their Bevy handlers, so the resource is the
    /// source of truth at this point in the frame.
    ///
    /// Unlike the modifier-held sync above, the legacy setters for
    /// these fields have side effects (persist `app_settings` to disk,
    /// re-snap the selected block, rebuild cursor vertices, remember
    /// the block in the recent-blocks list). To avoid repeating those
    /// side effects every frame, the sync only invokes the setter when
    /// the value actually differs from the legacy state — once the
    /// legacy field is brought back in sync, no further setter calls
    /// fire until the user changes the setting again. The resource is
    /// seeded from the legacy state in `Runtime::new`, so the first
    /// frame after construction is a no-op for unchanged fields.
    fn sync_app_editor_config_state(state: &mut State, world: &World) {
        let config = &world.resource::<EditorConfigRes>().0;
        if state.editor_selected_block_id() != &config.selected_block_id {
            state.set_editor_block_id(config.selected_block_id.clone());
        }
        if state.editor_snap_to_grid() != config.snap_to_grid {
            state.set_editor_snap_to_grid(config.snap_to_grid);
        }
        if state.editor_snap_step() != config.snap_step {
            state.set_editor_snap_step(config.snap_step);
        }
        if state.editor_snap_rotation() != config.snap_rotation {
            state.set_editor_snap_rotation(config.snap_rotation);
        }
        if state.editor_snap_rotation_step_degrees() != config.snap_rotation_step_degrees {
            state.set_editor_snap_rotation_step(config.snap_rotation_step_degrees);
        }
    }

    /// Sync the legacy `State.editor.timing.bpm_tap_*` from the
    /// App-world `EditorTimingRes`. The `bpm_tap_times` and
    /// `bpm_tap_result` fields are carved out in `events::bpm`:
    /// their handlers compute the new state in the App world and
    /// write the resource directly.
    ///
    /// The sync is change-guarded so a no-op frame does not
    /// re-invoke the (relatively cheap, but still mutating) legacy
    /// setter. The resource is seeded at `Runtime::new` from the
    /// legacy state, so the first frame after construction is a
    /// no-op for unchanged fields.
    fn sync_app_editor_timing_state(state: &mut State, world: &World) {
        let timing = &world.resource::<EditorTimingRes>().0;
        if state.editor_bpm_tap_times() != timing.bpm_tap_times.as_slice()
            || state.editor_bpm_tap_result() != timing.bpm_tap_result
        {
            state.set_editor_bpm_tap_state(timing.bpm_tap_times.clone(), timing.bpm_tap_result);
        }
    }

    /// Sync the legacy `State.session` simple UI-window fields from
    /// the App-world `SessionStateRes`. The carved-out fields
    /// (`editor_show_metadata`, `editor_show_settings`,
    /// `editor_settings_section`, `editor_keybind_capture_action`)
    /// are written to the resource directly by their Bevy handlers
    /// in `events::session`. The sync is change-guarded so a no-op
    /// frame does not re-invoke the legacy setters. The resource
    /// is seeded at `Runtime::new` from the legacy state, so the
    /// first frame after construction is a no-op for unchanged
    /// fields.
    fn sync_app_session_state(state: &mut State, world: &World) {
        let session = &world.resource::<SessionStateRes>();
        if state.editor_show_metadata() != session.editor_show_metadata {
            state.set_editor_show_metadata(session.editor_show_metadata);
        }
        if state.editor_show_settings() != session.editor_show_settings {
            state.set_editor_show_settings(session.editor_show_settings);
        }
        if state.editor_settings_section() != session.editor_settings_section {
            state.set_editor_settings_section(session.editor_settings_section);
        }
        if state.editor_keybind_capture_action() != session.editor_keybind_capture_action.as_ref() {
            state.set_editor_keybind_capture_action(session.editor_keybind_capture_action.clone());
        }
    }

    fn dispatch_app_pending_commands(state: &mut State, world: &mut World) {
        let mut pending = std::mem::take(&mut *world.resource_mut::<PendingCommands>());
        for command in pending.app_commands.drain(..) {
            state.dispatch(command);
        }
        for command in pending.editor_commands.drain(..) {
            state.dispatch_editor(command);
        }
    }

    /// Queue platform input in the App world, then apply it through the
    /// legacy State input path during the compatibility window.
    pub fn process_input_event(&mut self, event: eg_state::commands::InputEvent) {
        enqueue_platform_input(self.app.world_mut(), event.clone());
        self.state.process_input_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eg_state::state::State;
    use eg_state::state::{
        AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
        EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, PerfRes,
        PlatformInputQueue, SessionStateRes,
    };
    use eg_ui::EguiContextRes;

    #[tokio::test]
    async fn test_runtime_new() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        // Verify the egui context was initialized
        let ctx = runtime.pipeline.ctx();
        assert!(ctx.pixels_per_point() > 0.0);

        // Verify the state was correctly moved into the runtime
        assert!(runtime.state.is_menu());
    }

    // ---- Phase 7 tests ----
    //
    // These pin the Phase 7-scaffold contract: `Runtime::new`
    // builds a `bevy_app::App` with the eg-state plugin graph,
    // and `run_app_frame` drives `app.update()` without
    // panicking. Future sub-phases (7-1: schedule migration,
    // 7-2: frame driver, 7-3: pipeline split) extend these
    // tests with system-level assertions.

    /// `Runtime::new` builds an `App` whose `World` contains every
    /// resource that `StatePlugins` is responsible for registering.
    /// `AppPhaseRes` defaults to `AppPhase::Menu`; gameplay, editor,
    /// session, and render resources are installed by their plugins.
    #[tokio::test]
    async fn runtime_app_has_state_plugin_resources() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        let world = runtime.app.world();
        assert!(
            world.contains_resource::<eg_game::GameClock>(),
            "GameClock registered by GameplayPlugin"
        );
        assert!(
            world.contains_resource::<AppPhaseRes>(),
            "AppPhaseRes registered by MenuPlugin"
        );
        assert!(
            world.contains_resource::<SessionStateRes>(),
            "SessionStateRes registered by SessionPlugin"
        );
        assert!(
            world.contains_resource::<EditorInputRes>(),
            "EditorInputRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorConfigRes>(),
            "EditorConfigRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorCameraRes>(),
            "EditorCameraRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTriggerRes>(),
            "EditorTriggerRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTimelineRes>(),
            "EditorTimelineRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTimingRes>(),
            "EditorTimingRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorRuntimeRes>(),
            "EditorRuntimeRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorHistoryRes>(),
            "EditorHistoryRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PerfRes>(),
            "PerfRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PlatformInputQueue>(),
            "PlatformInputQueue registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EguiContextRes>(),
            "EguiContextRes registered by Runtime UI bridge"
        );
    }

    /// `register_state_command_pipeline` adds the per-frame
    /// `StateSchedule` to the App's World via Bevy's
    /// `World::add_schedule`. Pin the schedule is reachable from
    /// the App so the future 7-1 sub-phase can move the
    /// `State::tick_state` invocation here.
    #[tokio::test]
    async fn runtime_app_registers_state_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // The schedule is named `StateSchedule` (defined in
        // `eg_state::state::sets`). Reaching it via the
        // `try_run_schedule` API is the cheapest way to assert
        // registration without exposing a Bevy-typed getter.
        let world = runtime.app.world_mut();
        let result = world.try_run_schedule(eg_state::state::StateSchedule);
        assert!(
            result.is_ok(),
            "StateSchedule should be registered on the App's World"
        );
    }

    /// `GameplayPlugin` installs the real eg-game ECS schedule into
    /// the App's World. This is the first single-source cut: gameplay
    /// resources and systems exist on the App, not only in the legacy
    /// `GameState` wrapper path.
    #[tokio::test]
    async fn runtime_app_registers_gameplay_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let result = runtime
            .app
            .world_mut()
            .try_run_schedule(eg_game::GameplaySchedule);
        assert!(
            result.is_ok(),
            "GameplaySchedule should be registered on the App's World"
        );
    }

    /// `Runtime::run_app_frame` is safe to call and produces normal
    /// egui output while driving the App first.
    #[tokio::test]
    async fn runtime_run_app_frame_returns_output() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let output = runtime.run_app_frame(egui::RawInput::default());
        assert!(output.pixels_per_point > 0.0);
    }

    /// `Runtime::run_app_frame` mirrors the live egui context into the
    /// App's World before the frame update. This gives Bevy UI systems
    /// an App-world anchor while the compatibility renderer still owns
    /// actual egui painting.
    #[tokio::test]
    async fn runtime_run_app_frame_syncs_egui_context_to_app_world() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        assert!(
            runtime.app.world().resource::<EguiContextRes>().0.is_none(),
            "egui context starts empty before the first frame"
        );

        let output = runtime.run_app_frame(egui::RawInput::default());
        let app_ctx = runtime
            .app
            .world()
            .resource::<EguiContextRes>()
            .0
            .as_ref()
            .expect("egui context mirrored into App World");

        assert_eq!(app_ctx.pixels_per_point(), output.pixels_per_point);
    }

    /// Platform input now enters the App world before the legacy
    /// `State::process_input_event` compatibility path consumes it.
    #[tokio::test]
    async fn runtime_process_input_event_queues_app_world_input() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.process_input_event(eg_state::commands::InputEvent::Resize {
            width: 640,
            height: 360,
        });

        let queue = runtime.app.world().resource::<PlatformInputQueue>();
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.events()[0],
            eg_state::commands::InputEvent::Resize {
                width: 640,
                height: 360
            }
        ));
        assert_eq!(runtime.state.surface_width(), 640);
        assert_eq!(runtime.state.surface_height(), 360);
    }

    /// `Runtime::run_app_frame` drives the App's `Update` schedule,
    /// then explicitly runs the eg-state `StateSchedule` on the App's
    /// World before draining pending commands.
    #[tokio::test]
    async fn runtime_run_app_frame_updates_app_state_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Playing);
        assert_eq!(app_phase.previous_app_phase, eg_types::AppPhase::Playing);
    }

    /// Plain `app.update()` no longer drives the mirror `StateSchedule`.
    /// Runtime owns that order so command draining stays visible in one
    /// frame-driver method.
    #[tokio::test]
    async fn runtime_app_update_does_not_run_state_schedule_directly() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);

        runtime.app.update();

        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Playing);
        assert_eq!(app_phase.previous_app_phase, eg_types::AppPhase::Menu);
    }

    /// `Runtime::run_app_frame` drains App-world messages and applies
    /// the recovered legacy command to `State` during the compatibility
    /// window. UI commands enter through this path before legacy dispatch.
    #[tokio::test]
    async fn runtime_run_app_frame_dispatches_app_world_commands() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(eg_state::events::navigation::ToggleEditor);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.is_editor());
    }

    /// `Runtime` keeps App-world setup from mutating legacy State before
    /// the frame driver explicitly applies queued commands.
    #[tokio::test]
    async fn runtime_app_world_setup_does_not_mutate_state() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        // The App's World has the plugin resources.
        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Menu);

        // Pin that the State is still in the Menu phase after
        // the App was built (no setup cross-talk).
        assert!(runtime.state.is_menu());
    }

    // --- Phase 7 carve-out sync tests (snap domain) ---
    //
    // The `snap` event handlers in `eg_state::events::snap` write to
    // `EditorConfigRes` directly. `Runtime::run_app_frame` then calls
    // `sync_app_editor_config_state` to copy the resource to the legacy
    // `State.editor.config` fields before any dispatch. The tests below
    // pin the end-to-end path: message → App-world resource → legacy
    // State field. They are intentionally written against the public
    // Runtime API (not the internal `sync_app_editor_config_state`
    // helper) so they survive internal refactors.

    use eg_state::events::snap::{
        SetSnapRotation, SetSnapRotationStep, SetSnapStep, SetSnapToGrid,
    };

    /// Writing a `SetSnapToGrid` message reaches the App world, mutates
    /// `EditorConfigRes.0.snap_to_grid`, and the per-frame sync propagates
    /// the value to the legacy `State.editor.config.snap_to_grid` so the
    /// existing snap-aware rendering / persistence code paths still work.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_to_grid_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed an opposite value on both sides to prove the message
        // actually flows through.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.state.set_editor_snap_to_grid(true);

        // Flip both the message resource and the runtime's pending message
        // so the next frame's schedule runs the carve-out handler.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.app.world_mut().write_message(SetSnapToGrid(false));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid,
            "SetSnapToGrid(false) handler must mutate EditorConfigRes"
        );
        // Legacy state was synced to match the resource.
        assert!(
            !runtime.state.editor_snap_to_grid(),
            "sync_app_editor_config_state must propagate EditorConfigRes \
             into the legacy State.editor.config.snap_to_grid field"
        );
    }

    /// `SetSnapStep` follows the same carve-out: the resource value
    /// is the source of truth, and the legacy state field reflects it
    /// after the per-frame sync.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_step_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let new_step = 0.5_f32;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_step = 1.0;
        runtime.state.set_editor_snap_step(1.0);

        runtime.app.world_mut().write_message(SetSnapStep(new_step));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_step
                - new_step)
                .abs()
                < 1e-6,
            "SetSnapStep handler must mutate EditorConfigRes.snap_step"
        );
        assert!(
            (runtime.state.editor_snap_step() - new_step).abs() < 1e-6,
            "sync_app_editor_config_state must propagate snap_step to legacy state"
        );
    }

    /// Rotation-snap fields follow the carve-out too.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_rotation_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.state.set_editor_snap_rotation(true);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_rotation = true;

        runtime
            .app
            .world_mut()
            .write_message(SetSnapRotation(false));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(!runtime.state.editor_snap_rotation());
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_rotation
        );
    }

    /// `SetSnapRotationStep` propagates as a f32.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_rotation_step_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.state.set_editor_snap_rotation_step(15.0);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_rotation_step_degrees = 15.0;

        runtime
            .app
            .world_mut()
            .write_message(SetSnapRotationStep(45.0));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!((runtime.state.editor_snap_rotation_step_degrees() - 45.0).abs() < 1e-6);
        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_rotation_step_degrees
                - 45.0)
                .abs()
                < 1e-6
        );
    }

    /// When the message and the resource are unchanged, the sync must
    /// not re-invoke the legacy setter (which has side effects: it
    /// persists `app_settings` to storage). Pin the setter side-effect
    /// guard by checking the State field matches the resource after a
    /// no-op frame.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_snap() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed both sides to a known state, then run a frame with no
        // snap message. The sync guard must not re-invoke the legacy
        // setter, so the State field stays at the value we set.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.state.set_editor_snap_to_grid(true);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message written -> handler does not run -> resource
        // unchanged. State value must remain `true` (the sync guard
        // sees no diff and does not touch the State field).
        assert!(runtime.state.editor_snap_to_grid());
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid
        );
    }

    /// Regression: `EditorConfigRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded config sync
    /// does not clobber user-loaded `app_settings` on the first
    /// frame. Before the seeding fix, the resource was
    /// Default-initialized (`snap_to_grid = true`) while the legacy
    /// state held a user value (`false`); the first frame's sync saw
    /// a diff and overwrote the user value back to the default (and
    /// re-persisted it). This test pins the seeding + the no-clobber
    /// property for the snap config.
    #[tokio::test]
    async fn runtime_seeds_editor_config_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-loaded settings: snap-to-grid disabled and a non-default
        // step. The resource defaults are `snap_to_grid = true` and
        // `snap_step = 1.0`, so these differ from the defaults and
        // would be clobbered without the seeding fix.
        state.set_editor_snap_to_grid(false);
        state.set_editor_snap_step(0.25);
        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user value,
        // not the `EditorConfigState` default.
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid,
            "EditorConfigRes must be seeded from legacy State at Runtime::new"
        );
        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_step
                - 0.25)
                .abs()
                < 1e-6,
            "EditorConfigRes.snap_step must be seeded from legacy State"
        );

        // Run a frame with no snap message. The change-guarded sync
        // must see no diff and must NOT invoke the legacy setter, so
        // the user value survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime.state.editor_snap_to_grid(),
            "first-frame sync must not clobber user snap_to_grid setting"
        );
        assert!(
            (runtime.state.editor_snap_step() - 0.25).abs() < 1e-6,
            "first-frame sync must not clobber user snap_step setting"
        );
    }

    /// `SetBlockId` carve-out end-to-end: writing the message reaches
    /// the App world, mutates `EditorConfigRes.0.selected_block_id`,
    /// and the per-frame sync propagates the value to the legacy
    /// `State.editor.config.selected_block_id` (which also remembers
    /// the block in `recent_block_ids` and persists). The resource is
    /// the source of truth; the legacy field mirrors it after the sync.
    #[tokio::test]
    async fn runtime_carve_out_set_block_id_propagates_to_legacy_state() {
        use eg_blocks::BlockId;
        use eg_state::events::editor_mode::SetBlockId;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let new_id = BlockId::new("core/lava");
        // Precondition: the new id differs from the seeded value.
        assert_ne!(
            runtime.state.editor_selected_block_id(),
            &new_id,
            "test precondition: new block id must differ from current"
        );

        runtime
            .app
            .world_mut()
            .write_message(SetBlockId(new_id.clone()));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_id,
            new_id,
            "SetBlockId handler must mutate EditorConfigRes.selected_block_id"
        );
        // Legacy state was synced to match the resource.
        assert_eq!(
            runtime.state.editor_selected_block_id(),
            &new_id,
            "sync_app_editor_config_state must propagate selected_block_id to legacy state"
        );
        // The legacy setter also remembers the block in recent_block_ids,
        // proving the change-guarded sync invoked the full setter (not
        // just a field copy).
        assert_eq!(
            runtime.state.editor_recent_block_ids().first(),
            Some(&new_id),
            "set_editor_block_id side effect must remember the block in recent_block_ids"
        );
    }

    /// `SetBlockId` carve-out no-op frame: when no `SetBlockId` message
    /// is written and the resource matches the legacy state (the
    /// seeded invariant), the sync must not re-invoke
    /// `set_editor_block_id`. Pin the side-effect guard by confirming
    /// the recent-blocks list is not reordered when nothing changed.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_block_id() {
        use eg_blocks::BlockId;

        let mut state = State::new_test().await;
        // Seed a known block id + recent list via the legacy setter so
        // both the resource (seeded at Runtime::new) and the legacy
        // state start in sync.
        state.set_editor_block_id(BlockId::new("core/lava"));
        let recent_before = Vec::from(state.editor_recent_block_ids());
        let mut runtime = Runtime::new(state);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked -> recent list stable.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_id,
            BlockId::new("core/lava")
        );
        assert_eq!(
            runtime.state.editor_selected_block_id(),
            &BlockId::new("core/lava")
        );
        assert_eq!(
            runtime.state.editor_recent_block_ids(),
            recent_before.as_slice(),
            "no-op frame must not reorder recent_block_ids"
        );
    }

    // --- Phase 3.7 / BPM carve-out tests ---
    //
    // The `bpm` event handlers in `eg_state::events::bpm` write
    // directly to `EditorTimingRes.0.bpm_tap_times` and
    // `bpm_tap_result`. `Runtime::run_app_frame` then calls
    // `sync_app_editor_timing_state` to copy the resource to the
    // legacy `State.editor.timing` fields before any dispatch.

    use eg_state::events::bpm::{BpmTap, BpmTapReset};

    /// Writing a `BpmTap` message reaches the App world, mutates
    /// `EditorTimingRes.0.bpm_tap_times`, and the per-frame sync
    /// propagates the value to the legacy `State.editor.timing`
    /// fields.
    #[tokio::test]
    async fn runtime_carve_out_bpm_tap_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset to a known empty state for the assertion.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_times
            .clear();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_result = None;
        runtime.state.set_editor_bpm_tap_state(Vec::new(), None);

        // Write the message; the next frame's schedule will run the
        // handler and write wall-clock time into the resource.
        runtime.app.world_mut().write_message(BpmTap);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth: one tap recorded.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .len(),
            1,
            "BpmTap handler must append to EditorTimingRes.bpm_tap_times"
        );
        // Legacy state was synced to match the resource.
        assert_eq!(
            runtime.state.editor_bpm_tap_times().len(),
            1,
            "sync_app_editor_timing_state must propagate bpm_tap_times to legacy state"
        );
    }

    /// `BpmTapReset` is a carve-out: writing the message clears the
    /// resource's tap history and the sync propagates the cleared
    /// list to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_bpm_tap_reset_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Pre-populate both sides with a fake tap history.
        let mut res = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
        res.0.bpm_tap_times = vec![1.0, 2.0, 3.0];
        res.0.bpm_tap_result = Some(120.0);
        drop(res);
        runtime
            .state
            .set_editor_bpm_tap_state(vec![1.0, 2.0, 3.0], Some(120.0));

        runtime.app.world_mut().write_message(BpmTapReset);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .is_empty(),
            "BpmTapReset handler must clear EditorTimingRes.bpm_tap_times"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_none(),
            "BpmTapReset handler must clear EditorTimingRes.bpm_tap_result"
        );
        assert!(
            runtime.state.editor_bpm_tap_times().is_empty(),
            "sync_app_editor_timing_state must propagate cleared bpm_tap_times to legacy state"
        );
        assert!(
            runtime.state.editor_bpm_tap_result().is_none(),
            "sync_app_editor_timing_state must propagate cleared bpm_tap_result to legacy state"
        );
    }

    /// No-op frame: when no `BpmTap`/`BpmTapReset` message is
    /// written and the resource matches the legacy state, the
    /// sync must not re-invoke the legacy setter.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_bpm_tap() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed both sides with a known tap history.
        let times = vec![1.0_f64, 2.0, 3.0];
        let result = Some(60.0_f32);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_times = times.clone();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_result = result;
        runtime
            .state
            .set_editor_bpm_tap_state(times.clone(), result);

        // Run a frame with no message: handler doesn't run, sync
        // sees no diff, setter not invoked.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // The resource and legacy state should still hold the seeded
        // values (the sync guard ensures no spurious write occurs).
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times,
            times
        );
        assert_eq!(runtime.state.editor_bpm_tap_times(), times.as_slice());
        assert_eq!(runtime.state.editor_bpm_tap_result(), result);
    }

    /// Regression: `EditorTimingRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded bpm-tap sync
    /// does not clobber an in-flight tap history on the first
    /// frame. Same pattern as the snap / block-id first-frame
    /// clobber regressions.
    #[tokio::test]
    async fn runtime_seeds_editor_timing_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-driven tap history + computed result. The default
        // resource is empty (`bpm_tap_times = []`,
        // `bpm_tap_result = None`); without seeding, the change-
        // guarded sync would see a diff and clobber the user value
        // back to defaults on the first frame.
        let times = vec![10.0_f64, 11.0, 12.0];
        let result = Some(60.0_f32);
        state.set_editor_bpm_tap_state(times.clone(), result);

        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user value,
        // not the `EditorTimingState` default.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times,
            times,
            "EditorTimingRes must be seeded from legacy State at Runtime::new"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result,
            result,
            "EditorTimingRes.bpm_tap_result must be seeded from legacy State"
        );

        // Run a frame with no bpm message. The change-guarded sync
        // must see no diff and must NOT invoke the legacy setter, so
        // the user value survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.editor_bpm_tap_times(),
            times.as_slice(),
            "first-frame sync must not clobber user bpm_tap_times"
        );
        assert_eq!(
            runtime.state.editor_bpm_tap_result(),
            result,
            "first-frame sync must not clobber user bpm_tap_result"
        );
    }

    // --- Phase 3.8 / Session carve-out tests ---
    //
    // The session event handlers in `eg_state::events::session`
    // write directly to `SessionStateRes.editor_show_metadata`,
    // `editor_show_settings`, `editor_settings_section`, and
    // `editor_keybind_capture_action`. `Runtime::run_app_frame` then
    // calls `sync_app_session_state` to copy the resource to the
    // legacy `State.session` fields before any dispatch.

    use eg_state::events::session::{
        SetKeybindCapture, SetSettingsSection, SetShowMetadata, SetShowSettings, ToggleSettings,
    };
    use eg_types::SettingsSection;

    /// Writing a `SetShowMetadata(true)` message reaches the App
    /// world, mutates `SessionStateRes.editor_show_metadata`, and
    /// the per-frame sync propagates the value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_show_metadata_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset the resource to a known value so we can detect the
        // mutation. The seeded value reflects the legacy state's
        // default (`editor_show_metadata = false`).
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_metadata = false;

        runtime.app.world_mut().write_message(SetShowMetadata(true));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_metadata,
            "SetShowMetadata handler must mutate SessionStateRes.editor_show_metadata"
        );
        assert!(
            runtime.state.editor_show_metadata(),
            "sync_app_session_state must propagate editor_show_metadata to legacy state"
        );
    }

    /// `SetShowSettings` carve-out: same pattern as
    /// `SetShowMetadata`.
    #[tokio::test]
    async fn runtime_carve_out_set_show_settings_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_settings = false;
        runtime.state.set_editor_show_settings(false);

        runtime.app.world_mut().write_message(SetShowSettings(true));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.editor_show_settings());
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings
        );
    }

    /// `ToggleSettings` flips the resource value; the sync
    /// propagates the new value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_toggle_settings_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = runtime
            .app
            .world()
            .resource::<SessionStateRes>()
            .editor_show_settings;
        runtime.state.set_editor_show_settings(initial);

        runtime.app.world_mut().write_message(ToggleSettings);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let expected = !initial;
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings,
            expected,
            "ToggleSettings must flip SessionStateRes.editor_show_settings"
        );
        assert_eq!(
            runtime.state.editor_show_settings(),
            expected,
            "sync_app_session_state must propagate toggled editor_show_settings to legacy state"
        );
    }

    /// `SetSettingsSection` propagates a non-default `SettingsSection`
    /// value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_settings_section_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_settings_section = SettingsSection::Backends;
        runtime
            .state
            .set_editor_settings_section(SettingsSection::Backends);

        let new_section = SettingsSection::Keybinds;
        runtime
            .app
            .world_mut()
            .write_message(SetSettingsSection(new_section));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_settings_section,
            new_section
        );
        assert_eq!(runtime.state.editor_settings_section(), new_section);
    }

    /// `SetKeybindCapture` propagates a `Some((String, usize))` to
    /// the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_keybind_capture_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_keybind_capture_action = None;
        runtime.state.set_editor_keybind_capture_action(None);

        let capture = Some(("editor.place".to_string(), 0));
        runtime
            .app
            .world_mut()
            .write_message(SetKeybindCapture(capture.clone()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_keybind_capture_action,
            capture
        );
        assert_eq!(
            runtime.state.editor_keybind_capture_action(),
            Some(&("editor.place".to_string(), 0))
        );
    }

    /// Regression: `SessionStateRes` (carved-out fields) must be
    /// seeded from the legacy `State` at `Runtime::new` so the
    /// change-guarded session sync does not clobber a user-driven
    /// UI-window state on the first frame.
    #[tokio::test]
    async fn runtime_seeds_session_state_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-driven UI-window state. The default resource has
        // `editor_show_metadata = false`, `editor_show_settings =
        // false`, `editor_settings_section = Backends`,
        // `editor_keybind_capture_action = None`. We flip one of
        // them to a non-default value so the first-frame sync
        // would clobber it without the seeding fix.
        state.set_editor_show_metadata(true);
        state.set_editor_show_settings(true);
        state.set_editor_settings_section(SettingsSection::Keybinds);
        state.set_editor_keybind_capture_action(Some(("editor.place".into(), 0)));

        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user
        // values, not the `SessionStateRes` default.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_metadata,
            "SessionStateRes.editor_show_metadata must be seeded from legacy State"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings,
            "SessionStateRes.editor_show_settings must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_settings_section,
            SettingsSection::Keybinds,
            "SessionStateRes.editor_settings_section must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_keybind_capture_action,
            Some(("editor.place".to_string(), 0))
        );

        // Run a frame with no session message. The change-guarded
        // sync must see no diff and must NOT invoke the legacy
        // setters, so the user values survive the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.editor_show_metadata());
        assert!(runtime.state.editor_show_settings());
        assert_eq!(
            runtime.state.editor_settings_section(),
            SettingsSection::Keybinds
        );
        assert_eq!(
            runtime.state.editor_keybind_capture_action(),
            Some(&("editor.place".to_string(), 0))
        );
    }
}
