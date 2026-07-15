/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy_ecs::prelude::Resource;
use eg_audio::audio::{AudioBackend, runtime_asset_source_key};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use super::State;
use eg_audio::service::{
    AudioByteCache, AudioPreloadResult, EditorAudioCache, WaveformCacheEntry, WaveformResult,
};
use eg_platform::io::save_waveform_to_storage;
use eg_platform::task::spawn_background;
use eg_types::PersistentWaveformCacheEntry;

// REFACTOR POINT(full-bevy): `AudioSubsystem` becomes Bevy audio resources or
// stays as a custom resource behind Bevy systems; direct `State` polling goes away.

/// Authoritative platform audio backend shared with the App world while the
/// remaining playback callers are removed from the compatibility `State`.
#[derive(Resource, Clone)]
pub struct AudioBackendRes(Arc<Mutex<AudioBackend>>);

impl Default for AudioBackendRes {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(AudioBackend::new())))
    }
}

impl AudioBackendRes {
    fn with_mut<R>(&self, f: impl FnOnce(&mut AudioBackend) -> R) -> R {
        f(&mut self.0.lock().expect("audio backend mutex poisoned"))
    }

    pub fn set_preferred_backend_name(&self, backend_name: &str) -> bool {
        self.with_mut(|backend| backend.set_preferred_backend_name(backend_name))
    }

    pub fn stop(&self) {
        self.with_mut(AudioBackend::stop);
    }

    pub fn pause(&self) {
        self.with_mut(AudioBackend::pause);
    }

    pub fn resume(&self) {
        self.with_mut(AudioBackend::resume);
    }

    pub fn ensure_device(&self) {
        self.with_mut(|backend| {
            backend.ensure_device();
        });
    }

    pub fn playback_time_seconds(&self) -> Option<f32> {
        self.with_mut(|backend| backend.playback_time_seconds())
    }

    pub fn is_playing(&self) -> bool {
        self.with_mut(|backend| backend.is_playing())
    }

    pub fn set_speed(&self, speed: f32) {
        self.with_mut(|backend| backend.set_speed(speed));
    }

    pub fn play_sfx(&self, asset_bytes: &'static [u8]) {
        self.with_mut(|backend| backend.play_sfx(asset_bytes));
    }

    pub fn start_with_bytes_at(&self, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| backend.start_with_bytes_at(bytes, start_seconds));
    }

    pub fn warmup_with_bytes_at(&self, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| backend.warmup_with_bytes_at(bytes, start_seconds));
    }

    pub fn start_preloaded_asset_at(&self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| {
            backend.start_preloaded_asset_at(level_name, bytes, start_seconds);
        });
    }

    pub fn warmup_preloaded_asset_at(&self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| {
            backend.warmup_preloaded_asset_at(level_name, bytes, start_seconds);
        });
    }

    pub fn start_at(&self, level_name: &str, start_seconds: f32) {
        self.with_mut(|backend| backend.start_at(level_name, start_seconds));
    }

    pub fn warmup_at(&self, level_name: &str, start_seconds: f32) {
        self.with_mut(|backend| backend.warmup_at(level_name, start_seconds));
    }
}

pub(crate) fn start_audio_source_at_seconds_for_resources(
    is_editor: bool,
    playtesting_editor: bool,
    level_name: &str,
    start_seconds: f32,
    audio: &AudioBackendRes,
    editor_audio: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
) {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");

    if (is_editor || playtesting_editor)
        && let Some(bytes) = editor_audio.bytes_for_level_audio(level_name)
    {
        audio.start_with_bytes_at(&bytes, start_seconds);
        return;
    }

    if let Some(bytes) = runtime_preload.bytes(&source_key) {
        audio.start_preloaded_asset_at(level_name, &bytes, start_seconds);
    } else {
        audio.start_at(level_name, start_seconds);
    }
}

/// Receiver end of the audio-preload channel. Production frames drain
/// this through `StateSchedule`. See [`AuthChannelReceiver`] in
/// `state::auth_state` for the shared-receiver rationale.
pub type AudioPreloadChannelReceiver =
    std::sync::Arc<std::sync::Mutex<Receiver<AudioPreloadResult>>>;

