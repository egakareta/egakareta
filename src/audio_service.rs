//! Audio service for loading and processing music waveforms.
//!
//! Provides functionality to asynchronously load audio files, decode them into
//! waveform data for visualization, and cache the raw bytes for playback.

use std::sync::mpsc::Sender;

use crate::platform::task::spawn_background;

const WAVEFORM_WINDOW: usize = 256;

pub type WaveformData = (Vec<f32>, u32);
pub type WaveformResult = (String, Option<WaveformData>, Option<Vec<u8>>);

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
            #[cfg(not(target_arch = "wasm32"))]
            {
                let audio_path = format!("assets/levels/{}/{}", level_name, source_for_load);
                std::fs::read(&audio_path).ok()
            }
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsCast as _;
                use wasm_bindgen_futures::JsFuture;

                let audio_path = format!("assets/levels/{}/{}", level_name, source_for_load);
                let window = web_sys::window();
                if let Some(window) = window {
                    if let Ok(response_value) =
                        JsFuture::from(window.fetch_with_str(&audio_path)).await
                    {
                        if let Ok(response) = response_value.dyn_into::<web_sys::Response>() {
                            if response.ok() {
                                if let Some(array_buffer_promise) = response.array_buffer().ok() {
                                    if let Ok(array_buffer) =
                                        JsFuture::from(array_buffer_promise).await
                                    {
                                        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                                        Some(uint8_array.to_vec())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
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
