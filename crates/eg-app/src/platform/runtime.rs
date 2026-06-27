/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::load_menu_textures;
use crate::platform::pipeline::FramePipeline;
use bevy_app::App;
use eg_state::state::State;
use eg_state::state::register_state_command_pipeline;
use eg_state::state::setup_state_app;

const SORA_TTF: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/Sora.ttf"
));

/// Runtime owns the winit-facing frame loop.
///
/// Phase 7-scaffold: alongside the legacy `state: State` and
/// `pipeline: FramePipeline` fields, the runtime now also owns a
/// `bevy_app::App` that is wired with the eg-state plugin graph
/// (see [`setup_state_app`]). The `App` is the future home of the
/// frame driver: Phase 7-1 will move the `StateSchedule` from
/// `State::world` into `app.world_mut()`, Phase 7-2 will route
/// `Runtime::run_frame` through `app.update()`, and Phase 7-3 will
/// delete the `State::tick_state` call from the pipeline. Until
/// then, the App's default schedules are empty (`app.update()` is
/// a no-op) and the legacy `FramePipeline::run_frame` is the
/// actual frame driver.
///
/// The two Worlds (the App's and `State::world`) coexist
/// independently for the scaffold. Future phases will move the
/// authoritative `State` fields into the App's world and the
/// `State::world` will be removed.
///
/// REFACTOR POINT(full-bevy): when full Bevy lands, the `state`
/// and `pipeline` fields disappear; the `App` owns the egui
/// context + render resources and the winit loop calls
/// `runtime.app.update()` directly.
pub struct Runtime {
    pub state: State,
    pub pipeline: FramePipeline,
    // Phase 7-scaffold: the App is built but not yet driven by
    // the production frame loop. The `#[allow(dead_code)]` is
    // unavoidable for the scaffold (no production caller exists
    // yet) and well-justified by the Phase 7 plan: 7-1/7-2/7-3
    // will consume the field.
    #[allow(dead_code)]
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

        Self {
            state,
            pipeline,
            app,
        }
    }

    /// Drive one frame through the legacy `FramePipeline` path.
    ///
    /// This is the current frame driver. Phase 7-2 will route this
    /// through `app.update()` instead.
    pub fn run_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        self.pipeline.run_frame(&mut self.state, raw_input)
    }

    /// Drive one frame through the Bevy `App` (Phase 7-scaffold).
    ///
    /// Today the App's default schedules are empty (no systems have
    /// been registered into `Update` yet — all eg-state systems live
    /// in the `StateSchedule` which is still invoked by
    /// `State::tick_state()`), so this call is a no-op that exercises
    /// the App construction + schedule wiring path. Future sub-phases
    /// register systems into the App and route the frame driver
    /// here.
    ///
    /// Returns `true` if the App's `update()` completed without
    /// panicking. (Bevy 0.19's `App::update` does not return a Result
    /// — we keep the `bool` return shape so the future
    /// frame-driver transition can surface wgpu errors through the
    /// same path as `run_frame`.)
    #[allow(dead_code)]
    pub fn run_app_frame(&mut self) -> bool {
        self.app.update();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eg_state::state::State;
    use eg_state::state::{
        AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
        EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, PerfRes,
        SessionStateRes,
    };

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

    #[tokio::test]
    async fn test_runtime_run_frame() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        ));

        let output = runtime.run_frame(raw_input);

        // Verify we get valid pixels_per_point and shapes were generated (menu UI)
        assert!(output.pixels_per_point > 0.0);
        assert!(!output.shapes.is_empty());
    }

    // ---- Phase 7-scaffold tests ----
    //
    // These pin the Phase 7-scaffold contract: `Runtime::new`
    // builds a `bevy_app::App` with the eg-state plugin graph,
    // and `run_app_frame` drives `app.update()` without
    // panicking. Future sub-phases (7-1: schedule migration,
    // 7-2: frame driver, 7-3: pipeline split) extend these
    // tests with system-level assertions.

    /// `Runtime::new` builds an `App` whose `World` contains every
    /// resource that `StatePlugins` is responsible for registering.
    /// `AppPhaseRes` is the only one that defaults to a meaningful
    /// value (`AppPhase::Menu`); the rest are `Default::default()`
    /// and exist as placeholders for Phase 2c+ systems.
    #[tokio::test]
    async fn runtime_app_has_state_plugin_resources() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        let world = runtime.app.world();
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

    /// `Runtime::run_app_frame` is safe to call: the App's default
    /// schedules are empty in Phase 7-scaffold, so the call is a
    /// no-op. This is the boundary future sub-phases extend.
    #[tokio::test]
    async fn runtime_run_app_frame_is_a_safe_no_op() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // First call: app.update() runs the empty default
        // schedules and the StateSchedule (no-op on a freshly
        // constructed state).
        assert!(runtime.run_app_frame());
        // Second call: same, still safe.
        assert!(runtime.run_app_frame());
    }

    /// `State::world` and the `App`'s `World` are independent in
    /// Phase 7-scaffold. The legacy `State::tick_state` path
    /// remains authoritative; the App is a parallel structure.
    /// Pin this so the future 7-1 sub-phase (which unifies the
    /// worlds) is a deliberate, observable change.
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
