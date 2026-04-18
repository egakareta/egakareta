/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{PerfFrameContributor, PerfFrameSnapshot, PerfFrameStageEntry, State};
use crate::types::{
    AppSettings, EditorMode, LevelObject, MusicMetadata, SettingsSection, SpawnDirection,
    TimedTrigger, TimingPoint,
};

pub(crate) struct EditorUiViewModel<'a> {
    pub(crate) mode: EditorMode,
    pub(crate) last_mode: Option<EditorMode>,
    pub(crate) available_levels: &'a [String],
    pub(crate) level_name: Option<&'a str>,
    pub(crate) show_metadata: bool,
    pub(crate) show_import: bool,
    pub(crate) show_settings: bool,
    pub(crate) settings_section: SettingsSection,
    pub(crate) keybind_capture_action: Option<&'a (String, usize)>,
    pub(crate) import_text: &'a str,
    pub(crate) music_metadata: &'a MusicMetadata,
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
    pub(crate) selected_block: Option<LevelObject>,
    pub(crate) playing: bool,
    pub(crate) timeline_time_seconds: f32,
    pub(crate) timeline_duration_seconds: f32,
    pub(crate) tap_times: &'a [f32],
    pub(crate) timeline_preview_position: [f32; 3],
    pub(crate) timeline_preview_direction: SpawnDirection,
    pub(crate) timing_points: &'a [TimingPoint],
    pub(crate) playback_speed: f32,
    pub(crate) timing_selected_index: Option<usize>,
    pub(crate) waveform_zoom: f32,
    pub(crate) waveform_scroll: f32,
    pub(crate) waveform_samples: &'a [f32],
    pub(crate) waveform_sample_rate: u32,
    pub(crate) bpm_tap_result: Option<f32>,
    pub(crate) triggers: &'a [TimedTrigger],
    pub(crate) trigger_selected_index: Option<usize>,
    pub(crate) simulate_trigger_hitboxes: bool,
    pub(crate) camera_position: [f32; 3],
    pub(crate) camera_preview_position: [f32; 3],
    pub(crate) camera_preview_target: [f32; 3],
    pub(crate) camera_rotation: f32,
    pub(crate) camera_pitch: f32,
    pub(crate) fps: f32,
    pub(crate) graphics_backend: String,
    pub(crate) audio_backend: String,
    pub(crate) perf_overlay_enabled: bool,
    pub(crate) perf_spike_count: u64,
    pub(crate) perf_last_spike_stage: &'static str,
    pub(crate) perf_paused: bool,
    pub(crate) perf_selected_history_index: Option<usize>,
    pub(crate) perf_frame_history: Vec<PerfFrameSnapshot>,
    pub(crate) perf_selected_frame: Option<PerfFrameSnapshot>,
    pub(crate) perf_selected_top_contributors: Vec<PerfFrameContributor>,
    pub(crate) perf_selected_stage_tree: Vec<PerfFrameStageEntry>,
    pub(crate) marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
}

impl State {
    pub(crate) fn editor_ui_view_model(&self) -> EditorUiViewModel<'_> {
        let (timeline_preview_position, timeline_preview_direction) =
            self.editor_timeline_preview();
        let perf_overlay_enabled = self.editor_perf_overlay_enabled();
        let perf_frame_history = if perf_overlay_enabled {
            self.editor_perf_frame_history()
        } else {
            Vec::new()
        };
        let perf_selected_frame = if perf_overlay_enabled {
            self.editor_perf_selected_frame()
        } else {
            None
        };
        let perf_selected_top_contributors = if perf_overlay_enabled {
            self.editor_perf_selected_top_contributors(6)
        } else {
            Vec::new()
        };
        let perf_selected_stage_tree = if perf_overlay_enabled {
            self.editor_perf_selected_stage_tree()
        } else {
            Vec::new()
        };

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
            show_import: self.editor_show_import(),
            show_settings: self.editor_show_settings(),
            settings_section: self.editor_settings_section(),
            keybind_capture_action: self.editor_keybind_capture_action(),
            import_text: self.editor_import_text(),
            music_metadata: self.editor_music_metadata(),
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
            selected_block: self.editor_selected_block(),
            playing: self.editor_is_playing(),
            timeline_time_seconds: self.editor_timeline_time_seconds(),
            timeline_duration_seconds: self.editor_timeline_duration_seconds(),
            tap_times: self.editor_tap_times(),
            timeline_preview_position,
            timeline_preview_direction,
            timing_points: self.editor_timing_points(),
            playback_speed: self.editor_playback_speed(),
            timing_selected_index: self.editor_timing_selected_index(),
            waveform_zoom: self.editor_waveform_zoom(),
            waveform_scroll: self.editor_waveform_scroll(),
            waveform_samples: self.editor_waveform_samples(),
            waveform_sample_rate: self.editor_waveform_sample_rate(),
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
            graphics_backend: format!("{:?}", self.render.gpu.adapter_info.backend),
            audio_backend: self.audio.state.runtime.backend_name(),
            perf_overlay_enabled,
            perf_spike_count: self.editor_perf_spike_count(),
            perf_last_spike_stage: self.editor_perf_last_spike_stage_name(),
            perf_paused: self.editor_perf_paused(),
            perf_selected_history_index: self.editor_perf_selected_history_index(),
            perf_frame_history,
            perf_selected_frame,
            perf_selected_top_contributors,
            perf_selected_stage_tree,
            marquee_selection_rect_screen: self.editor_marquee_selection_rect_screen(),
        }
    }
}
