/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{num::NonZeroUsize, sync::Arc};

use rodisnyaa::{AudioBackend as NyaaBackend, AudioOutput, Nyaa, NyaaError, Waveform};
use web_time::Duration;

#[cfg(target_arch = "wasm32")]
static RAYON_INIT_SCHEDULED_AFTER_AUDIO: AtomicBool = AtomicBool::new(false);

fn host_label(backend: NyaaBackend) -> String {
    format!("{backend:?}")
}

fn parse_host_id_by_label(label: &str) -> Option<NyaaBackend> {
    AudioOutput::available_backends()
        .into_iter()
        .find(|backend| host_label(*backend).eq_ignore_ascii_case(label.trim()))
}

#[cfg(target_arch = "wasm32")]
fn schedule_rayon_after_audio_worklet() {
    use wasm_bindgen::JsCast;

    if RAYON_INIT_SCHEDULED_AFTER_AUDIO.swap(true, Ordering::AcqRel) {
        return;
    }

    let Some(window) = web_sys::window() else {
        return;
    };

    let callback = wasm_bindgen::closure::Closure::<dyn FnMut()>::once(move || {
        let Some(window) = web_sys::window() else {
            return;
        };

        let Ok(init_rayon) = js_sys::Reflect::get(
            &window,
            &wasm_bindgen::JsValue::from_str("__EGAKARETA_INIT_RAYON"),
        ) else {
            log::warn!("Rayon worker pool initializer is unavailable");
            return;
        };

        if let Some(init_rayon) = init_rayon.dyn_ref::<js_sys::Function>() {
            if let Err(err) = init_rayon.call0(&wasm_bindgen::JsValue::NULL) {
                log::warn!("Failed to start Rayon worker pool: {:?}", err);
            }
        }
    });

    if let Err(err) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        750,
    ) {
        log::warn!("Failed to schedule Rayon worker pool startup: {:?}", err);
    }

    callback.forget();
}

pub fn available_host_labels() -> Vec<String> {
    let mut labels = vec!["Default".to_string()];
    labels.extend(
        AudioOutput::available_backends()
            .into_iter()
            .map(host_label),
    );
    labels.sort();
    labels.dedup();
    labels
}

pub fn canonical_backend_name(backend_name: &str) -> Option<String> {
    let trimmed = backend_name.trim();
    if trimmed.eq_ignore_ascii_case("Default") {
        return Some("Default".to_string());
    }
    parse_host_id_by_label(trimmed).map(host_label)
}

pub fn runtime_asset_source_key(level_name: &str, asset_path: &str) -> String {
    format!("asset:{level_name}/{asset_path}")
}

fn runtime_bytes_source_key(asset_path: &str) -> String {
    format!("bytes:{asset_path}")
}

fn playback_position(seconds: f32) -> Duration {
    Duration::try_from_secs_f32(seconds.max(0.0)).unwrap_or_default()
}

#[derive(Clone)]
enum AudioBytes {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl AudioBytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Static(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }

    fn duration(&self) -> Result<Option<Duration>, NyaaError> {
        match self {
            Self::Static(bytes) => Nyaa::duration_from_static_bytes(bytes),
            Self::Shared(bytes) => Nyaa::duration_from_shared_bytes(bytes),
        }
    }

    fn play(&self, nyaa: &mut Nyaa) -> Result<(), NyaaError> {
        match self {
            Self::Static(bytes) => nyaa.play_static_bytes(bytes),
            Self::Shared(bytes) => nyaa.play_shared_bytes(bytes),
        }
    }
}

#[derive(Clone)]
struct CachedTrack {
    source_key: String,
    bytes: AudioBytes,
    duration: Option<Duration>,
}

pub struct AudioBackend {
    output: Option<AudioOutput>,
    nyaa: Option<Nyaa>,
    preferred_backend: Option<NyaaBackend>,
    current_audio_source: Option<String>,
    cached_track: Option<CachedTrack>,
    playback_speed: f32,
    active_sfx: Vec<Nyaa>,
}

