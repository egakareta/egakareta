/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use eg_blocks::BlockId;
use eg_types::{
    AppSettings, BasicLevelMetadata, Direction, EditorMode, LevelObject, MusicMetadata,
    PlayerLevelProgress, SettingsSection, TimingPoint,
};

#[derive(Clone, Copy)]
pub struct SelectedTapViewModel {
    pub index: usize,
    pub time_seconds: f32,
    pub position: [f32; 3],
}

pub struct UiViewModel<'a> {
    pub is_editor: bool,
    pub is_menu: bool,
    pub is_game_paused: bool,
    pub is_practice_mode_enabled: bool,
    pub mode: EditorMode,
    pub last_mode: Option<EditorMode>,
    pub available_levels: &'a [String],
    pub level_name: Option<&'a str>,
    pub menu_level_name: Option<&'a str>,
    pub menu_selected_level_progress: Option<PlayerLevelProgress>,
    pub show_metadata: bool,
    pub show_place_window: bool,
    pub show_settings: bool,
    pub settings_section: SettingsSection,
    pub keybind_capture_action: Option<&'a (String, usize)>,
    pub music_metadata: &'a MusicMetadata,
    pub creator_metadata: BasicLevelMetadata,
    pub sky_color: [f32; 3],
    pub app_settings: &'a AppSettings,
    pub configured_graphics_backend: &'a str,
    pub configured_audio_backend: &'a str,
    pub graphics_backend_locked: bool,
    pub audio_backend_locked: bool,
    pub graphics_backend_options: &'a [String],
    pub audio_backend_options: &'a [String],
    pub settings_restart_required: bool,
    pub auth_display_name: Option<&'a str>,
    pub auth_pending: bool,
    pub auth_message: Option<&'a str>,
    pub perf_overlay_enabled: bool,
    pub perf_graphics_backend: String,
    pub perf_audio_backend: String,
    pub perf_has_latest_frame: bool,
    pub snap_to_grid: bool,
    pub snap_step: f32,
    pub snap_rotation: bool,
    pub snap_rotation_step_degrees: f32,
    pub selected_block_id: &'a BlockId,
    pub place_preview_position: [f32; 3],
    pub place_preview_size: [f32; 3],
    pub recent_block_ids: &'a [BlockId],
    pub selected_block: Option<LevelObject>,
    pub selected_block_count: usize,
    pub clipboard_block_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub playing: bool,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub tap_times: &'a [f32],
    pub selected_tap: Option<SelectedTapViewModel>,
    pub timeline_preview_position: [f32; 3],
    pub timeline_preview_direction: Direction,
    pub timing_points: &'a [TimingPoint],
    pub playback_speed: f32,
    pub timing_selected_index: Option<usize>,
    pub waveform_zoom: f32,
    pub waveform_scroll: f32,
    pub waveform_samples: &'a [f32],
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

impl State {
    pub fn ui_view_model(&self) -> UiViewModel<'_> {
        let (timeline_preview_position, timeline_preview_direction) =
            self.editor_timeline_preview();
        let selected_block_indices = self.editor.selected_indices_normalized();
        let selected_tap = self
            .editor
            .selected_tap()
            .map(|(index, time_seconds, position)| SelectedTapViewModel {
                index,
                time_seconds,
                position,
            });

        let camera_target = glam::Vec3::new(
            self.editor.camera.editor_pan[0],
            self.editor.camera.editor_target_z,
            self.editor.camera.editor_pan[1],
        );
        let camera_position = (camera_target + self.editor.camera_offset()).to_array();
        let (camera_preview_position, camera_preview_target) = self.editor_preview_camera_view();

