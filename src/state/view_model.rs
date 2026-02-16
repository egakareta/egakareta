use super::{PerfOverlayEntry, State};
use crate::types::{EditorMode, LevelObject, MusicMetadata, SpawnDirection, TimingPoint};

pub(crate) struct EditorUiViewModel<'a> {
    pub(crate) mode: EditorMode,
    pub(crate) available_levels: &'a [String],
    pub(crate) level_name: Option<&'a str>,
    pub(crate) show_metadata: bool,
    pub(crate) show_import: bool,
    pub(crate) import_text: &'a str,
    pub(crate) music_metadata: &'a MusicMetadata,
    pub(crate) snap_to_grid: bool,
    pub(crate) snap_step: f32,
    pub(crate) selected_block_id: &'a str,
    pub(crate) selected_block: Option<LevelObject>,
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
    pub(crate) camera_position: [f32; 3],
    pub(crate) fps: f32,
    pub(crate) perf_overlay_enabled: bool,
    pub(crate) perf_overlay_lines: Vec<String>,
    pub(crate) perf_overlay_entries: Vec<PerfOverlayEntry>,
    pub(crate) marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
}

impl State {
    pub(crate) fn editor_ui_view_model(&self) -> EditorUiViewModel<'_> {
        let (timeline_preview_position, timeline_preview_direction) =
            self.editor_timeline_preview();
        let perf_overlay_enabled = self.editor_perf_overlay_enabled();
        let perf_overlay_lines = if perf_overlay_enabled {
            self.editor_perf_overlay_lines()
        } else {
            Vec::new()
        };
        let perf_overlay_entries = if perf_overlay_enabled {
            self.editor_perf_overlay_entries()
        } else {
            Vec::new()
        };

        let camera_target = glam::Vec3::new(
            self.editor.camera.editor_pan[0],
            self.editor.camera.editor_pan[1],
            0.0,
        );
        let camera_position = (camera_target + self.editor.camera_offset()).to_array();

        EditorUiViewModel {
            mode: self.editor_mode(),
            available_levels: self.available_levels(),
            level_name: self.session.editor_level_name.as_deref(),
            show_metadata: self.editor_show_metadata(),
            show_import: self.editor_show_import(),
            import_text: self.editor_import_text(),
            music_metadata: self.editor_music_metadata(),
            snap_to_grid: self.editor_snap_to_grid(),
            snap_step: self.editor_snap_step(),
            selected_block_id: self.editor_selected_block_id(),
            selected_block: self.editor_selected_block(),
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
            camera_position,
            fps: self.editor_fps(),
            perf_overlay_enabled,
            perf_overlay_lines,
            perf_overlay_entries,
            marquee_selection_rect_screen: self.editor_marquee_selection_rect_screen(),
        }
    }
}
