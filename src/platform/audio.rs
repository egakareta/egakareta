pub(crate) struct PlatformAudio {
    #[cfg(target_arch = "wasm32")]
    current_audio: Option<web_sys::HtmlAudioElement>,
    #[cfg(not(target_arch = "wasm32"))]
    _output_stream: Option<rodio::OutputStream>,
    #[cfg(not(target_arch = "wasm32"))]
    output_handle: Option<rodio::OutputStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    current_sink: Option<rodio::Sink>,
}

impl PlatformAudio {
    pub(crate) fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                current_audio: None,
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let (output_stream, output_handle) = match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => (Some(stream), Some(handle)),
                Err(err) => {
                    log::warn!("Failed to initialize native audio output: {}", err);
                    (None, None)
                }
            };

            Self {
                _output_stream: output_stream,
                output_handle,
                current_sink: None,
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = self.current_audio.take() {
            let _ = audio.pause();
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }
    }

    pub(crate) fn start(&mut self, level_name: &str, music_source: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let audio_url = format!("assets/levels/{}/{}", level_name, music_source);
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
                let _ = audio.play();
                self.current_audio = Some(audio);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(handle) = &self.output_handle {
                let audio_path = format!("assets/levels/{}/{}", level_name, music_source);

                match std::fs::read(&audio_path) {
                    Ok(audio_bytes) => {
                        match rodio::Decoder::new(std::io::Cursor::new(audio_bytes)) {
                            Ok(source) => match rodio::Sink::try_new(handle) {
                                Ok(sink) => {
                                    sink.append(source);
                                    sink.play();
                                    self.current_sink = Some(sink);
                                }
                                Err(err) => {
                                    log::warn!(
                                        "Failed to create audio sink for '{}': {}",
                                        audio_path,
                                        err
                                    );
                                }
                            },
                            Err(err) => {
                                log::warn!(
                                    "Failed to decode level music '{}': {}",
                                    audio_path,
                                    err
                                );
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to read level music '{}': {}", audio_path, err);
                    }
                }
            }
        }
    }
}
