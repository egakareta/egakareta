pub(crate) struct PlatformAudio {
    #[cfg(target_arch = "wasm32")]
    current_audio: Option<web_sys::HtmlAudioElement>,
    #[cfg(target_arch = "wasm32")]
    current_audio_source: Option<String>,
    #[cfg(target_arch = "wasm32")]
    current_blob_url: Option<String>,
    #[cfg(target_arch = "wasm32")]
    playback_speed: f32,
    #[cfg(not(target_arch = "wasm32"))]
    _output_stream: Option<rodio::OutputStream>,
    #[cfg(not(target_arch = "wasm32"))]
    output_handle: Option<rodio::OutputStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    current_sink: Option<rodio::Sink>,
    #[cfg(not(target_arch = "wasm32"))]
    current_audio_source: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    playback_started_at: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    playback_start_offset_seconds: f32,
    #[cfg(not(target_arch = "wasm32"))]
    playback_speed: f32,
}

fn accumulate_waveform_frame_peak(
    peaks: &mut Vec<f32>,
    window_peak: &mut f32,
    window_count: &mut usize,
    frame_peak: f32,
    window_size: usize,
) {
    *window_peak = (*window_peak).max(frame_peak.abs());
    *window_count += 1;

    if *window_count >= window_size {
        peaks.push(*window_peak);
        *window_peak = 0.0;
        *window_count = 0;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn accumulate_interleaved_samples(
    samples: &[f32],
    channel_count: usize,
    peaks: &mut Vec<f32>,
    window_peak: &mut f32,
    window_count: &mut usize,
    window_size: usize,
) {
    for frame in samples.chunks(channel_count.max(1)) {
        let frame_peak = frame
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        accumulate_waveform_frame_peak(peaks, window_peak, window_count, frame_peak, window_size);
    }
}

impl PlatformAudio {
    pub(crate) fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                current_audio: None,
                current_audio_source: None,
                current_blob_url: None,
                playback_speed: 1.0,
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
                current_audio_source: None,
                playback_started_at: None,
                playback_start_offset_seconds: 0.0,
                playback_speed: 1.0,
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn revoke_blob_url(&mut self) {
        if let Some(url) = self.current_blob_url.take() {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    }

    pub(crate) fn stop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = &self.current_audio {
            let _ = audio.pause();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(sink) = &self.current_sink {
                sink.pause();
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
        let source_key = format!("bytes:{}", _music_source);
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        #[cfg(target_arch = "wasm32")]
        {
            if self.current_audio_source.as_deref() == Some(source_key.as_str()) {
                if let Some(audio) = &self.current_audio {
                    let _ = audio.set_current_time(start_seconds as f64);
                    audio.set_playback_rate(self.playback_speed as f64);
                    let _ = audio.play();
                    return;
                }
            }

            self.revoke_blob_url();
            let uint8_array = unsafe { js_sys::Uint8Array::view(bytes) };
            let blob =
                web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&uint8_array.into()))
                    .unwrap();
            let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&url) {
                let _ = audio.set_current_time(start_seconds as f64);
                audio.set_playback_rate(self.playback_speed as f64);
                let _ = audio.play();
                self.current_audio = Some(audio);
                self.current_audio_source = Some(source_key);
                self.current_blob_url = Some(url);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::time::Duration;

            if self.current_audio_source.as_deref() == Some(source_key.as_str()) {
                if let Some(sink) = &self.current_sink {
                    if let Err(err) = sink.try_seek(Duration::from_secs_f32(start_seconds)) {
                        log::warn!(
                            "Failed to seek imported audio '{}' to {:.3}s: {}",
                            _music_source,
                            start_seconds,
                            err
                        );
                    }
                    sink.set_speed(self.playback_speed);
                    sink.play();
                    self.playback_started_at = Some(std::time::Instant::now());
                    self.playback_start_offset_seconds = start_seconds;
                    return;
                }
            }

            if let Some(sink) = self.current_sink.take() {
                sink.stop();
            }
            self.current_audio_source = None;

            if let Some(handle) = &self.output_handle {
                match rodio::Decoder::new(std::io::Cursor::new(bytes.to_vec())) {
                    Ok(source) => match rodio::Sink::try_new(handle) {
                        Ok(sink) => {
                            sink.append(source);
                            if let Err(err) = sink.try_seek(Duration::from_secs_f32(start_seconds))
                            {
                                log::warn!(
                                    "Failed to seek imported audio '{}' to {:.3}s: {}",
                                    _music_source,
                                    start_seconds,
                                    err
                                );
                            }
                            sink.set_speed(self.playback_speed);
                            sink.play();
                            self.current_sink = Some(sink);
                            self.current_audio_source = Some(source_key);
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
        let source_key = format!("asset:{}/{}", level_name, music_source);
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        #[cfg(target_arch = "wasm32")]
        {
            let audio_url = format!("assets/levels/{}/{}", level_name, music_source);
            if self.current_audio_source.as_deref() == Some(source_key.as_str()) {
                if let Some(audio) = &self.current_audio {
                    let _ = audio.set_current_time(start_seconds as f64);
                    audio.set_playback_rate(self.playback_speed as f64);
                    let _ = audio.play();
                    return;
                }
            }

            self.revoke_blob_url();
            if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
                let _ = audio.set_current_time(start_seconds as f64);
                audio.set_playback_rate(self.playback_speed as f64);
                let _ = audio.play();
                self.current_audio = Some(audio);
                self.current_audio_source = Some(source_key);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::time::Duration;

            if self.current_audio_source.as_deref() == Some(source_key.as_str()) {
                if let Some(sink) = &self.current_sink {
                    if let Err(err) = sink.try_seek(Duration::from_secs_f32(start_seconds)) {
                        log::warn!(
                            "Failed to seek level music '{}/{}' to {:.3}s: {}",
                            level_name,
                            music_source,
                            start_seconds,
                            err
                        );
                    }
                    sink.set_speed(self.playback_speed);
                    sink.play();
                    self.playback_started_at = Some(std::time::Instant::now());
                    self.playback_start_offset_seconds = start_seconds;
                    return;
                }
            }

            if let Some(sink) = self.current_sink.take() {
                sink.stop();
            }
            self.current_audio_source = None;

            if let Some(handle) = &self.output_handle {
                let audio_path = format!("assets/levels/{}/{}", level_name, music_source);

                match std::fs::read(&audio_path) {
                    Ok(audio_bytes) => {
                        match rodio::Decoder::new(std::io::Cursor::new(audio_bytes)) {
                            Ok(source) => match rodio::Sink::try_new(handle) {
                                Ok(sink) => {
                                    sink.append(source);
                                    if let Err(err) =
                                        sink.try_seek(Duration::from_secs_f32(start_seconds))
                                    {
                                        log::warn!(
                                            "Failed to seek level music '{}' to {:.3}s: {}",
                                            audio_path,
                                            start_seconds,
                                            err
                                        );
                                    }
                                    sink.set_speed(self.playback_speed);
                                    sink.play();
                                    self.current_sink = Some(sink);
                                    self.current_audio_source = Some(source_key);
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
            self.playback_speed = speed;
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
    bytes: &[u8],
    window_size: usize,
) -> Option<(Vec<f32>, u32)> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let source = std::io::Cursor::new(bytes.to_vec());
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
            accumulate_interleaved_samples(
                buf.samples(),
                channel_count,
                &mut peaks,
                &mut window_peak,
                &mut window_count,
                window_size,
            );
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
    for frame_index in 0..frame_len {
        let mut frame_peak = 0.0f32;
        for channel in &channel_data {
            frame_peak = frame_peak.max(channel[frame_index].abs());
        }
        accumulate_waveform_frame_peak(
            &mut peaks,
            &mut window_peak,
            &mut window_count,
            frame_peak,
            window_size,
        );
    }

    if window_count > 0 {
        peaks.push(window_peak);
    }

    let _ = context.close();

    Some((peaks, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::{accumulate_interleaved_samples, accumulate_waveform_frame_peak};

    #[test]
    fn interleaved_stereo_accumulates_per_frame_not_per_channel() {
        let interleaved = vec![
            0.2, 0.7, // frame 0 -> 0.7
            0.4, 0.1, // frame 1 -> 0.4
            0.8, 0.3, // frame 2 -> 0.8
            0.1, 0.9, // frame 3 -> 0.9
        ];

        let mut peaks = Vec::new();
        let mut window_peak = 0.0;
        let mut window_count = 0usize;

        accumulate_interleaved_samples(
            &interleaved,
            2,
            &mut peaks,
            &mut window_peak,
            &mut window_count,
            2,
        );

        assert_eq!(peaks, vec![0.7, 0.9]);
        assert_eq!(window_count, 0);
        assert_eq!(window_peak, 0.0);
    }

    #[test]
    fn carries_partial_window_across_chunks() {
        let mut peaks = Vec::new();
        let mut window_peak = 0.0;
        let mut window_count = 0usize;

        accumulate_waveform_frame_peak(&mut peaks, &mut window_peak, &mut window_count, 0.3, 3);
        accumulate_waveform_frame_peak(&mut peaks, &mut window_peak, &mut window_count, 0.8, 3);

        assert!(peaks.is_empty());
        assert_eq!(window_count, 2);
        assert_eq!(window_peak, 0.8);

        accumulate_waveform_frame_peak(&mut peaks, &mut window_peak, &mut window_count, 0.5, 3);

        assert_eq!(peaks, vec![0.8]);
        assert_eq!(window_count, 0);
        assert_eq!(window_peak, 0.0);
    }
}
