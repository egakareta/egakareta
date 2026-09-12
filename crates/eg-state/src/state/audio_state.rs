/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Application audio scene and editor audio caches.
//!
//! One typed [`Soundscape`] owns every sound: music (whose source is swapped
//! per level) plus static one-shot effects. Required sounds cannot be
//! removed, so lookups by [`AppSound`] key are total. Waveform decoding,
//! preload channels, and editor byte caches live here too; waveform work is
//! independent of scene ownership and decodes through Euphorium directly.

use std::sync::mpsc::{Receiver, Sender};

#[cfg(not(target_arch = "wasm32"))]
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use euphorium::{Output, SoundSource, Soundscape};
use web_time::Duration;

// REFACTOR POINT(full-bevy): custom audio backend and channel resources become
// Bevy audio resources and messages; direct platform polling goes away.

euphorium::sound_key! {
    /// Every sound owned by the application audio scene.
    ///
    /// `Music` is a placeholder whose source is swapped per level. The three
    /// effects are static bytes registered once at startup and re-triggered
    /// with `replay()`.
    pub enum AppSound {
        Music => "music",
        MetronomeMajor => "sfx/metronome_major",
        MetronomeMinor => "sfx/metronome_minor",
        Death => "sfx/death",
    }
}

const METRONOME_MAJOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_major.mp3"
));
const METRONOME_MINOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_minor.mp3"
));
const DEAD_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/dead.mp3"
));

/// Authoritative application audio scene, shared with the App world.
///
/// Euphorium sounds hold `Rc` state on WASM, so the scene is `!Send` there
/// and must travel as a [`NonSend`](bevy_ecs::change_detection::NonSend)
/// resource; on native it is an ordinary [`Resource`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
pub struct AudioSceneRes(pub Soundscape<AppSound>);

#[cfg(target_arch = "wasm32")]
pub struct AudioSceneRes(pub Soundscape<AppSound>);

#[cfg(not(target_arch = "wasm32"))]
pub type AudioSceneParam<'w> = bevy_ecs::change_detection::Res<'w, AudioSceneRes>;

#[cfg(target_arch = "wasm32")]
pub type AudioSceneParam<'w> = bevy_ecs::change_detection::NonSend<'w, AudioSceneRes>;

pub(crate) fn with_audio_scene<R>(world: &World, f: impl FnOnce(&AudioSceneRes) -> R) -> R {
    #[cfg(not(target_arch = "wasm32"))]
    {
        f(world.resource::<AudioSceneRes>())
    }
    #[cfg(target_arch = "wasm32")]
    {
        f(world.non_send::<AudioSceneRes>())
    }
}

impl Default for AudioSceneRes {
    fn default() -> Self {
        // The builder starts deferred, preserving lazy output behavior: no
        // sink opens until playback (or `ensure_audio_output`) requires it,
        // so warmups never touch the audio device.
        let scene = Soundscape::<AppSound>::builder()
            .sound(AppSound::MetronomeMajor, METRONOME_MAJOR_SFX)
            .sound(AppSound::MetronomeMinor, METRONOME_MINOR_SFX)
            .sound(AppSound::Death, DEAD_SFX)
            .placeholders()
            .expect("AppSound paths are valid sound paths");
        // Keep headless tests silent; audibility is never asserted.
        #[cfg(test)]
        let _ = scene.set_volume(0.0);
        Self(scene)
    }
}

fn host_label(backend: euphorium::Backend) -> String {
    Output::backend_label(backend)
}

pub fn available_host_labels() -> Vec<String> {
    let mut labels = vec!["Default".to_string()];
    labels.extend(Output::available_backends().into_iter().map(host_label));
    labels.sort();
    labels.dedup();
    labels
}

pub fn canonical_backend_name(backend_name: &str) -> Option<String> {
    let trimmed = backend_name.trim();
    if trimmed.eq_ignore_ascii_case("Default") {
        return Some("Default".to_string());
    }
    Output::parse_backend_label(trimmed).map(host_label)
}

/// Remembers a backend preference by its user-facing label, replacing the
/// scene output. `"Default"` (and `""`) clear the preference. Active playback
/// keeps its source and position across the switch.
pub fn set_preferred_backend_name(scene: &Soundscape<AppSound>, backend_name: &str) -> bool {
    let trimmed = backend_name.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Default") {
        scene.set_preferred_backend(None);
        pump_audio_events(scene);
        return true;
    }
    if scene.set_preferred_backend_by_name(trimmed) {
        pump_audio_events(scene);
        return true;
    }
    false
}

/// Ensures the physical output is open. Returns `false` (and warns) when no
/// device is available, e.g. headless CI or a locked browser gesture.
pub fn ensure_audio_output(scene: &Soundscape<AppSound>) -> bool {
    let result = scene.ensure_output();
    pump_audio_events(scene);
    match result {
        Ok(()) => true,
        Err(error) => {
            log::warn!("Failed to initialize audio output: {error}");
            false
        }
    }
}