impl AudioBackend {
    pub fn new() -> Self {
        Self {
            output: None,
            nyaa: None,
            preferred_backend: None,
            current_audio_source: None,
            cached_track: None,
            playback_speed: 1.0,
            active_sfx: Vec::new(),
        }
    }

    pub fn ensure_device(&mut self) -> bool {
        if self.output.is_none() {
            let preferred = self.preferred_backend;
            #[cfg(target_arch = "wasm32")]
            let preferred = preferred.or_else(|| {
                AudioOutput::available_backends()
                    .contains(&NyaaBackend::AudioWorklet)
                    .then_some(NyaaBackend::AudioWorklet)
            });

            let output = if let Some(backend) = preferred {
                match AudioOutput::try_new_with_backend(backend) {
                    Ok(output) => output,
                    Err(error) => {
                        log::warn!("Failed to open Nyaa audio output: {error}");
                        return false;
                    }
                }
            } else {
                AudioOutput::new()
            };
            self.nyaa = Some(Nyaa::new_with_output(output.clone()));
            self.output = Some(output);
        }

        let Some(nyaa) = self.nyaa.as_mut() else {
            return false;
        };
        if !nyaa.has_audio_output() {
            if let Err(error) = nyaa.retry_audio_output() {
                log::warn!("Failed to initialize Nyaa audio output: {error}");
                while nyaa.poll_event().is_some() {}
                return false;
            }
        }

        #[cfg(target_arch = "wasm32")]
        if nyaa.audio_backend() == Some(NyaaBackend::AudioWorklet) {
            schedule_rayon_after_audio_worklet();
        }

        true
    }

    pub fn set_preferred_backend_name(&mut self, backend_name: &str) -> bool {
        let trimmed = backend_name.trim();
        let parsed = parse_host_id_by_label(trimmed);
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("Default") && parsed.is_none() {
            return false;
        }
        if self.preferred_backend == parsed {
            return true;
        }

        self.stop();
        self.active_sfx.clear();
        self.nyaa = None;
        self.output = None;
        self.preferred_backend = parsed;
        true
    }

    pub fn pause(&mut self) {
        if let Some(nyaa) = &self.nyaa {
            nyaa.pause();
            while nyaa.poll_event().is_some() {}
        }
    }

    pub fn resume(&mut self) {
        if self.current_audio_source.is_some()
            && let Some(nyaa) = &self.nyaa
        {
            nyaa.resume();
            while nyaa.poll_event().is_some() {}
        }
    }

    pub fn stop(&mut self) {
        if let Some(nyaa) = &mut self.nyaa {
            nyaa.stop();
            while nyaa.poll_event().is_some() {}
        }
        self.current_audio_source = None;
    }

    pub fn can_reuse_source(&self, source_key: &str) -> bool {
        self.current_audio_source.as_deref() == Some(source_key)
    }

    fn cache_track(&mut self, source_key: String, bytes: AudioBytes) -> bool {
        if self.cached_track.as_ref().is_some_and(|track| {
            track.source_key == source_key && track.bytes.as_slice() == bytes.as_slice()
        }) {
            return true;
        }

        self.stop();
        self.cached_track = None;
        match bytes.duration() {
            Ok(duration) => {
                self.cached_track = Some(CachedTrack {
                    source_key,
                    bytes,
                    duration,
                });
                true
            }
            Err(error) => {
                log::warn!("Failed to load music '{source_key}' with Nyaa: {error}");
                false
            }
        }
    }

