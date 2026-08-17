/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! The Bevy-owned application composition.
//!
//! This module is the single production root for the final runner cutover.
//! It deliberately composes domain plugins after Bevy's `DefaultPlugins`, so
//! the app, window, task pools, assets, render world, input, time, and state
//! schedules have one owner.

#[cfg(not(test))]
use bevy::tasks::{IoTaskPool, Task, TaskPool, futures::check_ready};
use bevy::{
    app::{App, Plugin, PluginGroup, PostStartup, PreUpdate, Startup, Update},
    camera::{
        Camera, ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
        visibility::RenderLayers,
    },
    ecs::{
        error::Result,
        message::MessageWriter,
        schedule::IntoScheduleConfigs,
        system::{Query, Res, ResMut},
        world::World,
    },
    image::Image,
    mesh::Mesh3d,
    pbr::MeshMaterial3d,
    prelude::{Camera3d, Color, DefaultPlugins, Resource, Transform, Vec3, With, default},
    render::render_resource::TextureFormat,
    state::prelude::State,
    window::{PrimaryWindow, Window, WindowPlugin},
};
use bevy_egui::{
    EguiContext, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPreUpdateSet,
    EguiPrimaryContextPass, EguiTextureHandle, EguiUserTextures, EguiZoomFactor,
    PrimaryEguiContext, egui,
};
use eg_blocks::BlockDefinition;
use eg_mesh::{build_block_template_geometry, mesh_geometry_to_bevy_mesh};
use eg_state::{
    events::{auth::CopyLatestPerfFrame, misc::TogglePerfOverlay},
    state::{
        AuthStateRes, BevyBlockRenderPlugin, BevyInputBridgeEnabled, BevyInputBridgeSet,
        BevyUiInputCaptureRes, BlockMaterial, BlockPresentationCamera, BlockPresentationMaterial,
        MenuStateRes, PerfRes, SessionStateRes, StateDomainPlugins,
    },
};
use eg_ui::{
    EditorUiInputs, MenuTextures, SettingsSidebarInputs, WorldUiCommandSink, load_menu_textures,
    register_custom_fonts, show_editor_ui, show_menu_favicon_ui, show_menu_play_ui,
    show_menu_topbar_ui, show_pause_menu_ui, show_practice_checkpoint_ui, show_settings_sidebar,
};

/// CSS selector for the web canvas owned by Bevy's winit backend.
pub const GAME_CANVAS_SELECTOR: &str = "#gameCanvas";

/// Build the Bevy-owned production app.
///
/// Native keeps Bevy's standard primary window. On the web the same window
/// targets the existing `#gameCanvas` element, avoiding a second canvas.
pub fn build_bevy_app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(game_window_plugin()));
    app.add_plugins(EgakaretaPlugin);
    app
}

fn game_window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            canvas: Some(GAME_CANVAS_SELECTOR.into()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }
}

/// Product-level plugin composition for the Bevy runtime.
///
/// The root owns presentation while `eg-game` remains headless through
/// `StateDomainPlugins` → `GameplayPlugin`. Block chunk entities use Bevy
/// mesh and material assets directly.
pub struct EgakaretaPlugin;

/// Retains the egui-managed menu images for the lifetime of the primary
/// Bevy egui context. The handles are registered through egui itself, so no
/// custom wgpu texture upload or renderer state is required.
#[derive(Resource, Default)]
struct BevyMenuTextures(Option<MenuTextures>);

/// Tracks whether custom fonts have been registered with the egui context.
#[derive(Resource, Default)]
struct BevyCustomFontsRegistered(bool);

/// Egui texture ids backed by Bevy render-to-image block previews.
///
/// Both the image handles and their egui registrations are retained by
/// `EguiUserTextures`, leaving this resource as the presentation lookup used
/// by editor widgets.
#[derive(Resource, Default)]
struct BevyBlockIconTextures(std::collections::HashMap<String, egui::TextureId>);

/// Marks meshes rendered solely into an editor block-icon image.
#[derive(bevy::ecs::component::Component)]
struct BlockIconPresentation;

/// Marks the camera associated with a single editor block-icon image.
#[derive(bevy::ecs::component::Component)]
struct BlockIconCamera;

