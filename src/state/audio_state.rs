/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::platform::audio::{runtime_asset_source_key, PlatformAudio};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use super::State;
use crate::audio_service::{PersistentWaveformCacheEntry, WaveformLoadMessage};
use crate::platform::io::{log_platform_error, save_waveform_to_storage};
use crate::platform::services::trigger_audio_import;
use crate::platform::task::spawn_background;
use crate::types::LevelMetadata;

pub(crate) type AudioImportData = (String, Vec<u8>);
pub(crate) type RuntimeAudioPreloadData = crate::audio_service::AudioPreloadResult;
pub(crate) type WaveformLoadData = crate::audio_service::WaveformResult;

#[derive(Clone, Debug)]
pub(crate) struct WaveformCacheEntry {
    pub(crate) samples: Vec<f32>,
    pub(crate) sample_rate: u32,
    pub(crate) window_size: usize,
    pub(crate) complete: bool,
}

impl Default for WaveformCacheEntry {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            sample_rate: 0,
            window_size: crate::audio_service::WAVEFORM_WINDOW,
            complete: false,
        }
    }
}

pub(crate) struct RuntimeAudioPreloadState {
    pub(crate) preloaded_audio: HashMap<String, Vec<u8>>,
    pub(crate) preloading_source_keys: HashSet<String>,
    pub(crate) preload_channel: (
        Sender<RuntimeAudioPreloadData>,
        Receiver<RuntimeAudioPreloadData>,
    ),
    #[cfg(test)]
    pub(crate) last_warmup_request: Option<(String, f32)>,
}

pub(crate) struct EditorAudioState {
    pub(crate) local_audio_cache: HashMap<String, Vec<u8>>,
    pub(crate) audio_import_channel: (Sender<AudioImportData>, Receiver<AudioImportData>),
    pub(crate) waveform_load_channel: (Sender<WaveformLoadData>, Receiver<WaveformLoadData>),
    pub(crate) waveform_cache: HashMap<String, WaveformCacheEntry>,
    pub(crate) waveform_loading_source: Option<String>,
}

pub(crate) struct AudioState {
    pub(crate) runtime: PlatformAudio,
    pub(crate) runtime_preload: RuntimeAudioPreloadState,
    pub(crate) editor: EditorAudioState,
}

pub(crate) struct AudioSubsystem {
    pub(crate) state: AudioState,
}

impl AudioState {
    pub(crate) fn new(local_audio_cache: HashMap<String, Vec<u8>>) -> Self {
        Self {
            runtime: PlatformAudio::new(),
            runtime_preload: RuntimeAudioPreloadState {
                preloaded_audio: HashMap::new(),
                preloading_source_keys: HashSet::new(),
                preload_channel: std::sync::mpsc::channel(),
                #[cfg(test)]
                last_warmup_request: None,
            },
            editor: EditorAudioState {
                local_audio_cache,
                audio_import_channel: std::sync::mpsc::channel(),
                waveform_load_channel: std::sync::mpsc::channel(),
                waveform_cache: HashMap::new(),
                waveform_loading_source: None,
            },
        }
    }

    pub(crate) fn preload_runtime_audio(&mut self, level_name: &str, music_source: &str) {
        let source_key = runtime_asset_source_key(level_name, music_source);
        if self
            .runtime_preload
            .preloaded_audio
            .contains_key(&source_key)
            || self
                .runtime_preload
                .preloading_source_keys
                .contains(&source_key)
        {
            return;
        }

        self.runtime_preload
            .preloading_source_keys
            .insert(source_key.clone());

        crate::audio_service::start_audio_preload(
            source_key,
            level_name.to_string(),
            music_source.to_string(),
            self.runtime_preload.preload_channel.0.clone(),
        );
    }
}

impl State {
    pub(crate) fn preload_runtime_audio(&mut self, level_name: &str, music_source: &str) {
        self.audio
            .state
            .preload_runtime_audio(level_name, music_source);
    }

