/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Audio service for loading and processing music waveforms.
//!
//! Provides functionality to asynchronously load audio files, decode them into
//! waveform data for visualization, and cache the raw bytes for playback.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::audio::runtime_asset_source_key;

pub const WAVEFORM_WINDOW: usize = 256;
const WAVEFORM_CHUNK_PEAKS: usize = 2048;
const WAVEFORM_CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct WaveformCacheEntry {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub window_size: usize,
    pub complete: bool,
}

impl Default for WaveformCacheEntry {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            sample_rate: 0,
            window_size: WAVEFORM_WINDOW,
            complete: false,
        }
    }
}

#[derive(Default)]
pub struct AudioByteCache {
    bytes_by_source: HashMap<String, Vec<u8>>,
}

impl AudioByteCache {
    pub fn insert(&mut self, source_key: String, bytes: Vec<u8>) {
        self.bytes_by_source.insert(source_key, bytes);
    }

    pub fn get(&self, source_key: &str) -> Option<&Vec<u8>> {
        self.bytes_by_source.get(source_key)
    }

    pub fn contains_key(&self, source_key: &str) -> bool {
        self.bytes_by_source.contains_key(source_key)
    }

    pub fn remove(&mut self, source_key: &str) -> Option<Vec<u8>> {
        self.bytes_by_source.remove(source_key)
    }
}

#[derive(Default)]
pub struct EditorAudioCache {
    source_bytes: AudioByteCache,
    local_file_bytes: HashMap<String, Vec<u8>>,
    waveform_entries: HashMap<String, WaveformCacheEntry>,
    loading_source: Option<String>,
}

impl EditorAudioCache {
    pub fn from_local_files(local_file_bytes: HashMap<String, Vec<u8>>) -> Self {
        Self {
            local_file_bytes,
            ..Self::default()
        }
    }

    /// Replace only the persisted local-file fallback cache, retaining
    /// in-memory level bytes and decoded waveform entries.
    pub fn replace_local_files(&mut self, local_file_bytes: HashMap<String, Vec<u8>>) {
        self.local_file_bytes = local_file_bytes;
    }

    pub fn insert_level_audio(&mut self, level_name: &str, bytes: Vec<u8>) -> String {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");
        self.source_bytes.insert(source_key.clone(), bytes);
        source_key
    }

    pub fn insert_source_bytes(&mut self, source_key: String, bytes: Vec<u8>) {
        self.source_bytes.insert(source_key, bytes);
    }

    pub fn bytes_for_level_audio(&self, level_name: &str) -> Option<&Vec<u8>> {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");
        self.source_bytes
            .get(&source_key)
            .or_else(|| self.local_file_bytes.get("audio.mp3"))
    }

    pub fn bytes_for_source(&self, source_key: &str) -> Option<&Vec<u8>> {
        self.source_bytes.get(source_key)
    }

    pub fn contains_source_bytes(&self, source_key: &str) -> bool {
        self.source_bytes.contains_key(source_key)
    }

    pub fn waveform(&self, source_key: &str) -> Option<&WaveformCacheEntry> {
        self.waveform_entries.get(source_key)
    }

    pub fn waveform_mut_or_default(&mut self, source_key: String) -> &mut WaveformCacheEntry {
        self.waveform_entries.entry(source_key).or_default()
    }

    pub fn insert_waveform(&mut self, source_key: String, entry: WaveformCacheEntry) {
        self.waveform_entries.insert(source_key, entry);
    }

    pub fn remove_waveform(&mut self, source_key: &str) {
        self.waveform_entries.remove(source_key);
    }

    pub fn contains_waveform(&self, source_key: &str) -> bool {
        self.waveform_entries.contains_key(source_key)
    }

    pub fn loading_source(&self) -> Option<&str> {
        self.loading_source.as_deref()
    }

    pub fn set_loading_source(&mut self, source_key: Option<String>) {
        self.loading_source = source_key;
    }

