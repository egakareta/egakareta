/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{
    AppPhaseRes, AuthStateRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
    EditorObjectsRes, EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
    MenuStateRes, PerfRes, SessionStateRes, evaluate_camera_track_for_objects,
};

use bevy_ecs::world::World;
use eg_blocks::{BlockDefinition, BlockId};
use eg_types::{
    AppSettings, BasicLevelMetadata, Direction, EditorMode, LevelObject, MusicMetadata,
    PlayerLevelProgress, SettingsSection, TimingPoint,
};
use glam::Vec3;

use super::render::resources::RenderGpuRes;

#[derive(Clone, Copy)]
pub struct SelectedTapViewModel {
    pub index: usize,
    pub time_seconds: f32,
    pub position: [f32; 3],
}

#[derive(Clone)]
pub struct UiStateData {
    pub is_editor: bool,
    pub is_menu: bool,
    pub is_game_paused: bool,
    pub is_practice_mode_enabled: bool,
    pub mode: EditorMode,
    pub last_mode: Option<EditorMode>,
    pub available_levels: Vec<String>,
    pub level_name: Option<String>,
    pub menu_level_name: Option<String>,
    pub menu_selected_level_progress: Option<PlayerLevelProgress>,
    pub show_metadata: bool,
    pub show_place_window: bool,
    pub show_settings: bool,
    pub settings_section: SettingsSection,
    pub keybind_capture_action: Option<(String, usize)>,
    pub music_metadata: MusicMetadata,
    pub creator_metadata: BasicLevelMetadata,
    pub sky_color: [f32; 3],
    pub app_settings: AppSettings,
    pub configured_graphics_backend: String,
    pub configured_audio_backend: String,
    pub graphics_backend_locked: bool,
    pub audio_backend_locked: bool,
    pub graphics_backend_options: Vec<String>,
    pub audio_backend_options: Vec<String>,
    pub settings_restart_required: bool,
    pub auth_display_name: Option<String>,
    pub auth_pending: bool,
    pub auth_message: Option<String>,
    pub perf_overlay_enabled: bool,
    pub perf_graphics_backend: String,
    pub perf_audio_backend: String,
    pub perf_has_latest_frame: bool,
    pub snap_to_grid: bool,
    pub snap_step: f32,
    pub snap_rotation: bool,
    pub snap_rotation_step_degrees: f32,
    pub selected_block_id: BlockId,
    pub place_preview_position: [f32; 3],
    pub place_preview_size: [f32; 3],
    pub recent_block_ids: Vec<BlockId>,
    pub selected_block: Option<LevelObject>,
    pub selected_block_count: usize,
    pub clipboard_block_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub playing: bool,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub tap_times: Vec<f32>,
    pub selected_tap: Option<SelectedTapViewModel>,
    pub timeline_preview_position: [f32; 3],
    pub timeline_preview_direction: Direction,
    pub timing_points: Vec<TimingPoint>,
    pub playback_speed: f32,
    pub timing_selected_index: Option<usize>,
    pub waveform_zoom: f32,
    pub waveform_scroll: f32,
    pub waveform_samples: Vec<f32>,
    pub waveform_sample_rate: u32,
    pub waveform_window_size: usize,
    pub waveform_loading: bool,
    pub waveform_complete: bool,
    pub bpm_tap_result: Option<f32>,
    pub trigger_objects: Vec<LevelObject>,
    pub trigger_selected_index: Option<usize>,
    pub camera_position: [f32; 3],
    pub camera_preview_position: [f32; 3],
    pub camera_preview_target: [f32; 3],
    pub camera_rotation: f32,
    pub camera_pitch: f32,
    pub fps: f32,
    pub marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
    pub object_count: usize,
}

impl UiStateData {
    pub fn from_world(world: &World) -> Self {
        let phase = world.resource::<AppPhaseRes>().current;
        let session = world.resource::<SessionStateRes>();
        let input = &world.resource::<EditorInputRes>().0;
        let objects = &world.resource::<EditorObjectsRes>().objects;
        let config = &world.resource::<EditorConfigRes>().0;
        let camera = &world.resource::<EditorCameraRes>().0;
        let triggers = &world.resource::<EditorTriggerRes>().0;
        let timeline = &world.resource::<EditorTimelineRes>().0;
        let timing = &world.resource::<EditorTimingRes>().0;
        let runtime = &world.resource::<EditorRuntimeRes>().0;
        let history = &world.resource::<EditorHistoryRes>().0;
        let perf = &world.resource::<PerfRes>().0;
        let auth = world.resource::<AuthStateRes>();
        let menu = world
            .get_resource::<MenuStateRes>()
            .map(|resource| &resource.0);

        let is_editor = phase == eg_types::AppPhase::Editor;
        let is_menu = phase == eg_types::AppPhase::Menu;
        let is_real_gameplay = phase == eg_types::AppPhase::Playing && !session.playtesting_editor;

        let available_levels = menu.map(|state| state.levels.clone()).unwrap_or_default();
        let menu_level_name = menu.and_then(|state| {
            state
                .levels
                .get(state.selected_level)
                .map(ToOwned::to_owned)
        });
        let menu_selected_level_progress = menu_level_name
            .as_deref()
            .and_then(|level_name| session.app_settings.level_progress.get(level_name).copied());

        let mut selected_indices = input
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < objects.len())
            .collect::<Vec<_>>();
        if let Some(index) = input
            .selected_block_index
            .filter(|index| *index < objects.len())
        {
            selected_indices.push(index);
        }
        selected_indices.sort_unstable();
        selected_indices.dedup();
        let selected_block = selected_indices
            .first()
            .and_then(|index| objects.get(*index).cloned());

