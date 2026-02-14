pub(crate) struct PlatformAudio {
    #[cfg(target_arch = "wasm32")]
    current_audio: Option<web_sys::HtmlAudioElement>,
    #[cfg(not(target_arch = "wasm32"))]
    _output_stream: Option<rodio::OutputStream>,
    #[cfg(not(target_arch = "wasm32"))]
    output_handle: Option<rodio::OutputStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    current_sink: Option<rodio::Sink>,
    #[cfg(not(target_arch = "wasm32"))]
    playback_started_at: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    playback_start_offset_seconds: f32,
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
                playback_started_at: None,
                playback_start_offset_seconds: 0.0,
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = self.current_audio.take() {
            let _ = audio.pause();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(sink) = self.current_sink.take() {
                sink.stop();
            }
            self.playback_started_at = None;
            self.playback_start_offset_seconds = 0.0;
        }
    }

    pub(crate) fn start_with_bytes_at(
        &mut self,
        _music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        #[cfg(target_arch = "wasm32")]
        {
            let uint8_array = unsafe { js_sys::Uint8Array::view(bytes) };
            let blob =
                web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&uint8_array.into()))
                    .unwrap();
            let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&url) {
                if start_seconds > 0.0 {
                    let _ = audio.set_current_time(start_seconds as f64);
                }
                let _ = audio.play();
                self.current_audio = Some(audio);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use rodio::Source as _;

            if let Some(handle) = &self.output_handle {
                match rodio::Decoder::new(std::io::Cursor::new(bytes.to_vec())) {
                    Ok(source) => match rodio::Sink::try_new(handle) {
                        Ok(sink) => {
                            sink.append(
                                source.skip_duration(std::time::Duration::from_secs_f32(
                                    start_seconds,
                                )),
                            );
                            sink.play();
                            self.current_sink = Some(sink);
                            self.playback_started_at = Some(std::time::Instant::now());
                            self.playback_start_offset_seconds = start_seconds;
                        }
                        Err(err) => {
                            log::warn!("Failed to create audio sink for imported audio: {}", err);
                        }
                    },
                    Err(err) => {
                        log::warn!(
                            "Failed to decode imported level music '{}': {}",
                            _music_source,
                            err
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn start_at(&mut self, level_name: &str, music_source: &str, start_seconds: f32) {
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        #[cfg(target_arch = "wasm32")]
        {
            let audio_url = format!("assets/levels/{}/{}", level_name, music_source);
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
                if start_seconds > 0.0 {
                    let _ = audio.set_current_time(start_seconds as f64);
                }
                let _ = audio.play();
                self.current_audio = Some(audio);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use rodio::Source as _;

            if let Some(handle) = &self.output_handle {
                let audio_path = format!("assets/levels/{}/{}", level_name, music_source);

                match std::fs::read(&audio_path) {
                    Ok(audio_bytes) => {
                        match rodio::Decoder::new(std::io::Cursor::new(audio_bytes)) {
                            Ok(source) => match rodio::Sink::try_new(handle) {
                                Ok(sink) => {
                                    sink.append(source.skip_duration(
                                        std::time::Duration::from_secs_f32(start_seconds),
                                    ));
                                    sink.play();
                                    self.current_sink = Some(sink);
                                    self.playback_started_at = Some(std::time::Instant::now());
                                    self.playback_start_offset_seconds = start_seconds;
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

    pub(crate) fn playback_time_seconds(&self) -> Option<f32> {
        #[cfg(target_arch = "wasm32")]
        {
            self.current_audio
                .as_ref()
                .map(|audio| audio.current_time() as f32)
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let sink = self.current_sink.as_ref()?;
            if sink.empty() {
                return None;
            }

            let started_at = self.playback_started_at?;
            Some(self.playback_start_offset_seconds + started_at.elapsed().as_secs_f32())
        }
    }

    pub(crate) fn is_playing(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(audio) = &self.current_audio {
                !audio.paused() && !audio.ended()
            } else {
                false
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.current_sink
                .as_ref()
                .map(|sink| !sink.empty())
                .unwrap_or(false)
        }
    }
}
