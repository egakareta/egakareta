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
use eg_state::state::State;
use eg_state::state::enqueue_platform_input;
use eg_state::state::register_state_app_update_driver;
use eg_state::state::register_state_command_pipeline;
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
/// The two Worlds (the App's and `State::world`) coexist during the
/// compatibility window. The App is the production command/schedule driver;
/// `State::world` remains the legacy render/input bridge until the
/// single-world deletion pass moves those resources into the App.
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
        // App owns its own `World`; `State::world` is independent
        // for the scaffold. Future sub-phases will move the
        // authoritative state into the App's world.
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_command_pipeline(app.world_mut());
        register_ui_resources_world(app.world_mut());
        register_state_app_update_driver(&mut app);

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
            Self::dispatch_app_pending_commands(state, app.world_mut());
            state.tick_state();
        })
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
    /// which in turn runs the eg-state `StateSchedule` on the App's
    /// World.
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

    /// `Runtime::run_app_frame` drains App-world messages and applies
    /// the recovered legacy command to `State` during the compatibility
    /// window. UI commands now enter through this path instead of
    /// `State::world`.
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

    /// `Runtime` keeps the App-world schedule and the legacy State
    /// compatibility world isolated while the command bridge is still
    /// being deleted. This keeps App setup from mutating legacy State
    /// before the frame driver explicitly applies queued commands.
    #[tokio::test]
    async fn runtime_app_world_is_separate_from_state_world() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        // The App's World has the plugin resources; the State's
        // World is independent. We verify by reading a value
        // from each side.
        let app_phase = &*runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Menu);

        // The State's world is the one used by `State::tick_state`.
        // Pin that the State is still in the Menu phase after
        // the App was built (no cross-talk).
        assert!(runtime.state.is_menu());
    }
}
