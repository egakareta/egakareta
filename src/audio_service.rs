/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Audio service for loading and processing music waveforms.
//!
//! Provides functionality to asynchronously load audio files, decode them into
//! waveform data for visualization, and cache the raw bytes for playback.

use std::sync::mpsc::Sender;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::platform::parallel::spawn_cpu_bound;
use crate::platform::task::spawn_background;

pub type AudioPreloadResult = (String, Option<Vec<u8>>);
pub(crate) const WAVEFORM_WINDOW: usize = 256;
const WAVEFORM_CHUNK_PEAKS: usize = 2048;
const WAVEFORM_CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PersistentWaveformCacheEntry {
    pub(crate) version: u32,
    pub(crate) samples: Vec<f32>,
    pub(crate) sample_rate: u32,
    pub(crate) window_size: usize,
}

impl PersistentWaveformCacheEntry {
    pub(crate) fn new(samples: Vec<f32>, sample_rate: u32, window_size: usize) -> Self {
        Self {
            version: WAVEFORM_CACHE_VERSION,
            samples,
            sample_rate,
            window_size,
        }
    }

    pub(crate) fn is_compatible(&self, window_size: usize) -> bool {
        self.version == WAVEFORM_CACHE_VERSION && self.window_size == window_size
    }
}

#[derive(Debug)]
pub(crate) enum WaveformLoadMessage {
    Started {
        window_size: usize,
    },
    Chunk {
        start_peak: usize,
        peaks: Vec<f32>,
        sample_rate: u32,
        window_size: usize,
    },
    Cached {
        samples: Vec<f32>,
        sample_rate: u32,
        window_size: usize,
        bytes: Option<Vec<u8>>,
    },
    Finished {
        sample_rate: u32,
        window_size: usize,
        total_peaks: usize,
        cache_key: Option<String>,
        bytes: Option<Vec<u8>>,
    },
    Failed {
        bytes: Option<Vec<u8>>,
    },
}

pub type WaveformResult = (String, String, WaveformLoadMessage);

pub(crate) fn waveform_cache_key(bytes: &[u8], window_size: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let digest_hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("waveform-v{WAVEFORM_CACHE_VERSION}-window-{window_size}-sha256-{digest_hex}")
}

async fn load_level_audio_bytes(level_name: &str, music_source: &str) -> Option<Vec<u8>> {
    crate::level_repository::get_builtin_audio(level_name, music_source).map(|bytes| bytes.to_vec())
}

pub fn start_audio_preload(
    source_key: String,
    level_name: String,
    music_source: String,
    sender: Sender<AudioPreloadResult>,
) {
    spawn_background(async move {
        let bytes = load_level_audio_bytes(&level_name, &music_source).await;
        let _ = sender.send((source_key, bytes));
    });
}

/// Starts asynchronous loading of waveform data for a music track.
/// Spawns a thread to decode the audio file.
///
/// # Arguments
/// * `music_source` - The filename of the music file
/// * `level_name` - The level directory name containing the music file
/// * `cached_bytes` - Optional pre-loaded audio bytes to avoid re-reading
/// * `sender` - Channel sender for the result
pub fn start_waveform_loading(
    music_source: String,
    level_name: String,
    cached_bytes: Option<Vec<u8>>,
    sender: Sender<WaveformResult>,
) {
    let source_for_send = music_source.clone();
    let level_for_send = level_name.clone();
    let source_for_load = music_source.clone();

    spawn_background(async move {
        let bytes = if let Some(bytes) = cached_bytes {
            Some(bytes)
        } else {
            load_level_audio_bytes(&level_name, &source_for_load).await
        };

        let Some(bytes) = bytes else {
            let _ = sender.send((
                source_for_send,
                level_for_send,
                WaveformLoadMessage::Failed { bytes: None },
            ));
            return;
        };

        let cache_key = waveform_cache_key(&bytes, WAVEFORM_WINDOW);
        if let Some(cached) = crate::platform::io::load_waveform_from_storage(&cache_key).await {
            if cached.is_compatible(WAVEFORM_WINDOW) {
                let _ = sender.send((
                    source_for_send,
                    level_for_send,
                    WaveformLoadMessage::Cached {
                        samples: cached.samples,
                        sample_rate: cached.sample_rate,
                        window_size: cached.window_size,
                        bytes: Some(bytes),
                    },
                ));
                return;
            }
        }

        spawn_cpu_bound(move || {
            let _ = sender.send((
                source_for_send.clone(),
                level_for_send.clone(),
                WaveformLoadMessage::Started {
                    window_size: WAVEFORM_WINDOW,
                },
            ));

            let decoded = crate::platform::audio::decode_audio_to_waveform_streaming(
                &bytes,
                WAVEFORM_WINDOW,
                WAVEFORM_CHUNK_PEAKS,
                |start_peak, peaks, sample_rate| {
                    let _ = sender.send((
                        source_for_send.clone(),
                        level_for_send.clone(),
                        WaveformLoadMessage::Chunk {
                            start_peak,
                            peaks,
                            sample_rate,
                            window_size: WAVEFORM_WINDOW,
                        },
                    ));
                },
            );

            if let Some(summary) = decoded {
                let _ = sender.send((
                    source_for_send,
                    level_for_send,
                    WaveformLoadMessage::Finished {
                        sample_rate: summary.sample_rate,
                        window_size: WAVEFORM_WINDOW,
                        total_peaks: summary.peak_count,
                        cache_key: Some(cache_key),
                        bytes: Some(bytes),
                    },
                ));
            } else {
                let _ = sender.send((
                    source_for_send,
                    level_for_send,
                    WaveformLoadMessage::Failed { bytes: Some(bytes) },
                ));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::waveform_cache_key;

    #[test]
    fn waveform_cache_key_tracks_audio_content_and_window_size() {
        let first = waveform_cache_key(b"audio-one", 256);
        let same = waveform_cache_key(b"audio-one", 256);
        let different_bytes = waveform_cache_key(b"audio-two", 256);
        let different_window = waveform_cache_key(b"audio-one", 512);

        assert_eq!(first, same);
        assert_ne!(first, different_bytes);
        assert_ne!(first, different_window);
        assert!(first.starts_with("waveform-v1-window-256-sha256-"));
    }
}