    pub fn clear_loading_source_if(&mut self, source_key: &str) {
        if self.loading_source.as_deref() == Some(source_key) {
            self.loading_source = None;
        }
    }
}

#[derive(Debug)]
pub enum WaveformLoadMessage {
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

pub fn waveform_cache_key(bytes: &[u8], window_size: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let digest_hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("waveform-v{WAVEFORM_CACHE_VERSION}-window-{window_size}-sha256-{digest_hex}")
}

async fn load_level_audio_bytes(level_name: &str) -> Option<Vec<u8>> {
    eg_level::get_builtin_audio(level_name).map(|bytes| bytes.to_vec())
}

/// Load the built-in audio bytes for a runtime preload request.
///
/// The caller owns the returned future through a Bevy task component. Keeping
/// this service as a result-returning future removes the previous detached
/// sender-based completion path.
pub async fn load_audio_preload(level_name: String) -> Option<Vec<u8>> {
    load_level_audio_bytes(&level_name).await
}

fn decode_waveform(
    source: String,
    level_name: String,
    bytes: Vec<u8>,
    cache_key: String,
) -> Vec<WaveformResult> {
    let mut results = vec![(
        source.clone(),
        level_name.clone(),
        WaveformLoadMessage::Started {
            window_size: WAVEFORM_WINDOW,
        },
    )];

    let decoded = crate::audio::decode_audio_to_waveform_streaming(
        &bytes,
        WAVEFORM_WINDOW,
        WAVEFORM_CHUNK_PEAKS,
        |start_peak, peaks, sample_rate| {
            results.push((
                source.clone(),
                level_name.clone(),
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
        results.push((
            source,
            level_name,
            WaveformLoadMessage::Finished {
                sample_rate: summary.sample_rate,
                window_size: WAVEFORM_WINDOW,
                total_peaks: summary.peak_count,
                cache_key: Some(cache_key),
                bytes: Some(bytes),
            },
        ));
    } else {
        results.push((
            source,
            level_name,
            WaveformLoadMessage::Failed { bytes: Some(bytes) },
        ));
    }
    results
}

/// Load and decode a waveform, returning ordered progress results owned by
/// the caller's Bevy task. The App world applies this stream with a frame
/// budget instead of receiving it from a detached channel producer.
pub async fn load_waveform(
    level_name: String,
    cached_bytes: Option<Vec<u8>>,
) -> Vec<WaveformResult> {
    let source = "audio.mp3".to_string();
    let bytes = if let Some(bytes) = cached_bytes {
        Some(bytes)
    } else {
        load_level_audio_bytes(&level_name).await
    };

    let Some(bytes) = bytes else {
        return vec![(
            source,
            level_name,
            WaveformLoadMessage::Failed { bytes: None },
        )];
    };

    let cache_key = waveform_cache_key(&bytes, WAVEFORM_WINDOW);
    if let Some(cached) = eg_platform::io::load_waveform_from_storage(&cache_key).await
        && cached.is_compatible(WAVEFORM_WINDOW)
    {
        return vec![(
            source,
            level_name,
            WaveformLoadMessage::Cached {
                samples: cached.samples,
                sample_rate: cached.sample_rate,
                window_size: cached.window_size,
                bytes: Some(bytes),
            },
        )];
    }

    decode_waveform(source, level_name, bytes, cache_key)
}

#[cfg(test)]
mod tests {
    use super::{WaveformLoadMessage, decode_waveform, waveform_cache_key};

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

    #[test]
    fn waveform_decode_task_returns_a_terminal_message() {
        let results = decode_waveform(
            "audio.mp3".to_string(),
            "Regression".to_string(),
            crate::audio::tiny_wav_bytes(),
            "test-cache-key".to_string(),
        );
        assert!(matches!(
            results.first().expect("decoder emits a start message").2,
            WaveformLoadMessage::Started { .. }
        ));
        assert!(matches!(
            results.last().expect("decoder emits a terminal result").2,
            WaveformLoadMessage::Finished { .. }
        ));
    }
}
