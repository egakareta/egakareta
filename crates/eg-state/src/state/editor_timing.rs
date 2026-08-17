/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_audio::service::WAVEFORM_WINDOW;
use eg_types::TimingPoint;
use smart_default::SmartDefault;

const BPM_TAP_HISTORY_LIMIT: usize = 16;

#[derive(SmartDefault, Clone)]
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

    pub fn record_bpm_tap_at(&mut self, now_seconds: f64) {
        self.bpm_tap_times.push(now_seconds);
        if self.bpm_tap_times.len() > 1 {
            let mut diffs = Vec::with_capacity(self.bpm_tap_times.len() - 1);
            for index in 1..self.bpm_tap_times.len() {
                diffs.push(self.bpm_tap_times[index] - self.bpm_tap_times[index - 1]);
            }
            let average_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
            self.bpm_tap_result = Some((60.0 / average_diff) as f32);
        }
        if self.bpm_tap_times.len() > BPM_TAP_HISTORY_LIMIT {
            let excess = self.bpm_tap_times.len() - BPM_TAP_HISTORY_LIMIT;
            self.bpm_tap_times.drain(0..excess);
        }
    }

    pub fn reset_bpm_tap(&mut self) {
        self.bpm_tap_times.clear();
        self.bpm_tap_result = None;
    }
}
