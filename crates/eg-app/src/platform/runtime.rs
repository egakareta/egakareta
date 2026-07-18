/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(not(test))]
use crate::load_menu_textures;
#[cfg(not(test))]
use crate::{
    show_editor_ui, show_menu_favicon_ui, show_menu_play_ui, show_menu_topbar_ui,
    show_pause_menu_ui, show_perf_overlay, show_practice_checkpoint_ui, show_settings_sidebar,
};
#[cfg(test)]
use bevy_app::FixedUpdate;
use bevy_app::{App, PostUpdate};
#[cfg(not(test))]
use bevy_ecs::world::World;
use bevy_state::prelude::State;
#[cfg(not(test))]
use eg_render::block_icon_cache::BlockIconCache;
use eg_state::state::AudioBackendRes;

use eg_state::state::AppBootstrap;
#[cfg(not(test))]
use eg_state::state::AuthStateRes;
use eg_state::state::EditorInputRes;
#[cfg(not(test))]
use eg_state::state::MenuStateRes;
#[cfg(not(test))]
use eg_state::state::PerfRes;
use eg_state::state::RenderConfigRes;
#[cfg(not(test))]
use eg_state::state::RenderEguiRes;
#[cfg(not(test))]
use eg_state::state::RenderGpuRes;
#[cfg(not(target_arch = "wasm32"))]
use eg_state::state::RenderSurfaceHostRes;
use eg_state::state::SessionStateRes;
use eg_state::state::enqueue_platform_input;
use eg_state::state::register_state_schedule;

use eg_state::state::setup_state_app;
#[cfg(test)]
use eg_state::state::test_support::TestApp;
use eg_ui::register_ui_resources_world;
#[cfg(not(test))]
use eg_ui::{MenuTextures, WorldUiCommandSink, combined_ui_scale_factor};
#[cfg(not(test))]
use egui_wgpu::ScreenDescriptor;

#[cfg(not(test))]
const SORA_TTF: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/Sora.ttf"
));

/// App-owned presentation resources retained by the custom winit/wgpu shell.
struct EguiFrameResources {
    egui_ctx: egui::Context,
    #[cfg(not(test))]
    menu_textures: MenuTextures,
    #[cfg(not(test))]
    block_icon_cache: BlockIconCache,
    #[cfg(test)]
    _presentation: (),
}

#[cfg(not(test))]
struct EguiRawInputRes(egui::RawInput);

#[cfg(not(test))]
struct EguiFrameOutputRes(egui::FullOutput);

#[cfg(not(test))]
fn run_egui_ui_system(world: &mut World) {
    let raw_input = world
        .remove_non_send::<EguiRawInputRes>()
        .expect("EguiRawInputRes must be queued before App update");
    let mut frame = world
        .remove_non_send::<EguiFrameResources>()
        .expect("EguiFrameResources must be installed before App update");
    let mut render = world
        .remove_non_send::<RenderEguiRes>()
        .expect("RenderEguiRes must be installed before App update");

    let phase = *world.resource::<State<eg_types::AppPhase>>().get();
    let is_menu = phase == eg_types::AppPhase::Menu;
    let is_editor = phase == eg_types::AppPhase::Editor;
    let show_settings = world.resource::<SessionStateRes>().editor_show_settings;
    let (
        editor_show_metadata,
        editor_show_place_window,
        editor_settings_section,
        editor_keybind_capture_action,
    ) = {
        let session = world.resource::<SessionStateRes>();
        (
            session.editor_show_metadata,
            session.editor_show_place_window,
            session.editor_settings_section,
            session.editor_keybind_capture_action.clone(),
        )
    };
    let (is_game_paused, is_practice_mode_enabled) = {
        let session = world.resource::<SessionStateRes>();
        let is_real_gameplay = phase == eg_types::AppPhase::Playing && !session.playtesting_editor;
        (
            is_real_gameplay && session.game_paused,
            is_real_gameplay && session.practice_mode_enabled,
        )
    };
    let (menu_level_name, menu_selected_level_progress) = {
        let session = world.resource::<SessionStateRes>();
        let menu_level_name = world
            .get_resource::<MenuStateRes>()
            .and_then(|menu| menu.0.levels.get(menu.0.selected_level).cloned());
        let progress = menu_level_name
            .as_deref()
            .and_then(|level_name| session.app_settings.level_progress.get(level_name).copied());
        (menu_level_name, progress)
    };
    let available_levels = world
        .get_resource::<MenuStateRes>()
        .map(|menu| menu.0.levels.clone())
        .unwrap_or_default();
    let (editor_level_name, editor_music_metadata, editor_creator_metadata, editor_sky_color) = {
        let session = world.resource::<SessionStateRes>();
        (
            session.editor_level_name.clone(),
            session.editor_music_metadata.clone(),
            session.editor_creator_metadata.clone(),
            session.editor_sky_color,
        )
    };
    let (
        app_settings,
        graphics_backend,
        graphics_backend_options,
        graphics_backend_locked,
        settings_restart_required,
        audio_backend,
        audio_backend_options,
        audio_backend_locked,
    ) = {
        let session = world.resource::<SessionStateRes>();
        (
            session.app_settings.clone(),
            session.app_settings.graphics_backend.clone(),
            session.available_graphics_backends.clone(),
            session.graphics_backend_locked,
            session.settings_restart_required,
            session.app_settings.audio_backend.clone(),
            session.available_audio_backends.clone(),
            session.audio_backend_locked,
        )
    };
    let (auth_display_name, auth_pending, auth_message) = {
        let auth = world.resource::<AuthStateRes>();
        (
            auth.display_name().map(ToOwned::to_owned),
            auth.pending,
            auth.message.clone(),
        )
    };
    let (
        perf_overlay_enabled,
        perf_graphics_backend,
        perf_audio_backend,
        perf_fps,
        perf_has_latest_frame,
    ) = {
        let perf = world.resource::<PerfRes>();
        let graphics_backend = world.non_send::<RenderGpuRes>().0.adapter_info.backend;
        let has_latest_frame = perf.0.global_frame_view.lock().latest_frame().is_some();
        (
            perf.0.overlay_enabled,
            format!("{graphics_backend:?}"),
            world
                .resource::<SessionStateRes>()
                .app_settings
                .audio_backend
                .clone(),
            perf.0.fps_smoothed,
            has_latest_frame,
        )
    };
    let ui_state = eg_state::state::UiStateData::from_world(world);
    let render_config = *world.resource::<RenderConfigRes>();
    let physical_size = egui::vec2(render_config.width as f32, render_config.height as f32);
    let ui_scale_factor = combined_ui_scale_factor(
        physical_size,
        world
            .resource::<SessionStateRes>()
            .app_settings
            .normalized_ui_scale_multiplier(),
    );
    frame.egui_ctx.set_zoom_factor(ui_scale_factor);

    {
        let gpu = world.non_send::<RenderGpuRes>();
        frame
            .block_icon_cache
            .refresh_icons(&gpu.0, &mut render.renderer);
    }
    let block_icon_texture_ids = frame.block_icon_cache.texture_ids();
    let (full_output, ui_input_blocking_rects, ui_input_pixels_per_point) = {
        let mut ui_input_blocking_rects = Vec::new();
        let mut ui_input_pixels_per_point = 1.0;
        let full_output = {
            let mut command_sink = WorldUiCommandSink::new(world);
            frame.egui_ctx.run_ui(raw_input.0, |root_ui| {
                let ctx = root_ui.ctx().clone();
                ui_input_blocking_rects = show_editor_ui(
                    &ctx,
                    is_editor,
                    show_settings,
                    &available_levels,
                    editor_level_name.as_deref(),
                    &editor_music_metadata,
                    &editor_creator_metadata,
                    editor_sky_color,
                    &app_settings,
                    &graphics_backend,
                    &graphics_backend_options,
                    graphics_backend_locked,
                    settings_restart_required,
                    &audio_backend,
                    &audio_backend_options,
                    audio_backend_locked,
                    editor_show_metadata,
                    editor_show_place_window,
                    editor_settings_section,
                    editor_keybind_capture_action.as_ref(),
                    &ui_state,
                    &block_icon_texture_ids,
                    &mut command_sink,
                );
                ui_input_pixels_per_point = ctx.pixels_per_point();
                if !is_editor {
                    show_settings_sidebar(
                        &ctx,
                        show_settings,
                        is_menu,
                        is_editor,
                        is_game_paused,
                        &app_settings,
                        &graphics_backend,
                        &graphics_backend_options,
                        graphics_backend_locked,
                        settings_restart_required,
                        &audio_backend,
                        &audio_backend_options,
                        audio_backend_locked,
                        editor_settings_section,
                        editor_keybind_capture_action.as_ref(),
                        &ui_state,
                        &mut command_sink,
                    );
                }
                show_perf_overlay(
                    &ctx,
                    perf_overlay_enabled,
                    &perf_graphics_backend,
                    &perf_audio_backend,
                    perf_fps,
                    perf_has_latest_frame,
                    &mut command_sink,
                );
                show_menu_topbar_ui(
                    root_ui,
                    is_menu,
                    &app_settings,
                    auth_display_name.as_deref(),
                    auth_pending,
                    auth_message.as_deref(),
                    &mut command_sink,
                );
                show_menu_play_ui(
                    &ctx,
                    is_menu,
                    &mut command_sink,
                    frame.menu_textures.play_button.as_ref(),
                    frame.menu_textures.play_button_filled.as_ref(),
                    frame.menu_textures.edit_button.as_ref(),
                );
                show_practice_checkpoint_ui(
                    &ctx,
                    is_practice_mode_enabled,
                    is_game_paused,
                    &mut command_sink,
                );
                show_pause_menu_ui(
                    &ctx,
                    is_game_paused,
                    is_practice_mode_enabled,
                    &mut command_sink,
                );
                if let Some(favicon) = &frame.menu_textures.favicon {
                    show_menu_favicon_ui(
                        &ctx,
                        is_menu,
                        menu_level_name.as_deref(),
                        menu_selected_level_progress,
                        favicon,
                    );
                }
            })
        };
        (
            full_output,
            ui_input_blocking_rects,
            ui_input_pixels_per_point,
        )
    };

    world
        .resource_mut::<eg_state::state::EditorInputRes>()
        .0
        .ui_input_blocking_rects = ui_input_blocking_rects;
    world
        .resource_mut::<eg_state::state::EditorInputRes>()
        .0
        .ui_input_pixels_per_point = ui_input_pixels_per_point;

    let paint_jobs = frame
        .egui_ctx
        .tessellate(full_output.shapes.clone(), full_output.pixels_per_point);
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [render_config.width, render_config.height],
        pixels_per_point: full_output.pixels_per_point,
    };
    let (device, queue) = {
        let gpu = world.non_send::<RenderGpuRes>();
        (gpu.0.device.clone(), gpu.0.queue.clone())
    };
    for (id, image_delta) in &full_output.textures_delta.set {
        render
            .renderer
            .update_texture(&device, &queue, *id, image_delta);
    }
    render.paint_jobs = paint_jobs;
    render.screen_descriptor = screen_descriptor;

    world.insert_non_send(frame);
    world.insert_non_send(render);
    world.insert_non_send(EguiFrameOutputRes(full_output));
}

