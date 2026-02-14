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
    #[cfg(not(target_arch = "wasm32"))]
    playback_speed: f32,
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
                playback_speed: 1.0,
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
            self.playback_speed = 1.0;
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
                            self.playback_speed = 1.0;
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
                                    self.playback_speed = 1.0;
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
            Some(
                self.playback_start_offset_seconds
                    + started_at.elapsed().as_secs_f32() * self.playback_speed,
            )
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

    pub(crate) fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(0.25, 2.0);

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(audio) = &self.current_audio {
                audio.set_playback_rate(speed as f64);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(started_at) = self.playback_started_at {
                let elapsed_real = started_at.elapsed().as_secs_f32();
                self.playback_start_offset_seconds += elapsed_real * self.playback_speed;
                self.playback_started_at = Some(std::time::Instant::now());
            }
            self.playback_speed = speed;
            if let Some(sink) = &self.current_sink {
                sink.set_speed(speed);
            }
        }
    }
}

/// Decode audio bytes to a downsampled waveform suitable for display.
/// Returns (peak_samples, sample_rate) where peak_samples contains one peak per window.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_audio_to_waveform(
    bytes: Vec<u8>,
    window_size: usize,
) -> Option<(Vec<f32>, u32)> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let source = std::io::Cursor::new(bytes);
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let hint = Hint::new();
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;

    let mut format = probed.format;
    let track = format.default_track()?;
    let sample_rate = track.codec_params.sample_rate?;
    let channel_count = track
        .codec_params
        .channels
        .map(|channels| channels.count())
        .unwrap_or(1)
        .max(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .ok()?;

    let mut peaks: Vec<f32> = Vec::new();
    let mut window_peak: f32 = 0.0;
    let mut window_count: usize = 0;
    let mut sample_buffer: Option<SampleBuffer<f32>> = None;
    let channel_scale = 1.0 / channel_count as f32;
    let track_id = track.id;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::ResetRequired) => return None,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(_)) => break,
            Err(_) => return None,
        };

        if sample_buffer
            .as_ref()
            .map(|buf| buf.capacity() < decoded.capacity())
            .unwrap_or(true)
        {
            sample_buffer = Some(SampleBuffer::<f32>::new(
                decoded.capacity() as u64,
                *decoded.spec(),
            ));
        }

        if let Some(buf) = sample_buffer.as_mut() {
            buf.copy_interleaved_ref(decoded);
            for sample in buf.samples() {
                let normalized = sample.abs() * channel_scale;
                window_peak = window_peak.max(normalized);
                window_count += 1;

                if window_count >= window_size {
                    peaks.push(window_peak);
                    window_peak = 0.0;
                    window_count = 0;
                }
            }
        }
    }

    if window_count > 0 {
        peaks.push(window_peak);
    }

    Some((peaks, sample_rate))
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn decode_audio_to_waveform_async(
    bytes: &[u8],
    window_size: usize,
) -> Option<(Vec<f32>, u32)> {
    use wasm_bindgen::JsCast as _;

    let context = web_sys::AudioContext::new().ok()?;
    let uint8_array = js_sys::Uint8Array::from(bytes);
    let array_buffer = uint8_array.buffer();

    let decode_promise = context.decode_audio_data(&array_buffer).ok()?;
    let decoded = wasm_bindgen_futures::JsFuture::from(decode_promise)
        .await
        .ok()?;
    let audio_buffer: web_sys::AudioBuffer = decoded.dyn_into().ok()?;

    let sample_rate = audio_buffer.sample_rate() as u32;
    let channels = audio_buffer.number_of_channels().max(1) as usize;
    let frame_len = audio_buffer.length() as usize;

    let mut channel_data = Vec::with_capacity(channels);
    for channel_index in 0..channels {
        let channel = audio_buffer.get_channel_data(channel_index as u32).ok()?;
        channel_data.push(channel);
    }

    let mut peaks: Vec<f32> = Vec::new();
    let mut window_peak: f32 = 0.0;
    let mut window_count: usize = 0;
    let channel_scale = channels as f32;

    for frame_index in 0..frame_len {
        for channel in &channel_data {
            let normalized = channel[frame_index].abs() / channel_scale;
            window_peak = window_peak.max(normalized);
            window_count += 1;

            if window_count >= window_size {
                peaks.push(window_peak);
                window_peak = 0.0;
                window_count = 0;
            }
        }
    }

    if window_count > 0 {
        peaks.push(window_peak);
    }

    let _ = context.close();

    Some((peaks, sample_rate))
}