#[derive(Resource, Default)]
struct BlockIconPreviewRenderState {
    initial_render_queued: bool,
    cameras_deactivated: bool,
}

const BLOCK_ICON_SIZE: u32 = 96;
const BLOCK_ICON_RENDER_LAYER: usize = 1;
const BLOCK_ICON_SCENE_SPACING: f32 = 8.0;
const BLOCK_ICON_VIEWPORT_HEIGHT: f32 = 1.8;

fn block_icon_scene_origin(index: usize) -> Vec3 {
    Vec3::X * (index as f32 * BLOCK_ICON_SCENE_SPACING)
}

/// Result of the Bevy-root startup storage read. Keeping the whole operation
/// in one App-owned task prevents platform I/O from delaying the first frame.
#[cfg(not(test))]
struct RootBootstrapResult {
    settings: eg_types::AppSettings,
    settings_migrated: bool,
    auth_session: Option<eg_types::AuthSession>,
    local_audio: std::collections::HashMap<String, Vec<u8>>,
    errors: Vec<String>,
}

#[cfg(not(test))]
#[derive(bevy::ecs::component::Component)]
struct RootBootstrapTask(Task<RootBootstrapResult>);

/// Observable startup status for root presentation and diagnostics.
#[cfg(not(test))]
#[derive(Resource, Default)]
struct RootBootstrapStatus {
    loading: bool,
    errors: Vec<String>,
}

impl Plugin for EgakaretaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            StateDomainPlugins,
            BevyBlockRenderPlugin,
            EguiPlugin::default(),
        ))
        .insert_resource(EguiGlobalSettings {
            auto_create_primary_context: false,
            ..default()
        })
        .init_resource::<BevyInputBridgeEnabled>()
        .init_resource::<BevyMenuTextures>()
        .init_resource::<BevyCustomFontsRegistered>()
        .init_resource::<BevyBlockIconTextures>()
        .init_resource::<BlockIconPreviewRenderState>()
        .add_systems(
            PreUpdate,
            (
                register_primary_egui_fonts_system
                    .after(EguiPreUpdateSet::InitContexts)
                    .before(EguiPreUpdateSet::BeginPass),
                sync_primary_egui_zoom_factor_system
                    .after(EguiPreUpdateSet::InitContexts)
                    .before(EguiPreUpdateSet::ProcessInput),
                sync_bevy_egui_input_capture_system
                    .after(EguiPreUpdateSet::ProcessInput)
                    .in_set(BevyInputBridgeSet::Capture),
            ),
        )
        .add_systems(Startup, sync_render_config_from_primary_window_system)
        .add_systems(
            PostStartup,
            (
                attach_primary_egui_context_system,
                initialize_block_icon_textures_system,
            ),
        )
        .add_systems(
            Update,
            (
                sync_render_config_from_primary_window_system,
                deactivate_block_icon_cameras_after_initial_render,
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                bevy_menu_ui_system,
                bevy_editor_ui_system,
                bevy_perf_overlay_system,
            ),
        );

        // Integration tests build the root app without touching user storage.
        // Production startup owns this asynchronous service boundary directly.
        #[cfg(not(test))]
        app.init_resource::<RootBootstrapStatus>()
            .add_systems(Startup, queue_root_bootstrap_task)
            .add_systems(Update, poll_root_bootstrap_task);
    }
}

fn register_primary_egui_fonts_system(
    mut contexts: Query<&mut EguiContext, With<PrimaryEguiContext>>,
    mut fonts_registered: ResMut<BevyCustomFontsRegistered>,
) {
    if fonts_registered.0 {
        return;
    }

    let Ok(mut context) = contexts.single_mut() else {
        return;
    };
    register_custom_fonts(context.get_mut());
    fonts_registered.0 = true;
}

fn sync_primary_egui_zoom_factor_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    session: Res<SessionStateRes>,
    mut contexts: Query<(&mut EguiContext, &mut EguiZoomFactor), With<PrimaryEguiContext>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((mut context, mut zoom_factor)) = contexts.single_mut() else {
        return;
    };
    let size = window.physical_size();
    let scale = eg_ui::combined_ui_scale_factor(
        egui::vec2(size.x.max(1) as f32, size.y.max(1) as f32),
        session.app_settings.normalized_ui_scale_multiplier(),
    );
    if (zoom_factor.zoom_factor - scale).abs() <= f32::EPSILON {
        return;
    }

    zoom_factor.zoom_factor = scale;
    context.get_mut().set_zoom_factor(scale);
}