    pub fn seek_and_play(&mut self, start_seconds: f32) -> bool {
        let Some(track) = self.cached_track.clone() else {
            return false;
        };
        let position = playback_position(start_seconds);
        let position = track
            .duration
            .map_or(position, |duration| position.min(duration));

        if self.can_reuse_source(&track.source_key)
            && let Some(nyaa) = self.nyaa.as_mut()
            && !nyaa.is_empty()
        {
            if let Err(error) = nyaa.try_seek(position) {
                log::warn!("Failed to seek music with Nyaa: {error}");
                self.stop();
                return false;
            }
            nyaa.resume();
            while nyaa.poll_event().is_some() {}
            return true;
        }

        self.stop();
        if !self.ensure_device() {
            return false;
        }
        let Some(output) = &self.output else {
            return false;
        };
        let mut nyaa = Nyaa::new_with_output(output.clone());
        nyaa.set_speed(self.playback_speed);
        if cfg!(test) {
            nyaa.set_volume(0.0);
        }
        let result = nyaa
            .try_seek(position)
            .and_then(|()| track.bytes.play(&mut nyaa));
        if let Err(error) = result {
            log::warn!(
                "Failed to play music '{}' with Nyaa: {error}",
                track.source_key
            );
            return false;
        }

        while nyaa.poll_event().is_some() {}
        self.nyaa = Some(nyaa);
        self.current_audio_source = Some(track.source_key);
        true
    }

    pub fn replace_with_bytes(&mut self, source_key: String, bytes: &[u8], start_seconds: f32) {
        if self.cache_track(source_key, AudioBytes::Shared(Arc::from(bytes))) {
            self.seek_and_play(start_seconds);
        }
    }

    pub fn playback_time_seconds(&self) -> Option<f32> {
        self.is_playing()
            .then(|| self.nyaa.as_ref().unwrap().position().as_secs_f32())
    }

    pub fn is_playing(&self) -> bool {
        self.current_audio_source.is_some()
            && self.nyaa.as_ref().is_some_and(|nyaa| {
                let playing = nyaa.is_playing();
                while nyaa.poll_event().is_some() {}
                playing
            })
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.playback_speed = if speed.is_finite() {
            speed.clamp(0.25, 2.0)
        } else {
            1.0
        };
        if let Some(nyaa) = &mut self.nyaa {
            nyaa.set_speed(self.playback_speed);
            while nyaa.poll_event().is_some() {}
        }
    }

    pub fn play_sfx(&mut self, asset_bytes: &'static [u8]) {
        self.active_sfx.retain(|nyaa| !nyaa.is_empty());
        if !self.ensure_device() {
            return;
        }
        let Some(output) = &self.output else {
            return;
        };
        let mut nyaa = Nyaa::new_with_output(output.clone());
        if cfg!(test) {
            nyaa.set_volume(0.0);
        }
        match nyaa.play_static_bytes(asset_bytes) {
            Ok(()) => self.active_sfx.push(nyaa),
            Err(error) => log::warn!("Failed to play sound effect with Nyaa: {error}"),
        }
    }

    pub fn backend_name(&self) -> String {
        if let Some(output) = &self.output {
            let backend = output
                .backend()
                .map(host_label)
                .unwrap_or_else(|| "Default".into());
            return output.device().map_or_else(
                || backend.clone(),
                |device| format!("{backend} ({})", device.name()),
            );
        }
        self.preferred_backend
            .map(host_label)
            .unwrap_or_else(|| "Default".into())
    }

    pub fn start_with_bytes_at(&mut self, bytes: &[u8], start_seconds: f32) {
        self.replace_with_bytes(runtime_bytes_source_key("audio.mp3"), bytes, start_seconds);
    }

    pub fn warmup_with_bytes_at(&mut self, bytes: &[u8], _start_seconds: f32) {
        self.stop();
        self.cache_track(
            runtime_bytes_source_key("audio.mp3"),
            AudioBytes::Shared(Arc::from(bytes)),
        );
    }

    pub fn start_preloaded_asset_at(&mut self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.replace_with_bytes(
            runtime_asset_source_key(level_name, "audio.mp3"),
            bytes,
            start_seconds,
        );
    }

    pub fn warmup_preloaded_asset_at(
        &mut self,
        level_name: &str,
        bytes: &[u8],
        _start_seconds: f32,
    ) {
        self.stop();
        self.cache_track(
            runtime_asset_source_key(level_name, "audio.mp3"),
            AudioBytes::Shared(Arc::from(bytes)),
        );
    }