#[derive(Default)]
struct RuntimeAudioPreloadCache {
    preloaded_audio: AudioByteCache,
    preloading_source_keys: HashSet<String>,
}

#[derive(Resource, Clone, Default)]
pub struct RuntimeAudioPreloadRes(Arc<Mutex<RuntimeAudioPreloadCache>>);

impl RuntimeAudioPreloadRes {
    pub fn contains_preloaded(&self, source_key: &str) -> bool {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .contains_key(source_key)
    }

    pub fn contains_preloading(&self, source_key: &str) -> bool {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .contains(source_key)
    }

    pub fn mark_preloading(&self, source_key: String) {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .insert(source_key);
    }

    pub fn apply_result(&self, source_key: String, bytes: Option<Vec<u8>>) {
        let mut cache = self
            .0
            .lock()
            .expect("runtime audio preload cache mutex poisoned");
        cache.preloading_source_keys.remove(&source_key);
        if let Some(bytes) = bytes {
            cache.preloaded_audio.insert(source_key, bytes);
        }
    }

    pub fn insert_preloaded(&self, source_key: String, bytes: Vec<u8>) {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .insert(source_key, bytes);
    }

    pub fn bytes(&self, source_key: &str) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .get(source_key)
            .cloned()
    }

    #[cfg(test)]
    pub fn preloading_len(&self) -> usize {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .len()
    }
}

#[derive(Resource, Clone)]
pub struct RuntimeAudioPreloadSenderRes(pub Sender<AudioPreloadResult>);

impl Default for RuntimeAudioPreloadSenderRes {
    fn default() -> Self {
        let (sender, _receiver) = std::sync::mpsc::channel();
        Self(sender)
    }
}

pub struct RuntimeAudioPreloadState {
    pub cache: RuntimeAudioPreloadRes,
    pub preload_channel: (Sender<AudioPreloadResult>, AudioPreloadChannelReceiver),
    #[cfg(test)]
    pub last_warmup_request: Option<(String, f32)>,
}

/// Receiver end of the audio-import channel. Production frames drain this
/// through `StateSchedule`.
pub type AudioImportChannelReceiver = std::sync::Arc<std::sync::Mutex<Receiver<(String, Vec<u8>)>>>;

pub type AudioImportChannel = (Sender<(String, Vec<u8>)>, AudioImportChannelReceiver);

/// Receiver end of the waveform-load channel. Production frames drain this
/// through `StateSchedule`.
pub type WaveformLoadChannelReceiver = std::sync::Arc<std::sync::Mutex<Receiver<WaveformResult>>>;

#[derive(Resource, Clone)]
pub struct WaveformLoadSenderRes(pub Sender<WaveformResult>);

impl Default for WaveformLoadSenderRes {
    fn default() -> Self {
        let (sender, _receiver) = std::sync::mpsc::channel();
        Self(sender)
    }
}

#[derive(Resource, Clone, Default)]
pub struct EditorAudioCacheRes(Arc<Mutex<EditorAudioCache>>);

impl EditorAudioCacheRes {
    pub fn from_local_files(local_file_bytes: HashMap<String, Vec<u8>>) -> Self {
        Self(Arc::new(Mutex::new(EditorAudioCache::from_local_files(
            local_file_bytes,
        ))))
    }

    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(&mut EditorAudioCache) -> R) -> R {
        f(&mut self.0.lock().expect("editor audio cache mutex poisoned"))
    }

    pub fn insert_level_audio(&self, level_name: &str, bytes: Vec<u8>) -> String {
        self.with_mut(|cache| cache.insert_level_audio(level_name, bytes))
    }

    pub fn bytes_for_level_audio(&self, level_name: &str) -> Option<Vec<u8>> {
        self.with_mut(|cache| cache.bytes_for_level_audio(level_name).cloned())
    }

    pub fn bytes_for_source(&self, source_key: &str) -> Option<Vec<u8>> {
        self.with_mut(|cache| cache.bytes_for_source(source_key).cloned())
    }

    pub fn waveform(&self, source_key: &str) -> Option<WaveformCacheEntry> {
        self.with_mut(|cache| cache.waveform(source_key).cloned())
    }

    pub fn insert_waveform(&self, source_key: String, entry: WaveformCacheEntry) {
        self.with_mut(|cache| cache.insert_waveform(source_key, entry));
    }

    pub fn remove_waveform(&self, source_key: &str) {
        self.with_mut(|cache| cache.remove_waveform(source_key));
    }

    pub fn contains_waveform(&self, source_key: &str) -> bool {
        self.with_mut(|cache| cache.contains_waveform(source_key))
    }

    pub fn loading_source(&self) -> Option<String> {
        self.with_mut(|cache| cache.loading_source().map(str::to_owned))
    }

    pub fn set_loading_source(&self, source_key: Option<String>) {
        self.with_mut(|cache| cache.set_loading_source(source_key));
    }

    pub fn clear_loading_source_if(&self, source_key: &str) {
        self.with_mut(|cache| cache.clear_loading_source_if(source_key));
    }
}

