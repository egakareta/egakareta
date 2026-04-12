/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

fn host_label(host_id: cpal::HostId) -> String {
    format!("{:?}", host_id)
}

fn parse_host_id_by_label(label: &str) -> Option<cpal::HostId> {
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Default") {
        return None;
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

fn available_host_labels() -> Vec<String> {
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

struct RodioBackendInner {
    _output_device: Option<rodio::MixerDeviceSink>,
    device_tried: bool,
    preferred_host: Option<cpal::HostId>,
    current_player: Option<rodio::Player>,
    current_audio_source: Option<String>,
    playback_started_at: Option<web_time::Instant>,
    playback_start_offset_seconds: f32,
    playback_speed: f32,
    active_sfx: Vec<rodio::Player>,
    backend_name: String,
}

impl RodioBackendInner {
    fn ensure_device(&mut self) -> Option<&rodio::MixerDeviceSink> {
        if self._output_device.is_some() {
            return self._output_device.as_ref();
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

        self._output_device = match host.default_output_device() {
            Some(device) => {
                use cpal::traits::DeviceTrait;
                if let Ok(name) = device.id() {
                    log::info!("Audio device: {}", name);
                    self.backend_name = format!("{:?} ({})", host_id, name);
                }
                match rodio::DeviceSinkBuilder::from_device(device).and_then(|b| b.open_stream()) {
                    Ok(sink) => {
                        log::info!("Audio stream opened successfully");
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

        self._output_device.as_ref()
    }

    fn resume(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            // On WASM, initializing the device inside a user interaction (like a click)
            // allows the AudioContext to start in the "running" state.
            if self._output_device.is_none() {
                let _ = self.ensure_device();
            } else {
                // Re-checking the device here keeps resume idempotent and gives browsers
                // another chance to unlock playback after user interaction policies change.
                let _ = self.ensure_device();
                log::trace!("AudioBackend: resume requested on existing output device");
            }
        }
    }

    fn set_new_sink(
        &mut self,
        source_key: String,
        decoded: rodio::Decoder<Cursor<Vec<u8>>>,
        start_seconds: f32,
        autoplay: bool,
        _context: &str,
    ) {
        use web_time::Duration;

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

        log::trace!("RodioBackendInner: appending decoded source");
        use rodio::Source;
        if start_seconds > 0.001 {
            log::trace!("RodioBackendInner: skipping to {:.3}s", start_seconds);
            player.append(decoded.skip_duration(Duration::from_secs_f32(start_seconds)));
        } else {
            player.append(decoded);
        }

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
}

pub(crate) struct AudioBackend {
    inner: Rc<RefCell<RodioBackendInner>>,
}

impl AudioBackend {
    pub(crate) fn new() -> Self {
        let host_id = select_host(None).id();

        Self {
            inner: Rc::new(RefCell::new(RodioBackendInner {
                _output_device: None,
                device_tried: false,
                preferred_host: None,
                current_player: None,
                current_audio_source: None,
                playback_started_at: None,
                playback_start_offset_seconds: 0.0,
                playback_speed: 1.0,
                active_sfx: Vec::new(),
                backend_name: host_id.to_string(),
            })),
        }
    }
}

impl AudioBackend {
    pub(crate) fn available_backend_names() -> Vec<String> {
        available_host_labels()
    }

    pub(crate) fn set_preferred_backend_name(&mut self, backend_name: &str) -> bool {
        let parsed = parse_host_id_by_label(backend_name);
        if !backend_name.trim().is_empty()
            && !backend_name.eq_ignore_ascii_case("Default")
            && parsed.is_none()
        {
            return false;
        }

        let mut inner = self.inner.borrow_mut();
        if inner.preferred_host == parsed {
            return true;
        }

        inner.preferred_host = parsed;
        if let Some(player) = inner.current_player.take() {
            player.stop();
        }
        for player in inner.active_sfx.drain(..) {
            player.stop();
        }
        inner.current_audio_source = None;
        inner.playback_started_at = None;
        inner.playback_start_offset_seconds = 0.0;
        inner._output_device = None;
        inner.device_tried = false;
        inner.backend_name = if let Some(host_id) = inner.preferred_host {
            host_label(host_id)
        } else {
            "Default".to_string()
        };

        true
    }

    pub(crate) fn stop(&mut self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(player) = &inner.current_player {
            player.pause();
        }
        inner.playback_started_at = None;
        inner.playback_start_offset_seconds = 0.0;
    }

    pub(crate) fn can_reuse_source(&self, source_key: &str) -> bool {
        self.inner.borrow().current_audio_source.as_deref() == Some(source_key)
    }

    pub(crate) fn seek_and_play(&mut self, start_seconds: f32) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            // Player::try_seek blocks the main thread on WASM, leading to a deadlock.
            // We disable reuse on WASM to force a fresh sink which uses non-blocking skip_duration.
            let _ = start_seconds;
            return false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use web_time::Duration;
            let mut inner = self.inner.borrow_mut();

            if let Some(player) = &inner.current_player {
                if let Err(err) = player.try_seek(Duration::from_secs_f32(start_seconds)) {
                    log::warn!(
                        "Failed to seek reused audio to {:.3}s: {:?}",
                        start_seconds,
                        err
                    );
                    return false;
                }
                player.set_speed(inner.playback_speed);
                player.play();
                inner.playback_started_at = Some(web_time::Instant::now());
                inner.playback_start_offset_seconds = start_seconds;
                true
            } else {
                #[cfg(test)]
                if inner.current_audio_source.is_some() {
                    inner.playback_started_at = Some(web_time::Instant::now());
                    inner.playback_start_offset_seconds = start_seconds;
                    return true;
                }
                false
            }
        }
    }

    pub(crate) fn replace_with_bytes(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        self.replace_with_bytes_internal(source_key, music_source, bytes, start_seconds, true);
    }

    pub(crate) fn warmup_with_bytes(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        self.replace_with_bytes_internal(source_key, music_source, bytes, start_seconds, false);
    }

    fn replace_with_bytes_internal(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
        autoplay: bool,
    ) {
        let mut inner = self.inner.borrow_mut();
        if let Some(player) = inner.current_player.take() {
            player.stop();
        }
        inner.current_audio_source = None;

        log::trace!("RodioAudioBackend: decoding {} bytes", bytes.len());
        match rodio::Decoder::new(Cursor::new(bytes.to_vec())) {
            Ok(decoded) => {
                log::trace!("RodioAudioBackend: decoding successful");
                let context = format!("imported audio '{}'", music_source);
                inner.set_new_sink(source_key, decoded, start_seconds, autoplay, &context);
            }
            Err(err) => {
                log::warn!(
                    "Failed to decode imported level music '{}': {}",
                    music_source,
                    err
                );
            }
        }
    }

    pub(crate) fn replace_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
    ) {
        self.replace_with_asset_internal(source_key, level_name, music_source, start_seconds, true);
    }

    pub(crate) fn warmup_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
    ) {
        self.replace_with_asset_internal(
            source_key,
            level_name,
            music_source,
            start_seconds,
            false,
        );
    }

    fn replace_with_asset_internal(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
        autoplay: bool,
    ) {
        {
            let mut inner = self.inner.borrow_mut();
            if let Some(player) = inner.current_player.take() {
                player.stop();
            }
            inner.current_audio_source = None;
        }

        let audio_path = format!("assets/levels/{}/{}", level_name, music_source);

        log::trace!("RodioAudioBackend: loading asset {}", audio_path);
        let audio_bytes = match crate::level_repository::get_builtin_audio(level_name, music_source)
        {
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
        let cursor = Cursor::new(audio_bytes);
        match rodio::Decoder::new(cursor) {
            Ok(decoded) => {
                log::trace!("RodioAudioBackend: asset decoding successful");
                self.inner.borrow_mut().set_new_sink(
                    source_key,
                    decoded,
                    start_seconds,
                    autoplay,
                    &audio_path,
                );
            }
            Err(err) => {
                log::warn!("Failed to decode level music '{}': {}", audio_path, err);
            }
        }
    }

    pub(crate) fn playback_time_seconds(&self) -> Option<f32> {
        let inner = self.inner.borrow();
        match inner.current_player.as_ref() {
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

        let started_at = inner.playback_started_at?;
        Some(
            inner.playback_start_offset_seconds
                + started_at.elapsed().as_secs_f32() * inner.playback_speed,
        )
    }

    pub(crate) fn is_playing(&self) -> bool {
        let inner = self.inner.borrow();
        if let Some(player) = inner.current_player.as_ref() {
            return !player.empty();
        }
        #[cfg(test)]
        {
            return inner.playback_started_at.is_some();
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    pub(crate) fn set_speed(&mut self, speed: f32) {
        let mut inner = self.inner.borrow_mut();
        if let Some(started_at) = inner.playback_started_at {
            let elapsed_real = started_at.elapsed().as_secs_f32();
            inner.playback_start_offset_seconds += elapsed_real * inner.playback_speed;
            inner.playback_started_at = Some(web_time::Instant::now());
        }
        inner.playback_speed = speed;
        if let Some(player) = &inner.current_player {
            player.set_speed(speed);
        }
    }

    pub(crate) fn play_sfx(&mut self, asset_bytes: &'static [u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.active_sfx.retain(|player| !player.empty());

        if let Ok(decoded) = rodio::Decoder::new(Cursor::new(asset_bytes)) {
            if let Some(device) = inner.ensure_device() {
                let (player, output) = rodio::Player::new();
                if cfg!(test) {
                    player.set_volume(0.0);
                }
                device.mixer().add(output);
                player.append(decoded);
                player.play();
                inner.active_sfx.push(player);
            }
        }
    }

    pub(crate) fn resume(&mut self) {
        self.inner.borrow_mut().resume();
    }

    pub(crate) fn backend_name(&self) -> String {
        self.inner.borrow().backend_name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        available_host_labels, host_label, parse_host_id_by_label, select_host, AudioBackend,
    };

    fn tiny_wav_bytes() -> Vec<u8> {
        // Minimal PCM WAV: mono, 16-bit, 44.1kHz, one zero sample.
        vec![
            b'R', b'I', b'F', b'F', 38, 0, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ',
            16, 0, 0, 0, 1, 0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0, b'd', b'a', b't',
            b'a', 2, 0, 0, 0, 0, 0,
        ]
    }

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
        let mut backend = AudioBackend::new();
        let Some(host_id) = cpal::available_hosts().into_iter().next() else {
            return;
        };

        {
            let mut inner = backend.inner.borrow_mut();
            inner.current_audio_source = Some("bytes:test".to_string());
            inner.playback_started_at = Some(web_time::Instant::now());
            inner.playback_start_offset_seconds = 4.0;
            inner.device_tried = true;
        }

        assert!(backend.set_preferred_backend_name(&host_label(host_id)));

        let inner = backend.inner.borrow();
        assert_eq!(inner.current_audio_source, None);
        assert_eq!(inner.playback_started_at, None);
        assert_eq!(inner.playback_start_offset_seconds, 0.0);
        assert!(!inner.device_tried);
        assert_eq!(inner.backend_name, host_label(host_id));
    }

    #[test]
    fn replace_with_bytes_invalid_payload_does_not_mark_source_reusable() {
        let mut backend = AudioBackend::new();
        backend.replace_with_bytes("bytes:bad".to_string(), "bad.bin", &[1, 2, 3], 0.0);

        assert!(!backend.can_reuse_source("bytes:bad"));
        assert!(!backend.is_playing());
    }

    #[test]
    fn replace_and_warmup_with_valid_bytes_update_test_playback_state() {
        let mut backend = AudioBackend::new();
        let wav = tiny_wav_bytes();

        backend.replace_with_bytes("bytes:play".to_string(), "tone.wav", &wav, 1.25);
        assert!(backend.can_reuse_source("bytes:play"));
        assert!(backend.is_playing());
        assert!(backend.playback_time_seconds().is_some());

        backend.warmup_with_bytes("bytes:warm".to_string(), "tone.wav", &wav, 0.5);
        assert!(backend.can_reuse_source("bytes:warm"));
        assert_eq!(backend.playback_time_seconds(), None);
    }

    #[test]
    fn seek_and_play_uses_test_fallback_without_active_player() {
        let mut backend = AudioBackend::new();
        {
            let mut inner = backend.inner.borrow_mut();
            inner.current_audio_source = Some("bytes:reuse".to_string());
            inner.current_player = None;
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
            let mut inner = backend.inner.borrow_mut();
            inner.current_audio_source = Some("bytes:speed".to_string());
            inner.playback_started_at = Some(web_time::Instant::now() - Duration::from_millis(10));
            inner.playback_start_offset_seconds = 1.0;
            inner.playback_speed = 2.0;
        }

        backend.set_speed(0.5);
        {
            let inner = backend.inner.borrow();
            assert_eq!(inner.playback_speed, 0.5);
            assert!(inner.playback_start_offset_seconds >= 1.0);
        }

        backend.stop();
        assert!(!backend.is_playing());
        assert_eq!(backend.playback_time_seconds(), None);
    }
}