    fn cache_asset(&mut self, level_name: &str) -> bool {
        let Some(bytes) = eg_level::get_builtin_audio(level_name) else {
            self.stop();
            self.cached_track = None;
            log::warn!("Failed to read level music for '{level_name}'");
            return false;
        };
        self.cache_track(
            runtime_asset_source_key(level_name, "audio.mp3"),
            AudioBytes::Static(bytes),
        )
    }

    pub fn start_at(&mut self, level_name: &str, start_seconds: f32) {
        if self.cache_asset(level_name) {
            self.seek_and_play(start_seconds);
        }
    }

    pub fn warmup_at(&mut self, level_name: &str, _start_seconds: f32) {
        self.stop();
        self.cache_asset(level_name);
    }
}

impl Default for AudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WaveformDecodeSummary {
    pub sample_rate: u32,
    pub peak_count: usize,
}

#[cfg(test)]
pub fn decode_audio_to_waveform(bytes: &[u8], window_size: usize) -> Option<(Vec<f32>, u32)> {
    let mut peaks = Vec::new();
    let summary =
        decode_audio_to_waveform_streaming(bytes, window_size, usize::MAX, |_, chunk, _| {
            peaks.extend(chunk);
        })?;
    Some((peaks, summary.sample_rate))
}

pub fn decode_audio_to_waveform_streaming<F>(
    bytes: &[u8],
    window_size: usize,
    chunk_peak_count: usize,
    mut on_chunk: F,
) -> Option<WaveformDecodeSummary>
where
    F: FnMut(usize, Vec<f32>, u32),
{
    let source = Nyaa::decoder_from_shared_bytes(Arc::from(bytes)).ok()?;
    let window_size = NonZeroUsize::new(window_size.max(1))?;
    let mut builder = Waveform::builder_from_source_with_base_frames(source, window_size);
    let sample_rate = builder.sample_rate();
    let frames_per_chunk = window_size
        .get()
        .saturating_mul(chunk_peak_count.clamp(1, 4096));
    let mut emitted_peak_count = 0usize;

    loop {
        let finished = builder.advance_frames(frames_per_chunk);
        let slice = builder.slice(Duration::ZERO..builder.decoded_duration(), usize::MAX);
        let chunk: Vec<f32> = slice
            .peaks()
            .iter()
            .skip(emitted_peak_count)
            .take_while(|peak| peak.is_available())
            .map(|peak| peak.amplitude())
            .collect();
        if !chunk.is_empty() {
            let chunk_len = chunk.len();
            on_chunk(emitted_peak_count, chunk, sample_rate);
            emitted_peak_count += chunk_len;
        }
        if finished {
            break;
        }
    }

    Some(WaveformDecodeSummary {
        sample_rate,
        peak_count: emitted_peak_count,
    })
}