pub struct EditorAudioState {
    pub cache: EditorAudioCacheRes,
    pub audio_import_channel: AudioImportChannel,
    pub waveform_load_channel: (Sender<WaveformResult>, WaveformLoadChannelReceiver),
}

pub struct AudioState {
    pub runtime: AudioBackendRes,
    pub runtime_preload: RuntimeAudioPreloadState,
    pub editor: EditorAudioState,
}

pub struct AudioSubsystem {
    pub state: AudioState,
}

impl AudioState {
    pub fn new(local_audio_cache: HashMap<String, Vec<u8>>) -> Self {
        let (preload_tx, preload_rx) = std::sync::mpsc::channel::<AudioPreloadResult>();
        let (audio_tx, audio_rx) = std::sync::mpsc::channel::<(String, Vec<u8>)>();
        let (waveform_tx, waveform_rx) = std::sync::mpsc::channel::<WaveformResult>();
        Self {
            runtime: AudioBackendRes::default(),
            runtime_preload: RuntimeAudioPreloadState {
                cache: RuntimeAudioPreloadRes::default(),
                preload_channel: (preload_tx, arc_receiver(preload_rx)),
                #[cfg(test)]
                last_warmup_request: None,
            },
            editor: EditorAudioState {
                cache: EditorAudioCacheRes::from_local_files(local_audio_cache),
                audio_import_channel: (audio_tx, arc_receiver(audio_rx)),
                waveform_load_channel: (waveform_tx, arc_receiver(waveform_rx)),
            },
        }
    }

    pub fn preload_runtime_audio(&mut self, level_name: &str, music_source: &str) {
        let source_key = runtime_asset_source_key(level_name, music_source);
        if self.runtime_preload.cache.contains_preloaded(&source_key)
            || self.runtime_preload.cache.contains_preloading(&source_key)
        {
            return;
        }

        self.runtime_preload
            .cache
            .mark_preloading(source_key.clone());

        #[cfg(not(test))]
        {
            eg_audio::service::start_audio_preload(
                source_key,
                level_name.to_string(),
                self.runtime_preload.preload_channel.0.clone(),
            );
        }
    }
}

fn arc_receiver<T>(rx: Receiver<T>) -> std::sync::Arc<std::sync::Mutex<Receiver<T>>> {
    std::sync::Arc::new(std::sync::Mutex::new(rx))
}

impl Default for RuntimeAudioPreloadState {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            cache: RuntimeAudioPreloadRes::default(),
            preload_channel: (tx, arc_receiver(rx)),
            #[cfg(test)]
            last_warmup_request: None,
        }
    }
}

impl Default for EditorAudioState {
    fn default() -> Self {
        let (audio_tx, audio_rx) = std::sync::mpsc::channel();
        let (waveform_tx, waveform_rx) = std::sync::mpsc::channel();
        Self {
            cache: EditorAudioCacheRes::default(),
            audio_import_channel: (audio_tx, arc_receiver(audio_rx)),
            waveform_load_channel: (waveform_tx, arc_receiver(waveform_rx)),
        }
    }
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            runtime: AudioBackendRes::default(),
            runtime_preload: RuntimeAudioPreloadState::default(),
            editor: EditorAudioState::default(),
        }
    }
}