fn attach_primary_egui_context_system(
    cameras: Query<bevy::ecs::entity::Entity, With<BlockPresentationCamera>>,
    mut commands: bevy::ecs::system::Commands,
) {
    let Ok(camera) = cameras.single() else {
        log::warn!("presentation camera was unavailable for the primary egui context");
        return;
    };
    log::warn!("attaching the primary egui context to the presentation camera");
    commands.entity(camera).insert(PrimaryEguiContext);
}

/// Build one retained render-to-image preview for every placeable block.
///
/// The preview meshes share the ordinary block material and texture array, so
/// their appearance tracks world geometry without the retired custom wgpu
/// snapshot renderer. A separate render layer keeps these entities out of the
/// main presentation camera while each camera targets its own Bevy `Image`.
fn initialize_block_icon_textures_system(
    material: Res<BlockPresentationMaterial>,
    mut icons: ResMut<BevyBlockIconTextures>,
    mut commands: bevy::ecs::system::Commands,
    mut images: ResMut<bevy::asset::Assets<Image>>,
    mut meshes: ResMut<bevy::asset::Assets<bevy::mesh::Mesh>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    if !icons.0.is_empty() {
        return;
    }
    let Some(material) = material.0.clone() else {
        return;
    };

    for (index, block) in BlockDefinition::all_placeable()
        .iter()
        .filter(|block| block.placeable)
        .enumerate()
    {
        let image = images.add(Image::new_target_texture(
            BLOCK_ICON_SIZE,
            BLOCK_ICON_SIZE,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let texture_id = user_textures.add_image(EguiTextureHandle::Strong(image.clone()));
        let block_id = block.id.to_string();
        icons.0.insert(block_id.clone(), texture_id);
        let scene_origin = block_icon_scene_origin(index);
        let target = scene_origin + Vec3::splat(0.5);

        commands.spawn((
            BlockIconPresentation,
            Mesh3d(
                meshes.add(mesh_geometry_to_bevy_mesh(&build_block_template_geometry(
                    &block_id,
                ))),
            ),
            MeshMaterial3d::<BlockMaterial>(material.clone()),
            Transform::from_translation(scene_origin),
            RenderLayers::layer(BLOCK_ICON_RENDER_LAYER),
        ));
        commands.spawn((
            BlockIconCamera,
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: BLOCK_ICON_VIEWPORT_HEIGHT,
                },
                ..OrthographicProjection::default_3d()
            }),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.into()),
            Transform::from_translation(target + Vec3::new(2.45, 2.0, 2.45))
                .looking_at(target, Vec3::Y),
            RenderLayers::layer(BLOCK_ICON_RENDER_LAYER),
        ));
    }
}

fn deactivate_block_icon_cameras_after_initial_render(
    mut preview_state: ResMut<BlockIconPreviewRenderState>,
    mut cameras: Query<&mut Camera, With<BlockIconCamera>>,
) {
    if preview_state.cameras_deactivated || cameras.is_empty() {
        return;
    }
    if !preview_state.initial_render_queued {
        preview_state.initial_render_queued = true;
        return;
    }

    for mut camera in &mut cameras {
        camera.is_active = false;
    }
    preview_state.cameras_deactivated = true;
}

#[cfg(not(test))]
fn queue_root_bootstrap_task(
    mut commands: bevy::ecs::system::Commands,
    mut status: ResMut<RootBootstrapStatus>,
) {
    status.loading = true;
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(async move {
        let mut errors = Vec::new();
        let mut settings = match eg_platform::io::load_app_settings_from_storage().await {
            Ok(settings) => settings,
            Err(error) => {
                errors.push(format!("Failed to load app settings: {error}"));
                eg_types::AppSettings::default()
            }
        };
        let settings_migrated = settings.migrate_defaults();
        if settings_migrated
            && let Err(error) = eg_platform::io::save_app_settings_to_storage(&settings).await
        {
            errors.push(format!("Failed to persist migrated app settings: {error}"));
        }

        let auth_session = match eg_platform::storage::load_auth_session().await {
            Ok(session) => session,
            Err(error) => {
                errors.push(format!("Failed to load auth session: {error}"));
                None
            }
        };

        RootBootstrapResult {
            settings,
            settings_migrated,
            auth_session,
            local_audio: eg_platform::io::load_all_local_audio().await,
            errors,
        }
    });
    commands.spawn(RootBootstrapTask(task));
}