/// Advances pending browser loads, discovers natural completion, and drains
/// the global event stream. Call after transport operations; the stream is
/// otherwise unbounded.
pub fn pump_audio_events(scene: &Soundscape<AppSound>) {
    for failure in scene.update() {
        log::debug!(
            "Async audio failure for {:?}: {:?}",
            failure.sound,
            failure.error
        );
    }
    while scene.poll_event().is_some() {}
}

fn playback_position(seconds: f32) -> Duration {
    Duration::try_from_secs_f32(seconds.max(0.0)).unwrap_or_default()
}

fn set_music_source(scene: &Soundscape<AppSound>, source: SoundSource) -> bool {
    let music = scene.sound(AppSound::Music);
    let _ = music.stop();
    match music.set_source(source) {
        Ok(()) => {
            pump_audio_events(scene);
            true
        }
        Err(error) => {
            log::warn!("Failed to load music with Euphorium: {error}");
            pump_audio_events(scene);
            false
        }
    }
}

fn seek_and_play_music(scene: &Soundscape<AppSound>, start_seconds: f32) -> bool {
    let music = scene.sound(AppSound::Music);
    let position = playback_position(start_seconds);
    let position = music
        .duration()
        .ok()
        .flatten()
        .map_or(position, |duration| position.min(duration));
    if let Err(error) = music.try_seek(position) {
        log::warn!("Failed to seek music with Euphorium: {error}");
        let _ = music.stop();
        pump_audio_events(scene);
        return false;
    }
    if let Err(error) = music.play() {
        log::warn!("Failed to play music with Euphorium: {error}");
        pump_audio_events(scene);
        return false;
    }
    pump_audio_events(scene);
    true
}

/// Stops any current music, swaps in runtime bytes, and plays from `start`.
pub fn start_music_with_bytes(
    scene: &Soundscape<AppSound>,
    bytes: &[u8],
    start_seconds: f32,
) -> bool {
    if !set_music_source(scene, SoundSource::shared_bytes(bytes)) {
        return false;
    }
    seek_and_play_music(scene, start_seconds)
}

/// Stops any current music and stages runtime bytes without playing.
pub fn warmup_music_with_bytes(scene: &Soundscape<AppSound>, bytes: &[u8]) {
    set_music_source(scene, SoundSource::shared_bytes(bytes));
}

/// Stops any current music, swaps in a built-in track, and plays from `start`.
/// Returns whether playback started.
pub fn start_music_builtin(
    scene: &Soundscape<AppSound>,
    level_name: &str,
    start_seconds: f32,
) -> bool {
    if cache_builtin_music(scene, level_name) {
        seek_and_play_music(scene, start_seconds)
    } else {
        false
    }
}

/// Stops any current music and stages a built-in track without playing.
pub fn warmup_music_builtin(scene: &Soundscape<AppSound>, level_name: &str) {
    cache_builtin_music(scene, level_name);
}

fn cache_builtin_music(scene: &Soundscape<AppSound>, level_name: &str) -> bool {
    let Some(bytes) = eg_level::get_builtin_audio(level_name) else {
        stop_music(scene);
        log::warn!("Failed to read level music for '{level_name}'");
        return false;
    };
    set_music_source(scene, SoundSource::static_bytes(bytes))
}

/// Stops music, retaining its source.
pub fn stop_music(scene: &Soundscape<AppSound>) {
    let _ = scene.sound(AppSound::Music).stop();
    pump_audio_events(scene);
}

/// Pauses music, keeping its position.
pub fn pause_music(scene: &Soundscape<AppSound>) {
    if let Err(error) = scene.sound(AppSound::Music).pause() {
        log::warn!("Failed to pause music: {error}");
    }
    pump_audio_events(scene);
}

/// Releases the music-local pause. No-op unless music is paused and waiting.
pub fn resume_music(scene: &Soundscape<AppSound>) {
    if let Err(error) = scene.sound(AppSound::Music).resume() {
        log::warn!("Failed to resume music: {error}");
    }
    pump_audio_events(scene);
}

/// Current music position in seconds, or `None` when not playing.
pub fn music_time_seconds(scene: &Soundscape<AppSound>) -> Option<f32> {
    let music = scene.sound(AppSound::Music);
    let position = music
        .is_playing()
        .unwrap_or(false)
        .then(|| music.position().ok())
        .flatten()
        .map(|position| position.as_secs_f32());
    pump_audio_events(scene);
    position
}

pub fn music_is_playing(scene: &Soundscape<AppSound>) -> bool {
    let playing = scene.sound(AppSound::Music).is_playing().unwrap_or(false);
    pump_audio_events(scene);
    playing
}

/// Re-triggers a one-shot effect from the start.
pub fn play_sfx(scene: &Soundscape<AppSound>, key: AppSound) {
    if let Err(error) = scene.sound(key).replay() {
        log::warn!("Failed to play sound effect: {error}");
    }
    pump_audio_events(scene);
}

