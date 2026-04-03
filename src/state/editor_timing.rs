/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::TimingPoint;

#[derive(Default)]
pub(crate) struct EditorTimingState {
    pub(crate) timing_points: Vec<TimingPoint>,
    pub(crate) playback_speed: f32,
    pub(crate) waveform_samples: Vec<f32>,
    pub(crate) waveform_sample_rate: u32,
    pub(crate) timing_selected_index: Option<usize>,
    pub(crate) waveform_zoom: f32,
    pub(crate) waveform_scroll: f32,
    pub(crate) bpm_tap_times: Vec<f64>,
    pub(crate) bpm_tap_result: Option<f32>,
}

impl EditorTimingState {
    pub(crate) fn new() -> Self {
        Self {
            playback_speed: 1.0,
            waveform_zoom: 1.0,
            ..Default::default()
        }
    }
}