/// Minimal PCM WAV mono, 16-bit, 44.1kHz, one zero sample.
/// Reserved for testing.
pub fn tiny_wav_bytes() -> Vec<u8> {
    vec![
        b'R', b'I', b'F', b'F', 38, 0, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ', 16, 0,
        0, 0, 1, 0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0, b'd', b'a', b't', b'a', 2, 0,
        0, 0, 0, 0,
    ]
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;

    fn build_wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_bytes = samples.len() as u32 * 2;
        let mut wav = Vec::with_capacity(44 + data_bytes as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn backend_names_are_canonical_sorted_and_unique() {
        assert_eq!(
            canonical_backend_name(" default ").as_deref(),
            Some("Default")
        );
        assert_eq!(canonical_backend_name(""), None);
        assert_eq!(canonical_backend_name("not-a-backend"), None);
        let labels = available_host_labels();
        let mut expected = labels.clone();
        expected.sort();
        expected.dedup();
        assert_eq!(labels, expected);
        assert!(labels.iter().any(|label| label == "Default"));
        for backend in AudioOutput::available_backends() {
            assert_eq!(
                parse_host_id_by_label(&host_label(backend).to_lowercase()),
                Some(backend)
            );
        }
    }

    #[test]
    fn output_is_lazy_and_unknown_backend_leaves_it_unchanged() {
        let mut backend = AudioBackend::new();
        assert!(!backend.set_preferred_backend_name("not-a-backend"));
        assert_eq!(backend.backend_name(), "Default");
        assert!(backend.output.is_none());
        assert!(backend.nyaa.is_none());
    }

    #[test]
    fn warmup_never_starts_playback_or_opens_output() {
        let mut backend = AudioBackend::new();
        backend.warmup_with_bytes_at(&tiny_wav_bytes(), 0.0);
        assert!(backend.cached_track.is_some());
        backend.resume();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
        assert!(backend.output.is_none());
        backend.warmup_preloaded_asset_at("test", &tiny_wav_bytes(), 1.0);
        assert!(!backend.is_playing());
        assert_eq!(
            backend.cached_track.as_ref().unwrap().source_key,
            "asset:test/audio.mp3"
        );
    }

    #[test]
    fn repeated_warmups_retain_encoded_bytes_and_changed_content_replaces_them() {
        let mut backend = AudioBackend::new();
        let first = build_wav(8000, 1, &[0; 8000]);
        backend.warmup_with_bytes_at(&first, 0.0);
        let AudioBytes::Shared(cached) = backend.cached_track.as_ref().unwrap().bytes.clone()
        else {
            panic!("runtime bytes should be shared");
        };
        backend.warmup_with_bytes_at(&first, 0.5);
        let AudioBytes::Shared(reused) = &backend.cached_track.as_ref().unwrap().bytes else {
            panic!("runtime bytes should stay shared");
        };
        assert!(Arc::ptr_eq(&cached, reused));

        let second = build_wav(8000, 1, &[0; 16000]);
        backend.warmup_with_bytes_at(&second, 0.0);
        assert_eq!(
            backend.cached_track.as_ref().unwrap().duration,
            Some(Duration::from_secs(2))
        );
        backend.replace_with_bytes("bytes:bad".into(), &[1, 2, 3], 0.0);
        assert!(backend.cached_track.is_none());
        assert!(!backend.can_reuse_source("bytes:bad"));
        assert!(!backend.seek_and_play(0.0));
    }

    #[test]
    fn transport_inputs_are_finite_and_clamped() {
        let mut backend = AudioBackend::new();
        for (input, expected) in [
            (0.0, 0.25),
            (3.0, 2.0),
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
        ] {
            backend.set_speed(input);
            assert!(backend.playback_speed.is_finite());
            assert_abs_diff_eq!(backend.playback_speed, expected, epsilon = f32::EPSILON);
        }
        assert_eq!(playback_position(-1.0), Duration::ZERO);
        assert_eq!(playback_position(f32::NAN), Duration::ZERO);
        assert_eq!(playback_position(f32::INFINITY), Duration::ZERO);
        assert_eq!(playback_position(f32::MAX), Duration::ZERO);
        assert_eq!(playback_position(1.25), Duration::from_millis(1250));
    }

    #[test]
    fn nyaa_transport_seeks_pauses_stops_and_restarts() {
        let mut backend = AudioBackend::new();
        if !backend.ensure_device() {
            eprintln!("Skipping output-dependent transport assertions: no audio output");
            return;
        }
        let wav = build_wav(8000, 1, &[0; 80000]);
        backend.set_speed(0.5);
        backend.start_with_bytes_at(&wav, 1.25);
        assert!(backend.is_playing());
        assert!(backend.playback_time_seconds().unwrap() >= 1.25);
        backend.pause();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
        assert!(backend.nyaa.as_ref().unwrap().is_paused());
        backend.resume();
        assert!(backend.is_playing());
        assert!(backend.seek_and_play(2.5));
        assert!(backend.playback_time_seconds().unwrap() >= 2.5);
        backend.stop();
        backend.resume();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
        assert!(backend.seek_and_play(0.5));
        assert!(backend.is_playing());
        assert!(backend.playback_time_seconds().unwrap() >= 0.5);
        assert_abs_diff_eq!(
            backend.nyaa.as_ref().unwrap().speed(),
            0.5,
            epsilon = f32::EPSILON
        );

        let embedded = eg_level::get_builtin_audio("Flowerfield").unwrap();
        backend.play_sfx(embedded);
        assert_eq!(backend.active_sfx.len(), 1);
        backend.pause();
        assert!(backend.active_sfx[0].is_playing());
        assert!(!backend.is_playing());
        let selected = backend.output.as_ref().unwrap().backend().unwrap();
        assert!(backend.set_preferred_backend_name(&host_label(selected)));
        assert!(backend.active_sfx.is_empty());
        assert!(backend.output.is_none());
        assert!(backend.cached_track.is_some());
        assert!(backend.set_preferred_backend_name(" default "));
        assert_eq!(backend.backend_name(), "Default");
        assert!(backend.seek_and_play(0.0));

        backend.start_at("Flowerfield", 10.0);
        assert!(backend.is_playing());
        assert!(backend.playback_time_seconds().unwrap() >= 10.0);
        assert!(backend.seek_and_play(2.0));
        assert!(backend.playback_time_seconds().unwrap() >= 2.0);
        backend.replace_with_bytes("bytes:bad".into(), &[1, 2, 3], 0.0);
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);

        backend.start_with_bytes_at(&tiny_wav_bytes(), 0.0);
        backend.nyaa.as_ref().unwrap().wait_until_end();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
        backend.resume();
        assert!(!backend.is_playing());
    }

    #[test]
    fn waveform_preserves_stereo_peaks_and_partial_windows() {
        let wav = build_wav(8000, 2, &[8192, -16384, 4096, 8192, -24576, 16384]);
        let mut chunks = Vec::new();
        let summary = decode_audio_to_waveform_streaming(&wav, 2, 1, |start, peaks, rate| {
            assert_eq!(rate, 8000);
            chunks.push((start, peaks));
        })
        .unwrap();
        assert_eq!(summary.peak_count, 2);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[1].0, 1);
        assert_abs_diff_eq!(chunks[0].1[0], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(chunks[1].1[0], 0.75, epsilon = 1e-6);
    }

    #[test]
    fn streaming_decoder_reports_chunks_from_peak_accumulation() {
        let wav = build_wav(44100, 1, &[8192; 132300]);
        let mut chunks = Vec::new();
        let summary = decode_audio_to_waveform_streaming(&wav, 256, 32, |start, peaks, _| {
            chunks.push((start, peaks));
        })
        .unwrap();
        assert_eq!(summary.sample_rate, 44100);
        assert_eq!(summary.peak_count, 132300usize.div_ceil(256));
        assert_eq!(chunks[0].0, 0);
        for window in chunks.windows(2) {
            assert_eq!(window[0].0 + window[0].1.len(), window[1].0);
        }
        assert!(chunks.iter().all(|(_, peaks)| peaks.len() <= 32));
        let collected = decode_audio_to_waveform(&wav, 256).unwrap();
        assert_eq!(
            collected.0,
            chunks
                .into_iter()
                .flat_map(|(_, peaks)| peaks)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn builtin_mp3_waveform_contains_audio() {
        let bytes = eg_level::get_builtin_audio("Flowerfield").unwrap();
        let (peaks, sample_rate) = decode_audio_to_waveform(bytes, 256).unwrap();
        assert!(sample_rate > 0);
        assert!(peaks.len() > sample_rate as usize / 256);
        assert!(peaks.iter().all(|peak| peak.is_finite()));
        assert!(peaks.iter().any(|peak| *peak > 0.01));
    }

    #[test]
    fn waveform_rejects_invalid_bytes_and_handles_zero_chunk_settings() {
        assert!(decode_audio_to_waveform(&[1, 2, 3], 256).is_none());
        let summary =
            decode_audio_to_waveform_streaming(&tiny_wav_bytes(), 0, 0, |_, _, _| {}).unwrap();
        assert_eq!(summary.peak_count, 1);
    }
}