#[cfg(not(test))]
fn poll_root_bootstrap_task(
    mut commands: bevy::ecs::system::Commands,
    mut tasks: Query<(bevy::ecs::entity::Entity, &mut RootBootstrapTask)>,
    mut status: ResMut<RootBootstrapStatus>,
    mut session: ResMut<SessionStateRes>,
    mut auth: ResMut<AuthStateRes>,
    audio_cache: Res<eg_state::state::EditorAudioCacheRes>,
    audio: eg_state::state::AudioBackendSystemParam<'_>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            status.loading = true;
            continue;
        };

        status.loading = false;
        status.errors = result.errors;
        for error in &status.errors {
            log::error!("{error}");
        }
        if result.settings_migrated {
            log::info!("Migrated persisted app settings to current defaults");
        }

        session.app_settings = result.settings;
        session.available_graphics_backends = supported_graphics_backends();
        session.available_audio_backends = eg_audio::audio::available_host_labels();
        session.graphics_backend_locked = false;
        session.audio_backend_locked = false;
        let _ = audio.set_preferred_backend_name(&session.app_settings.audio_backend);
        audio_cache.replace_local_files(result.local_audio);

        auth.session = result.auth_session;
        auth.pending = false;
        auth.refresh_started = false;
        auth.external_navigation_enabled = true;
        commands.entity(entity).despawn();
    }
}

#[cfg(not(test))]
fn supported_graphics_backends() -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        vec!["BrowserWebGpu".to_string(), "Gl".to_string()]
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        vec![
            "Auto".to_string(),
            "Vulkan".to_string(),
            "Metal".to_string(),
            "Dx12".to_string(),
            "Gl".to_string(),
        ]
    }
}

/// Keep extraction and UI scale inputs aligned with Bevy's authoritative
/// primary-window size instead of the retired custom surface configuration.
fn sync_render_config_from_primary_window_system(
    windows: bevy::ecs::system::Query<&Window, bevy::ecs::query::With<PrimaryWindow>>,
    mut config: ResMut<eg_state::state::RenderConfigRes>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.physical_size();
    config.width = size.x.max(1);
    config.height = size.y.max(1);
}

