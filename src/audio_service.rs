use std::sync::mpsc::Sender;

const WAVEFORM_WINDOW: usize = 256;

pub type WaveformData = (Vec<f32>, u32);
pub type WaveformResult = (String, Option<WaveformData>);

pub fn start_waveform_loading(
    music_source: String,
    level_name: String,
    cached_bytes: Option<Vec<u8>>,
    sender: Sender<WaveformResult>,
) {
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    {
        use crate::platform::audio::decode_audio_to_waveform;
        let source_for_thread = music_source.clone();

        std::thread::spawn(move || {
            let bytes = cached_bytes.or_else(|| {
                let audio_path = format!("assets/levels/{}/{}", level_name, source_for_thread);
                std::fs::read(&audio_path).ok()
            });

            let decoded = if let Some(bytes) = bytes {
                decode_audio_to_waveform(bytes, WAVEFORM_WINDOW)
            } else {
                None
            };

            let _ = sender.send((source_for_thread, decoded));
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), test))]
    {
        use crate::platform::audio::decode_audio_to_waveform;

        let bytes = cached_bytes.or_else(|| {
            let audio_path = format!("assets/levels/{}/{}", level_name, music_source);
            std::fs::read(&audio_path).ok()
        });

        let decoded = if let Some(bytes) = bytes {
            decode_audio_to_waveform(bytes, WAVEFORM_WINDOW)
        } else {
            None
        };

        let _ = sender.send((music_source, decoded));
    }

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast as _;
        use wasm_bindgen_futures::{spawn_local, JsFuture};

        let source_for_fetch = music_source.clone();
        let source_for_send = music_source.clone();

        spawn_local(async move {
            let bytes = if let Some(bytes) = cached_bytes {
                Some(bytes)
            } else {
                let audio_path = format!("assets/levels/{}/{}", level_name, source_for_fetch);
                let fetched = async {
                    let window = web_sys::window()?;
                    let response_value = JsFuture::from(window.fetch_with_str(&audio_path))
                        .await
                        .ok()?;
                    let response: web_sys::Response = response_value.dyn_into().ok()?;
                    if !response.ok() {
                        return None;
                    }
                    let array_buffer = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
                    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                    Some(uint8_array.to_vec())
                }
                .await;

                fetched
            };

            let decoded = if let Some(bytes) = bytes {
                crate::platform::audio::decode_audio_to_waveform_async(&bytes, WAVEFORM_WINDOW)
                    .await
            } else {
                None
            };

            let _ = sender.send((source_for_send, decoded));
        });
    }
}