impl State {
    pub fn preload_runtime_audio(&mut self, level_name: &str, audio_source: &str) {
        self.audio
            .state
            .preload_runtime_audio(level_name, audio_source);
    }

    pub fn stop_audio(&mut self) {
        self.audio.state.runtime.stop();
    }

    pub fn pause_game_audio(&mut self) {
        self.audio.state.runtime.pause();
    }

    pub fn resume_game_audio(&mut self) {
        self.audio.state.runtime.resume();
    }

    pub(super) fn start_audio_source_at_seconds(&mut self, level_name: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");

        if (self.phase == eg_types::AppPhase::Editor || self.session.playtesting_editor)
            && let Some(bytes) = self
                .audio
                .state
                .editor
                .cache
                .bytes_for_level_audio(level_name)
        {
            self.audio
                .state
                .runtime
                .start_with_bytes_at(&bytes, start_seconds);
            return;
        }

        if let Some(bytes) = self.audio.state.runtime_preload.cache.bytes(&source_key) {
            self.audio
                .state
                .runtime
                .start_preloaded_asset_at(level_name, &bytes, start_seconds);
        } else {
            self.audio.state.runtime.start_at(level_name, start_seconds);
        }
    }

    pub fn warmup_audio_at_seconds(&mut self, level_name: &str, start_seconds: f32) {
        let source_key = runtime_asset_source_key(level_name, "audio.mp3");

        #[cfg(test)]
        {
            self.audio.state.runtime_preload.last_warmup_request =
                Some((source_key, start_seconds.max(0.0)));
        }

        #[cfg(not(test))]
        {
            if (self.phase == eg_types::AppPhase::Editor || self.session.playtesting_editor)
                && let Some(bytes) = self
                    .audio
                    .state
                    .editor
                    .cache
                    .bytes_for_level_audio(level_name)
            {
                self.audio
                    .state
                    .runtime
                    .warmup_with_bytes_at(&bytes, start_seconds);
                return;
            }

            if let Some(bytes) = self.audio.state.runtime_preload.cache.bytes(&source_key) {
                self.audio.state.runtime.warmup_preloaded_asset_at(
                    level_name,
                    &bytes,
                    start_seconds,
                );
            } else {
                self.audio
                    .state
                    .runtime
                    .warmup_at(level_name, start_seconds);
            }
        }
    }

    pub fn load_waveform_for_current_audio(&mut self) {
        let level_name = self
            .session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let source_key = runtime_asset_source_key(&level_name, "audio.mp3");

        if let Some(entry) = self.audio.state.editor.cache.waveform(&source_key) {
            self.editor.timing.waveform_samples = entry.samples.clone();
            self.editor.timing.waveform_sample_rate = entry.sample_rate;
            self.editor.timing.waveform_window_size = entry.window_size;
            self.editor.timing.waveform_loading = !entry.complete;
            self.editor.timing.waveform_complete = entry.complete;
            if entry.complete {
                self.audio
                    .state
                    .editor
                    .cache
                    .clear_loading_source_if(&source_key);
            }
            return;
        }

        if self.audio.state.editor.cache.loading_source().as_deref() == Some(source_key.as_str()) {
            return;
        }

        self.audio
            .state
            .editor
            .cache
            .set_loading_source(Some(source_key.clone()));
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;
        self.editor.timing.waveform_window_size = eg_audio::service::WAVEFORM_WINDOW;
        self.editor.timing.waveform_loading = true;
        self.editor.timing.waveform_complete = false;

        let cached_bytes = self
            .audio
            .state
            .editor
            .cache
            .bytes_for_level_audio(&level_name)
            .or_else(|| self.audio.state.runtime_preload.cache.bytes(&source_key));
        let sender = self.audio.state.editor.waveform_load_channel.0.clone();

        eg_audio::service::start_waveform_loading(level_name, cached_bytes, sender);
    }
}

