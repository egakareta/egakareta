use crate::platform::audio::{runtime_asset_source_key, PlatformAudio};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use super::State;
use crate::platform::services::trigger_audio_import;
use crate::types::LevelMetadata;

pub(crate) type AudioImportData = (String, Vec<u8>);
pub(crate) type RuntimeAudioPreloadData = crate::audio_service::AudioPreloadResult;
pub(crate) type WaveformLoadData = crate::audio_service::WaveformResult;

pub(crate) struct RuntimeAudioPreloadState {
    pub(crate) preloaded_audio: HashMap<String, Vec<u8>>,
    pub(crate) preloading_source_keys: HashSet<String>,
    pub(crate) preload_channel: (
        Sender<RuntimeAudioPreloadData>,
        Receiver<RuntimeAudioPreloadData>,
    ),
}

pub(crate) struct EditorAudioState {
    pub(crate) local_audio_cache: HashMap<String, Vec<u8>>,
    pub(crate) audio_import_channel: (Sender<AudioImportData>, Receiver<AudioImportData>),
    pub(crate) waveform_load_channel: (Sender<WaveformLoadData>, Receiver<WaveformLoadData>),
    pub(crate) waveform_cache: HashMap<String, (Vec<f32>, u32)>,
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
        let editor_bytes =
            if self.phase == crate::types::AppPhase::Editor || self.session.playtesting_editor {
                self.audio
                    .state
                    .editor
                    .local_audio_cache
                    .get(&metadata.music.source)
                    .cloned()
            } else {
                None
            };

        self.update_runtime_audio_preloads();

        if let Some(bytes) = editor_bytes {
            self.audio.state.runtime.start_with_bytes_at(
                &metadata.music.source,
                &bytes,
                start_seconds,
            );
        } else if let Some(bytes) = self
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
        while let Ok((source, level_name, decoded, bytes)) =
            self.audio.state.editor.waveform_load_channel.1.try_recv()
        {
            let source_key = runtime_asset_source_key(&level_name, &source);

            if let Some((samples, sample_rate)) = decoded {
                self.audio
                    .state
                    .editor
                    .waveform_cache
                    .insert(source_key.clone(), (samples.clone(), sample_rate));

                if source != self.session.editor_music_metadata.source
                    || self.session.editor_level_name.as_deref() != Some(&level_name)
                {
                    continue;
                }

                self.editor.timing.waveform_samples = samples;
                self.editor.timing.waveform_sample_rate = sample_rate;
            } else {
                if source != self.session.editor_music_metadata.source
                    || self.session.editor_level_name.as_deref() != Some(&level_name)
                {
                    continue;
                }

                self.editor.timing.waveform_samples.clear();
                self.editor.timing.waveform_sample_rate = 0;
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

            if self.audio.state.editor.waveform_loading_source.as_deref()
                == Some(source_key.as_str())
            {
                self.audio.state.editor.waveform_loading_source = None;
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

        if let Some((samples, sample_rate)) =
            self.audio.state.editor.waveform_cache.get(&source_key)
        {
            self.editor.timing.waveform_samples = samples.clone();
            self.editor.timing.waveform_sample_rate = *sample_rate;
            self.audio.state.editor.waveform_loading_source = None;
            return;
        }

        if self.audio.state.editor.waveform_loading_source.as_deref() == Some(source_key.as_str()) {
            return;
        }

        self.audio.state.editor.waveform_loading_source = Some(source_key.clone());
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;

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

#[cfg(test)]
mod tests {
    use super::AudioState;
    use crate::platform::audio::runtime_asset_source_key;
    use std::collections::HashMap;

    #[test]
    fn queues_runtime_audio_preload_by_level_and_source() {
        let mut state = AudioState::new(HashMap::new());
        state.preload_runtime_audio("Flowerfield", "music.mp3");

        assert!(state
            .runtime_preload
            .preloading_source_keys
            .contains(&runtime_asset_source_key("Flowerfield", "music.mp3")));
    }
}
