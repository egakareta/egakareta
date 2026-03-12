pub(crate) trait AudioBackend {
    fn stop(&mut self);
    fn can_reuse_source(&self, source_key: &str) -> bool;
    fn seek_and_play(&mut self, start_seconds: f32) -> bool;
    fn replace_with_bytes(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    );
    fn replace_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
    );
    fn playback_time_seconds(&self) -> Option<f32>;
    fn is_playing(&self) -> bool;
    fn set_speed(&mut self, speed: f32);
    fn play_sfx(&mut self, asset_path: &str);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn create_audio_backend() -> Box<dyn AudioBackend> {
    Box::new(WebAudioBackend::new())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn create_audio_backend() -> Box<dyn AudioBackend> {
    Box::new(NativeAudioBackend::new())
}

#[cfg(target_arch = "wasm32")]
struct WebAudioBackend {
    current_audio: Option<web_sys::HtmlAudioElement>,
    current_audio_source: Option<String>,
    current_blob_url: Option<gloo_file::ObjectUrl>,
    playback_speed: f32,
    sfx_pool: std::collections::HashMap<String, Vec<web_sys::HtmlAudioElement>>,
}

#[cfg(target_arch = "wasm32")]
impl WebAudioBackend {
    fn new() -> Self {
        Self {
            current_audio: None,
            current_audio_source: None,
            current_blob_url: None,
            playback_speed: 1.0,
            sfx_pool: std::collections::HashMap::new(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl AudioBackend for WebAudioBackend {
    fn stop(&mut self) {
        if let Some(audio) = &self.current_audio {
            let _ = audio.pause();
        }
    }

    fn can_reuse_source(&self, source_key: &str) -> bool {
        self.current_audio_source.as_deref() == Some(source_key)
    }

    fn seek_and_play(&mut self, start_seconds: f32) -> bool {
        if let Some(audio) = &self.current_audio {
            audio.set_current_time(start_seconds as f64);
            audio.set_playback_rate(self.playback_speed as f64);
            let _ = audio.play();
            true
        } else {
            false
        }
    }

    fn replace_with_bytes(
        &mut self,
        source_key: String,
        _music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        let blob = gloo_file::Blob::new(bytes);
        let url = gloo_file::ObjectUrl::from(blob);

        match web_sys::HtmlAudioElement::new_with_src(&url) {
            Ok(audio) => {
                audio.set_current_time(start_seconds as f64);
                audio.set_playback_rate(self.playback_speed as f64);
                let _ = audio.play();
                self.current_audio = Some(audio);
                self.current_audio_source = Some(source_key);
                self.current_blob_url = Some(url);
            }
            Err(err) => {
                gloo_console::error!("Failed to create HTML audio element: {:?}", err);
            }
        }
    }

    fn replace_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
    ) {
        self.current_blob_url = None;
        let audio_url = format!("assets/levels/{}/{}", level_name, music_source);

        if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
            audio.set_current_time(start_seconds as f64);
            audio.set_playback_rate(self.playback_speed as f64);
            let _ = audio.play();
            self.current_audio = Some(audio);
            self.current_audio_source = Some(source_key);
        }
    }

    fn playback_time_seconds(&self) -> Option<f32> {
        self.current_audio
            .as_ref()
            .map(|audio| audio.current_time() as f32)
    }

    fn is_playing(&self) -> bool {
        if let Some(audio) = &self.current_audio {
            !audio.paused() && !audio.ended()
        } else {
            false
        }
    }

    fn set_speed(&mut self, speed: f32) {
        self.playback_speed = speed;
        if let Some(audio) = &self.current_audio {
            audio.set_playback_rate(speed as f64);
        }
    }

    fn play_sfx(&mut self, asset_path: &str) {
        let pool = self.sfx_pool.entry(asset_path.to_string()).or_default();
        if let Some(audio) = pool.iter().find(|a| a.ended() || a.paused()) {
            audio.set_current_time(0.0);
            let _ = audio.play();
        } else if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(asset_path) {
            let _ = audio.play();
            pool.push(audio);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct NativeAudioBackend {
    _output_device: Option<rodio::MixerDeviceSink>,
    current_player: Option<rodio::Player>,
    current_audio_source: Option<String>,
    playback_started_at: Option<std::time::Instant>,
    playback_start_offset_seconds: f32,
    playback_speed: f32,
    active_sfx: Vec<rodio::Player>,
    sfx_cache: std::collections::HashMap<String, Vec<u8>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeAudioBackend {
    fn new() -> Self {
        let output_device =
            match rodio::DeviceSinkBuilder::from_default_device().and_then(|b| b.open_stream()) {
                Ok(device) => Some(device),
                Err(err) => {
                    log::warn!("Failed to initialize native audio output: {}", err);
                    None
                }
            };

        Self {
            _output_device: output_device,
            current_player: None,
            current_audio_source: None,
            playback_started_at: None,
            playback_start_offset_seconds: 0.0,
            playback_speed: 1.0,
            active_sfx: Vec::new(),
            sfx_cache: std::collections::HashMap::new(),
        }
    }

    fn set_new_sink(
        &mut self,
        source_key: String,
        decoded: rodio::Decoder<std::io::Cursor<Vec<u8>>>,
        start_seconds: f32,
        context: &str,
    ) {
        use std::time::Duration;

        let Some(device) = &self._output_device else {
            return;
        };

        let (player, output) = rodio::Player::new();
        device.mixer().add(output);

        player.append(decoded);
        if let Err(err) = player.try_seek(Duration::from_secs_f32(start_seconds)) {
            log::warn!(
                "Failed to seek audio '{}' to {:.3}s: {:?}",
                context,
                start_seconds,
                err
            );
        }

        player.set_speed(self.playback_speed);
        player.play();
        self.current_player = Some(player);
        self.current_audio_source = Some(source_key);
        self.playback_started_at = Some(std::time::Instant::now());
        self.playback_start_offset_seconds = start_seconds;
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioBackend for NativeAudioBackend {
    fn stop(&mut self) {
        if let Some(player) = &self.current_player {
            player.pause();
        }
        self.playback_started_at = None;
        self.playback_start_offset_seconds = 0.0;
    }

    fn can_reuse_source(&self, source_key: &str) -> bool {
        self.current_audio_source.as_deref() == Some(source_key)
    }

    fn seek_and_play(&mut self, start_seconds: f32) -> bool {
        use std::time::Duration;

        if let Some(player) = &self.current_player {
            if let Err(err) = player.try_seek(Duration::from_secs_f32(start_seconds)) {
                log::warn!(
                    "Failed to seek reused audio to {:.3}s: {:?}",
                    start_seconds,
                    err
                );
                return false;
            }
            player.set_speed(self.playback_speed);
            player.play();
            self.playback_started_at = Some(std::time::Instant::now());
            self.playback_start_offset_seconds = start_seconds;
            true
        } else {
            false
        }
    }

    fn replace_with_bytes(
        &mut self,
        source_key: String,
        music_source: &str,
        bytes: &[u8],
        start_seconds: f32,
    ) {
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
        self.current_audio_source = None;

        match rodio::Decoder::new(std::io::Cursor::new(bytes.to_vec())) {
            Ok(decoded) => {
                let context = format!("imported audio '{}'", music_source);
                self.set_new_sink(source_key, decoded, start_seconds, &context);
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

    fn replace_with_asset(
        &mut self,
        source_key: String,
        level_name: &str,
        music_source: &str,
        start_seconds: f32,
    ) {
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
        self.current_audio_source = None;

        let audio_path = format!("assets/levels/{}/{}", level_name, music_source);
        let audio_bytes = match std::fs::read(&audio_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                log::warn!("Failed to read level music '{}': {}", audio_path, err);
                return;
            }
        };

        match rodio::Decoder::new(std::io::Cursor::new(audio_bytes)) {
            Ok(decoded) => {
                self.set_new_sink(source_key, decoded, start_seconds, &audio_path);
            }
            Err(err) => {
                log::warn!("Failed to decode level music '{}': {}", audio_path, err);
            }
        }
    }

    fn playback_time_seconds(&self) -> Option<f32> {
        let player = self.current_player.as_ref()?;
        if player.empty() {
            return None;
        }

        let started_at = self.playback_started_at?;
        Some(
            self.playback_start_offset_seconds
                + started_at.elapsed().as_secs_f32() * self.playback_speed,
        )
    }

    fn is_playing(&self) -> bool {
        self.current_player
            .as_ref()
            .map(|player| !player.empty())
            .unwrap_or(false)
    }

    fn set_speed(&mut self, speed: f32) {
        if let Some(started_at) = self.playback_started_at {
            let elapsed_real = started_at.elapsed().as_secs_f32();
            self.playback_start_offset_seconds += elapsed_real * self.playback_speed;
            self.playback_started_at = Some(std::time::Instant::now());
        }
        self.playback_speed = speed;
        if let Some(player) = &self.current_player {
            player.set_speed(speed);
        }
    }

    fn play_sfx(&mut self, asset_path: &str) {
        self.active_sfx.retain(|player| !player.empty());

        if let Some(device) = &self._output_device {
            let bytes = if let Some(cached) = self.sfx_cache.get(asset_path) {
                cached.clone()
            } else if let Ok(b) = std::fs::read(asset_path) {
                self.sfx_cache.insert(asset_path.to_string(), b.clone());
                b
            } else {
                return;
            };

            if let Ok(decoded) = rodio::Decoder::new(std::io::Cursor::new(bytes)) {
                let (player, output) = rodio::Player::new();
                device.mixer().add(output);
                player.append(decoded);
                player.play();
                self.active_sfx.push(player);
            }
        }
    }
}