/// Persists a waveform completed by the App-world async result handler.
pub(crate) fn persist_waveform_cache_entry(cache_key: String, entry: &WaveformCacheEntry) {
    let cached = PersistentWaveformCacheEntry::new(
        entry.samples.clone(),
        entry.sample_rate,
        entry.window_size,
    );

    spawn_background(async move {
        if let Err(error) = save_waveform_to_storage(&cache_key, &cached).await {
            log::error!("Failed to save waveform cache: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{AudioState, WaveformCacheEntry};
    use crate::state::State;
    use eg_audio::audio::runtime_asset_source_key;
    use std::collections::HashMap;

    async fn new_editor_state() -> State {
        let mut state = State::new_test().await;
        state.enter_editor_phase("Test Level".to_string());
        state
    }

    #[test]
    fn queues_runtime_audio_preload_by_level_and_source() {
        let mut state = AudioState::new(HashMap::new());
        state.preload_runtime_audio("Ambition", "music.mp3");

        assert!(
            state
                .runtime_preload
                .cache
                .contains_preloading(&runtime_asset_source_key("Ambition", "music.mp3"))
        );
    }

    #[test]
    fn preload_runtime_audio_deduplicates_and_skips_preloaded_entries() {
        let mut state = AudioState::new(HashMap::new());
        let preload_key = runtime_asset_source_key("Ambition", "music.mp3");
        state.preload_runtime_audio("Ambition", "music.mp3");
        state.preload_runtime_audio("Ambition", "music.mp3");

        assert_eq!(state.runtime_preload.cache.preloading_len(), 1);
        assert!(
            state
                .runtime_preload
                .cache
                .contains_preloading(&preload_key)
        );

        let already_loaded = runtime_asset_source_key("Golden Haze", "audio.mp3");
        state
            .runtime_preload
            .cache
            .insert_preloaded(already_loaded.clone(), vec![1, 2, 3]);
        state.preload_runtime_audio("Golden Haze", "audio.mp3");
        assert!(
            !state
                .runtime_preload
                .cache
                .contains_preloading(&already_loaded)
        );
    }

    #[tokio::test]
    async fn load_waveform_for_current_audio_prefers_cache_and_short_circuits_inflight_requests() {
        let mut state = new_editor_state().await;
        state.session.editor_level_name = Some("Test Level".to_string());
        let source_key = runtime_asset_source_key("Test Level", "audio.mp3");

        state.audio.state.editor.cache.insert_waveform(
            source_key.clone(),
            WaveformCacheEntry {
                samples: vec![0.3, 0.9],
                sample_rate: 32_000,
                window_size: eg_audio::service::WAVEFORM_WINDOW,
                complete: true,
            },
        );
        state
            .audio
            .state
            .editor
            .cache
            .set_loading_source(Some("other:key".to_string()));
        state.load_waveform_for_current_audio();
        assert_eq!(state.editor.timing.waveform_samples, vec![0.3, 0.9]);
        assert_eq!(state.editor.timing.waveform_sample_rate, 32_000);
        assert_eq!(
            state.audio.state.editor.cache.loading_source(),
            Some("other:key".to_string())
        );

        state.audio.state.editor.cache.remove_waveform(&source_key);
        state.editor.timing.waveform_samples = vec![7.0];
        state.editor.timing.waveform_sample_rate = 7;
        state
            .audio
            .state
            .editor
            .cache
            .set_loading_source(Some(source_key.clone()));
        state.load_waveform_for_current_audio();
        assert_eq!(state.editor.timing.waveform_samples, vec![7.0]);
        assert_eq!(state.editor.timing.waveform_sample_rate, 7);
    }

    #[tokio::test]
    async fn load_waveform_for_current_audio_marks_loading_and_clears_timing_when_uncached() {
        let mut state = new_editor_state().await;
        state.session.editor_level_name = Some("Test Level".to_string());

        let source_key = runtime_asset_source_key("Test Level", "audio.mp3");
        state.editor.timing.waveform_samples = vec![1.0, 2.0];
        state.editor.timing.waveform_sample_rate = 11_025;
        state.audio.state.editor.cache.set_loading_source(None);

        state.load_waveform_for_current_audio();

        assert!(state.editor.timing.waveform_samples.is_empty());
        assert_eq!(state.editor.timing.waveform_sample_rate, 0);
        assert_eq!(
            state.audio.state.editor.cache.loading_source(),
            Some(source_key.clone())
        );
    }
}
