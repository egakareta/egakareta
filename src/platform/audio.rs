/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::platform::audio_backend::AudioBackend;

pub(crate) fn available_backend_names() -> Vec<String> {
    AudioBackend::available_backend_names()
}

pub(crate) struct PlatformAudio {
    backend: AudioBackend,
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

pub(crate) struct WaveformDecodeSummary {
    pub(crate) sample_rate: u32,
    pub(crate) peak_count: usize,
}

impl PlatformAudio {
    pub(crate) fn new() -> Self {
        Self {
            backend: AudioBackend::new(),
        }
    }

    pub(crate) fn stop(&mut self) {
        self.backend.stop();
    }

    pub(crate) fn pause(&mut self) {
        self.backend.pause();
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

    pub(crate) fn warmup_with_bytes_at(
        &mut self,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let source_key = runtime_bytes_source_key(music_source);
        let start_seconds = start_seconds.max(0.0);
        self.backend.stop();

        if self.backend.can_reuse_source(&source_key) {
            return;
        }

        self.backend
            .warmup_with_bytes(source_key, music_source, bytes, start_seconds);
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

    pub(crate) fn warmup_preloaded_asset_at(
        &mut self,
        level_name: &str,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let source_key = runtime_asset_source_key(level_name, music_source);
        let start_seconds = start_seconds.max(0.0);
        self.backend.stop();

        if self.backend.can_reuse_source(&source_key) {
            return;
        }

        self.backend
            .warmup_with_bytes(source_key, music_source, bytes, start_seconds);
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

    pub(crate) fn warmup_at(&mut self, level_name: &str, music_source: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, music_source);
        let start_seconds = start_seconds.max(0.0);
        self.backend.stop();

        if self.backend.can_reuse_source(&source_key) {
            return;
        }

        self.backend
            .warmup_with_asset(source_key, level_name, music_source, start_seconds);
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

    pub(crate) fn play_sfx(&mut self, asset_bytes: &'static [u8]) {
        self.backend.play_sfx(asset_bytes);
    }

    pub(crate) fn resume(&mut self) {
        self.backend.resume();
    }

    pub(crate) fn resume_playback(&mut self) {
        self.backend.resume_playback();
    }

    pub(crate) fn backend_name(&self) -> String {
        self.backend.backend_name()
    }

    pub(crate) fn set_preferred_backend_name(&mut self, backend_name: &str) -> bool {
        self.backend.set_preferred_backend_name(backend_name)
    }
}

/// Decode audio bytes to a downsampled waveform suitable for display.
/// Returns (peak_samples, sample_rate) where peak_samples contains one peak per window.
#[cfg(test)]
pub(crate) fn decode_audio_to_waveform(
    bytes: &[u8],
    window_size: usize,
) -> Option<(Vec<f32>, u32)> {
    let mut peaks = Vec::new();
    let summary =
        decode_audio_to_waveform_streaming(bytes, window_size, usize::MAX, |_, chunk, _| {
            peaks.extend(chunk);
        })?;

    Some((peaks, summary.sample_rate))
}

pub(crate) fn decode_audio_to_waveform_streaming<F>(
    bytes: &[u8],
    window_size: usize,
    chunk_peak_count: usize,
    mut on_chunk: F,
) -> Option<WaveformDecodeSummary>
where
    F: FnMut(usize, Vec<f32>, u32),
{
    use symphonia::core::audio::sample::Sample;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::formats::TrackType;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let source = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    log::info!("Decoding audio to waveform ({} bytes)", bytes.len());
    let hint = Hint::new();

    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let dec_opts: AudioDecoderOptions = Default::default();

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .ok()?;

    let track = format.default_track(TrackType::Audio)?;
    let audio_params = track.codec_params.as_ref()?.audio()?;
    let sample_rate = audio_params.sample_rate?;
    let channel_count = audio_params
        .channels
        .as_ref()
        .map(|channels| channels.count())
        .unwrap_or(1)
        .max(1);

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_params, &dec_opts)
        .ok()?;

    let chunk_peak_count = chunk_peak_count.max(1);
    let mut peaks: Vec<f32> = Vec::with_capacity(chunk_peak_count.min(4096));
    let mut emitted_peak_count = 0usize;
    let mut window_peak: f32 = 0.0;
    let mut window_count: usize = 0;
    let track_id = track.id;
    let mut interleaved_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::ResetRequired) => return None,
            Err(_) => break,
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(_)) => break,
            Err(_) => return None,
        };

        interleaved_samples.resize(decoded.samples_interleaved(), f32::MID);
        decoded.copy_to_slice_interleaved(&mut interleaved_samples);
        accumulate_interleaved_samples(
            &interleaved_samples,
            channel_count,
            &mut peaks,
            &mut window_peak,
            &mut window_count,
            window_size,
        );

        if peaks.len() >= chunk_peak_count {
            let chunk = std::mem::take(&mut peaks);
            let chunk_len = chunk.len();
            on_chunk(emitted_peak_count, chunk, sample_rate);
            emitted_peak_count += chunk_len;
        }
    }

    if window_count > 0 {
        peaks.push(window_peak);
    }

    if !peaks.is_empty() {
        let chunk = std::mem::take(&mut peaks);
        let chunk_len = chunk.len();
        on_chunk(emitted_peak_count, chunk, sample_rate);
        emitted_peak_count += chunk_len;
    }

    log::info!(
        "Audio waveform decoding complete ({} peaks)",
        emitted_peak_count
    );
    Some(WaveformDecodeSummary {
        sample_rate,
        peak_count: emitted_peak_count,
    })
}

#[cfg(test)]
mod tests {
    use super::accumulate_interleaved_samples;
    use super::accumulate_waveform_frame_peak;
    use super::decode_audio_to_waveform;
    use super::decode_audio_to_waveform_streaming;

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

    /// Build a minimal valid WAV file (16-bit mono PCM) from the given samples.
    fn build_wav_mono_16bit(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_bytes = samples.len() as u32 * 2;
        let mut wav = Vec::with_capacity(44 + data_bytes as usize);
        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        // fmt sub-chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * 2; // mono 16-bit
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
                                                     // data sub-chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for &s in samples {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        wav
    }

    #[test]
    fn streaming_decoder_reports_chunks_from_peak_accumulation() {
        // Use a small synthetic WAV instead of a 4 MB MP3 to keep the test fast
        // while exercising the same streaming decode → chunk emission paths.
        let sample_rate = 44100u32;
        let num_samples = sample_rate as usize * 3; // 3 seconds
        let samples: Vec<i16> = (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate as f64;
                (t * 440.0 * 2.0 * std::f64::consts::PI).sin() * 16000.0
            })
            .map(|v| v as i16)
            .collect();
        let bytes = build_wav_mono_16bit(sample_rate, &samples);

        let mut chunks = Vec::new();
        let summary = decode_audio_to_waveform_streaming(&bytes, 256, 32, |start, peaks, _| {
            chunks.push((start, peaks));
        })
        .expect("synthetic wav should decode");

        assert!(summary.sample_rate > 0);
        assert!(summary.peak_count > 0);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].0, 0);
        for window in chunks.windows(2) {
            assert_eq!(window[0].0 + window[0].1.len(), window[1].0);
        }
        let chunked_peak_count: usize = chunks.iter().map(|(_, peaks)| peaks.len()).sum();
        assert_eq!(chunked_peak_count, summary.peak_count);

        let collected = decode_audio_to_waveform(&bytes, 256).expect("collector should decode");
        assert_eq!(collected.0.len(), summary.peak_count);
        assert_eq!(collected.1, summary.sample_rate);
    }
}