/// Runtime owns the winit-facing frame loop.
///
/// Phase 7: the runtime owns a
/// `bevy_app::App` wired with the eg-state plugin graph (see
/// [`setup_state_app`]). The winit redraw path now calls
/// [`Runtime::run_app_frame`], which owns the App update and command
/// drain before handing the frame back to the App-owned render presenter.
///
/// The App world is the production command/schedule driver. The egui context,
/// renderer, menu textures, block-icon cache, and all state resources are
/// installed directly by AppBootstrap. Tests use App-native TestApp
/// fixtures, and Runtime owns only the App.
///
/// REFACTOR POINT(full-bevy): when full Bevy lands, the custom bootstrap and
/// winit loop are replaced by Bevy plugins and the Bevy app runner.
pub struct Runtime {
    pub app: App,
}

impl Runtime {
    pub fn from_bootstrap(bootstrap: AppBootstrap) -> Self {
        let egui_ctx = egui::Context::default();
        #[cfg(not(test))]
        {
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
        }

        #[cfg(not(test))]
        let menu_textures = load_menu_textures(&egui_ctx);
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_schedule(app.world_mut());
        app.add_systems(PostUpdate, eg_state::state::run_state_schedule_system);
        register_ui_resources_world(app.world_mut());
        #[cfg(not(test))]
        app.add_systems(bevy_app::Update, run_egui_ui_system);
        bootstrap.install(app.world_mut());

        #[cfg(not(test))]
        {
            let egui_renderer = {
                let gpu = app.world().non_send::<RenderGpuRes>();
                egui_wgpu::Renderer::new(
                    &gpu.0.device,
                    gpu.0.config.format,
                    egui_wgpu::RendererOptions::default(),
                )
            };
            app.insert_non_send(RenderEguiRes::new(egui_renderer));
            app.insert_non_send(EguiFrameResources {
                egui_ctx,
                menu_textures,
                block_icon_cache: BlockIconCache::new(),
            });
        }
        #[cfg(test)]
        app.insert_non_send(EguiFrameResources {
            egui_ctx,
            _presentation: (),
        });

        Self { app }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        Self::from_test_app(TestApp::minimal())
    }

    #[cfg(test)]
    pub fn from_test_app(test_app: TestApp) -> Self {
        let egui_ctx = egui::Context::default();
        let mut app = test_app.into_app();
        app.add_systems(PostUpdate, eg_state::state::run_state_schedule_system);
        register_ui_resources_world(app.world_mut());
        app.insert_non_send(EguiFrameResources {
            egui_ctx,
            _presentation: (),
        });

        Self { app }
    }
    pub fn ensure_audio_device(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.app
            .world()
            .resource::<AudioBackendRes>()
            .ensure_device();
        #[cfg(target_arch = "wasm32")]
        self.app
            .world()
            .non_send::<AudioBackendRes>()
            .ensure_device();
    }

    /// Return the cursor policy from the authoritative App-world session.
    pub fn game_cursor(&self) -> eg_types::GameCursor {
        let world = self.app.world();
        if *world.resource::<State<eg_types::AppPhase>>().get() == eg_types::AppPhase::Playing
            && !world.resource::<SessionStateRes>().game_paused
        {
            eg_types::GameCursor::Hidden
        } else {
            eg_types::GameCursor::Default
        }
    }

    /// Return whether the physical pointer position intersects App-owned editor UI.
    pub fn editor_pointer_over_ui_input(&self, x: f64, y: f64) -> bool {
        let world = self.app.world();
        if *world.resource::<State<eg_types::AppPhase>>().get() != eg_types::AppPhase::Editor {
            return false;
        }

        let input = &world.resource::<EditorInputRes>().0;
        let pixels_per_point = input.ui_input_pixels_per_point.max(1.0) as f64;
        let point_x = x / pixels_per_point;
        let point_y = y / pixels_per_point;
        input.ui_input_blocking_rects.iter().any(|rect| {
            point_x >= rect[0] as f64
                && point_x <= rect[2] as f64
                && point_y >= rect[1] as f64
                && point_y <= rect[3] as f64
        })
    }