        let selected_tap = timeline.taps.selected_index.and_then(|index| {
            Some(SelectedTapViewModel {
                index,
                time_seconds: *timeline.taps.tap_times.get(index)?,
                position: *timeline.taps.tap_indicator_positions.get(index)?,
            })
        });

        let mut trigger_objects = objects
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

        let camera_target = camera.editor_camera_target();
        let camera_position = (camera_target + camera.editor_camera_offset()).to_array();
        let preview_sample = evaluate_camera_track_for_objects(
            camera,
            objects,
            timeline.clock.time_seconds,
            Vec3::from_array(timeline.preview.position),
            true,
        );

        let auth_display_name = auth.display_name().map(ToOwned::to_owned);

        let marquee_selection_rect_screen = input.marquee_start_screen.map(|start| {
            let current = input.marquee_current_screen.unwrap_or(start);
            let dx = current[0] - start[0];
            let dy = current[1] - start[1];
            (start, current, (dx * dx + dy * dy) >= 16.0)
        });

        let perf_graphics_backend = world
            .get_non_send::<RenderGpuRes>()
            .map(|gpu| format!("{:?}", gpu.0.adapter_info.backend))
            .unwrap_or_else(|| "Unknown".to_string());
        let perf_has_latest_frame = perf.global_frame_view.lock().latest_frame().is_some();

        Self {
            is_editor,
            is_menu,
            is_game_paused: is_real_gameplay && session.game_paused,
            is_practice_mode_enabled: is_real_gameplay && session.practice_mode_enabled,
            mode: input.mode,
            last_mode: runtime.interaction.last_mode,
            available_levels,
            level_name: session.editor_level_name.clone(),
            menu_level_name,
            menu_selected_level_progress,
            show_metadata: session.editor_show_metadata,
            show_place_window: session.editor_show_place_window,
            show_settings: session.editor_show_settings,
            settings_section: session.editor_settings_section,
            keybind_capture_action: session.editor_keybind_capture_action.clone(),
            music_metadata: session.editor_music_metadata.clone(),
            creator_metadata: session.editor_creator_metadata.clone(),
            sky_color: session.editor_sky_color,
            app_settings: session.app_settings.clone(),
            configured_graphics_backend: session.app_settings.graphics_backend.clone(),
            configured_audio_backend: session.app_settings.audio_backend.clone(),
            graphics_backend_locked: session.graphics_backend_locked,
            audio_backend_locked: session.audio_backend_locked,
            graphics_backend_options: session.available_graphics_backends.clone(),
            audio_backend_options: session.available_audio_backends.clone(),
            settings_restart_required: session.settings_restart_required,
            auth_display_name,
            auth_pending: auth.pending,
            auth_message: auth.message.clone(),
            perf_overlay_enabled: perf.overlay_enabled,
            perf_graphics_backend,
            perf_audio_backend: session.app_settings.audio_backend.clone(),
            perf_has_latest_frame,
            snap_to_grid: config.snap_to_grid,
            snap_step: config.snap_step,
            snap_rotation: config.snap_rotation,
            snap_rotation_step_degrees: config.snap_rotation_step_degrees,
            selected_block_id: config.selected_block_id.clone(),
            place_preview_position: input.cursor,
            place_preview_size: BlockDefinition::get(&config.selected_block_id)
                .render
                .default_size,
            recent_block_ids: config.recent_block_ids.clone(),
            selected_block,
            selected_block_count: selected_indices.len(),
            clipboard_block_count: runtime
                .interaction
                .clipboard
                .as_ref()
                .map(|clipboard| clipboard.objects.len())
                .unwrap_or(0),
            can_undo: !history.undo.is_empty(),
            can_redo: !history.redo.is_empty(),
            playing: timeline.playback.playing,
            timeline_time_seconds: timeline.clock.time_seconds,
            timeline_duration_seconds: timeline.clock.duration_seconds,
            tap_times: timeline.taps.tap_times.clone(),
            selected_tap,
            timeline_preview_position: timeline.preview.position,
            timeline_preview_direction: timeline.preview.direction,
            timing_points: timing.timing_points.clone(),
            playback_speed: timing.playback_speed,
            timing_selected_index: timing.timing_selected_index,
            waveform_zoom: timing.waveform_zoom,
            waveform_scroll: timing.waveform_scroll,
            waveform_samples: timing.waveform_samples.clone(),
            waveform_sample_rate: timing.waveform_sample_rate,
            waveform_window_size: timing.waveform_window_size,
            waveform_loading: timing.waveform_loading,
            waveform_complete: timing.waveform_complete,
            bpm_tap_result: timing.bpm_tap_result,
            trigger_objects,
            trigger_selected_index: triggers.selected_index,
            camera_position,
            camera_preview_position: preview_sample.eye.to_array(),
            camera_preview_target: preview_sample.target.to_array(),
            camera_rotation: camera.editor_yaw,
            camera_pitch: camera.editor_pitch,
            fps: perf.fps_smoothed,
            marquee_selection_rect_screen,
            object_count: objects.len(),
        }
    }
}
