/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

const WAVEFORM_CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistentWaveformCacheEntry {
    pub version: u32,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub window_size: usize,
}

impl PersistentWaveformCacheEntry {
    pub fn new(samples: Vec<f32>, sample_rate: u32, window_size: usize) -> Self {
        Self {
            version: WAVEFORM_CACHE_VERSION,
            samples,
            sample_rate,
            window_size,
        }
    }

    pub fn is_compatible(&self, window_size: usize) -> bool {
        self.version == WAVEFORM_CACHE_VERSION && self.window_size == window_size
    }
}