    pub(crate) fn stop_audio(&mut self) {
        self.audio.state.runtime.stop();
    }

    pub(crate) fn resume_audio(&mut self) {
        self.audio.state.runtime.resume();
    }

    pub(crate) fn start_audio(&mut self, level_name: &str, metadata: &LevelMetadata) {
        self.start_audio_at_seconds(level_name, metadata, 0.0);
    }

    pub(crate) fn start_audio_at_seconds(
        &mut self,
        level_name: &str,
        metadata: &LevelMetadata,
        start_seconds: f32,
    ) {
        let source_key = runtime_asset_source_key(level_name, &metadata.music.source);
        self.update_runtime_audio_preloads();

        if self.phase == crate::types::AppPhase::Editor || self.session.playtesting_editor {
            if let Some(bytes) = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&metadata.music.source)
            {
                self.audio.state.runtime.start_with_bytes_at(
                    &metadata.music.source,
                    bytes,
                    start_seconds,
                );
                return;
            }
        }

        if let Some(bytes) = self
            .audio
            .state
            .runtime_preload
            .preloaded_audio
            .get(&source_key)
        {
            self.audio.state.runtime.start_preloaded_asset_at(
                level_name,
                &metadata.music.source,
                bytes,
                start_seconds,
            );
        } else {
            self.audio
                .state
                .runtime
                .start_at(level_name, &metadata.music.source, start_seconds);
        }
    }

    pub(crate) fn warmup_audio_at_seconds(
        &mut self,
        level_name: &str,
        metadata: &LevelMetadata,
        start_seconds: f32,
    ) {
        let source_key = runtime_asset_source_key(level_name, &metadata.music.source);

        self.update_runtime_audio_preloads();

        if self.phase == crate::types::AppPhase::Editor || self.session.playtesting_editor {
            if let Some(bytes) = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&metadata.music.source)
            {
                self.audio.state.runtime.warmup_with_bytes_at(
                    &metadata.music.source,
                    bytes,
                    start_seconds,
                );
                #[cfg(test)]
                {
                    self.audio.state.runtime_preload.last_warmup_request =
                        Some((source_key, start_seconds.max(0.0)));
                }
                return;
            }
        }

        if let Some(bytes) = self
            .audio
            .state
            .runtime_preload
            .preloaded_audio
            .get(&source_key)
        {
            self.audio.state.runtime.warmup_preloaded_asset_at(
                level_name,
                &metadata.music.source,
                bytes,
                start_seconds,
            );
        } else {
            self.audio
                .state
                .runtime
                .warmup_at(level_name, &metadata.music.source, start_seconds);
        }

        #[cfg(test)]
        {
            self.audio.state.runtime_preload.last_warmup_request =
                Some((source_key, start_seconds.max(0.0)));
        }
    }

    pub(crate) fn trigger_audio_import(&self) {
        trigger_audio_import(self.audio.state.editor.audio_import_channel.0.clone());
    }

    pub(crate) fn update_audio_imports(&mut self) {
        while let Ok((filename, bytes)) = self.audio.state.editor.audio_import_channel.1.try_recv()
        {
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let source_key = runtime_asset_source_key(&level_name, &filename);

            self.session.editor_music_metadata.source = filename.clone();
            self.audio
                .state
                .editor
                .local_audio_cache
                .insert(filename, bytes);
            self.audio.state.editor.waveform_cache.remove(&source_key);
            self.audio.state.editor.waveform_loading_source = None;
            self.load_waveform_for_current_audio();
        }
    }

    pub(crate) fn update_runtime_audio_preloads(&mut self) {
        while let Ok((source_key, bytes)) = self
            .audio
            .state
            .runtime_preload
            .preload_channel
            .1
            .try_recv()
        {
            self.audio
                .state
                .runtime_preload
                .preloading_source_keys
                .remove(&source_key);

            if let Some(bytes) = bytes {
                self.audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .insert(source_key, bytes);
            }
        }
    }

    pub(crate) fn update_waveform_loading(&mut self) {
        while let Ok((source, level_name, message)) =
            self.audio.state.editor.waveform_load_channel.1.try_recv()
        {
            let source_key = runtime_asset_source_key(&level_name, &source);
            let is_current = source == self.session.editor_music_metadata.source
                && self.session.editor_level_name.as_deref() == Some(&level_name);

            match message {
                WaveformLoadMessage::Started { window_size } => {
                    self.audio.state.editor.waveform_cache.insert(
                        source_key.clone(),
                        WaveformCacheEntry {
                            window_size,
                            ..WaveformCacheEntry::default()
                        },
                    );

                    if is_current {
                        self.editor.timing.waveform_samples.clear();
                        self.editor.timing.waveform_sample_rate = 0;
                        self.editor.timing.waveform_window_size = window_size;
                        self.editor.timing.waveform_loading = true;
                        self.editor.timing.waveform_complete = false;
                    }
                }
                WaveformLoadMessage::Chunk {
                    start_peak,
                    peaks,
                    sample_rate,
                    window_size,
                } => {
                    let entry = self
                        .audio
                        .state
                        .editor
                        .waveform_cache
                        .entry(source_key.clone())
                        .or_default();
                    entry.sample_rate = sample_rate;
                    entry.window_size = window_size;
                    entry.complete = false;
                    apply_waveform_chunk(&mut entry.samples, start_peak, peaks);

                    if is_current {
                        self.editor.timing.waveform_samples = entry.samples.clone();
                        self.editor.timing.waveform_sample_rate = sample_rate;
                        self.editor.timing.waveform_window_size = window_size;
                        self.editor.timing.waveform_loading = true;
                        self.editor.timing.waveform_complete = false;
                    }
                }
                WaveformLoadMessage::Cached {
                    samples,
                    sample_rate,
                    window_size,
                    bytes,
                } => {
                    self.audio.state.editor.waveform_cache.insert(
                        source_key.clone(),
                        WaveformCacheEntry {
                            samples,
                            sample_rate,
                            window_size,
                            complete: true,
                        },
                    );

                    if is_current {
                        if let Some(entry) = self.audio.state.editor.waveform_cache.get(&source_key)
                        {
                            self.editor.timing.waveform_samples = entry.samples.clone();
                            self.editor.timing.waveform_sample_rate = entry.sample_rate;
                            self.editor.timing.waveform_window_size = entry.window_size;
                        }
                        self.editor.timing.waveform_loading = false;
                        self.editor.timing.waveform_complete = true;
                        self.audio.state.editor.waveform_loading_source = None;
                    }

                    if let Some(bytes) = bytes {
                        self.audio
                            .state
                            .runtime_preload
                            .preloaded_audio
                            .insert(source_key.clone(), bytes);
                    }
                }
                WaveformLoadMessage::Finished {
                    sample_rate,
                    window_size,
                    total_peaks,
                    cache_key,
                    bytes,
                } => {
                    let entry = self
                        .audio
                        .state
                        .editor
                        .waveform_cache
                        .entry(source_key.clone())
                        .or_default();
                    entry.sample_rate = sample_rate;
                    entry.window_size = window_size;
                    entry.complete = true;
                    entry.samples.truncate(total_peaks);

                    if let Some(cache_key) = cache_key {
                        persist_waveform_cache_entry(cache_key, entry);
                    }

                    if is_current {
                        self.editor.timing.waveform_samples = entry.samples.clone();
                        self.editor.timing.waveform_sample_rate = sample_rate;
                        self.editor.timing.waveform_window_size = window_size;
                        self.editor.timing.waveform_loading = false;
                        self.editor.timing.waveform_complete = true;
                    }

                    if let Some(bytes) = bytes {
                        // We cache builtin bytes in runtime_preload instead of local_audio_cache
                        // to avoid filename collisions (e.g., both Flowerfield and Golden Haze using "audio.mp3")
                        self.audio
                            .state
                            .runtime_preload
                            .preloaded_audio
                            .insert(source_key.clone(), bytes);
                    }

                    if is_current
                        && self.audio.state.editor.waveform_loading_source.as_deref()
                            == Some(source_key.as_str())
                    {
                        self.audio.state.editor.waveform_loading_source = None;
                    }
                }
                WaveformLoadMessage::Failed { bytes } => {
                    self.audio.state.editor.waveform_cache.remove(&source_key);

                    if is_current {
                        self.editor.timing.waveform_samples.clear();
                        self.editor.timing.waveform_sample_rate = 0;
                        self.editor.timing.waveform_loading = false;
                        self.editor.timing.waveform_complete = false;
                    }

                    if let Some(bytes) = bytes {
                        self.audio
                            .state
                            .runtime_preload
                            .preloaded_audio
                            .insert(source_key.clone(), bytes);
                    }

                    if is_current
                        && self.audio.state.editor.waveform_loading_source.as_deref()
                            == Some(source_key.as_str())
                    {
                        self.audio.state.editor.waveform_loading_source = None;
                    }
                }
            }
        }
    }

    pub(crate) fn load_waveform_for_current_audio(&mut self) {
        let music_source = self.session.editor_music_metadata.source.clone();
        let level_name = self
            .session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let source_key = runtime_asset_source_key(&level_name, &music_source);

        if let Some(entry) = self.audio.state.editor.waveform_cache.get(&source_key) {
            self.editor.timing.waveform_samples = entry.samples.clone();
            self.editor.timing.waveform_sample_rate = entry.sample_rate;
            self.editor.timing.waveform_window_size = entry.window_size;
            self.editor.timing.waveform_loading = !entry.complete;
            self.editor.timing.waveform_complete = entry.complete;
            if entry.complete {
                self.audio.state.editor.waveform_loading_source = None;
            }
            return;
        }

        if self.audio.state.editor.waveform_loading_source.as_deref() == Some(source_key.as_str()) {
            return;
        }

        self.audio.state.editor.waveform_loading_source = Some(source_key.clone());
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;
        self.editor.timing.waveform_window_size = crate::audio_service::WAVEFORM_WINDOW;
        self.editor.timing.waveform_loading = true;
        self.editor.timing.waveform_complete = false;

        let cached_bytes = self
            .audio
            .state
            .editor
            .local_audio_cache
            .get(&music_source)
            .cloned()
            .or_else(|| {
                self.audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .get(&source_key)
                    .cloned()
            });
        let sender = self.audio.state.editor.waveform_load_channel.0.clone();

        crate::audio_service::start_waveform_loading(
            music_source,
            level_name,
            cached_bytes,
            sender,
        );
    }
}

