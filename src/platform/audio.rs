use crate::platform::audio_backend::{create_audio_backend, AudioBackend};

pub(crate) struct PlatformAudio {
    backend: Box<dyn AudioBackend>,
}

pub(crate) fn runtime_asset_source_key(level_name: &str, music_source: &str) -> String {
    format!("asset:{}/{}", level_name, music_source)
}

fn runtime_bytes_source_key(music_source: &str) -> String {
    format!("bytes:{}", music_source)
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
        Self {
            backend: create_audio_backend(),
        }
    }

    pub(crate) fn stop(&mut self) {
        self.backend.stop();
    }

    fn start_with_source_key_at(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let start_seconds = start_seconds.max(0.0);
        self.backend.stop();

        if self.backend.can_reuse_source(&source_key) && self.backend.seek_and_play(start_seconds) {
            return;
        }

        self.backend
            .replace_with_bytes(source_key, music_source, bytes, start_seconds);
    }

    pub(crate) fn start_with_bytes_at(
        &mut self,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        self.start_with_source_key_at(
            runtime_bytes_source_key(music_source),
            music_source,
            bytes,
            start_seconds,
        );
    }

    pub(crate) fn start_preloaded_asset_at(
        &mut self,
        level_name: &str,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        self.start_with_source_key_at(
            runtime_asset_source_key(level_name, music_source),
            music_source,
            bytes,
            start_seconds,
        );
    }

    pub(crate) fn start_at(&mut self, level_name: &str, music_source: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, music_source);
        let start_seconds = start_seconds.max(0.0);
        self.backend.stop();

        if self.backend.can_reuse_source(&source_key) && self.backend.seek_and_play(start_seconds) {
            return;
        }

        self.backend
            .replace_with_asset(source_key, level_name, music_source, start_seconds);
    }

    pub(crate) fn playback_time_seconds(&self) -> Option<f32> {
        self.backend.playback_time_seconds()
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.backend.is_playing()
    }

    pub(crate) fn set_speed(&mut self, speed: f32) {
        self.backend.set_speed(speed.clamp(0.25, 2.0));
    }

    pub(crate) fn play_sfx(&mut self, asset_path: &str) {
        self.backend.play_sfx(asset_path);
    }

    pub(crate) fn backend_name(&self) -> String {
        self.backend.backend_name()
    }

    #[cfg(test)]
    fn with_backend(backend: Box<dyn AudioBackend>) -> Self {
        Self { backend }
    }
}

/// Decode audio bytes to a downsampled waveform suitable for display.
/// Returns (peak_samples, sample_rate) where peak_samples contains one peak per window.
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

#[cfg(test)]
mod tests {
    use super::{accumulate_waveform_frame_peak, PlatformAudio};
    use crate::platform::audio_backend::AudioBackend;
    use std::sync::{Arc, Mutex};

    use super::accumulate_interleaved_samples;

    #[derive(Default)]
    struct MockState {
        stop_calls: usize,
        reuse: bool,
        seek_success: bool,
        seek_calls: usize,
        replace_bytes_calls: usize,
        replace_asset_calls: usize,
        last_bytes_source_key: Option<String>,
        last_speed: Option<f32>,
    }

    struct MockBackend {
        state: Arc<Mutex<MockState>>,
    }

    impl MockBackend {
        fn new(state: Arc<Mutex<MockState>>) -> Self {
            Self { state }
        }
    }

    impl AudioBackend for MockBackend {
        fn stop(&mut self) {
            self.state.lock().unwrap().stop_calls += 1;
        }

        fn can_reuse_source(&self, _source_key: &str) -> bool {
            self.state.lock().unwrap().reuse
        }

        fn seek_and_play(&mut self, _start_seconds: f32) -> bool {
            let mut state = self.state.lock().unwrap();
            state.seek_calls += 1;
            state.seek_success
        }

        fn replace_with_bytes(
            &mut self,
            source_key: String,
            _music_source: &str,
            _bytes: &[u8],
            _start_seconds: f32,
        ) {
            let mut state = self.state.lock().unwrap();
            state.replace_bytes_calls += 1;
            state.last_bytes_source_key = Some(source_key);
        }

        fn replace_with_asset(
            &mut self,
            _source_key: String,
            _level_name: &str,
            _music_source: &str,
            _start_seconds: f32,
        ) {
            self.state.lock().unwrap().replace_asset_calls += 1;
        }

        fn playback_time_seconds(&self) -> Option<f32> {
            None
        }

        fn is_playing(&self) -> bool {
            false
        }

        fn set_speed(&mut self, speed: f32) {
            self.state.lock().unwrap().last_speed = Some(speed);
        }

        fn play_sfx(&mut self, _asset_path: &str) {
            // no-op for mock
        }

        fn backend_name(&self) -> String {
            "MockBackend".to_string()
        }
    }

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

    #[test]
    fn reuses_existing_source_before_replacing() {
        let state = Arc::new(Mutex::new(MockState {
            reuse: true,
            seek_success: true,
            ..Default::default()
        }));
        let backend = Box::new(MockBackend::new(state.clone()));
        let mut audio = PlatformAudio::with_backend(backend);
        audio.start_with_bytes_at("music.mp3", &[1, 2, 3], 1.0);

        let state = state.lock().unwrap();
        assert_eq!(state.stop_calls, 1);
        assert_eq!(state.seek_calls, 1);
        assert_eq!(state.replace_bytes_calls, 0);
    }

    #[test]
    fn replaces_when_reuse_seek_fails() {
        let state = Arc::new(Mutex::new(MockState {
            reuse: true,
            seek_success: false,
            ..Default::default()
        }));
        let backend = Box::new(MockBackend::new(state.clone()));
        let mut audio = PlatformAudio::with_backend(backend);
        audio.start_at("level", "music.mp3", 0.0);

        let state = state.lock().unwrap();
        assert_eq!(state.stop_calls, 1);
        assert_eq!(state.seek_calls, 1);
        assert_eq!(state.replace_asset_calls, 1);
    }

    #[test]
    fn replaces_when_source_is_not_reusable() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let backend = Box::new(MockBackend::new(state.clone()));
        let mut audio = PlatformAudio::with_backend(backend);
        audio.start_at("level", "music.mp3", 2.0);

        let state = state.lock().unwrap();
        assert_eq!(state.stop_calls, 1);
        assert_eq!(state.seek_calls, 0);
        assert_eq!(state.replace_asset_calls, 1);
    }

    #[test]
    fn preloaded_assets_use_level_scoped_source_keys() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let backend = Box::new(MockBackend::new(state.clone()));
        let mut audio = PlatformAudio::with_backend(backend);
        audio.start_preloaded_asset_at("level", "music.mp3", &[1, 2, 3], 0.5);

        let state = state.lock().unwrap();
        assert_eq!(state.replace_bytes_calls, 1);
        assert_eq!(
            state.last_bytes_source_key.as_deref(),
            Some("asset:level/music.mp3")
        );
    }

    #[test]
    fn clamps_speed_before_delegating() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let backend = Box::new(MockBackend::new(state.clone()));
        let mut audio = PlatformAudio::with_backend(backend);
        audio.set_speed(10.0);

        let state = state.lock().unwrap();
        assert_eq!(state.last_speed, Some(2.0));
    }
}
