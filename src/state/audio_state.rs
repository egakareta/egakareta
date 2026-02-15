use crate::platform::audio::PlatformAudio;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

pub(crate) type AudioImportData = (String, Vec<u8>);
pub(crate) type WaveformLoadData = (String, Option<(Vec<f32>, u32)>);

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