/// Render the editor interface from authoritative App-world resources in the
/// primary Bevy egui pass. Until icon cameras are migrated, block catalog
/// buttons use the editor's built-in non-textured fallback.
fn bevy_editor_ui_system(world: &mut World) {
    let phase = *world.resource::<State<eg_types::AppPhase>>().get();
    if phase != eg_types::AppPhase::Editor {
        return;
    }
    let editor_objects = eg_state::state::collect_editor_objects_from_world(world);
    let (
        level_name,
        music_metadata,
        creator_metadata,
        sky_color,
        show_metadata,
        show_place_window,
        settings_section,
        keybind_capture_action,
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
            session.editor_level_name.clone(),
            session.editor_music_metadata.clone(),
            session.editor_creator_metadata.clone(),
            session.editor_sky_color,
            session.editor_show_metadata,
            session.editor_show_place_window,
            session.editor_settings_section,
            session.editor_keybind_capture_action.clone(),
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
    let available_levels = world.resource::<MenuStateRes>().0.levels.clone();
    let editor_fps = world.resource::<PerfRes>().0.fps_smoothed;
    let (editor_marquee_selection_rect_screen, editor_place_preview_position, editor_mode) = {
        let input = &world.resource::<eg_state::state::EditorInputRes>().0;
        let marquee = input.marquee_start_screen.map(|start| {
            let current = input.marquee_current_screen.unwrap_or(start);
            let dx = current[0] - start[0];
            let dy = current[1] - start[1];
            (start, current, (dx * dx + dy * dy) >= 16.0)
        });
        (marquee, input.cursor, input.mode)
    };
    let editor_config = world
        .resource::<eg_state::state::EditorConfigRes>()
        .0
        .clone();
    let editor_last_mode = world
        .resource::<eg_state::state::EditorRuntimeRes>()
        .0
        .interaction
        .last_mode;
    let (
        editor_camera_position,
        editor_camera_preview_position,
        editor_camera_preview_target,
        editor_camera_rotation,
        editor_camera_pitch,
    ) = {
        let camera = &world.resource::<eg_state::state::EditorCameraRes>().0;
        let timeline = &world.resource::<eg_state::state::EditorTimelineRes>().0;
        let position = (camera.editor_camera_target() + camera.editor_camera_offset()).to_array();
        let (preview_position, preview_target) = eg_state::state::editor_camera_preview_for_objects(
            camera,
            &editor_objects,
            timeline.clock.time_seconds,
            timeline.preview.position,
            true,
        );
        (
            position,
            preview_position,
            preview_target,
            camera.editor_yaw,
            camera.editor_pitch,
        )
    };
    let (
        editor_selected_block,
        editor_selected_block_count,
        editor_clipboard_block_count,
        editor_can_undo,
        editor_can_redo,
    ) = {
        let input = &world.resource::<eg_state::state::EditorInputRes>().0;
        let mut selected = input
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < editor_objects.len())
            .collect::<Vec<_>>();
        if let Some(index) = input
            .selected_block_index
            .filter(|index| *index < editor_objects.len())
        {
            selected.push(index);
        }
        selected.sort_unstable();
        selected.dedup();
        let runtime = &world.resource::<eg_state::state::EditorRuntimeRes>().0;
        let history = &world.resource::<eg_state::state::EditorHistoryRes>().0;
        (
            selected
                .first()
                .and_then(|index| editor_objects.get(*index).cloned()),
            selected.len(),
            runtime
                .interaction
                .clipboard
                .as_ref()
                .map(|clipboard| clipboard.objects.len())
                .unwrap_or(0),
            !history.undo.is_empty(),
            !history.redo.is_empty(),
        )
    };
    let (
        timeline_playing,
        timeline_time_seconds,
        timeline_duration_seconds,
        timeline_tap_times,
        timeline_selected_tap,
        timeline_preview_position,
        timeline_preview_direction,
        timing_points,
        playback_speed,
        timing_selected_index,
        waveform_zoom,
        waveform_samples,
        waveform_sample_rate,
        waveform_window_size,
        waveform_loading,
        waveform_complete,
        bpm_tap_result,
        trigger_objects,
        trigger_selected_index,
    ) = {
        let timeline = &world.resource::<eg_state::state::EditorTimelineRes>().0;
        let timing = &world.resource::<eg_state::state::EditorTimingRes>().0;
        let mut trigger_objects = editor_objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .cloned()
            .collect::<Vec<_>>();
        trigger_objects.sort_by(|left, right| {
            f32::total_cmp(
                &left
                    .trigger
                    .as_ref()
                    .map(|trigger| trigger.time_seconds)
                    .unwrap_or(0.0),
                &right
                    .trigger
                    .as_ref()
                    .map(|trigger| trigger.time_seconds)
                    .unwrap_or(0.0),
            )
        });
        (
            timeline.playback.playing,
            timeline.clock.time_seconds,
            timeline.clock.duration_seconds,
            timeline.taps.tap_times.clone(),
            timeline.taps.selected_index.and_then(|index| {
                Some((
                    index,
                    *timeline.taps.tap_times.get(index)?,
                    *timeline.taps.tap_indicator_positions.get(index)?,
                ))
            }),
            timeline.preview.position,
            timeline.preview.direction,
            timing.timing_points.clone(),
            timing.playback_speed,
            timing.timing_selected_index,
            timing.waveform_zoom,
            timing.waveform_samples.clone(),
            timing.waveform_sample_rate,
            timing.waveform_window_size,
            timing.waveform_loading,
            timing.waveform_complete,
            timing.bpm_tap_result,
            trigger_objects,
            world
                .resource::<eg_state::state::EditorTriggerRes>()
                .0
                .selected_index,
        )
    };
    let ctx = {
        let mut contexts =
            world.query_filtered::<&mut EguiContext, bevy::ecs::query::With<PrimaryEguiContext>>();
        let Ok(mut context) = contexts.single_mut(world) else {
            return;
        };
        context.get_mut().clone()
    };
    let block_icon_texture_ids = world.resource::<BevyBlockIconTextures>().0.clone();
    let settings = SettingsSidebarInputs {
        show_settings: world.resource::<SessionStateRes>().editor_show_settings,
        is_menu: false,
        is_editor: true,
        is_game_paused: false,
        app_settings: &app_settings,
        graphics_backend: &graphics_backend,
        graphics_backend_options: &graphics_backend_options,
        graphics_backend_locked,
        settings_restart_required,
        audio_backend: &audio_backend,
        audio_backend_options: &audio_backend_options,
        audio_backend_locked,
        settings_section,
        keybind_capture_action: keybind_capture_action.as_ref(),
    };
    let blocking_rects = {
        let mut commands = WorldUiCommandSink::new(world);
        show_editor_ui(
            &ctx,
            EditorUiInputs {
                is_editor: true,
                available_levels: &available_levels,
                level_name: level_name.as_deref(),
                music_metadata: &music_metadata,
                creator_metadata: &creator_metadata,
                sky_color,
                settings,
                editor_fps,
                editor_object_count: editor_objects.len(),
                editor_marquee_selection_rect_screen,
                editor_config: &editor_config,
                editor_place_preview_position,
                editor_selected_block: editor_selected_block.as_ref(),
                editor_selected_block_count,
                editor_clipboard_block_count,
                editor_can_undo,
                editor_can_redo,
                editor_mode,
                editor_last_mode,
                editor_camera_position,
                editor_camera_preview_position,
                editor_camera_preview_target,
                editor_camera_rotation,
                editor_camera_pitch,
                show_metadata,
                show_place_window,
                timeline_playing,
                timeline_time_seconds,
                timeline_duration_seconds,
                timeline_tap_times: &timeline_tap_times,
                timeline_selected_tap,
                timeline_preview_position,
                timeline_preview_direction,
                timing_points: &timing_points,
                playback_speed,
                timing_selected_index,
                waveform_zoom,
                waveform_samples: &waveform_samples,
                waveform_sample_rate,
                waveform_window_size,
                waveform_loading,
                waveform_complete,
                bpm_tap_result,
                trigger_objects: &trigger_objects,
                trigger_selected_index,
                block_icon_texture_ids: &block_icon_texture_ids,
            },
            &mut commands,
        )
    };
    let input = &mut world.resource_mut::<eg_state::state::EditorInputRes>().0;
    input.ui_input_blocking_rects = blocking_rects;
    input.ui_input_pixels_per_point = ctx.pixels_per_point();
}

