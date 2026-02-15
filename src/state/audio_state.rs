use crate::platform::audio::PlatformAudio;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

use super::State;
use crate::platform::services::trigger_audio_import;
use crate::types::LevelMetadata;

pub(crate) type AudioImportData = (String, Vec<u8>);
pub(crate) type WaveformLoadData = (String, Option<(Vec<f32>, u32)>, Option<Vec<u8>>);

pub(crate) struct EditorAudioState {
    pub(crate) local_audio_cache: HashMap<String, Vec<u8>>,
    pub(crate) audio_import_channel: (Sender<AudioImportData>, Receiver<AudioImportData>),
    pub(crate) waveform_load_channel: (Sender<WaveformLoadData>, Receiver<WaveformLoadData>),
    pub(crate) waveform_cache: HashMap<String, (Vec<f32>, u32)>,
    pub(crate) waveform_loading_source: Option<String>,
}

pub(crate) struct AudioState {
    pub(crate) runtime: PlatformAudio,
    pub(crate) editor: EditorAudioState,
}

pub(crate) struct AudioSubsystem {
    pub(crate) state: AudioState,
}

impl AudioState {
    pub(crate) fn new(local_audio_cache: HashMap<String, Vec<u8>>) -> Self {
        Self {
            runtime: PlatformAudio::new(),
            editor: EditorAudioState {
                local_audio_cache,
                audio_import_channel: std::sync::mpsc::channel(),
                waveform_load_channel: std::sync::mpsc::channel(),
                waveform_cache: HashMap::new(),
                waveform_loading_source: None,
            },
        }
    }
}

impl State {
    pub(crate) fn stop_audio(&mut self) {
        self.audio.state.runtime.stop();
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
            self.session.editor_music_metadata.source = filename.clone();
            self.audio
                .state
                .editor
                .local_audio_cache
                .insert(filename, bytes);
            self.audio
                .state
                .editor
                .waveform_cache
                .remove(&self.session.editor_music_metadata.source);
            self.audio.state.editor.waveform_loading_source = None;
            self.load_waveform_for_current_audio();
        }
    }

    pub(crate) fn update_waveform_loading(&mut self) {
        while let Ok((source, decoded, bytes)) =
            self.audio.state.editor.waveform_load_channel.1.try_recv()
        {
            if let Some((samples, sample_rate)) = decoded {
                self.audio
                    .state
                    .editor
                    .waveform_cache
                    .insert(source.clone(), (samples.clone(), sample_rate));

                if source != self.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples = samples;
                self.editor.timing.waveform_sample_rate = sample_rate;
            } else {
                if source != self.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples.clear();
                self.editor.timing.waveform_sample_rate = 0;
            }

            if let Some(bytes) = bytes {
                self.audio
                    .state
                    .editor
                    .local_audio_cache
                    .insert(source.clone(), bytes);
            }

            if self.audio.state.editor.waveform_loading_source.as_deref() == Some(source.as_str()) {
                self.audio.state.editor.waveform_loading_source = None;
            }
        }
    }

    pub(crate) fn load_waveform_for_current_audio(&mut self) {
        let music_source = self.session.editor_music_metadata.source.clone();

        if let Some((samples, sample_rate)) =
            self.audio.state.editor.waveform_cache.get(&music_source)
        {
            self.editor.timing.waveform_samples = samples.clone();
            self.editor.timing.waveform_sample_rate = *sample_rate;
            self.audio.state.editor.waveform_loading_source = None;
            return;
        }

        if self.audio.state.editor.waveform_loading_source.as_deref() == Some(music_source.as_str())
        {
            return;
        }

        self.audio.state.editor.waveform_loading_source = Some(music_source.clone());
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;

        let level_name = self
            .session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let cached_bytes = self
            .audio
            .state
            .editor
            .local_audio_cache
            .get(&music_source)
            .cloned();
        let sender = self.audio.state.editor.waveform_load_channel.0.clone();

        crate::audio_service::start_waveform_loading(
            music_source,
            level_name,
            cached_bytes,
            sender,
        );
    }
}