pub(crate) fn start_audio_source_at_seconds_for_resources(
    level_name: &str,
    start_seconds: f32,
    audio: &AudioSceneRes,
    custom_bytes: Option<&[u8]>,
) {
    if let Some(bytes) = custom_bytes {
        start_music_with_bytes(&audio.0, bytes, start_seconds);
    } else {
        start_music_builtin(&audio.0, level_name, start_seconds);
    }
}

/// Receiver end of the audio-import channel. Production frames drain this
/// through `StateSchedule`.
pub type AudioImportChannelReceiver = std::sync::Arc<std::sync::Mutex<Receiver<(String, Vec<u8>)>>>;

pub(crate) type AudioImportChannel = (Sender<(String, Vec<u8>)>, AudioImportChannelReceiver);

#[cfg(test)]
mod tests {
    use super::{
        AppSound, AudioSceneRes, available_host_labels, canonical_backend_name,
        ensure_audio_output, music_is_playing, music_time_seconds, pause_music, play_sfx,
        resume_music, set_preferred_backend_name, start_music_builtin, start_music_with_bytes,
        stop_music, warmup_music_builtin, warmup_music_with_bytes,
    };

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
        for backend in euphorium::Output::available_backends() {
            assert_eq!(
                canonical_backend_name(&euphorium::Output::backend_label(backend).to_lowercase())
                    .as_deref(),
                Some(euphorium::Output::backend_label(backend).as_str())
            );
        }
    }

    #[test]
    fn output_is_lazy_and_unknown_backend_leaves_it_unchanged() {
        let scene = AudioSceneRes::default();
        assert!(!set_preferred_backend_name(&scene.0, "not-a-backend"));
        assert!(!scene.0.has_output());
        assert!(!music_is_playing(&scene.0));
    }

    #[test]
    fn warmup_never_starts_playback_or_opens_output() {
        let scene = AudioSceneRes::default();
        let wav = build_wav(44_100, 1, &[0]);
        warmup_music_with_bytes(&scene.0, &wav);
        assert!(
            scene
                .0
                .sound(AppSound::Music)
                .source()
                .is_ok_and(|source| !source.is_empty())
        );
        resume_music(&scene.0);
        assert!(!music_is_playing(&scene.0));
        assert_eq!(music_time_seconds(&scene.0), None);
        assert!(!scene.0.has_output());
        warmup_music_builtin(&scene.0, "MissingLevel");
        assert!(!music_is_playing(&scene.0));
    }

    #[test]
    fn invalid_bytes_never_start_playback() {
        let scene = AudioSceneRes::default();
        assert!(!start_music_with_bytes(&scene.0, &[1, 2, 3], 0.0));
        assert!(!music_is_playing(&scene.0));
        assert_eq!(music_time_seconds(&scene.0), None);
    }

    #[test]
    fn transport_seeks_pauses_stops_and_restarts() {
        let scene = AudioSceneRes::default();
        if !ensure_audio_output(&scene.0) {
            eprintln!("Skipping output-dependent transport assertions: no audio output");
            return;
        }
        let wav = build_wav(8000, 1, &[0; 80000]);
        assert!(start_music_with_bytes(&scene.0, &wav, 1.25));
        assert!(music_is_playing(&scene.0));
        assert!(music_time_seconds(&scene.0).unwrap() >= 1.25);
        pause_music(&scene.0);
        assert!(!music_is_playing(&scene.0));
        assert_eq!(music_time_seconds(&scene.0), None);
        assert!(scene.0.sound(AppSound::Music).is_paused().unwrap());
        resume_music(&scene.0);
        assert!(music_is_playing(&scene.0));
        assert!(start_music_with_bytes(&scene.0, &wav, 2.5));
        assert!(music_time_seconds(&scene.0).unwrap() >= 2.5);
        stop_music(&scene.0);
        resume_music(&scene.0);
        assert!(!music_is_playing(&scene.0));
        assert_eq!(music_time_seconds(&scene.0), None);
        assert!(start_music_with_bytes(&scene.0, &wav, 0.5));
        assert!(music_is_playing(&scene.0));
        assert!(music_time_seconds(&scene.0).unwrap() >= 0.5);

        play_sfx(&scene.0, AppSound::Death);
        pause_music(&scene.0);
        assert!(scene.0.sound(AppSound::Death).is_playing().unwrap());
        assert!(!music_is_playing(&scene.0));
        let selected = scene.0.backend().unwrap();
        assert!(set_preferred_backend_name(
            &scene.0,
            &euphorium::Output::backend_label(selected)
        ));
        assert!(start_music_with_bytes(&scene.0, &wav, 0.0));

        assert!(start_music_builtin(&scene.0, "Flowerfield", 10.0));
        assert!(music_is_playing(&scene.0));
        assert!(music_time_seconds(&scene.0).unwrap() >= 10.0);
        assert!(start_music_with_bytes(&scene.0, &wav, 2.0));
        assert!(music_time_seconds(&scene.0).unwrap() >= 2.0);
        assert!(!start_music_with_bytes(&scene.0, &[1, 2, 3], 0.0));
        assert!(!music_is_playing(&scene.0));
        assert_eq!(music_time_seconds(&scene.0), None);
    }
}
