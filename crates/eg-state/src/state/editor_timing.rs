/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::TimingPoint;

#[derive(Default)]
pub struct EditorTimingState {
    pub timing_points: Vec<TimingPoint>,
    pub revision: u64,
    pub playback_speed: f32,
    pub waveform_samples: Vec<f32>,
    pub waveform_sample_rate: u32,
    pub waveform_window_size: usize,
    pub waveform_loading: bool,
    pub waveform_complete: bool,
    pub timing_selected_index: Option<usize>,
    pub waveform_zoom: f32,
    pub waveform_scroll: f32,
    pub bpm_tap_times: Vec<f64>,
    pub bpm_tap_result: Option<f32>,
}

impl EditorTimingState {
    pub fn new() -> Self {
        Self {
            revision: 1,
            playback_speed: 1.0,
            waveform_zoom: 1.0,
            waveform_window_size: eg_audio::service::WAVEFORM_WINDOW,
            ..Default::default()
        }
    }

    pub fn mark_timing_points_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
