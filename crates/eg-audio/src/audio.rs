/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use rodio::Source;
use std::io::Cursor;
use std::str::FromStr;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};
use web_time::Duration;

#[cfg(target_arch = "wasm32")]
static RAYON_INIT_SCHEDULED_AFTER_AUDIO: AtomicBool = AtomicBool::new(false);

fn host_label(host_id: cpal::HostId) -> String {
    format!("{:?}", host_id)
}

fn parse_host_id_by_label(label: &str) -> Option<cpal::HostId> {
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Default") {
        return None;
    }

    if let Ok(host_id) = cpal::HostId::from_str(trimmed) {
        return Some(host_id);
    }

    if let Some(host_id) = cpal::available_hosts()
        .into_iter()
        .find(|host_id| host_label(*host_id).eq_ignore_ascii_case(trimmed))
    {
        return Some(host_id);
    }

    #[cfg(target_arch = "wasm32")]
    {
        if host_label(cpal::HostId::AudioWorklet).eq_ignore_ascii_case(trimmed) {
            return Some(cpal::HostId::AudioWorklet);
        }
    }

    None
}

fn select_host(preferred_host: Option<cpal::HostId>) -> cpal::Host {
    if let Some(host_id) = preferred_host {
        if let Ok(host) = cpal::host_from_id(host_id) {
            return host;
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Ok(host) = cpal::host_from_id(cpal::HostId::AudioWorklet) {
            return host;
        }
    }

    cpal::default_host()
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

    for host_id in cpal::available_hosts() {
        labels.push(host_label(host_id));
    }

    #[cfg(target_arch = "wasm32")]
    {
        if cpal::host_from_id(cpal::HostId::AudioWorklet).is_ok() {
            let worklet = host_label(cpal::HostId::AudioWorklet);
            if !labels.iter().any(|entry| entry == &worklet) {
                labels.push(worklet);
            }
        }
    }

    labels.sort();
    labels.dedup();
    labels
}

pub fn canonical_backend_name(backend_name: &str) -> Option<String> {
    let trimmed = backend_name.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("Default") {
        return Some("Default".to_string());
    }

    parse_host_id_by_label(trimmed).map(host_label)
}

pub fn runtime_asset_source_key(level_name: &str, asset_path: &str) -> String {
    format!("asset:{}/{}", level_name, asset_path)
}

fn runtime_bytes_source_key(asset_path: &str) -> String {
    format!("bytes:{}", asset_path)
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

pub struct WaveformDecodeSummary {
    pub sample_rate: u32,
    pub peak_count: usize,
}

pub struct AudioBackend {
    output_device: Option<rodio::MixerDeviceSink>,
    device_tried: bool,
    preferred_host: Option<cpal::HostId>,
    current_player: Option<rodio::Player>,
    current_audio_source: Option<String>,
    /// Cached, fully-decoded PCM for the current source keyed by its source key.
    /// Reused across seeks so we never re-decode and never need a blocking seek.
    decoded_cache: Option<(String, rodio::buffer::SamplesBuffer)>,
    playback_started_at: Option<web_time::Instant>,
    playback_start_offset_seconds: f32,
    playback_speed: f32,
    active_sfx: Vec<rodio::Player>,
    backend_name: String,
}

impl AudioBackend {
    pub fn new() -> Self {
        let host_id = select_host(None).id();

        Self {
            output_device: None,
            device_tried: false,
            preferred_host: None,
            current_player: None,
            current_audio_source: None,
            decoded_cache: None,
            playback_started_at: None,
            playback_start_offset_seconds: 0.0,
            playback_speed: 1.0,
            active_sfx: Vec::new(),
            backend_name: host_id.to_string(),
        }
    }

    /// Ensure that the audio output device is initialized and available.
    /// Returns a reference to the device sink if available, or None if initialization failed.
    pub fn ensure_device(&mut self) -> Option<&rodio::MixerDeviceSink> {
        if self.output_device.is_some() {
            return self.output_device.as_ref();
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.device_tried {
            return None;
        }
        self.device_tried = true;

        log::info!("Initializing audio backend on demand");

        use cpal::traits::HostTrait;
        log::info!("Available hosts: {:?}", cpal::available_hosts());
        let host = select_host(self.preferred_host);
        let host_id = host.id();

        log::info!("Selected audio host: {:?}", host_id);
        self.backend_name = host_id.to_string();

        self.output_device = match host.default_output_device() {
            Some(device) => {
                use cpal::traits::DeviceTrait;
                if let Ok(name) = device.id() {
                    log::info!("Audio device: {}", name);
                    self.backend_name = format!("{:?} ({})", host_id, name);
                }
                match rodio::DeviceSinkBuilder::from_device(device).and_then(|b| b.open_stream()) {
                    Ok(sink) => {
                        log::info!("Audio stream opened successfully");
                        #[cfg(target_arch = "wasm32")]
                        if host_id == cpal::HostId::AudioWorklet {
                            schedule_rayon_after_audio_worklet();
                        }
                        Some(sink)
                    }
                    Err(err) => {
                        log::warn!("Failed to open audio stream: {}", err);
                        None
                    }
                }
            }
            None => {
                log::warn!("No default audio output device found");
                None
            }
        };

        self.output_device.as_ref()
    }

    /// Pause the current playback, if any, and update playback_start_offset_seconds.
    pub fn pause(&mut self) {
        if let Some(started_at) = self.playback_started_at {
            self.playback_start_offset_seconds +=
                started_at.elapsed().as_secs_f32() * self.playback_speed;
        }
        if let Some(player) = &self.current_player {
            player.pause();
        }
        self.playback_started_at = None;
    }

    /// Resume playback from the current position, if a player is available.
    /// If no player is available, this will do nothing.
    pub fn resume(&mut self) {
        self.ensure_device();
        if let Some(player) = &self.current_player {
            player.set_speed(self.playback_speed);
            player.play();
            self.playback_started_at = Some(web_time::Instant::now());
        } else {
            #[cfg(test)]
            if self.current_audio_source.is_some() {
                self.playback_started_at = Some(web_time::Instant::now());
            }
        }
    }

    /// Decode compressed audio bytes into an in-memory PCM buffer.
    ///
    /// The decode happens once; the resulting [`rodio::buffer::SamplesBuffer`] is
    /// cheap to clone and supports instant, non-blocking in-memory seeking. This
    /// lets us reuse already-decoded audio when seeking instead of relying on
    /// [`rodio::Player::try_seek`], which blocks the calling thread and therefore
    /// cannot run on the WASM main thread.
    fn decode_to_buffer(bytes: &[u8]) -> Option<rodio::buffer::SamplesBuffer> {
        use rodio::Source;
        let decoded = rodio::Decoder::new(Cursor::new(bytes.to_vec())).ok()?;
        let channels = decoded.channels();
        let sample_rate = decoded.sample_rate();
        let samples: Vec<f32> = decoded.collect();
        Some(rodio::buffer::SamplesBuffer::new(
            channels,
            sample_rate,
            samples,
        ))
    }

    /// Build a fresh player from an in-memory PCM buffer, seeking to `start_seconds`
    /// before playback. Seeking is performed on the buffer itself (an in-memory
    /// index jump), so this never blocks and works identically on native and WASM.
    /// If autoplay is true, playback starts immediately; otherwise it is paused.
    fn play_buffer(
        &mut self,
        source_key: String,
        mut buffer: rodio::buffer::SamplesBuffer,
        start_seconds: f32,
        autoplay: bool,
    ) {
        if start_seconds > 0.001 {
            log::trace!("RodioBackendInner: seeking buffer to {:.3}s", start_seconds);
            let _ = buffer.try_seek(Duration::from_secs_f32(start_seconds));
        }

        let Some(device) = self.ensure_device() else {
            #[cfg(test)]
            {
                self.current_audio_source = Some(source_key);
                if autoplay {
                    self.playback_started_at = Some(web_time::Instant::now());
                    self.playback_start_offset_seconds = start_seconds;
                } else {
                    self.playback_started_at = None;
                    self.playback_start_offset_seconds = 0.0;
                }
            }
            return;
        };

        log::trace!("RodioBackendInner: creating new player");
        let (player, output) = rodio::Player::new();
        log::trace!("RodioBackendInner: adding output to mixer");
        device.mixer().add(output);

        log::trace!("RodioBackendInner: appending buffered source");
        player.append(buffer);

        player.set_speed(self.playback_speed);
        if cfg!(test) {
            player.set_volume(0.0);
        }
        if autoplay {
            player.play();
            self.playback_started_at = Some(web_time::Instant::now());
            self.playback_start_offset_seconds = start_seconds;
        } else {
            player.pause();
            self.playback_started_at = None;
            self.playback_start_offset_seconds = 0.0;
        }
        self.current_player = Some(player);
        self.current_audio_source = Some(source_key);
    }

    /// Set the preferred audio backend by name. If the name is valid and available, it will be set as the preferred backend.
    /// Returns true if the backend was successfully set, false if the name was invalid or unavailable.
    pub fn set_preferred_backend_name(&mut self, backend_name: &str) -> bool {
        let parsed = parse_host_id_by_label(backend_name);
        if !backend_name.trim().is_empty()
            && !backend_name.eq_ignore_ascii_case("Default")
            && parsed.is_none()
        {
            return false;
        }

        if self.preferred_host == parsed {
            return true;
        }

        self.preferred_host = parsed;
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
        for player in self.active_sfx.drain(..) {
            player.stop();
        }
        self.current_audio_source = None;
        self.playback_started_at = None;
        self.playback_start_offset_seconds = 0.0;
        self.output_device = None;
        self.device_tried = false;
        self.backend_name = if let Some(host_id) = self.preferred_host {
            host_label(host_id)
        } else {
            "Default".to_string()
        };

        true
    }

    /// Stop the current playback and reset the playback state.
    pub fn stop(&mut self) {
        if let Some(player) = &self.current_player {
            player.pause();
        }
        self.playback_started_at = None;
        self.playback_start_offset_seconds = 0.0;
    }

    /// Check if the current audio source can be reused based on the provided source key.
    pub fn can_reuse_source(&self, source_key: &str) -> bool {
        self.current_audio_source.as_deref() == Some(source_key)
    }

    /// Seek to `start_seconds` and play, reusing already-decoded PCM when available.
    ///
    /// Reuse rebuilds a player from the cached in-memory buffer (an instant,
    /// non-blocking seek) rather than calling [`rodio::Player::try_seek`], which
    /// blocks the calling thread and is therefore unusable on the WASM main thread.
    /// Returns `false` when no cached PCM matches the current source, signalling the
    /// caller to decode it fresh.
    pub fn seek_and_play(&mut self, start_seconds: f32) -> bool {
        if let Some(source_key) = self.current_audio_source.clone() {
            let cached = match &self.decoded_cache {
                Some((key, buffer)) if key == &source_key => Some(buffer.clone()),
                _ => None,
            };
            if let Some(buffer) = cached {
                if let Some(player) = self.current_player.take() {
                    player.stop();
                }
                self.play_buffer(source_key, buffer, start_seconds, true);
                return true;
            }
        }

        #[cfg(test)]
        if self.current_audio_source.is_some() {
            self.playback_started_at = Some(web_time::Instant::now());
            self.playback_start_offset_seconds = start_seconds;
            return true;
        }

        false
    }

    /// Replace the current audio source with the provided bytes, starting at the specified time, and start playback immediately.
    pub fn replace_with_bytes(&mut self, source_key: String, bytes: &[u8], start_seconds: f32) {
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
        self.current_audio_source = None;

        log::trace!("RodioAudioBackend: decoding {} bytes", bytes.len());
        match Self::decode_to_buffer(bytes) {
            Some(buffer) => {
                log::trace!("RodioAudioBackend: decoding successful");
                self.decoded_cache = Some((source_key.clone(), buffer.clone()));
                self.play_buffer(source_key, buffer, start_seconds, true);
            }
            None => {
                log::warn!("Failed to decode imported level music '{}'", source_key);
            }
        }
    }

    /// Replace the current audio source with an asset from the specified level, starting at the specified time, and start playback immediately.
    fn replace_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        start_seconds: f32,
        autoplay: bool,
    ) {
        {
            if let Some(player) = self.current_player.take() {
                player.stop();
            }
            self.current_audio_source = None;
        }

        let audio_path = format!("assets/levels/{}/{}", level_name, "audio.mp3");

        log::trace!("RodioAudioBackend: loading asset {}", audio_path);
        let audio_bytes = match eg_level::get_builtin_audio(level_name) {
            Some(bytes) => bytes.to_vec(),
            None => {
                log::warn!("Failed to read level music '{}'", audio_path);
                return;
            }
        };

        log::trace!(
            "RodioAudioBackend: decoding {} bytes from asset",
            audio_bytes.len()
        );
        match Self::decode_to_buffer(&audio_bytes) {
            Some(buffer) => {
                log::trace!("RodioAudioBackend: asset decoding successful");
                self.decoded_cache = Some((source_key.clone(), buffer.clone()));
                self.play_buffer(source_key, buffer, start_seconds, autoplay);
            }
            None => {
                log::warn!("Failed to decode level music '{}'", audio_path);
            }
        }
    }

    /// Get the current playback time in seconds, accounting for the playback speed and any offset. Returns None if no playback is active.
    pub fn playback_time_seconds(&self) -> Option<f32> {
        match self.current_player.as_ref() {
            Some(player) => {
                if player.empty() {
                    return None;
                }
            }
            None => {
                #[cfg(not(test))]
                {
                    return None;
                }
            }
        }

        let started_at = self.playback_started_at?;
        Some(
            self.playback_start_offset_seconds
                + started_at.elapsed().as_secs_f32() * self.playback_speed,
        )
    }

    /// Check if the audio backend is currently playing any audio. Returns true if playback is active, false otherwise.
    pub fn is_playing(&self) -> bool {
        if let Some(player) = self.current_player.as_ref() {
            return !player.empty();
        }
        #[cfg(test)]
        {
            return self.playback_started_at.is_some();
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    /// Set the playback speed for the current audio source. The speed is clamped between 0.25x and 2.0x. If playback is active, the current position is adjusted to account for the new speed.
    pub fn set_speed(&mut self, speed: f32) {
        let clamped_speed = speed.clamp(0.25, 2.0);
        if let Some(started_at) = self.playback_started_at {
            let elapsed_real = started_at.elapsed().as_secs_f32();
            self.playback_start_offset_seconds += elapsed_real * self.playback_speed;
            self.playback_started_at = Some(web_time::Instant::now());
        }
        self.playback_speed = clamped_speed;
        if let Some(player) = &self.current_player {
            player.set_speed(clamped_speed);
        }
    }

    /// Play a sound effect from the provided asset bytes. The sound effect will be played immediately and will not affect the current music playback.
    pub fn play_sfx(&mut self, asset_bytes: &'static [u8]) {
        self.active_sfx.retain(|player| !player.empty());

        if let Ok(decoded) = rodio::Decoder::new(Cursor::new(asset_bytes)) {
            if let Some(device) = self.ensure_device() {
                let (player, output) = rodio::Player::new();
                if cfg!(test) {
                    player.set_volume(0.0);
                }
                device.mixer().add(output);
                player.append(decoded);
                player.play();
                self.active_sfx.push(player);
            }
        }
    }

    /// Get the name of the currently selected audio backend. This may include the host and device name if available.
    pub fn backend_name(&self) -> String {
        self.backend_name.clone()
    }

    /// Start playing audio from a specific source key at the given time.
    /// If the source can be reused, it will seek to the specified time and play; otherwise, it will replace the current source with the new one.
    fn start_with_source_key_at(&mut self, source_key: String, bytes: &[u8], start_seconds: f32) {
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        if self.can_reuse_source(&source_key) && self.seek_and_play(start_seconds) {
            return;
        }

        self.replace_with_bytes(source_key, bytes, start_seconds);
    }

    /// Start playing audio from the provided bytes at the specified time. This will replace the current audio source with the new one and start playback immediately.
    pub fn start_with_bytes_at(&mut self, bytes: &[u8], start_seconds: f32) {
        self.start_with_source_key_at(runtime_bytes_source_key("audio.mp3"), bytes, start_seconds);
    }

    /// Warm up the audio system with the provided bytes at the specified time. This will replace the current audio source with the new one but will not start playback.
    pub fn warmup_with_bytes_at(&mut self, bytes: &[u8], start_seconds: f32) {
        let source_key = runtime_bytes_source_key("audio.mp3");
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        if self.can_reuse_source(&source_key) {
            return;
        }

        self.replace_with_bytes(source_key, bytes, start_seconds);
    }

    /// Start playing a preloaded asset at the specified time.
    /// If the asset can be reused, it will seek to the specified time and play; otherwise, it will replace the current source with the new one.
    pub fn start_preloaded_asset_at(&mut self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.start_with_source_key_at(
            runtime_asset_source_key(level_name, "audio.mp3"),
            bytes,
            start_seconds,
        );
    }

    /// Warm up a preloaded asset at the specified time. This will replace the current audio source with the new one but will not start playback.
    /// If the asset can be reused, it will not replace the current source.
    pub fn warmup_preloaded_asset_at(
        &mut self,
        level_name: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        if self.can_reuse_source(&source_key) {
            return;
        }

        self.replace_with_bytes(source_key, bytes, start_seconds);
    }

    /// Start playing a level asset at the specified time. This will replace the current audio source with the new one and start playback immediately.
    /// If the asset can be reused, it will seek to the specified time and play; otherwise, it will replace the current source with the new one.
    pub fn start_at(&mut self, level_name: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        if self.can_reuse_source(&source_key) && self.seek_and_play(start_seconds) {
            return;
        }

        self.replace_with_asset(source_key, level_name, start_seconds, true);
    }

    /// Warm up a level asset at the specified time. This will replace the current audio source with the new one but will not start playback.
    /// If the asset can be reused, it will not replace the current source.
    pub fn warmup_at(&mut self, level_name: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");
        let start_seconds = start_seconds.max(0.0);
        self.stop();

        if self.can_reuse_source(&source_key) {
            return;
        }

        self.replace_with_asset(source_key, level_name, start_seconds, false);
    }
}

impl Default for AudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode audio bytes to a downsampled waveform suitable for display.
/// Returns (peak_samples, sample_rate) where peak_samples contains one peak per window.
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
    use crate::audio::AudioBackend;

    use super::accumulate_interleaved_samples;
    use super::accumulate_waveform_frame_peak;
    use super::decode_audio_to_waveform;
    use super::decode_audio_to_waveform_streaming;

    use super::{available_host_labels, host_label, parse_host_id_by_label, select_host};

    #[test]
    fn parse_host_id_by_label_handles_default_and_case_insensitive_labels() {
        assert_eq!(parse_host_id_by_label(""), None);
        assert_eq!(parse_host_id_by_label("   "), None);
        assert_eq!(parse_host_id_by_label("Default"), None);
        assert_eq!(parse_host_id_by_label("default"), None);

        if let Some(host_id) = cpal::available_hosts().into_iter().next() {
            let lower = host_label(host_id).to_lowercase();
            assert_eq!(parse_host_id_by_label(&lower), Some(host_id));
        }
    }

    #[test]
    fn select_host_returns_default_or_requested_available_host() {
        let selected_default = select_host(None).id();
        assert_eq!(selected_default, cpal::default_host().id());

        if let Some(host_id) = cpal::available_hosts().into_iter().next() {
            let selected = select_host(Some(host_id)).id();
            assert_eq!(selected, host_id);
        }
    }

    #[test]
    fn available_host_labels_are_sorted_unique_and_include_default() {
        let labels = available_host_labels();
        assert!(labels.iter().any(|label| label == "Default"));

        let mut expected = labels.clone();
        expected.sort();
        expected.dedup();
        assert_eq!(labels, expected);
    }

    #[test]
    fn set_preferred_backend_name_rejects_unknown_label() {
        let mut backend = AudioBackend::new();
        let before = backend.backend_name();

        assert!(!backend.set_preferred_backend_name("definitely-not-a-real-backend"));
        assert_eq!(backend.backend_name(), before);
    }

    #[test]
    fn set_preferred_backend_name_default_resets_runtime_state() {
        let mut backend: AudioBackend = AudioBackend::new();
        let Some(host_id) = cpal::available_hosts().into_iter().next() else {
            return;
        };

        {
            backend.current_audio_source = Some("bytes:test".to_string());
            backend.playback_started_at = Some(web_time::Instant::now());
            backend.playback_start_offset_seconds = 4.0;
            backend.device_tried = true;
        }

        assert!(backend.set_preferred_backend_name(&host_label(host_id)));

        assert_eq!(backend.current_audio_source, None);
        assert_eq!(backend.playback_started_at, None);
        assert_eq!(backend.playback_start_offset_seconds, 0.0);
        assert!(!backend.device_tried);
        assert_eq!(backend.backend_name, host_label(host_id));
    }

    #[test]
    fn replace_with_bytes_invalid_payload_does_not_mark_source_reusable() {
        let mut backend = AudioBackend::new();
        backend.replace_with_bytes("bytes:bad".to_string(), &[1, 2, 3], 0.0);

        assert!(!backend.can_reuse_source("bytes:bad"));
        assert!(!backend.is_playing());
    }

    #[test]
    fn replace_with_valid_bytes_tracks_and_reuses_playback_state() {
        let mut backend = AudioBackend::new();
        let wav = super::tiny_wav_bytes();

        backend.replace_with_bytes("bytes:play".to_string(), &wav, 1.25);
        assert!(backend.can_reuse_source("bytes:play"));
        assert!(backend.is_playing());
        assert!(backend.playback_time_seconds().is_some());

        // Decoding a new key replaces the reusable source and caches its PCM.
        backend.replace_with_bytes("bytes:warm".to_string(), &wav, 0.5);
        assert!(backend.can_reuse_source("bytes:warm"));

        // Stopping clears playback tracking deterministically.
        backend.stop();
        assert_eq!(backend.playback_time_seconds(), None);
    }

    #[test]
    fn seek_and_play_reuses_cached_pcm_without_redecode() {
        let mut backend = AudioBackend::new();
        let wav = super::tiny_wav_bytes();

        backend.replace_with_bytes("bytes:song".to_string(), &wav, 0.0);
        assert!(backend.decoded_cache.is_some());
        assert!(backend.can_reuse_source("bytes:song"));

        // Same source key -> reuse the cached buffer (no re-decode, non-blocking seek).
        assert!(backend.seek_and_play(0.25));
        assert_eq!(
            backend.decoded_cache.as_ref().map(|(key, _)| key.as_str()),
            Some("bytes:song")
        );
        assert!(backend.can_reuse_source("bytes:song"));
    }

    #[test]
    fn seek_and_play_uses_test_fallback_without_active_player() {
        let mut backend = AudioBackend::new();
        {
            backend.current_audio_source = Some("bytes:reuse".to_string());
            backend.current_player = None;
        }

        assert!(backend.seek_and_play(2.5));
        assert!(backend.is_playing());
        let playback_time = backend.playback_time_seconds().unwrap_or(0.0);
        assert!(playback_time >= 2.5);
    }

    #[test]
    fn stop_and_set_speed_manage_playback_tracking() {
        use web_time::Duration;

        let mut backend = AudioBackend::new();
        {
            backend.current_audio_source = Some("bytes:speed".to_string());
            backend.playback_started_at =
                Some(web_time::Instant::now() - Duration::from_millis(10));
            backend.playback_start_offset_seconds = 1.0;
            backend.playback_speed = 2.0;
        }

        backend.set_speed(0.5);
        {
            assert_eq!(backend.playback_speed, 0.5);
            assert!(backend.playback_start_offset_seconds >= 1.0);
        }

        backend.stop();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
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