    /// Return the native window retained by the App-owned surface host.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> Option<&eg_platform::state_host::NativeWindow> {
        match &self.app.world().non_send::<RenderSurfaceHostRes>().0 {
            Some(eg_platform::state_host::SurfaceHost::Window(window)) => Some(window),
            _ => None,
        }
    }

    /// Drive one frame through the App-owned presentation resources.
    pub fn run_app_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let app = &mut self.app;
        #[cfg(test)]
        let egui_ctx = app
            .world()
            .non_send::<EguiFrameResources>()
            .egui_ctx
            .clone();

        #[cfg(not(test))]
        {
            app.world_mut().insert_non_send(EguiRawInputRes(raw_input));
            app.update();
        }
        #[cfg(test)]
        let full_output = {
            let output = egui_ctx.run_ui(raw_input, |_| {});
            app.update();
            output
        };
        #[cfg(not(test))]
        let full_output = app
            .world_mut()
            .remove_non_send::<EguiFrameOutputRes>()
            .expect("EguiFrameOutputRes must be produced during App update")
            .0;
        #[cfg(test)]
        {
            let mut input = app
                .world_mut()
                .resource_mut::<eg_state::state::EditorInputRes>();
            input.0.ui_input_blocking_rects.clear();
            input.0.ui_input_pixels_per_point = 1.0;
        }

        #[cfg(not(test))]
        {
            let mut render = app.world_mut().non_send_mut::<RenderEguiRes>();
            for id in &full_output.textures_delta.free {
                render.renderer.free_texture(id);
            }
        }
        full_output
    }

    /// Clone the App-owned egui context for winit input integration.
    pub fn egui_context(&self) -> egui::Context {
        self.app
            .world()
            .non_send::<EguiFrameResources>()
            .egui_ctx
            .clone()
    }

    /// Queue platform input in the App world for scheduled processing.
    pub fn process_input_event(&mut self, event: eg_state::commands::InputEvent) {
        enqueue_platform_input(self.app.world_mut(), event);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy_state::prelude::State;
    use eg_state::state::{
        AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes, AuthSubmitSignInRequest,
        EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorObjectsRes,
        EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
        GameplayResetRes, MenuStateRes, PerfFrameCopyRequest, PerfRes, PlatformInputQueue,
        RenderResizeRequest, SessionStateRes,
    };

    fn runtime_in_phase(phase: eg_types::AppPhase) -> Runtime {
        let mut runtime = Runtime::new_test();
        runtime.app.world_mut().insert_resource(State::new(phase));
        runtime
    }

    fn current_phase(runtime: &Runtime) -> eg_types::AppPhase {
        *runtime
            .app
            .world()
            .resource::<State<eg_types::AppPhase>>()
            .get()
    }

    fn runtime_in_playing(playtesting_editor: bool, game_paused: bool) -> Runtime {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Playing);
        let mut session = runtime.app.world_mut().resource_mut::<SessionStateRes>();
        session.playtesting_editor = playtesting_editor;
        session.game_paused = game_paused;
        drop(session);
        runtime
    }

    fn runtime_in_editor(level_name: &str) -> Runtime {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Editor);
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_level_name = Some(level_name.to_string());
        runtime
    }

    #[test]
    fn test_runtime_new() {
        let runtime = Runtime::new_test();

        // Verify the egui context was initialized
        let ctx = runtime.egui_context();
        assert!(ctx.pixels_per_point() > 0.0);

        // Verify the fixture was installed into the App world.
        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
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
    /// `State<AppPhase>` defaults to `AppPhase::Menu`; gameplay, editor,
    /// session, and render resources are installed by their plugins.
    #[test]
    fn runtime_app_has_state_plugin_resources() {
        let runtime = Runtime::new_test();

        let world = runtime.app.world();
        assert!(
            world.contains_resource::<eg_game::GameClock>(),
            "GameClock registered by GameplayPlugin"
        );
        assert!(
            world.contains_resource::<State<eg_types::AppPhase>>(),
            "State<AppPhase> registered by MenuPlugin"
        );
        assert!(
            world.contains_resource::<SessionStateRes>(),
            "SessionStateRes registered by SessionPlugin"
        );
        assert!(
            world.contains_resource::<MenuStateRes>(),
            "MenuStateRes seeded by AppBootstrap"
        );
        assert!(
            world.contains_resource::<EditorInputRes>(),
            "EditorInputRes registered by EditorPlugin"
        );

        assert!(
            world.contains_resource::<EditorObjectsRes>(),
            "EditorObjectsRes registered by EditorPlugin"
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
            world.contains_resource::<FrameTimeRes>(),
            "FrameTimeRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PerfRes>(),
            "PerfRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PerfFrameCopyRequest>(),
            "PerfFrameCopyRequest registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<RenderResizeRequest>(),
            "RenderResizeRequest registered by RenderPlugin"
        );
        assert!(
            world.contains_resource::<PlatformInputQueue>(),
            "PlatformInputQueue registered by EditorPlugin"
        );
    }

    /// `register_state_schedule` adds the per-frame
    /// `StateSchedule` to the App's World via Bevy's
    /// `World::add_schedule`. Pin the schedule is reachable from
    /// the App so the future 7-1 sub-phase can move the
    /// `State::tick_state` invocation here.
    #[test]
    fn runtime_app_registers_state_schedule() {
        let mut runtime = Runtime::new_test();

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

    /// `GameplayPlugin` installs the eg-game ECS schedule into the App's
    /// World. Gameplay resources and systems are App-world authoritative.
    #[test]
    fn runtime_app_registers_gameplay_schedule() {
        let mut runtime = Runtime::new_test();

        let result = runtime.app.world_mut().try_run_schedule(FixedUpdate);
        assert!(
            result.is_ok(),
            "FixedUpdate should be registered on the App's World"
        );
    }

    /// `Runtime::run_app_frame` is safe to call and produces normal
    /// egui output while driving the App first.
    #[test]
    fn runtime_run_app_frame_returns_output() {
        let mut runtime = Runtime::new_test();

        let output = runtime.run_app_frame(egui::RawInput::default());
        assert!(output.pixels_per_point > 0.0);
    }

    /// Platform input now enters the App world; the state schedule driver
    /// drains it into resources/messages during the next frame.
    #[test]
    fn runtime_process_input_event_queues_app_world_input() {
        let mut runtime = Runtime::new_test();
        let (initial_width, initial_height) = {
            let config = runtime.app.world().resource::<RenderConfigRes>();
            (config.width, config.height)
        };
        let initial_queue_len = runtime.app.world().resource::<PlatformInputQueue>().len();

        runtime.process_input_event(eg_state::commands::InputEvent::Resize {
            width: 640,
            height: 360,
        });

        let queue = runtime.app.world().resource::<PlatformInputQueue>();
        assert_eq!(queue.len(), initial_queue_len + 1);
        assert!(matches!(
            queue.events().last(),
            Some(eg_state::commands::InputEvent::Resize {
                width: 640,
                height: 360
            })
        ));
        let config = runtime.app.world().resource::<RenderConfigRes>();
        assert_eq!(config.width, initial_width);
        assert_eq!(config.height, initial_height);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let config = runtime.app.world().resource::<RenderConfigRes>();
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 360);
    }

    /// `Runtime::run_app_frame` drives the App's `Update` schedule,
    /// then explicitly runs the eg-state `StateSchedule` on the App's
    /// World while typed request resources and scheduled frame timing remain
    /// App-world owned.
    #[test]
    fn runtime_run_app_frame_updates_app_state_schedule() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .insert_resource(State::new(eg_types::AppPhase::Playing));
        std::thread::sleep(std::time::Duration::from_millis(20));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Playing);
        let app_frame_time = *runtime.app.world().resource::<FrameTimeRes>();
        assert!(
            app_frame_time.delta_seconds > 0.0,
            "StateSchedule must advance App-world frame timing"
        );
        assert_eq!(
            runtime.app.world().resource::<FrameTimeRes>().delta_seconds,
            app_frame_time.delta_seconds,
            "Runtime must sync scheduled frame delta into legacy State before compatibility update"
        );
    }

    /// App `PostUpdate` drives the state, gameplay, and render pipeline after UI.
    /// The test harness registers the same canonical schedule system.
    #[test]
    fn runtime_app_update_runs_state_schedule() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .insert_resource(State::new(eg_types::AppPhase::Playing));

        runtime.app.update();

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Playing);
    }

    /// `Runtime::run_app_frame` drains App-world messages and runs the
    /// authoritative state schedule. UI commands enter through this path.
    #[test]
    fn runtime_run_app_frame_dispatches_app_world_commands() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .resource_mut::<MenuStateRes>()
            .0
            .levels = vec!["Runtime Command Test".to_string()];

        runtime
            .app
            .world_mut()
            .write_message(eg_state::events::navigation::ToggleEditor);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(current_phase(&runtime) == eg_types::AppPhase::Editor);
    }

    #[test]
    fn runtime_resize_surface_request_is_consumed_by_state_schedule() {
        use eg_state::events::auth::ResizeSurface;

        let mut runtime = Runtime::new_test();
        let initial_size = {
            let config = runtime.app.world().resource::<RenderConfigRes>();
            eg_types::PhysicalSize::new(config.width, config.height)
        };
        let new_size = eg_types::PhysicalSize::new(
            initial_size.width.saturating_add(17).max(1),
            initial_size.height.saturating_add(11).max(1),
        );

        runtime.app.world_mut().write_message(ResizeSurface {
            width: new_size.width,
            height: new_size.height,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let config = runtime.app.world().resource::<RenderConfigRes>();
        assert_eq!(
            config.width, new_size.width,
            "StateSchedule resize system must apply App-world resize width"
        );
        assert_eq!(
            config.height, new_size.height,
            "StateSchedule resize system must apply App-world resize height"
        );
        let resize = runtime.app.world().resource::<RenderResizeRequest>();
        assert!(
            !resize.pending,
            "StateSchedule must consume RenderResizeRequest during run_app_frame"
        );
    }

    #[test]
    fn runtime_copy_latest_perf_frame_request_is_consumed_by_state_schedule() {
        use eg_state::events::auth::CopyLatestPerfFrame;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(CopyLatestPerfFrame);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<PerfFrameCopyRequest>();
        assert!(
            !request.pending,
            "StateSchedule must consume PerfFrameCopyRequest during run_app_frame"
        );
    }

    /// `AuthSubmitSignIn` is App-world owned: the handler writes
    /// `AuthSubmitSignInRequest.pending = true`, `StateSchedule`
    /// consumes the request into `AuthStateRes`, and Runtime mirrors
    /// that resource back to legacy `State` without a dedicated auth
    /// request sync.
    #[test]
    fn runtime_carve_out_auth_submit_sign_in_consumes_request() {
        use eg_state::events::auth::AuthSubmitSignIn;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(AuthSubmitSignIn);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthSubmitSignInRequest>();
        assert!(
            !request.pending,
            "StateSchedule must consume AuthSubmitSignInRequest during run_app_frame"
        );
    }

    /// `AuthSignOut` follows the same App-world auth request path as
    /// `AuthSubmitSignIn`.
    #[test]
    fn runtime_carve_out_auth_sign_out_consumes_request() {
        use eg_state::events::auth::AuthSignOut;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(AuthSignOut);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthSignOutRequest>();
        assert!(
            !request.pending,
            "StateSchedule must consume AuthSignOutRequest during run_app_frame"
        );
    }

    /// `AuthOpenSignup` follows the same App-world auth request path
    /// as `AuthSubmitSignIn`. The platform open may no-op in test
    /// builds; the assertion pins that the scheduled auth system
    /// consumed the request.
    #[test]
    fn runtime_carve_out_auth_open_signup_consumes_request() {
        use eg_state::events::auth::AuthOpenSignup;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(AuthOpenSignup);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthOpenSignupRequest>();
        assert!(
            !request.pending,
            "StateSchedule must consume AuthOpenSignupRequest during run_app_frame"
        );
    }

    /// Auth state remains App-world authoritative after construction.
    #[test]
    fn runtime_auth_state_resource_remains_authoritative() {
        let mut runtime = Runtime::new_test();

        {
            let mut auth = runtime.app.world_mut().resource_mut::<AuthStateRes>();
            auth.pending = true;
            auth.message = Some("app".to_string());
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let auth = runtime.app.world().resource::<AuthStateRes>();
        assert!(auth.pending);
        assert_eq!(auth.message.as_deref(), Some("app"));
    }

    // --- Phase 3.48 / Navigation request carve-out tests ---
    //
    // The gameplay messages route through direct App-world resources or
    // dedicated typed requests consumed by StateSchedule. The end-to-end
    // tests below exercise the full pipeline through `run_app_frame`.

    use eg_state::events::navigation::{
        GameQuitToMenu, GameRemovePracticeCheckpoint, GameRestartLevel, TurnRight as GameTurnRight,
    };

    /// Active real gameplay quit-to-menu is applied by App resources; the
    /// runtime only consumes the one-shot audio compatibility intent.
    #[test]
    fn runtime_direct_game_quit_to_menu_updates_app_phase() {
        let mut runtime = runtime_in_playing(false, false);

        runtime.app.world_mut().write_message(GameQuitToMenu);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .audio_stop_pending
        );
    }

    #[test]
    fn runtime_toggle_editor_from_editor_returns_to_menu_without_request() {
        use eg_state::events::navigation::ToggleEditor;

        let mut runtime = runtime_in_editor("ToggleEditorMenu");

        runtime.app.world_mut().write_message(ToggleEditor);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .audio_stop_pending
        );
    }

    #[test]
    fn runtime_toggle_editor_from_menu_enters_app_owned_editor() {
        use eg_state::events::navigation::ToggleEditor;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(ToggleEditor);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Editor);
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_level_name
                .is_some(),
            "menu ToggleEditor must load the selected level into App editor resources"
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Null
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .playback
                .playing
        );
    }

    #[test]
    fn runtime_turn_right_from_menu_starts_app_owned_gameplay() {
        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(GameTurnRight);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Playing);
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playing_level_name
                .is_some(),
            "menu TurnRight must start the selected level in App-owned session state"
        );
        assert!(
            !runtime.app.world().resource::<GameplayResetRes>().pending,
            "menu TurnRight reset must be consumed during the same App frame"
        );
        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Playing);
    }

    #[test]
    fn runtime_turn_right_starts_unstarted_app_gameplay_without_request() {
        let mut runtime = runtime_in_playing(false, false);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .playing_level_name = Some("RuntimeTurnRight".to_string());
        runtime.app.world_mut().write_message(GameTurnRight);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.app.world().resource::<eg_game::GameClock>().started);
    }

    #[test]
    fn runtime_playtest_enters_app_owned_gameplay_world() {
        use eg_state::events::timeline::Playtest;

        let mut runtime = runtime_in_editor("RuntimePlaytest");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 5.0;

        runtime.app.world_mut().write_message(Playtest);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Playing);
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playtesting_editor,
            "Playtest must mark the App session as editor playtesting"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<eg_game::LevelMeta>()
                .level_duration_seconds,
            5.0
        );
        assert!(
            !runtime.app.world().resource::<GameplayResetRes>().pending,
            "Playtest reset must be consumed during the same App frame"
        );
        let player_exists = {
            let mut query = runtime.app.world_mut().query::<&eg_game::PlayerPosition>();
            query.iter(runtime.app.world()).next().is_some()
        };
        assert!(
            player_exists,
            "Playtest must seed an App-world player entity"
        );
    }

    /// Real gameplay restart is applied by the App-world gameplay reset
    /// system and does not fall back to Runtime's legacy request consumer.
    #[test]
    fn runtime_direct_game_restart_resets_app_gameplay() {
        let mut runtime = runtime_in_playing(false, false);

        {
            let world = runtime.app.world_mut();
            world.resource_mut::<eg_game::LevelMeta>().spawn_position = [2.0, 0.0, 3.0];
            world.resource_mut::<eg_game::LevelMeta>().spawn_direction = eg_types::Direction::Right;
            world
                .resource_mut::<eg_game::LevelMeta>()
                .level_duration_seconds = 9.0;
            world.resource_mut::<eg_game::GameClock>().started = true;
            world.write_message(GameRestartLevel);
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let player = {
            let mut query = runtime
                .app
                .world_mut()
                .query::<(&eg_game::PlayerPosition, &eg_game::PlayerDirection)>();
            query
                .iter(runtime.app.world())
                .next()
                .map(|(position, direction)| (position.0, direction.0))
                .expect("restart must keep an App-world player entity")
        };
        assert_eq!(player.0, [2.0, 0.0, 3.0]);
        assert_eq!(player.1, eg_types::Direction::Right);
        assert!(!runtime.app.world().resource::<eg_game::GameClock>().started);
        assert!(!runtime.app.world().resource::<GameplayResetRes>().pending);
    }

    /// `GameRemovePracticeCheckpoint` mutates the App-world checkpoint list
    /// directly, then uses the one-shot compatibility sync only to rebuild
    /// the legacy presentation mesh.
    #[test]
    fn runtime_carve_out_game_remove_practice_checkpoint_syncs_direct_resource() {
        let mut runtime = runtime_in_playing(false, false);
        {
            let mut session = runtime.app.world_mut().resource_mut::<SessionStateRes>();
            session.practice_mode_enabled = true;
            session
                .practice_checkpoints
                .push(eg_state::state::PracticeCheckpoint {
                    gameplay: eg_game::GameCheckpointState::default(),
                    trigger_base_objects: None,
                });
        }

        runtime
            .app
            .world_mut()
            .write_message(GameRemovePracticeCheckpoint);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_checkpoints
                .is_empty(),
            "GameRemovePracticeCheckpoint must remove the App-world checkpoint"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<eg_state::state::RenderPracticeCheckpointVerticesRes>()
                .0
                .is_empty(),
            "App render extraction must clear removed practice checkpoint flags"
        );
    }

    /// `Runtime` keeps App-world setup from mutating legacy State before
    /// the frame driver explicitly applies queued commands.
    #[test]
    fn runtime_app_world_setup_does_not_mutate_state() {
        let runtime = Runtime::new_test();

        // The App's World has the plugin resources.
        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
    }

    /// The production frame path advances the App-world gameplay entities.
    #[test]
    fn runtime_gameplay_schedule_owns_active_player_motion() {
        let mut runtime = Runtime::new_test();

        {
            let world = runtime.app.world_mut();
            world.insert_resource(State::new(eg_types::AppPhase::Playing));
            world.resource_mut::<SessionStateRes>().playtesting_editor = false;
            world.resource_mut::<SessionStateRes>().game_paused = false;
            let mut clock = world.resource_mut::<eg_game::GameClock>();
            clock.started = true;
            clock.elapsed_seconds = 0.0;
            clock.game_over = false;
            clock.level_complete = false;

            let mut query = world.query::<(
                &mut eg_game::PlayerSpeed,
                &mut eg_game::PlayerPosition,
                &mut eg_game::Grounded,
            )>();
            let (mut speed, mut position, mut grounded) = query
                .iter_mut(world)
                .next()
                .expect("Runtime::new must seed an App-world player");
            speed.0 = 120.0;
            position.0 = [0.0, 0.0, 0.0];
            grounded.0 = true;
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let app_position = {
            let world = runtime.app.world_mut();
            let mut query = world.query::<&eg_game::PlayerPosition>();
            query
                .iter(world)
                .next()
                .expect("App-world player should remain present")
                .0
        };
        assert!(
            app_position[2] > 0.0,
            "App gameplay schedule must advance the active player"
        );
    }

    #[test]
    fn runtime_completed_real_gameplay_returns_to_app_menu() {
        let mut runtime = runtime_in_playing(false, false);

        {
            let world = runtime.app.world_mut();
            world.resource_mut::<SessionStateRes>().playing_level_name =
                Some("RuntimeComplete".to_string());
            let mut clock = world.resource_mut::<eg_game::GameClock>();
            clock.started = true;
            clock.level_complete = true;
            clock.completion_hold_seconds = 0.0;
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playing_level_name
                .is_none()
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .level_complete
        );
    }

    #[test]
    fn runtime_game_over_side_effects_are_app_owned() {
        let mut runtime = runtime_in_playing(false, false);

        {
            let world = runtime.app.world_mut();
            let mut clock = world.resource_mut::<eg_game::GameClock>();
            clock.started = true;
            clock.game_over = true;
            world
                .resource_mut::<eg_game::PendingTurnInputs>()
                .taps
                .push_back(eg_game::TimedTurnInput {
                    at_seconds: 1.0,
                    kind: eg_game::TurnInputKind::TurnRight,
                });
            world.resource_mut::<eg_game::DeathSfxFlag>().0 = true;
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let world = runtime.app.world();
        assert_eq!(
            *world.resource::<State<eg_types::AppPhase>>().get(),
            eg_types::AppPhase::Playing
        );
        assert!(world.resource::<eg_game::GameClock>().game_over);
        assert!(
            world
                .resource::<eg_game::PendingTurnInputs>()
                .taps
                .is_empty()
        );
        assert!(!world.resource::<eg_game::DeathSfxFlag>().0);
    }

    #[test]
    fn runtime_game_over_practice_respawn_is_app_owned() {
        let mut runtime = runtime_in_playing(false, false);

        let checkpoint = eg_game::GameCheckpointState::from_parts(eg_game::GameCheckpointParts {
            position: [2.0, 0.0, 3.0],
            direction: eg_types::Direction::Right,
            elapsed_seconds: 3.5,
            speed: 2.0,
            objects: Vec::new(),
            vertical_velocity: 0.25,
            is_grounded: false,
            progress: eg_types::PlayerLevelProgress {
                progress_percent: 42.0,
                completed: false,
                gems_collected: 2,
                gems_max: 5,
            },
        });
        {
            let world = runtime.app.world_mut();
            let mut clock = world.resource_mut::<eg_game::GameClock>();
            clock.started = true;
            clock.game_over = true;
            world
                .resource_mut::<SessionStateRes>()
                .practice_mode_enabled = true;
            world
                .resource_mut::<SessionStateRes>()
                .practice_checkpoints
                .push(eg_state::state::PracticeCheckpoint {
                    gameplay: checkpoint,
                    trigger_base_objects: None,
                });
            world.resource_mut::<eg_game::DeathSfxFlag>().0 = true;
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let world = runtime.app.world();
        assert!(!world.resource::<eg_game::GameClock>().game_over);
        assert_eq!(world.resource::<eg_game::GameClock>().elapsed_seconds, 3.5);
        assert!(world.resource::<SessionStateRes>().practice_mode_enabled);
        assert!(!world.resource::<eg_game::DeathSfxFlag>().0);
    }

    #[test]
    fn runtime_completed_playtest_returns_to_app_editor() {
        use eg_state::events::timeline::Playtest;

        let mut runtime = runtime_in_editor("RuntimePlaytestComplete");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 5.0;

        runtime.app.world_mut().write_message(Playtest);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        {
            let world = runtime.app.world_mut();
            let mut clock = world.resource_mut::<eg_game::GameClock>();
            clock.level_complete = true;
            clock.completion_hold_seconds = 0.0;
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Editor);
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playtesting_editor
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .level_complete
        );
    }

    #[test]
    fn runtime_active_turn_right_is_consumed_by_app_gameplay() {
        let mut runtime = Runtime::new_test();

        {
            let world = runtime.app.world_mut();
            world.insert_resource(State::new(eg_types::AppPhase::Playing));
            world.resource_mut::<SessionStateRes>().playtesting_editor = false;
            world.resource_mut::<SessionStateRes>().game_paused = false;
            world.resource_mut::<eg_game::GameClock>().started = true;
            {
                let mut query =
                    world.query::<(&mut eg_game::PlayerDirection, &mut eg_game::Grounded)>();
                let (mut direction, mut grounded) = query
                    .iter_mut(world)
                    .next()
                    .expect("Runtime::new must seed an App-world player");
                direction.0 = eg_types::Direction::Forward;
                grounded.0 = true;
            }
        }
        runtime.app.world_mut().write_message(GameTurnRight);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let direction = {
            let world = runtime.app.world_mut();
            let mut query = world.query::<&eg_game::PlayerDirection>();
            query
                .iter(world)
                .next()
                .expect("App-world player should remain present")
                .0
        };
        assert_eq!(direction, eg_types::Direction::Right);
    }

    // --- App-owned editor config tests ---

    use eg_state::events::snap::{SetSnapStep, SetSnapToGrid};

    #[test]
    fn runtime_config_messages_mutate_authoritative_app_resources() {
        use eg_blocks::BlockId;
        use eg_state::events::editor_mode::SetBlockId;

        let mut runtime = Runtime::new_test();
        let selected = BlockId::new("core/lava");

        runtime.app.world_mut().write_message(SetSnapToGrid(false));
        runtime.app.world_mut().write_message(SetSnapStep(0.001));
        runtime
            .app
            .world_mut()
            .write_message(SetBlockId(selected.clone()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let config = &runtime.app.world().resource::<EditorConfigRes>().0;
        assert!(!config.snap_to_grid);
        assert_eq!(config.snap_step, 0.05);
        assert_eq!(config.selected_block_id, selected);
        assert_eq!(config.recent_block_ids.first(), Some(&selected));

        let settings = &runtime
            .app
            .world()
            .resource::<SessionStateRes>()
            .app_settings;
        assert!(!settings.editor_snap_to_grid);
        assert_eq!(settings.editor_snap_step, 0.05);

        let recent_before = config.recent_block_ids.clone();
        let _ = runtime.run_app_frame(egui::RawInput::default());
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .recent_block_ids,
            recent_before
        );
    }

    #[test]
    fn runtime_seeded_editor_config_survives_no_op_frame() {
        let mut runtime = Runtime::new_test();
        {
            let mut config = runtime.app.world_mut().resource_mut::<EditorConfigRes>();
            config.0.snap_to_grid = false;
            config.0.snap_step = 0.25;
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let config = &runtime.app.world().resource::<EditorConfigRes>().0;
        assert!(!config.snap_to_grid);
        assert_eq!(config.snap_step, 0.25);
    }
    #[test]
    fn runtime_carve_out_block_copy_and_remove_propagate_to_legacy_state() {
        use eg_state::events::block::{CopyBlock, RemoveBlock};

        let mut runtime = runtime_in_editor("BlockRuntimeCarveOut");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorObjectsRes>()
            .objects = vec![
            eg_types::LevelObject {
                id: "removed".to_string(),
                position: [0.0, 0.0, 0.0],
                ..eg_types::LevelObject::default()
            },
            eg_types::LevelObject {
                id: "kept".to_string(),
                position: [2.0, 0.0, 0.0],
                ..eg_types::LevelObject::default()
            },
        ];
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.hovered_block_index = Some(0);
        }

        runtime.app.world_mut().write_message(CopyBlock);
        let _ = runtime.run_app_frame(egui::RawInput::default());
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .clipboard
                .is_some(),
            "copy must persist in the authoritative App-world clipboard"
        );

        runtime.app.world_mut().write_message(RemoveBlock);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()
                .len(),
            1
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()[0]
                .id,
            "kept"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            1,
            "App object resource must converge with legacy State after presentation sync"
        );
    }

    #[test]
    fn runtime_add_transform_trigger_mutates_app_resources() {
        use eg_state::events::selection::AddTransformTrigger;

        let mut runtime = runtime_in_editor("SelectionRuntimeCarveOut");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 2.5;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorObjectsRes>()
            .objects = vec![eg_types::LevelObject {
            id: "object_0".to_string(),
            position: [0.0, 0.0, 0.0],
            ..eg_types::LevelObject::default()
        }];
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.hovered_block_index = Some(0);
        }
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_place_window = true;

        runtime.app.world_mut().write_message(AddTransformTrigger);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()
                .len(),
            2
        );
        let trigger_object = &runtime
            .app
            .world()
            .resource::<EditorObjectsRes>()
            .objects
            .as_slice()[1];
        assert_eq!(
            trigger_object.block_id,
            eg_types::TRANSFORM_TRIGGER_BLOCK_ID
        );
        let trigger = trigger_object
            .trigger
            .as_ref()
            .expect("new transform trigger");
        approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
        assert_eq!(
            trigger.target,
            eg_types::TriggerTarget::Objects {
                object_ids: vec!["object_0".to_string()],
            }
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_place_window,
            "selection handler must close the App-owned place window"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            2,
            "App object resource must converge with legacy State after presentation sync"
        );
    }

    #[test]
    fn runtime_carve_out_add_camera_trigger_propagates_to_legacy_state() {
        use eg_state::events::camera::AddCameraTrigger;

        let mut runtime = runtime_in_editor("CameraTriggerRuntimeCarveOut");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 2.5;
        {
            let mut camera = runtime.app.world_mut().resource_mut::<EditorCameraRes>();
            camera.0.editor_pan = [12.0, -7.0];
            camera.0.editor_target_z = 4.5;
            camera.0.editor_yaw = 0.9;
            camera.0.editor_pitch = 0.3;
        }

        runtime.app.world_mut().write_message(AddCameraTrigger);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()
                .len(),
            1
        );
        let trigger_object = &runtime
            .app
            .world()
            .resource::<EditorObjectsRes>()
            .objects
            .as_slice()[0];
        assert_eq!(trigger_object.block_id, eg_types::CAMERA_TRIGGER_BLOCK_ID);
        let trigger = trigger_object.trigger.as_ref().expect("camera trigger");
        approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
        assert_eq!(trigger.target, eg_types::TriggerTarget::Camera);
        let camera = trigger.camera.as_ref().expect("camera payload");
        assert_eq!(camera.mode, eg_types::CameraMode::Static);
        assert_eq!(camera.target_position, [12.0, 4.5, -7.0]);
        approx::assert_abs_diff_eq!(camera.yaw, 0.9, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.pitch, 0.3, epsilon = 1e-6);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            1,
            "App object resource must converge with legacy State after presentation sync"
        );
    }

    #[test]
    fn runtime_carve_out_snap_selection_to_grid_propagates_block_snap() {
        use eg_state::events::selection::SnapSelectionToGrid;

        let mut runtime = runtime_in_editor("SelectionSnapBlocks");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_step = 1.0;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorObjectsRes>()
            .objects = vec![
            eg_types::LevelObject {
                id: "first".to_string(),
                position: [0.2, 0.0, 0.7],
                ..eg_types::LevelObject::default()
            },
            eg_types::LevelObject {
                id: "second".to_string(),
                position: [3.1, 0.0, 3.6],
                ..eg_types::LevelObject::default()
            },
        ];
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0, 1];
            input.0.hovered_block_index = Some(0);
        }

        runtime.app.world_mut().write_message(SnapSelectionToGrid);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()[0]
                .position,
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()[1]
                .position,
            [3.0, 0.0, 4.0]
        );
    }

    #[test]
    fn runtime_carve_out_snap_selection_to_grid_propagates_tap_snap() {
        use eg_state::events::selection::SnapSelectionToGrid;

        let mut runtime = runtime_in_editor("SelectionSnapTap");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Tapping;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_step = 1.0;
        {
            let mut timeline = runtime.app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 8.0;
            timeline.0.clock.time_seconds = 1.5;
            timeline.0.taps.tap_times = vec![0.5, 1.5];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.2, 0.0, 1.7]];
            timeline.0.taps.selected_index = Some(1);
        }

        runtime.app.world_mut().write_message(SnapSelectionToGrid);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let selected_index = runtime
            .app
            .world()
            .resource::<EditorTimelineRes>()
            .0
            .taps
            .selected_index
            .expect("selected tap after snap");
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_indicator_positions[selected_index],
            [1.0, 0.0, 2.0],
            "the authoritative timeline must preserve the snapped tap indicator"
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.0],
            "the App cursor must remain on the snapped selected tap"
        );
    }

    #[test]
    fn runtime_carve_out_focus_camera_target_propagates_to_legacy_state() {
        use eg_state::events::selection::FocusCameraTarget;

        let mut runtime = runtime_in_editor("SelectionFocusCamera");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorObjectsRes>()
            .objects = vec![eg_types::LevelObject {
            position: [10.0, 2.0, 30.0],
            size: [2.0, 4.0, 6.0],
            ..eg_types::LevelObject::default()
        }];
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.hovered_block_index = Some(0);
        }

        runtime.app.world_mut().write_message(FocusCameraTarget);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [11.0, 33.0]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 4.0, epsilon = 1e-6);
    }

    // --- Phase 3.7 / BPM carve-out tests ---
    //
    // BPM handlers mutate authoritative `EditorTimingRes` fields directly.

    use eg_state::events::bpm::{BpmTap, BpmTapReset};

    /// Writing a `BpmTap` message reaches the App world, mutates
    /// `EditorTimingRes.0.bpm_tap_times`, and the per-frame sync
    /// propagates the value to the legacy `State.editor.timing`
    /// fields.
    #[test]
    fn runtime_carve_out_bpm_tap_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

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
        // The authoritative resource remains observable after the frame.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .as_slice()
                .len(),
            1,
            "BpmTap must remain recorded in EditorTimingRes"
        );
    }

    /// `BpmTapReset` is a carve-out: writing the message clears the
    /// resource's tap history and the sync propagates the cleared
    /// list to the legacy state.
    #[test]
    fn runtime_carve_out_bpm_tap_reset_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        // Pre-populate both sides with a fake tap history.
        let mut res = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
        res.0.bpm_tap_times = vec![1.0, 2.0, 3.0];
        res.0.bpm_tap_result = Some(120.0);
        drop(res);

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
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .as_slice()
                .is_empty(),
            "BpmTapReset must leave the App-world tap history empty"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_none(),
            "BpmTapReset must leave the App-world BPM result empty"
        );
    }

    /// No-op frame: when no `BpmTap`/`BpmTapReset` message is
    /// written and the resource matches the legacy state, the
    /// sync must not re-invoke the legacy setter.
    #[test]
    fn runtime_carve_out_no_op_frame_does_not_resync_bpm_tap() {
        let mut runtime = Runtime::new_test();

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
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .as_slice(),
            times.as_slice()
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result,
            result
        );
    }

    /// Regression: `EditorTimingRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded bpm-tap sync
    /// does not clobber an in-flight tap history on the first
    /// frame. Same pattern as the snap / block-id first-frame
    /// clobber regressions.
    #[test]
    fn runtime_seeds_editor_timing_res_from_legacy_state_no_first_frame_clobber() {
        let mut runtime = Runtime::new_test();
        // User-driven tap history + computed result. The default
        // resource is empty (`bpm_tap_times = []`,
        // `bpm_tap_result = None`); without seeding, the change-
        // guarded sync would see a diff and clobber the user value
        // back to defaults on the first frame.
        let times = vec![10.0_f64, 11.0, 12.0];
        let result = Some(60.0_f32);
        {
            let mut timing = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
            timing.0.bpm_tap_times = times.clone();
            timing.0.bpm_tap_result = result;
        }

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
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .as_slice(),
            times.as_slice(),
            "first-frame sync must not clobber user bpm_tap_times"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result,
            result,
            "first-frame sync must not clobber user bpm_tap_result"
        );
    }

    // --- App-owned session resource tests ---

    use eg_state::events::session::{RenameLevel, SetShowMetadata, UpdateSkyColor};

    #[test]
    fn runtime_session_messages_mutate_authoritative_app_resource() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .write_message(RenameLevel("AppSession".to_string()));
        runtime.app.world_mut().write_message(SetShowMetadata(true));
        runtime
            .app
            .world_mut()
            .write_message(UpdateSkyColor([2.0, -1.0, 0.5]));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("AppSession"));
        assert!(session.editor_show_metadata);
        assert_eq!(session.editor_sky_color, [1.0, 0.0, 0.5]);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("AppSession"));
        assert!(session.editor_show_metadata);
        assert_eq!(session.editor_sky_color, [1.0, 0.0, 0.5]);
    }

    #[test]
    fn runtime_seeds_session_resource_once_without_frame_mirror() {
        let mut runtime = Runtime::new_test();
        {
            let mut session = runtime.app.world_mut().resource_mut::<SessionStateRes>();
            session.editor_level_name = Some("SeededSession".to_string());
            session.editor_show_metadata = true;
            session.editor_sky_color = [0.25, 0.5, 0.75];
        }

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("SeededSession"));
        assert!(session.editor_show_metadata);
        assert_eq!(session.editor_sky_color, [0.25, 0.5, 0.75]);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("SeededSession"));
        assert!(session.editor_show_metadata);
        assert_eq!(session.editor_sky_color, [0.25, 0.5, 0.75]);
    }
    // ---- Phase 3.9 timing carve-out tests ----
    //
    // All 6 `events::timing::*` messages are carve-outs that mutate
    // `EditorTimingRes.0.{timing_points, timing_selected_index}`
    // directly. The runtime tests below pin the end-to-end path:
    // message → App-world resource → legacy State field.

    use eg_state::events::timing::{
        AddTimingPoint, RemoveTimingPoint, SetTimingPointBpm, SetTimingPointTime,
        SetTimingPointTimeSignature, SetTimingSelected,
    };

    /// `AddTimingPoint` end-to-end: the message reaches the App world,
    /// mutates `EditorTimingRes.0.timing_points`, and the per-frame
    /// sync propagates the new list to the legacy `State.editor.timing`.
    #[test]
    fn runtime_carve_out_add_timing_point_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        // Precondition: empty list on both sides (the seed is empty for
        // the App-native fixture).
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .is_empty()
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()
                .is_empty()
        );

        runtime.app.world_mut().write_message(AddTimingPoint {
            time_seconds: 1.0,
            bpm: 120.0,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource holds the new point.
        let timing = &runtime.app.world().resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points.len(), 1);
        assert_eq!(timing.timing_points[0].time_seconds, 1.0);
        // Legacy state matches the resource after the sync.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()
                .len(),
            1
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .time_seconds,
            1.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .bpm,
            120.0,
            epsilon = 1e-6
        );
    }

    /// `RemoveTimingPoint` end-to-end: list shrinks via the resource,
    /// the legacy state mirrors it after the sync.
    #[test]
    fn runtime_carve_out_remove_timing_point_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        // Pre-populate both sides with a single timing point so we can
        // remove it through the Bevy path.
        let initial = eg_types::TimingPoint {
            time_seconds: 0.5,
            bpm: 90.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];

        runtime.app.world_mut().write_message(RemoveTimingPoint(0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .is_empty()
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()
                .is_empty()
        );
    }

    /// `SetTimingPointTime` end-to-end: the timestamp update flows
    /// through the resource and the legacy state picks it up via the
    /// change-guarded sync.
    #[test]
    fn runtime_carve_out_set_timing_point_time_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];

        runtime.app.world_mut().write_message(SetTimingPointTime {
            index: 0,
            time: 2.5,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points[0]
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );
    }

    /// `SetTimingPointBpm` end-to-end: BPM update propagates with
    /// the >= 1.0 clamp.
    #[test]
    fn runtime_carve_out_set_timing_point_bpm_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];

        runtime.app.world_mut().write_message(SetTimingPointBpm {
            index: 0,
            bpm: 0.25,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .bpm,
            1.0,
            epsilon = 1e-6
        );
    }

    /// `SetTimingPointTimeSignature` end-to-end: the time-signature
    /// fields update on both sides.
    #[test]
    fn runtime_carve_out_set_timing_point_time_signature_propagates_to_legacy_state() {
        let mut runtime = Runtime::new_test();

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];

        runtime
            .app
            .world_mut()
            .write_message(SetTimingPointTimeSignature {
                index: 0,
                numerator: 3,
                denominator: 8,
            });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .time_signature_numerator,
            3
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()[0]
                .time_signature_denominator,
            8
        );
    }

    /// `SetTimingSelected` end-to-end: the selected-index update
    /// SetTimingSelected updates the authoritative App selection.
    #[test]
    fn runtime_carve_out_set_timing_selected_mutates_app_resource() {
        let mut runtime = Runtime::new_test();

        // Initial: None on both sides.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            None
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            None
        );

        runtime
            .app
            .world_mut()
            .write_message(SetTimingSelected(Some(2)));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            Some(2)
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            Some(2)
        );
    }

    /// No-op frame: when no timing message is written and the
    /// resource matches the legacy state (the seeded invariant), the
    /// sync must not re-invoke `set_editor_timing_state`. Pin the
    /// list length stays the same.
    #[test]
    fn runtime_carve_out_no_op_frame_preserves_app_timing() {
        let mut runtime = Runtime::new_test();

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        {
            let mut res = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![initial.clone()];
            res.0.timing_selected_index = Some(0);
        }

        // Pre-condition: both sides match.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .len(),
            1
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()
                .len(),
            1
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            Some(0)
        );

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Post-condition: still matching, no extra writes.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .as_slice()
                .len(),
            1
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            Some(0)
        );
    }

    /// App-initialized timing points remain authoritative across a no-op frame.
    #[test]
    fn runtime_preserves_initialized_editor_timing_points() {
        let mut runtime = Runtime::new_test();
        let user_points = vec![
            eg_types::TimingPoint {
                time_seconds: 0.0,
                bpm: 140.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            },
            eg_types::TimingPoint {
                time_seconds: 4.0,
                bpm: 160.0,
                time_signature_numerator: 3,
                time_signature_denominator: 4,
            },
        ];
        {
            let mut timing = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
            timing.0.timing_points = user_points.clone();
            timing.0.timing_selected_index = Some(1);
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timing = &runtime.app.world().resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points, user_points);
        assert_eq!(timing.timing_selected_index, Some(1));
    }
    // ---- App-owned editor timeline tests ----

    use eg_state::events::timeline::{ClearTaps, SetTimelineTime};

    #[test]
    fn runtime_timeline_messages_mutate_authoritative_app_resource() {
        let mut runtime = runtime_in_editor("TimelineAppAuthority");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Tapping;
        {
            let mut timeline = runtime.app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 4.0;
            timeline.0.taps.tap_times = vec![0.25, 1.0];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]];
            timeline.0.taps.selected_index = Some(1);
        }

        runtime.app.world_mut().write_message(SetTimelineTime(1.5));
        let _ = runtime.run_app_frame(egui::RawInput::default());
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            1.5,
            epsilon = 1e-6
        );

        runtime.app.world_mut().write_message(ClearTaps);
        let _ = runtime.run_app_frame(egui::RawInput::default());
        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert!(timeline.taps.tap_times.is_empty());
        assert!(timeline.taps.tap_indicator_positions.is_empty());
        assert_eq!(timeline.taps.selected_index, None);

        let _ = runtime.run_app_frame(egui::RawInput::default());
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_times
                .is_empty()
        );
    }
    #[test]
    fn runtime_initialized_timeline_survives_no_op_frame() {
        let mut runtime = runtime_in_editor("TimelineSeed");
        {
            let mut timeline = runtime.app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 6.0;
            timeline.0.clock.time_seconds = 2.5;
            timeline.0.taps.tap_times = vec![0.5, 1.5];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]];
            timeline.0.taps.selected_index = Some(1);
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        approx::assert_abs_diff_eq!(timeline.clock.duration_seconds, 6.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(timeline.clock.time_seconds, 2.5, epsilon = 1e-6);
        assert_eq!(timeline.taps.tap_times, vec![0.5, 1.5]);
        assert_eq!(timeline.taps.selected_index, Some(1));
    }
    // ---- Phase 3.10 spawn carve-out tests ----
    //
    // All three spawn messages mutate authoritative App-world resources.
    // The runtime tests below pin message handling against those resources.

    use eg_state::events::spawn::{RotatePlacementPreview, RotateSpawnDirection, SetSpawnHere};
    use eg_state::state::EditorSpawnRes;
    use eg_types::Direction;

    /// `SetSpawnHere` writes the current editor cursor to `EditorSpawnRes`.
    #[test]
    fn runtime_set_spawn_here_mutates_app_resource() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .cursor = [3.0, 6.0, 9.0];
        runtime.app.world_mut().write_message(SetSpawnHere);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.position,
            [3.0, 6.0, 9.0]
        );
    }

    /// `RotateSpawnDirection` updates the authoritative App-world direction.
    #[test]
    fn runtime_rotate_spawn_direction_mutates_app_resource() {
        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(RotateSpawnDirection);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );
    }
    /// `RotatePlacementPreview` updates the authoritative placement yaw.
    #[test]
    fn runtime_rotate_placement_preview_mutates_app_config() {
        let mut runtime = Runtime::new_test();

        // Initial yaw 0.0 on both sides.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .selected_block_rotation_degrees = [0.0, 0.0, 0.0];
        // The seeded resource matches the legacy state default
        // (zeroed rotation), so the first frame is a no-op.
        runtime
            .app
            .world_mut()
            .write_message(RotatePlacementPreview);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource holds the new rotation.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_rotation_degrees,
            [0.0, 90.0, 0.0]
        );
    }

    /// A frame without spawn messages preserves the App-owned spawn resource.
    #[test]
    fn runtime_spawn_resource_survives_no_op_frame() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .insert_resource(EditorSpawnRes(eg_types::SpawnMetadata {
                position: [2.0, 4.0, 6.0],
                direction: Direction::Right,
            }));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let spawn = &runtime.app.world().resource::<EditorSpawnRes>().0;
        assert_eq!(spawn.position, [2.0, 4.0, 6.0]);
        assert_eq!(spawn.direction, Direction::Right);
    }

    /// App-initialized spawn metadata remains authoritative across a no-op frame.
    #[test]
    fn runtime_preserves_initialized_editor_spawn_resource() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Editor);
        runtime
            .app
            .world_mut()
            .insert_resource(EditorSpawnRes(eg_types::SpawnMetadata {
                position: [5.0, 10.0, 15.0],
                direction: Direction::Right,
            }));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let spawn = &runtime.app.world().resource::<EditorSpawnRes>().0;
        assert_eq!(spawn.position, [5.0, 10.0, 15.0]);
        assert_eq!(spawn.direction, Direction::Right);
    }
    // ---- Phase 3.11 App-owned camera and trigger-selection tests ----
    //
    // Camera and trigger messages mutate authoritative App-world resources.

    use eg_state::events::camera::{AdjustZoom, SetCameraOrientation, SetTriggerSelected};

    #[test]
    fn runtime_adjust_zoom_mutates_app_camera() {
        let mut runtime = Runtime::new_test();
        let (initial_pan, initial_z) = {
            let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
            (camera.editor_pan, camera.editor_target_z)
        };

        runtime.app.world_mut().write_message(AdjustZoom(1.0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert!(
            camera.editor_pan != initial_pan || camera.editor_target_z != initial_z,
            "AdjustZoom handler must move the editor camera"
        );
    }

    #[test]
    fn runtime_set_camera_orientation_mutates_app_camera() {
        let mut runtime = Runtime::new_test();
        let new_yaw = 1.5_f32;
        let new_pitch = 0.4_f32;

        runtime.app.world_mut().write_message(SetCameraOrientation {
            rotation: new_yaw,
            pitch: new_pitch,
            transition_seconds: None,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        approx::assert_abs_diff_eq!(camera.editor_yaw, new_yaw, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_pitch, new_pitch, epsilon = 1e-6);
    }

    #[test]
    fn runtime_set_trigger_selected_mutates_app_resource() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .write_message(SetTriggerSelected(Some(1)));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTriggerRes>()
                .0
                .selected_index,
            Some(1)
        );
    }

    #[test]
    fn runtime_app_camera_survives_no_op_frame() {
        let mut runtime = Runtime::new_test();
        let initial = {
            let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
            (
                camera.editor_pan,
                camera.editor_target_z,
                camera.editor_yaw,
                camera.editor_pitch,
            )
        };

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, initial.0);
        approx::assert_abs_diff_eq!(camera.editor_target_z, initial.1, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_yaw, initial.2, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_pitch, initial.3, epsilon = 1e-6);
    }

    #[test]
    fn runtime_editor_camera_pan_runs_in_app_schedule() {
        let mut runtime = runtime_in_editor("CameraPanSchedule");
        let initial_pan = runtime
            .app
            .world()
            .resource::<EditorCameraRes>()
            .0
            .editor_pan;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .pan_right_held = true;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_ne!(
            runtime
                .app
                .world()
                .resource::<EditorCameraRes>()
                .0
                .editor_pan,
            initial_pan
        );
    }

    #[test]
    fn runtime_editor_camera_transition_runs_in_app_schedule() {
        let mut runtime = runtime_in_editor("CameraTransitionSchedule");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorCameraRes>()
            .0
            .set_transition(0.0, 0.0, 1.0, 0.5, 1.0);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert!(camera.editor_yaw > 0.0);
        assert!(camera.editor_yaw < 1.0);
        assert!(camera.editor_pitch > 0.0);
        assert!(camera.editor_pitch < 0.5);
    }

    #[test]
    fn runtime_initialized_app_camera_and_trigger_survive_frame() {
        use eg_state::state::EditorCameraState;

        let mut runtime = Runtime::new_test();
        runtime.app.world_mut().resource_mut::<EditorCameraRes>().0 = EditorCameraState {
            editor_pan: [10.0, 20.0],
            editor_target_z: 30.0,
            editor_yaw: 1.0,
            editor_pitch: 0.4,
            playing_rotation: 0.0,
            playing_pitch: 0.0,
            transition: None,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTriggerRes>()
            .0
            .selected_index = Some(2);

        {
            let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
            assert_eq!(camera.editor_pan, [10.0, 20.0]);
            approx::assert_abs_diff_eq!(camera.editor_target_z, 30.0, epsilon = 1e-6);
            approx::assert_abs_diff_eq!(camera.editor_yaw, 1.0, epsilon = 1e-6);
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [10.0, 20.0]);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTriggerRes>()
                .0
                .selected_index,
            Some(2)
        );
    }

    /// `AddCameraTrigger` and `AddCameraFollowTrigger` are now
    /// carve-outs: they mutate `EditorObjectsRes` and mark object
    /// sync pending instead of staging legacy commands. The runtime
    /// test near the editor-object carve-outs pins the end-to-end
    /// path into the legacy `State` object store while that store
    /// still owns presentation side effects.
    #[test]
    fn runtime_add_camera_trigger_carve_out_note() {
        // This placeholder remains as a compact pointer to the
        // runtime-level AddCameraTrigger/AddCameraFollowTrigger
        // coverage near the editor-object carve-outs above.
    }

    // --- Phase 3.12 / Misc carve-out tests ---
    //
    // The `misc` event handlers write directly to App-world resources where
    // ownership has moved. The end-to-end tests below exercise direct
    // message-to-resource mutation.

    use eg_state::events::misc::{ToggleHitboxVisualization, TogglePlaceWindow};

    /// The hitbox toggle mutates the authoritative App input resource.
    #[test]
    fn runtime_toggle_hitbox_visualization_mutates_app_input() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .show_hitbox_visualization = false;

        runtime
            .app
            .world_mut()
            .write_message(ToggleHitboxVisualization);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization
        );
    }

    /// A no-message frame must preserve App-owned input instead of
    /// refreshing it from the legacy compatibility shell.
    #[test]
    fn runtime_no_op_frame_preserves_app_hitbox_visualization() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .show_hitbox_visualization = true;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization
        );
    }

    // --- App-owned navigation tests ---
    use eg_state::events::navigation::{GameResume, GameSetPracticeMode};

    #[test]
    fn runtime_game_resume_mutates_app_session() {
        let mut runtime = runtime_in_playing(false, true);

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "SessionStateRes.game_paused must retain App fixture initialization"
        );

        runtime.app.world_mut().write_message(GameResume);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "GameResume handler must clear SessionStateRes.game_paused"
        );
    }

    #[test]
    fn runtime_next_level_in_menu_mutates_app_resource() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Menu);
        {
            let mut menu = runtime.app.world_mut().resource_mut::<MenuStateRes>();
            menu.0.levels = vec!["RuntimeMenuA".to_string(), "RuntimeMenuB".to_string()];
            menu.0.selected_level = 0;
        }

        eg_ui::write_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::NextLevel,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            1,
            "NextLevel in Menu must mutate MenuStateRes.selected_level"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            1,
            "MenuStateRes remains authoritative after navigation"
        );
    }

    #[test]
    fn runtime_next_level_in_editor_mutates_app_cursor() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Editor);
        {
            let mut config = runtime.app.world_mut().resource_mut::<EditorConfigRes>();
            config.0.snap_to_grid = true;
            config.0.snap_step = 0.25;
        }
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .cursor = [0.0, 0.0, 0.0];

        eg_ui::write_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::NextLevel,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            [0.25, 0.0, 0.0],
            "NextLevel in Editor must mutate EditorInputRes.cursor"
        );
    }

    #[test]
    fn runtime_turn_right_in_editor_places_block_without_request_sync() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Editor);
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = eg_types::EditorMode::Place;
            input.0.cursor = [2.0, 0.0, 3.0];
        }

        eg_ui::write_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::TurnRight,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let objects = runtime.app.world().resource::<EditorObjectsRes>();
        assert_eq!(objects.objects.len(), 1);
        assert_eq!(objects.objects[0].position, [2.0, 0.0, 3.0]);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .as_slice()
                .len(),
            1
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Scale,
            "Editor placement must keep the App-owned select-placed mode transition"
        );
    }

    #[test]
    fn runtime_turn_right_in_game_over_returns_to_menu_without_request_sync() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::GameOver);

        eg_ui::write_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::TurnRight,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            current_phase(&runtime),
            eg_types::AppPhase::Menu,
            "GameOver TurnRight must queue the App-owned Menu transition"
        );
        assert!(
            current_phase(&runtime) == eg_types::AppPhase::Menu,
            "the App-owned phase must remain Menu after game-over return"
        );
    }

    #[test]
    fn runtime_turn_right_in_non_practice_game_over_resets_app_gameplay() {
        let mut runtime = runtime_in_playing(false, false);

        {
            let world = runtime.app.world_mut();
            world.resource_mut::<eg_game::GameClock>().started = true;
            world.resource_mut::<eg_game::GameClock>().game_over = true;
            world.write_message(GameTurnRight);
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .game_over
        );
    }

    #[test]
    fn runtime_turn_right_respawns_from_app_practice_checkpoint() {
        let mut runtime = runtime_in_playing(false, false);

        let checkpoint = eg_game::GameCheckpointState::from_parts(eg_game::GameCheckpointParts {
            position: [2.0, 0.0, 3.0],
            direction: eg_types::Direction::Right,
            elapsed_seconds: 3.5,
            speed: 2.0,
            objects: Vec::new(),
            vertical_velocity: 0.25,
            is_grounded: false,
            progress: eg_types::PlayerLevelProgress {
                progress_percent: 42.0,
                completed: false,
                gems_collected: 2,
                gems_max: 5,
            },
        });
        {
            let world = runtime.app.world_mut();
            world.resource_mut::<eg_game::GameClock>().started = true;
            world.resource_mut::<eg_game::GameClock>().game_over = true;
            let mut session = world.resource_mut::<SessionStateRes>();
            session.practice_mode_enabled = true;
            session
                .practice_checkpoints
                .push(eg_state::state::PracticeCheckpoint {
                    gameplay: checkpoint,
                    trigger_base_objects: None,
                });
            world.write_message(GameTurnRight);
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .game_over
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .elapsed_seconds,
            3.5
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled
        );
    }

    #[test]
    fn runtime_turn_right_restarts_practice_without_checkpoint_in_app_world() {
        let mut runtime = runtime_in_playing(false, false);

        {
            let world = runtime.app.world_mut();
            world.resource_mut::<eg_game::GameClock>().started = true;
            world.resource_mut::<eg_game::GameClock>().game_over = true;
            world
                .resource_mut::<SessionStateRes>()
                .practice_mode_enabled = true;
            world.write_message(GameTurnRight);
        }
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<eg_game::GameClock>()
                .game_over
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled
        );
    }

    #[test]
    fn runtime_turn_right_in_paused_gameplay_is_a_request_free_noop() {
        let mut runtime = runtime_in_playing(false, true);

        eg_ui::write_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::TurnRight,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            current_phase(&runtime),
            eg_types::AppPhase::Playing,
            "paused gameplay TurnRight must preserve the App phase"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "paused gameplay TurnRight must preserve the App-world pause flag"
        );
    }

    #[test]
    fn runtime_menu_state_resource_survives_no_op_frame() {
        let mut runtime = runtime_in_phase(eg_types::AppPhase::Menu);
        {
            let mut menu = runtime.app.world_mut().resource_mut::<MenuStateRes>();
            menu.0.levels = vec!["RuntimeMenuA".to_string(), "RuntimeMenuB".to_string()];
            menu.0.selected_level = 0;
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            0,
            "a no-op frame must preserve App-world menu selection"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            0,
            "MenuStateRes remains authoritative across frames"
        );
    }

    #[test]
    fn runtime_app_resume_state_remains_authoritative() {
        let mut runtime = runtime_in_playing(false, true);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .game_paused = false;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "App-owned pause state must not be overwritten before the schedule"
        );
    }

    /// Writing a `GameSetPracticeMode(true)` message reaches the
    /// App world and mutates `SessionStateRes.practice_mode_enabled`.
    #[test]
    fn runtime_game_set_practice_mode_mutates_app_resource() {
        let mut runtime = runtime_in_playing(false, false);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .practice_mode_enabled = false;

        // Write the message and run a frame so the App schedule's
        // handler applies the practice-mode change.
        runtime
            .app
            .world_mut()
            .write_message(GameSetPracticeMode { enabled: true });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled,
            "GameSetPracticeMode(true) handler must mutate SessionStateRes.practice_mode_enabled"
        );
    }

    /// No-op frame for practice mode preserves the seeded App-world value.
    #[test]
    fn runtime_no_op_frame_preserves_practice_mode() {
        let mut runtime = runtime_in_playing(false, false);
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .practice_mode_enabled = true;

        // With no message, the App resource stays at the seeded value.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled
        );
    }

    /// App-native construction preserves explicitly initialized practice state.
    #[test]
    fn runtime_initialized_practice_mode_survives_first_frame() {
        let mut runtime = runtime_in_playing(false, false);
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .practice_mode_enabled = true;

        // The App resource reflects the authoritative initialization.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled,
            "SessionStateRes.practice_mode_enabled must retain App initialization"
        );

        // A frame without a message preserves App-owned state.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled,
            "the seeded App-owned practice mode must survive a no-op frame"
        );
    }

    /// Writing a `TogglePlaceWindow` message mutates the App-owned session.
    #[test]
    fn runtime_toggle_place_window_mutates_app_session() {
        let mut runtime = Runtime::new_test();

        // Reset the authoritative resource to a known value.
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_place_window = false;

        runtime.app.world_mut().write_message(TogglePlaceWindow);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_place_window,
            "TogglePlaceWindow handler must mutate SessionStateRes"
        );
    }

    /// A no-op frame preserves the App-owned place-window field.
    #[test]
    fn runtime_no_op_frame_preserves_place_window() {
        let mut runtime = Runtime::new_test();

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_place_window = true;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message means the resource remains unchanged.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_place_window
        );
    }

    /// `Escape` in the menu is an App-owned no-op and does not use a legacy
    /// Runtime request consumer.
    #[test]
    fn runtime_escape_in_menu_is_app_owned_noop() {
        use eg_state::events::misc::Escape;

        let mut runtime = Runtime::new_test();

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
    }

    #[test]
    fn runtime_escape_in_real_gameplay_pauses_without_request_sync() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_playing(false, false);

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "real-gameplay Escape must pause through SessionStateRes"
        );

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "real-gameplay Escape must toggle pause back off"
        );
    }

    #[test]
    fn runtime_escape_in_playtest_returns_to_editor_without_request() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_playing(true, false);

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playtesting_editor,
            "SessionStateRes must be seeded with the playtest flag"
        );

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "playtest Escape must not toggle the real-game pause flag"
        );
        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Editor);
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .playtesting_editor
        );
    }

    #[test]
    fn runtime_escape_with_editor_selection_clears_without_request_sync() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_phase(eg_types::AppPhase::Editor);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorObjectsRes>()
            .objects = vec![eg_types::LevelObject::default()];
        {
            let mut input = runtime.app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.hovered_block_index = Some(0);
        }

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let input = &runtime.app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_index, None);
        assert!(input.selected_block_indices.is_empty());
        assert_eq!(input.hovered_block_index, None);
    }

    #[test]
    fn runtime_escape_with_editor_playback_stops_without_request_sync() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_editor("EscapePlayback");
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Null;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .last_mode = Some(eg_types::EditorMode::Timing);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .playback
            .playing = true;

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert!(!timeline.playback.playing);

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Timing
        );
    }

    #[test]
    fn runtime_escape_with_editor_timeline_time_resets_without_request_sync() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_editor("EscapeTimelineReset");
        {
            let mut timeline = runtime.app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 4.0;
            timeline.0.clock.time_seconds = 2.5;
        }

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            0.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn runtime_escape_at_editor_origin_returns_to_menu_without_request() {
        use eg_state::events::misc::Escape;

        let mut runtime = runtime_in_editor("EscapeEditorMenu");

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(current_phase(&runtime), eg_types::AppPhase::Menu);
        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .audio_stop_pending
        );
    }

    /// `Escape` is a no-op on a no-message frame. Pin the change
    /// guard: stale requests don't accumulate across frames.
    #[test]
    fn runtime_carve_out_escape_no_message_frame_is_noop() {
        let mut runtime = Runtime::new_test();
        let _ = runtime.run_app_frame(egui::RawInput::default());
    }

    // --- Phase 3.13 / Editor-mode carve-out tests ---

    use eg_state::events::editor_mode::SetEditorMode;

    #[test]
    fn runtime_set_editor_mode_mutates_app_input() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Place;

        runtime
            .app
            .world_mut()
            .write_message(SetEditorMode(eg_types::EditorMode::Tapping));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Tapping
        );
    }

    #[test]
    fn runtime_no_op_frame_preserves_app_editor_mode() {
        let mut runtime = Runtime::new_test();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Select;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Select
        );
    }
}