/// Render the menu, pause, practice, and settings surfaces through Bevy's
/// primary egui pass. Commands are written straight to the App world's typed
/// message buffers, keeping this path independent of the retired runtime UI
/// command queue.
fn bevy_menu_ui_system(world: &mut World, mut reported: bevy::ecs::system::Local<bool>) {
    let phase = *world.resource::<State<eg_types::AppPhase>>().get();
    let is_menu = phase == eg_types::AppPhase::Menu;
    let (is_game_paused, is_practice_mode_enabled, settings_inputs) = {
        let session = world.resource::<SessionStateRes>();
        let is_real_gameplay = phase == eg_types::AppPhase::Playing && !session.playtesting_editor;
        (
            is_real_gameplay && session.game_paused,
            is_real_gameplay && session.practice_mode_enabled,
            (
                session.editor_show_settings,
                session.app_settings.clone(),
                session.app_settings.graphics_backend.clone(),
                session.available_graphics_backends.clone(),
                session.graphics_backend_locked,
                session.settings_restart_required,
                session.app_settings.audio_backend.clone(),
                session.available_audio_backends.clone(),
                session.audio_backend_locked,
                session.editor_settings_section,
                session.editor_keybind_capture_action.clone(),
            ),
        )
    };
    let (
        show_settings,
        app_settings,
        graphics_backend,
        graphics_backend_options,
        graphics_backend_locked,
        settings_restart_required,
        audio_backend,
        audio_backend_options,
        audio_backend_locked,
        settings_section,
        keybind_capture_action,
    ) = settings_inputs;
    let (menu_level_name, menu_selected_level_progress) = {
        let session = world.resource::<SessionStateRes>();
        let menu = world.resource::<MenuStateRes>();
        let level_name = menu.0.levels.get(menu.0.selected_level).cloned();
        let progress = level_name
            .as_deref()
            .and_then(|name| session.app_settings.level_progress.get(name).copied());
        (level_name, progress)
    };
    let (auth_display_name, auth_pending, auth_message) = {
        let auth = world.resource::<AuthStateRes>();
        (
            auth.display_name().map(ToOwned::to_owned),
            auth.pending,
            auth.message.clone(),
        )
    };
    let ctx = {
        let mut contexts =
            world.query_filtered::<&mut EguiContext, bevy::ecs::query::With<PrimaryEguiContext>>();
        let Ok(mut context) = contexts.single_mut(world) else {
            return;
        };
        context.get_mut().clone()
    };
    if !*reported {
        log::warn!("rendering menu UI in {phase:?}");
        *reported = true;
    }
    #[cfg(not(test))]
    if world
        .get_resource::<RootBootstrapStatus>()
        .is_some_and(|status| status.loading)
    {
        egui::Area::new(egui::Id::new("root-bootstrap-loading"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 14.0))
            .show(&ctx, |ui| {
                ui.label("Loading local settings…");
            });
    }
    let (favicon, play_button, play_button_filled, edit_button) = {
        let mut textures = world.resource_mut::<BevyMenuTextures>();
        let textures = textures.0.get_or_insert_with(|| load_menu_textures(&ctx));
        (
            textures.favicon.clone(),
            textures.play_button.clone(),
            textures.play_button_filled.clone(),
            textures.edit_button.clone(),
        )
    };
    let settings = SettingsSidebarInputs {
        show_settings,
        is_menu,
        is_editor: false,
        is_game_paused,
        app_settings: &app_settings,
        graphics_backend: &graphics_backend,
        graphics_backend_options: &graphics_backend_options,
        graphics_backend_locked,
        settings_restart_required,
        audio_backend: &audio_backend,
        audio_backend_options: &audio_backend_options,
        audio_backend_locked,
        settings_section,
        keybind_capture_action: keybind_capture_action.as_ref(),
    };
    let mut commands = WorldUiCommandSink::new(world);
    show_settings_sidebar(&ctx, settings, &mut commands);
    show_menu_topbar_ui(
        &ctx,
        is_menu,
        &app_settings,
        auth_display_name.as_deref(),
        auth_pending,
        auth_message.as_deref(),
        &mut commands,
    );
    show_menu_play_ui(
        &ctx,
        is_menu,
        &mut commands,
        play_button.as_ref(),
        play_button_filled.as_ref(),
        edit_button.as_ref(),
    );
    show_practice_checkpoint_ui(
        &ctx,
        is_practice_mode_enabled,
        is_game_paused,
        &mut commands,
    );
    show_pause_menu_ui(
        &ctx,
        is_game_paused,
        is_practice_mode_enabled,
        &mut commands,
    );
    if let Some(favicon) = &favicon {
        show_menu_favicon_ui(
            &ctx,
            is_menu,
            menu_level_name.as_deref(),
            menu_selected_level_progress,
            favicon,
        );
    }
}

