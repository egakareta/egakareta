//! Audio service for loading and processing music waveforms.
//!
//! Provides functionality to asynchronously load audio files, decode them into
//! waveform data for visualization, and cache the raw bytes for playback.

use std::sync::mpsc::Sender;

use crate::platform::task::spawn_background;

pub type AudioPreloadResult = (String, Option<Vec<u8>>);
const WAVEFORM_WINDOW: usize = 256;

pub type WaveformData = (Vec<f32>, u32);
pub type WaveformResult = (String, Option<WaveformData>, Option<Vec<u8>>);

async fn load_level_audio_bytes(level_name: &str, music_source: &str) -> Option<Vec<u8>> {
    let audio_path = format!("assets/levels/{}/{}", level_name, music_source);

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(&audio_path).ok()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let response = gloo_net::http::Request::get(&audio_path)
            .send()
            .await
            .ok()?;
        if !response.ok() {
            return None;
        }

        response.binary().await.ok()
    }
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
///
/// On native platforms, spawns a thread to decode the audio file.
/// On WASM, uses fetch API to load and decode the audio.
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
    let source_for_load = music_source.clone();

    spawn_background(async move {
        let bytes = if let Some(bytes) = cached_bytes {
            Some(bytes)
        } else {
            load_level_audio_bytes(&level_name, &source_for_load).await
        };

        let decoded = if let Some(ref bytes) = bytes {
            #[cfg(not(target_arch = "wasm32"))]
            {
                crate::platform::audio::decode_audio_to_waveform(bytes, WAVEFORM_WINDOW)
            }
            #[cfg(target_arch = "wasm32")]
            {
                crate::platform::audio::decode_audio_to_waveform_async(bytes, WAVEFORM_WINDOW).await
            }
        } else {
            None
        };

        let _ = sender.send((source_for_send, decoded, bytes));
    });
}
