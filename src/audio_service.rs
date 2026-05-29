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

use crate::platform::parallel::spawn_cpu_bound;
use crate::platform::task::spawn_background;

pub type AudioPreloadResult = (String, Option<Vec<u8>>);
pub(crate) const WAVEFORM_WINDOW: usize = 256;
const WAVEFORM_CHUNK_PEAKS: usize = 2048;

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
    Finished {
        sample_rate: u32,
        window_size: usize,
        total_peaks: usize,
        bytes: Option<Vec<u8>>,
    },
    Failed {
        bytes: Option<Vec<u8>>,
    },
}

pub type WaveformResult = (String, String, WaveformLoadMessage);

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