        UiViewModel {
            is_editor: self.is_editor(),
            is_menu: self.is_menu(),
            is_game_paused: self.is_game_paused(),
            is_practice_mode_enabled: self.is_practice_mode_enabled(),
            mode: self.editor_mode(),
            last_mode: self.editor.runtime.interaction.last_mode,
            available_levels: self.available_levels(),
            level_name: self.session.editor_level_name.as_deref(),
            menu_level_name: self.menu_level_name(),
            menu_selected_level_progress: self.menu_selected_level_progress(),
            show_metadata: self.editor_show_metadata(),
            show_place_window: self.editor_show_place_window(),
            show_settings: self.editor_show_settings(),
            settings_section: self.editor_settings_section(),
            keybind_capture_action: self.editor_keybind_capture_action(),
            music_metadata: self.editor_music_metadata(),
            creator_metadata: self.editor_creator_metadata().clone(),
            sky_color: self.editor_sky_color(),
            app_settings: self.app_settings(),
            configured_graphics_backend: self.app_settings().graphics_backend.as_str(),
            configured_audio_backend: self.app_settings().audio_backend.as_str(),
            graphics_backend_locked: self.graphics_backend_locked(),
            audio_backend_locked: self.audio_backend_locked(),
            graphics_backend_options: self.available_graphics_backends(),
            audio_backend_options: self.available_audio_backends(),
            settings_restart_required: self.settings_restart_required(),
            auth_display_name: self.auth_display_name(),
            auth_pending: self.auth_pending(),
            auth_message: self.auth_message(),
            perf_overlay_enabled: self.perf_overlay_enabled(),
            perf_graphics_backend: self.perf_graphics_backend(),
            perf_audio_backend: self.perf_audio_backend(),
            perf_has_latest_frame: self.perf_has_latest_frame(),
            snap_to_grid: self.editor_snap_to_grid(),
            snap_step: self.editor_snap_step(),
            snap_rotation: self.editor_snap_rotation(),
            snap_rotation_step_degrees: self.editor_snap_rotation_step_degrees(),
            selected_block_id: self.editor_selected_block_id(),
            place_preview_position: self.editor.ui.cursor,
            place_preview_size: self.editor.selected_block_default_size(),
            recent_block_ids: &self.editor.config.recent_block_ids,
            selected_block: self.editor_selected_block(),
            selected_block_count: selected_block_indices.len(),
            clipboard_block_count: self
                .editor
                .runtime
                .interaction
                .clipboard
                .as_ref()
                .map(|clipboard| clipboard.objects.len())
                .unwrap_or(0),
            can_undo: !self.editor.runtime.history.undo.is_empty(),
            can_redo: !self.editor.runtime.history.redo.is_empty(),
            playing: self.editor_is_playing(),
            timeline_time_seconds: self.editor_timeline_time_seconds(),
            timeline_duration_seconds: self.editor_timeline_duration_seconds(),
            tap_times: self.editor_tap_times(),
            selected_tap,
            timeline_preview_position,
            timeline_preview_direction,
            timing_points: self.editor_timing_points(),
            playback_speed: self.editor_playback_speed(),
            timing_selected_index: self.editor_timing_selected_index(),
            waveform_zoom: self.editor_waveform_zoom(),
            waveform_scroll: self.editor_waveform_scroll(),
            waveform_samples: self.editor_waveform_samples(),
            waveform_sample_rate: self.editor_waveform_sample_rate(),
            waveform_window_size: self.editor_waveform_window_size(),
            waveform_loading: self.editor_waveform_loading(),
            waveform_complete: self.editor_waveform_complete(),
            bpm_tap_result: self.editor_bpm_tap_result(),
            trigger_objects: self.editor_triggers(),
            trigger_selected_index: self.editor_selected_trigger_index(),
            camera_position,
            camera_preview_position,
            camera_preview_target,
            camera_rotation: self.editor.camera.editor_yaw,
            camera_pitch: self.editor.camera.editor_pitch,
            fps: self.editor_fps(),
            marquee_selection_rect_screen: self.editor_marquee_selection_rect_screen(),
            object_count: self.editor.objects.len(),
        }
    }
}
