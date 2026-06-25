/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_audio::service::WAVEFORM_WINDOW;
use eg_types::TimingPoint;
use smart_default::SmartDefault;

#[derive(SmartDefault)]
pub struct EditorTimingState {
    pub timing_points: Vec<TimingPoint>,
    #[default = 1]
    pub revision: u64,
    #[default = 1.0]
    pub playback_speed: f32,
    pub waveform_samples: Vec<f32>,
    pub waveform_sample_rate: u32,
    #[default(WAVEFORM_WINDOW)]
    pub waveform_window_size: usize,
    pub waveform_loading: bool,
    pub waveform_complete: bool,
    pub timing_selected_index: Option<usize>,
    #[default = 1.0]
    pub waveform_zoom: f32,
    pub waveform_scroll: f32,
    pub bpm_tap_times: Vec<f64>,
    pub bpm_tap_result: Option<f32>,
}

impl EditorTimingState {
    pub fn mark_timing_points_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
