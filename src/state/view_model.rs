/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use crate::triggers::TimedTrigger;
use crate::types::{
    AppSettings, EditorMode, LevelCreatorMetadata, LevelObject, MusicMetadata, SettingsSection,
    SpawnDirection, TimingPoint,
};

#[derive(Clone, Copy)]
pub(crate) struct SelectedTapViewModel {
    pub(crate) index: usize,
    pub(crate) time_seconds: f32,
    pub(crate) position: [f32; 3],
}

pub(crate) struct EditorUiViewModel<'a> {
    pub(crate) mode: EditorMode,
    pub(crate) last_mode: Option<EditorMode>,
    pub(crate) available_levels: &'a [String],
    pub(crate) level_name: Option<&'a str>,
    pub(crate) show_metadata: bool,
    pub(crate) show_place_window: bool,
    pub(crate) show_settings: bool,
    pub(crate) settings_section: SettingsSection,
    pub(crate) keybind_capture_action: Option<&'a (String, usize)>,
    pub(crate) music_metadata: &'a MusicMetadata,
    pub(crate) creator_metadata: LevelCreatorMetadata,
    pub(crate) sky_color: [f32; 3],
    pub(crate) app_settings: &'a AppSettings,
    pub(crate) configured_graphics_backend: &'a str,
    pub(crate) configured_audio_backend: &'a str,
    pub(crate) graphics_backend_options: &'a [String],
    pub(crate) audio_backend_options: &'a [String],
    pub(crate) settings_restart_required: bool,
    pub(crate) snap_to_grid: bool,
    pub(crate) snap_step: f32,
    pub(crate) snap_rotation: bool,
    pub(crate) snap_rotation_step_degrees: f32,
    pub(crate) selected_block_id: &'a str,
    pub(crate) place_preview_position: [f32; 3],
    pub(crate) place_preview_size: [f32; 3],
    pub(crate) recent_block_ids: &'a [String],
    pub(crate) selected_block: Option<LevelObject>,
    pub(crate) selected_block_count: usize,
    pub(crate) transform_trigger_capture_active: bool,
    pub(crate) clipboard_block_count: usize,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) playing: bool,
    pub(crate) timeline_time_seconds: f32,
    pub(crate) timeline_duration_seconds: f32,
    pub(crate) tap_times: &'a [f32],
    pub(crate) selected_tap: Option<SelectedTapViewModel>,
    pub(crate) timeline_preview_position: [f32; 3],
    pub(crate) timeline_preview_direction: SpawnDirection,
    pub(crate) timing_points: &'a [TimingPoint],
    pub(crate) playback_speed: f32,
    pub(crate) timing_selected_index: Option<usize>,
    pub(crate) waveform_zoom: f32,
    pub(crate) waveform_scroll: f32,
    pub(crate) waveform_samples: &'a [f32],
    pub(crate) waveform_sample_rate: u32,
    pub(crate) waveform_window_size: usize,
    pub(crate) waveform_loading: bool,
    pub(crate) waveform_complete: bool,
    pub(crate) bpm_tap_result: Option<f32>,
    pub(crate) triggers: Vec<TimedTrigger>,
    pub(crate) trigger_selected_index: Option<usize>,
    pub(crate) simulate_trigger_hitboxes: bool,
    pub(crate) camera_position: [f32; 3],
    pub(crate) camera_preview_position: [f32; 3],
    pub(crate) camera_preview_target: [f32; 3],
    pub(crate) camera_rotation: f32,
    pub(crate) camera_pitch: f32,
    pub(crate) fps: f32,
    pub(crate) marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
    pub(crate) object_count: usize,
}

impl State {
    pub(crate) fn editor_ui_view_model(&self) -> EditorUiViewModel<'_> {
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

        EditorUiViewModel {
            mode: self.editor_mode(),
            last_mode: self.editor.runtime.interaction.last_mode,
            available_levels: self.available_levels(),
            level_name: self.session.editor_level_name.as_deref(),
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
            graphics_backend_options: self.available_graphics_backends(),
            audio_backend_options: self.available_audio_backends(),
            settings_restart_required: self.settings_restart_required(),
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
            transform_trigger_capture_active: self.editor_transform_trigger_capture_active(),
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
            triggers: self.editor_triggers(),
            trigger_selected_index: self.editor_selected_trigger_index(),
            simulate_trigger_hitboxes: self.editor_simulate_trigger_hitboxes(),
            camera_position,
            camera_preview_position,
            camera_preview_target,
            camera_rotation: self.editor.camera.editor_rotation,
            camera_pitch: self.editor.camera.editor_pitch,
            fps: self.editor_fps(),
            marquee_selection_rect_screen: self.editor_marquee_selection_rect_screen(),
            object_count: self.editor.objects.len(),
        }
    }
}