fn apply_waveform_chunk(samples: &mut Vec<f32>, start_peak: usize, peaks: Vec<f32>) {
    if samples.len() < start_peak {
        samples.resize(start_peak, 0.0);
    }

    let end_peak = start_peak + peaks.len();
    if samples.len() < end_peak {
        samples.resize(end_peak, 0.0);
    }

    samples[start_peak..end_peak].copy_from_slice(&peaks);
}

fn persist_waveform_cache_entry(cache_key: String, entry: &WaveformCacheEntry) {
    let cached = PersistentWaveformCacheEntry::new(
        entry.samples.clone(),
        entry.sample_rate,
        entry.window_size,
    );

    spawn_background(async move {
        if let Err(error) = save_waveform_to_storage(&cache_key, &cached).await {
            log_platform_error(&format!("Failed to save waveform cache: {error}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{AudioState, WaveformCacheEntry};
    use crate::audio_service::WaveformLoadMessage;
    use crate::platform::audio::runtime_asset_source_key;
    use crate::state::State;
    use std::collections::HashMap;

    async fn new_editor_state() -> State {
        let mut state = State::new_test().await;
        state.enter_editor_phase("Test Level".to_string());
        state
    }

    #[test]
    fn queues_runtime_audio_preload_by_level_and_source() {
        let mut state = AudioState::new(HashMap::new());
        state.preload_runtime_audio("Flowerfield", "music.mp3");

        assert!(state
            .runtime_preload
            .preloading_source_keys
            .contains(&runtime_asset_source_key("Flowerfield", "music.mp3")));
    }

    #[test]
    fn preload_runtime_audio_deduplicates_and_skips_preloaded_entries() {
        let mut state = AudioState::new(HashMap::new());
        let preload_key = runtime_asset_source_key("Flowerfield", "music.mp3");
        state.preload_runtime_audio("Flowerfield", "music.mp3");
        state.preload_runtime_audio("Flowerfield", "music.mp3");

        assert_eq!(state.runtime_preload.preloading_source_keys.len(), 1);
        assert!(state
            .runtime_preload
            .preloading_source_keys
            .contains(&preload_key));

        let already_loaded = runtime_asset_source_key("Golden Haze", "audio.mp3");
        state
            .runtime_preload
            .preloaded_audio
            .insert(already_loaded.clone(), vec![1, 2, 3]);
        state.preload_runtime_audio("Golden Haze", "audio.mp3");
        assert!(!state
            .runtime_preload
            .preloading_source_keys
            .contains(&already_loaded));
    }

    #[test]
    fn update_runtime_audio_preloads_moves_channel_results_into_cache() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            let loaded_key = runtime_asset_source_key("Flowerfield", "audio.mp3");
            let missing_key = runtime_asset_source_key("Flowerfield", "missing.mp3");

            state
                .audio
                .state
                .runtime_preload
                .preloading_source_keys
                .insert(loaded_key.clone());
            state
                .audio
                .state
                .runtime_preload
                .preloading_source_keys
                .insert(missing_key.clone());

            state
                .audio
                .state
                .runtime_preload
                .preload_channel
                .0
                .send((loaded_key.clone(), Some(vec![9, 8, 7])))
                .expect("loaded preload send should succeed");
            state
                .audio
                .state
                .runtime_preload
                .preload_channel
                .0
                .send((missing_key.clone(), None))
                .expect("missing preload send should succeed");

            state.update_runtime_audio_preloads();

            assert!(state
                .audio
                .state
                .runtime_preload
                .preloaded_audio
                .contains_key(&loaded_key));
            assert!(!state
                .audio
                .state
                .runtime_preload
                .preloaded_audio
                .contains_key(&missing_key));
            assert!(!state
                .audio
                .state
                .runtime_preload
                .preloading_source_keys
                .contains(&loaded_key));
            assert!(!state
                .audio
                .state
                .runtime_preload
                .preloading_source_keys
                .contains(&missing_key));
        });
    }

    #[test]
    fn update_waveform_loading_applies_current_source_and_caches_bytes() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "song.mp3".to_string();

            let source_key = runtime_asset_source_key("Test Level", "song.mp3");
            state.audio.state.editor.waveform_loading_source = Some(source_key.clone());

            state
                .audio
                .state
                .editor
                .waveform_load_channel
                .0
                .send((
                    "song.mp3".to_string(),
                    "Test Level".to_string(),
                    WaveformLoadMessage::Chunk {
                        start_peak: 0,
                        peaks: vec![0.25, 0.75],
                        sample_rate: 44_100,
                        window_size: crate::audio_service::WAVEFORM_WINDOW,
                    },
                ))
                .expect("waveform send should succeed");
            state
                .audio
                .state
                .editor
                .waveform_load_channel
                .0
                .send((
                    "song.mp3".to_string(),
                    "Test Level".to_string(),
                    WaveformLoadMessage::Finished {
                        sample_rate: 44_100,
                        window_size: crate::audio_service::WAVEFORM_WINDOW,
                        total_peaks: 2,
                        cache_key: None,
                        bytes: Some(vec![4, 5, 6]),
                    },
                ))
                .expect("waveform finish send should succeed");

            state.update_waveform_loading();

            assert_eq!(state.editor.timing.waveform_samples, vec![0.25, 0.75]);
            assert_eq!(state.editor.timing.waveform_sample_rate, 44_100);
            assert!(state.editor.timing.waveform_complete);
            assert_eq!(state.audio.state.editor.waveform_loading_source, None);

            let cached = state
                .audio
                .state
                .editor
                .waveform_cache
                .get(&source_key)
                .expect("decoded waveform should be cached");
            assert_eq!(cached.samples, vec![0.25, 0.75]);
            assert_eq!(cached.sample_rate, 44_100);
            assert!(cached.complete);

            assert_eq!(
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .get(&source_key),
                Some(&vec![4, 5, 6])
            );
        });
    }

    #[test]
    fn update_waveform_loading_applies_persistent_cache_hit() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "song.mp3".to_string();

            let source_key = runtime_asset_source_key("Test Level", "song.mp3");
            state.audio.state.editor.waveform_loading_source = Some(source_key.clone());

            state
                .audio
                .state
                .editor
                .waveform_load_channel
                .0
                .send((
                    "song.mp3".to_string(),
                    "Test Level".to_string(),
                    WaveformLoadMessage::Cached {
                        samples: vec![0.15, 0.45, 0.9],
                        sample_rate: 44_100,
                        window_size: crate::audio_service::WAVEFORM_WINDOW,
                        bytes: Some(vec![7, 8, 9]),
                    },
                ))
                .expect("cached waveform send should succeed");

            state.update_waveform_loading();

            assert_eq!(state.editor.timing.waveform_samples, vec![0.15, 0.45, 0.9]);
            assert_eq!(state.editor.timing.waveform_sample_rate, 44_100);
            assert!(state.editor.timing.waveform_complete);
            assert!(!state.editor.timing.waveform_loading);
            assert_eq!(state.audio.state.editor.waveform_loading_source, None);

            let cached = state
                .audio
                .state
                .editor
                .waveform_cache
                .get(&source_key)
                .expect("persistent cache hit should be cached in memory");
            assert!(cached.complete);
            assert_eq!(cached.samples, vec![0.15, 0.45, 0.9]);
            assert_eq!(
                state
                    .audio
                    .state
                    .runtime_preload
                    .preloaded_audio
                    .get(&source_key),
                Some(&vec![7, 8, 9])
            );
        });
    }

    #[test]
    fn update_waveform_loading_keeps_timing_for_stale_source_but_still_caches() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "current.mp3".to_string();
            state.editor.timing.waveform_samples = vec![1.0];
            state.editor.timing.waveform_sample_rate = 22_050;

            let stale_key = runtime_asset_source_key("Test Level", "stale.mp3");
            state.audio.state.editor.waveform_loading_source = Some(stale_key.clone());

            state
                .audio
                .state
                .editor
                .waveform_load_channel
                .0
                .send((
                    "stale.mp3".to_string(),
                    "Test Level".to_string(),
                    WaveformLoadMessage::Chunk {
                        start_peak: 0,
                        peaks: vec![0.1, 0.2],
                        sample_rate: 48_000,
                        window_size: crate::audio_service::WAVEFORM_WINDOW,
                    },
                ))
                .expect("stale waveform send should succeed");

            state.update_waveform_loading();

            assert_eq!(state.editor.timing.waveform_samples, vec![1.0]);
            assert_eq!(state.editor.timing.waveform_sample_rate, 22_050);
            assert!(state
                .audio
                .state
                .editor
                .waveform_cache
                .contains_key(&stale_key));
            assert_eq!(
                state.audio.state.editor.waveform_loading_source,
                Some(stale_key)
            );
        });
    }

    #[test]
    fn update_waveform_loading_clears_timing_when_current_decode_fails() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "broken.mp3".to_string();
            state.editor.timing.waveform_samples = vec![0.5, 0.6];
            state.editor.timing.waveform_sample_rate = 44_100;

            let key = runtime_asset_source_key("Test Level", "broken.mp3");
            state.audio.state.editor.waveform_loading_source = Some(key.clone());

            state
                .audio
                .state
                .editor
                .waveform_load_channel
                .0
                .send((
                    "broken.mp3".to_string(),
                    "Test Level".to_string(),
                    WaveformLoadMessage::Failed { bytes: None },
                ))
                .expect("failed waveform send should succeed");

            state.update_waveform_loading();

            assert!(state.editor.timing.waveform_samples.is_empty());
            assert_eq!(state.editor.timing.waveform_sample_rate, 0);
            assert_eq!(state.audio.state.editor.waveform_loading_source, None);
        });
    }

    #[test]
    fn load_waveform_for_current_audio_prefers_cache_and_short_circuits_inflight_requests() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "song.mp3".to_string();
            let source_key = runtime_asset_source_key("Test Level", "song.mp3");

            state.audio.state.editor.waveform_cache.insert(
                source_key.clone(),
                WaveformCacheEntry {
                    samples: vec![0.3, 0.9],
                    sample_rate: 32_000,
                    window_size: crate::audio_service::WAVEFORM_WINDOW,
                    complete: true,
                },
            );
            state.audio.state.editor.waveform_loading_source = Some("other:key".to_string());
            state.load_waveform_for_current_audio();
            assert_eq!(state.editor.timing.waveform_samples, vec![0.3, 0.9]);
            assert_eq!(state.editor.timing.waveform_sample_rate, 32_000);
            assert_eq!(state.audio.state.editor.waveform_loading_source, None);

            state.audio.state.editor.waveform_cache.remove(&source_key);
            state.editor.timing.waveform_samples = vec![7.0];
            state.editor.timing.waveform_sample_rate = 7;
            state.audio.state.editor.waveform_loading_source = Some(source_key.clone());
            state.load_waveform_for_current_audio();
            assert_eq!(state.editor.timing.waveform_samples, vec![7.0]);
            assert_eq!(state.editor.timing.waveform_sample_rate, 7);
        });
    }

    #[test]
    fn load_waveform_for_current_audio_marks_loading_and_clears_timing_when_uncached() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "uncached.mp3".to_string();

            let source_key = runtime_asset_source_key("Test Level", "uncached.mp3");
            state.editor.timing.waveform_samples = vec![1.0, 2.0];
            state.editor.timing.waveform_sample_rate = 11_025;
            state.audio.state.editor.waveform_loading_source = None;

            state.load_waveform_for_current_audio();

            assert!(state.editor.timing.waveform_samples.is_empty());
            assert_eq!(state.editor.timing.waveform_sample_rate, 0);
            assert_eq!(
                state.audio.state.editor.waveform_loading_source,
                Some(source_key)
            );
        });
    }

    #[test]
    fn update_audio_imports_updates_source_cache_and_invalidates_waveform_cache() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.session.editor_level_name = Some("Test Level".to_string());
            state.session.editor_music_metadata.source = "old.mp3".to_string();

            let imported_name = "imported.mp3".to_string();
            let source_key = runtime_asset_source_key("Test Level", &imported_name);
            state.audio.state.editor.waveform_cache.insert(
                source_key.clone(),
                WaveformCacheEntry {
                    samples: vec![0.5],
                    sample_rate: 8_000,
                    window_size: crate::audio_service::WAVEFORM_WINDOW,
                    complete: true,
                },
            );
            state.audio.state.editor.waveform_loading_source = Some("some:old:key".to_string());

            state
                .audio
                .state
                .editor
                .audio_import_channel
                .0
                .send((imported_name.clone(), vec![1, 2, 3, 4]))
                .expect("audio import send should succeed");

            state.update_audio_imports();

            assert_eq!(state.session.editor_music_metadata.source, imported_name);
            assert_eq!(
                state
                    .audio
                    .state
                    .editor
                    .local_audio_cache
                    .get("imported.mp3"),
                Some(&vec![1, 2, 3, 4])
            );
            assert!(!state
                .audio
                .state
                .editor
                .waveform_cache
                .contains_key(&source_key));
            assert_eq!(
                state.audio.state.editor.waveform_loading_source,
                Some(source_key)
            );
        });
    }
}