/// Synchronize egui's current input ownership before semantic game input runs.
fn sync_bevy_egui_input_capture_system(
    mut contexts: EguiContexts,
    mut capture: ResMut<BevyUiInputCaptureRes>,
) -> Result {
    let Ok(ctx) = contexts.ctx_mut() else {
        return Ok(());
    };
    capture.keyboard = ctx.egui_wants_keyboard_input();
    capture.pointer = ctx.egui_wants_pointer_input();
    Ok(())
}

/// Render the profiler through Bevy's primary egui pass.
///
/// This is deliberately a normal Bevy system rather than a wrapper around
/// the former `Runtime` frame. Its buttons write semantic App-world messages
/// directly, so the root path has no command-enum dependency for this UI.
fn bevy_perf_overlay_system(
    mut contexts: EguiContexts,
    perf: Res<PerfRes>,
    session: Res<SessionStateRes>,
    mut toggle_overlay: MessageWriter<TogglePerfOverlay>,
    mut copy_frame: MessageWriter<CopyLatestPerfFrame>,
) -> Result {
    if !perf.0.overlay_enabled {
        return Ok(());
    }

    let mut profiler_open = true;
    let has_latest_frame = perf.0.global_frame_view.lock().latest_frame().is_some();
    let fps = perf.0.fps_smoothed;
    let audio_backend = &session.app_settings.audio_backend;

    egui::Window::new("Profiler")
        .order(egui::Order::Foreground)
        .default_size([1024.0, 600.0])
        .open(&mut profiler_open)
        .show(contexts.ctx_mut()?, |ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!("{audio_backend} | FPS {fps:.1}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(has_latest_frame, egui::Button::new("📋 Copy Frame"))
                        .on_hover_text("Copy the latest profiling frame to clipboard")
                        .clicked()
                    {
                        copy_frame.write(CopyLatestPerfFrame);
                    }
                });
            });
            ui.separator();
            puffin_egui::profiler_ui(ui);
        });

    if !profiler_open {
        toggle_overlay.write(TogglePerfOverlay);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        camera::Camera,
        prelude::{MinimalPlugins, Vec3},
        window::{PrimaryWindow, Window},
    };
    use eg_state::state::{RenderConfigRes, StateDomainPlugins};

    use super::{
        BLOCK_ICON_SCENE_SPACING, BlockIconCamera, BlockIconPreviewRenderState,
        GAME_CANVAS_SELECTOR, block_icon_scene_origin,
        deactivate_block_icon_cameras_after_initial_render, game_window_plugin,
        sync_render_config_from_primary_window_system,
    };

    #[test]
    fn block_icon_scenes_do_not_overlap() {
        let first = block_icon_scene_origin(0);
        let second = block_icon_scene_origin(1);

        assert_eq!(first, Vec3::ZERO);
        assert!(second.distance(first) > BLOCK_ICON_SCENE_SPACING * 0.5);
    }

    #[test]
    fn block_icon_cameras_deactivate_after_the_initial_render_frame() {
        let mut app = App::new();
        app.init_resource::<BlockIconPreviewRenderState>()
            .add_systems(Update, deactivate_block_icon_cameras_after_initial_render);
        let entity = app
            .world_mut()
            .spawn((BlockIconCamera, Camera::default()))
            .id();

        app.update();
        assert!(
            app.world().get::<Camera>(entity).unwrap().is_active,
            "preview cameras must remain active for their initial texture render"
        );

        app.update();
        assert!(
            !app.world().get::<Camera>(entity).unwrap().is_active,
            "preview cameras must stop producing offscreen passes after initialization"
        );
    }

    #[test]
    fn root_window_configuration_targets_the_existing_web_canvas() {
        let plugin = game_window_plugin();
        let primary_window = plugin
            .primary_window
            .expect("a primary window is configured");

        assert_eq!(primary_window.canvas.as_deref(), Some(GAME_CANVAS_SELECTOR));
        assert!(primary_window.fit_canvas_to_parent);
    }

    #[test]
    fn state_domains_remain_headless_composable_and_seed_menu_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(StateDomainPlugins);
        app.update();

        assert!(app.is_plugin_added::<StateDomainPlugins>());
        assert!(
            app.world()
                .contains_resource::<eg_state::state::MenuStateRes>()
        );
        let player_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<
                bevy::ecs::entity::Entity,
                bevy::ecs::query::With<eg_game::Player>,
            >();
            query.iter(world).count()
        };
        assert_eq!(
            player_count, 1,
            "the ordinary Bevy root must seed an empty gameplay world"
        );
    }

    #[test]
    fn primary_window_size_drives_root_render_configuration() {
        let mut app = App::new();
        app.init_resource::<RenderConfigRes>()
            .add_systems(Update, sync_render_config_from_primary_window_system);
        app.world_mut().spawn((
            PrimaryWindow,
            Window {
                resolution: (960, 540).into(),
                ..Default::default()
            },
        ));

        app.update();

        assert_eq!(
            (
                app.world().resource::<RenderConfigRes>().width,
                app.world().resource::<RenderConfigRes>().height
            ),
            (960, 540)
        );
    }
}
