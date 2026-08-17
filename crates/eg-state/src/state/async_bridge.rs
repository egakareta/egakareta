/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Async platform-result ingestion.
//!
//! Platform APIs return futures owned by Bevy task components. Their pollers
//! publish typed result messages which the App-owned `Update` schedule applies
//! directly to authoritative resources.
//!
//! ## Schedule graph
//!
//! ```text
//!   CommandHandlerSet
//!     events::handle_* (per-message)   // Phase 3 message drain
//!   ApplyAsyncResultsSet
//!     auth_handler_system              // AuthServiceResultEvent
//!     waveform_handler_system          // WaveformResultEvent → timing + audio caches
//!     level_import_handler_system      // LevelImportResultEvent → editor/session resources
//! ```
//!
use bevy::{app::App, ecs::prelude::*};
use eg_audio::service::WaveformLoadMessage;
use eg_platform::services::AuthServiceMessage;

use crate::{
    events::async_results::{
        AudioImportResultEvent, AuthServiceResultEvent, LevelImportResultEvent, WaveformResultEvent,
    },
    state::{
        AuthStateRes, EditorAudioCacheRes, EditorCameraRes, EditorHistoryRes, EditorInputRes,
        EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTimingState, EditorTriggerRes,
        RuntimeAudioPreloadRes, SessionStateRes, queue_waveform_load_task, replace_editor_objects,
    },
};

/// World-flavoured registration for callers that build a `World`
/// directly. The App-owned schedule uses this registration path for both
/// production bootstrap and headless fixtures.
#[allow(dead_code)]
pub fn register_async_channel_resources_world(world: &mut World) {
    use bevy::ecs::message::Messages;

    world.init_resource::<Messages<AuthServiceResultEvent>>();
    world.init_resource::<Messages<AudioImportResultEvent>>();
    world.init_resource::<Messages<WaveformResultEvent>>();
    world.init_resource::<Messages<LevelImportResultEvent>>();
}

/// Register async-result messages with an [`App`].
///
/// Unlike the world-only helper above, this also installs Bevy's message
/// maintenance systems. Production uses this path so completed task results
/// have the normal frame-bounded lifetime of every other Bevy message.
pub fn register_async_result_messages(app: &mut App) {
    app.add_message::<AuthServiceResultEvent>()
        .add_message::<AudioImportResultEvent>()
        .add_message::<WaveformResultEvent>()
        .add_message::<LevelImportResultEvent>();
}

// ---------------------------------------------------------------------------
// Phase 6b: result handler systems
// ---------------------------------------------------------------------------

/// System set for result-handler systems. It runs after
/// `CommandHandlerSet` so task result application observes any request made
/// in the same frame.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApplyAsyncResultsSet;

/// Consume `AuthServiceResultEvent`s and apply them to App-world auth state.
/// Auth results mutate the App-owned auth resource directly.
#[allow(dead_code)]
pub fn auth_handler_system(
    mut events: MessageReader<AuthServiceResultEvent>,
    mut auth: ResMut<AuthStateRes>,
) {
    for event in events.read() {
        let message = event.message.clone();
        match message {
            AuthServiceMessage::SignedIn(result) => auth.complete_auth_sign_in(result),
            AuthServiceMessage::Refreshed(result) => auth.complete_auth_refresh(result),
            AuthServiceMessage::SignedOut(result) => auth.complete_auth_sign_out(result),
        }
    }
}

/// Consume `AudioImportResultEvent`s, update the App-owned editor audio cache,
/// reset waveform presentation, and spawn a task owned by the App world.
#[allow(dead_code)]
pub fn audio_import_handler_system(
    mut commands: Commands,
    mut events: MessageReader<AudioImportResultEvent>,
    session: Res<SessionStateRes>,
    cache: Res<EditorAudioCacheRes>,
    preload: Res<RuntimeAudioPreloadRes>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        let Some(bytes) = event.bytes.clone() else {
            continue;
        };
        let level_name = session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let source_key = cache.insert_level_audio(&level_name, bytes.clone());
        cache.remove_waveform(&source_key);
        cache.clear_loading_source_if(&source_key);
        queue_waveform_load_task(&mut commands, level_name, &cache, &preload, &mut timing.0);
    }
}

/// Consume `WaveformResultEvent`s and update App-owned waveform presentation,
/// editor cache, persistent cache, and runtime preload resources directly.
///
/// Phase 6b note: `WaveformResultEvent` is not `Clone` (its
/// payload is a non-`Clone` `WaveformLoadMessage`), so we can't
/// use the standard `MessageReader<T>` + `for event in events.read()`
/// pattern (which iterates over `&T`). Instead we take `&mut
/// World` and `drain` the buffer to get owned `T` values.
#[allow(dead_code)]
pub fn waveform_handler_system(world: &mut World) {
    use bevy::ecs::message::Messages;
    use eg_audio::audio::runtime_asset_source_key;

    let current_level_name = world
        .resource::<SessionStateRes>()
        .editor_level_name
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());
    let current_source_key = runtime_asset_source_key(&current_level_name, "audio.mp3");
    let editor_cache = world.resource::<EditorAudioCacheRes>().clone();
    let runtime_preload = world.resource::<RuntimeAudioPreloadRes>().clone();
    let drained: Vec<WaveformResultEvent> = {
        let mut messages = world.resource_mut::<Messages<WaveformResultEvent>>();
        messages.drain().collect()
    };
    for event in drained {
        let source = event.source;
        let level_name = event.level_name;
        let message = event.message;
        let source_key = runtime_asset_source_key(&level_name, &source);
        if source_key == current_source_key {
            apply_waveform_timing_message(&mut world.resource_mut::<EditorTimingRes>().0, &message);
        }
        apply_waveform_cache_message(
            world,
            &editor_cache,
            &runtime_preload,
            source,
            level_name,
            message,
        );
    }
}

fn apply_waveform_chunk(samples: &mut Vec<f32>, start_peak: usize, peaks: &[f32]) {
    if samples.len() < start_peak {
        samples.resize(start_peak, 0.0);
    }
    let end_peak = start_peak + peaks.len();
    if samples.len() < end_peak {
        samples.resize(end_peak, 0.0);
    }
    samples[start_peak..end_peak].copy_from_slice(peaks);
}

fn apply_waveform_timing_message(timing: &mut EditorTimingState, message: &WaveformLoadMessage) {
    match message {
        WaveformLoadMessage::Started { window_size } => {
            timing.waveform_samples.clear();
            timing.waveform_sample_rate = 0;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = true;
            timing.waveform_complete = false;
        }
        WaveformLoadMessage::Chunk {
            start_peak,
            peaks,
            sample_rate,
            window_size,
        } => {
            apply_waveform_chunk(&mut timing.waveform_samples, *start_peak, peaks);
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = true;
            timing.waveform_complete = false;
        }
        WaveformLoadMessage::Cached {
            samples,
            sample_rate,
            window_size,
            ..
        } => {
            timing.waveform_samples = samples.clone();
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = false;
            timing.waveform_complete = true;
        }
        WaveformLoadMessage::Finished {
            sample_rate,
            window_size,
            total_peaks,
            ..
        } => {
            timing.waveform_samples.truncate(*total_peaks);
            timing.waveform_sample_rate = *sample_rate;
            timing.waveform_window_size = *window_size;
            timing.waveform_loading = false;
            timing.waveform_complete = true;
        }
        WaveformLoadMessage::Failed { .. } => {
            timing.waveform_samples.clear();
            timing.waveform_sample_rate = 0;
            timing.waveform_loading = false;
            timing.waveform_complete = false;
        }
    }
}

fn apply_waveform_cache_message(
    world: &mut World,
    editor_cache: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
    source: String,
    level_name: String,
    message: WaveformLoadMessage,
) {
    use eg_audio::{audio::runtime_asset_source_key, service::WaveformCacheEntry};

    let source_key = runtime_asset_source_key(&level_name, &source);
    match message {
        WaveformLoadMessage::Started { window_size } => {
            editor_cache.insert_waveform(
                source_key.clone(),
                WaveformCacheEntry {
                    window_size,
                    ..WaveformCacheEntry::default()
                },
            );
        }
        WaveformLoadMessage::Chunk {
            start_peak,
            peaks,
            sample_rate,
            window_size,
        } => {
            editor_cache.with_mut(|cache| {
                let entry = cache.waveform_mut_or_default(source_key.clone());
                entry.sample_rate = sample_rate;
                entry.window_size = window_size;
                entry.complete = false;
                apply_waveform_chunk(&mut entry.samples, start_peak, &peaks);
            });
        }
        WaveformLoadMessage::Cached {
            samples,
            sample_rate,
            window_size,
            bytes,
        } => {
            let entry = WaveformCacheEntry {
                samples,
                sample_rate,
                window_size,
                complete: true,
            };
            editor_cache.insert_waveform(source_key.clone(), entry);
            editor_cache.clear_loading_source_if(&source_key);
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
        }
        WaveformLoadMessage::Finished {
            sample_rate,
            window_size,
            total_peaks,
            cache_key,
            bytes,
        } => {
            let entry = editor_cache.with_mut(|cache| {
                let entry = cache.waveform_mut_or_default(source_key.clone());
                entry.sample_rate = sample_rate;
                entry.window_size = window_size;
                entry.complete = true;
                entry.samples.truncate(total_peaks);
                entry.clone()
            });
            if let Some(cache_key) = cache_key {
                let cached = crate::state::audio_state::persistent_waveform_cache_entry(&entry);
                crate::state::queue_waveform_persist_world(world, cache_key, cached);
            }
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
            editor_cache.clear_loading_source_if(&source_key);
        }
        WaveformLoadMessage::Failed { bytes } => {
            editor_cache.remove_waveform(&source_key);
            if let Some(bytes) = bytes {
                runtime_preload.insert_preloaded(source_key.clone(), bytes);
            }
            editor_cache.clear_loading_source_if(&source_key);
        }
    }
}

/// Consume `LevelImportResultEvent`s and replace the App-owned editor session.
/// Bundled `.egz` audio bytes are inserted into the shared editor cache directly.
#[allow(dead_code)]
pub fn level_import_handler_system(world: &mut World) {
    use bevy::ecs::message::Messages;

    let events: Vec<LevelImportResultEvent> = world
        .resource_mut::<Messages<LevelImportResultEvent>>()
        .drain()
        .collect();
    for event in events {
        let (metadata, audio_bytes) = match eg_level::read_metadata_from_egz(&event.bytes) {
            Ok(import) => import,
            Err(egz_error) => match eg_codec::decode_level_metadata_binary(&event.bytes) {
                Ok(metadata) => (metadata, None),
                Err(binary_error) => {
                    log::error!(
                        "Level import failed (.egz parse error: {egz_error}; binary parse error: {binary_error})"
                    );
                    continue;
                }
            },
        };

        let level_name = apply_imported_level_to_world(world, metadata);
        if let Some(bytes) = audio_bytes {
            world
                .resource::<EditorAudioCacheRes>()
                .insert_level_audio(&level_name, bytes);
        }
    }
}

pub(crate) fn apply_imported_level_to_world(
    world: &mut World,
    metadata: eg_types::LevelMetadata,
) -> String {
    let level_name = metadata.basic.name.clone();
    let mut init = eg_editor_domain::editor_session_init_from_metadata(Some(metadata));
    init.timing_points
        .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    let tap_indicator_positions = eg_editor_domain::derive_tap_indicator_positions(
        init.spawn.position,
        init.spawn.direction,
        &init.tap_times,
        &init.objects,
    );

    replace_editor_objects(world, init.objects);
    {
        let mut input = world.resource_mut::<EditorInputRes>();
        input.0.cursor = init.cursor;
        input.0.selected_block_index = None;
        input.0.selected_block_indices.clear();
        input.0.hovered_block_index = None;
    }
    {
        let mut timeline = world.resource_mut::<EditorTimelineRes>();
        timeline.0.taps.tap_times = init.tap_times;
        timeline.0.taps.tap_indicator_positions = tap_indicator_positions;
        timeline.0.taps.selected_index = None;
        timeline.0.tap_cursor_override = None;
        timeline.0.clock.time_seconds = init.timeline_time_seconds;
        timeline.0.clock.duration_seconds = init.timeline_duration_seconds;
    }
    {
        let mut timing = world.resource_mut::<EditorTimingRes>();
        timing.0.timing_points = init.timing_points;
        timing.0.timing_selected_index = None;
        timing.0.mark_timing_points_changed();
    }
    world.resource_mut::<EditorSpawnRes>().0 = init.spawn;
    {
        let mut camera = world.resource_mut::<EditorCameraRes>();
        camera.0.editor_pan = init.camera_pan;
        camera.0.editor_target_z = init.cursor[1];
    }
    world.resource_mut::<EditorTriggerRes>().0.selected_index = None;
    {
        let mut history = world.resource_mut::<EditorHistoryRes>();
        history.0.undo.clear();
        history.0.redo.clear();
    }
    {
        let mut session = world.resource_mut::<SessionStateRes>();
        session.editor_level_name = Some(level_name.clone());
        session.editor_music_metadata = init.music;
        session.editor_creator_metadata = init.creator_metadata;
        session.editor_sky_color = init.sky_color;
        session.editor_menu_preview_camera = init.menu_preview_camera;
    }

    level_name
}

/// Add the channel-backed result handler systems to a `Schedule`. Each system
/// is assigned to [`ApplyAsyncResultsSet`] so cross-set ordering
/// can be expressed via `.in_set(ApplyAsyncResultsSet)` / `.before()`.
#[allow(dead_code)]
pub fn add_apply_async_results_systems(schedule: &mut bevy::ecs::schedule::Schedule) {
    schedule.add_systems(
        (
            auth_handler_system,
            audio_import_handler_system,
            waveform_handler_system,
            level_import_handler_system,
        )
            .in_set(ApplyAsyncResultsSet),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn build_world() -> World {
        let mut world = World::new();
        crate::state::plugins::register_state_resources(&mut world);
        register_async_channel_resources_world(&mut world);
        world
    }

    #[test]
    fn register_async_bridge_world_initializes_message_resources() {
        let world = build_world();
        assert!(world.contains_resource::<Messages<AuthServiceResultEvent>>());
        assert!(world.contains_resource::<Messages<AudioImportResultEvent>>());
        assert!(world.contains_resource::<Messages<WaveformResultEvent>>());
        assert!(world.contains_resource::<Messages<LevelImportResultEvent>>());
    }

    #[test]
    fn auth_handler_system_updates_auth_resource_for_sign_out() {
        // Write an `AuthServiceResultEvent::SignedOut(Ok(()))` into
        // the buffer, run the handler system, and verify the
        // App-world auth resource was updated (`session = None`,
        // `pending = false`).
        use crate::events::async_results::AuthServiceResultEvent;
        let mut world = build_world();
        {
            let mut auth = world.resource_mut::<AuthStateRes>();
            auth.pending = true;
            auth.session = Some(test_auth_session());
        }
        world.write_message(AuthServiceResultEvent::new(AuthServiceMessage::SignedOut(
            Ok(()),
        )));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(auth_handler_system);
        schedule.run(&mut world);

        let auth = world.resource::<AuthStateRes>();
        assert!(!auth.pending);
        assert!(auth.session.is_none());
    }

    #[test]
    fn level_import_handler_system_is_safe_on_invalid_bytes() {
        // The handler should not panic or mutate App resources on invalid bytes.
        use crate::events::async_results::LevelImportResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name = Some("Original".to_string());
        world.write_message(LevelImportResultEvent::new(vec![0xDE, 0xAD]));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(level_import_handler_system);
        schedule.run(&mut world);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("Original")
        );
    }

    #[test]
    fn level_import_handler_system_replaces_app_editor_session_without_state_closure() {
        use eg_types::{Direction, LevelMetadata, LevelObject, SpawnMetadata};

        use crate::events::async_results::LevelImportResultEvent;

        let mut world = build_world();
        let metadata = LevelMetadata {
            basic: eg_types::BasicLevelMetadata {
                name: "Imported".to_string(),
                ..eg_types::BasicLevelMetadata::default()
            },
            spawn: SpawnMetadata {
                position: [2.0, 3.0, 4.0],
                direction: Direction::Right,
            },
            tap_times: vec![0.8, 0.2],
            timeline_duration_seconds: 9.0,
            objects: vec![LevelObject {
                position: [6.0, 2.0, 1.0],
                ..LevelObject::default()
            }],
            ..LevelMetadata::default()
        };
        let bytes = eg_codec::encode_level_metadata_binary(&metadata).expect("encode level");
        world.write_message(LevelImportResultEvent::new(bytes));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(level_import_handler_system);
        schedule.run(&mut world);

        let session = world.resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("Imported"));
        let mut query = world
            .query_filtered::<&eg_game::ObjectData, With<crate::state::EditorAuthoredObject>>();
        let object = query.single(&world).expect("imported authored object");
        assert_eq!(object.0.position, [6.0, 2.0, 1.0]);
        assert_eq!(world.resource::<EditorSpawnRes>().0, metadata.spawn);
        let timeline = &world.resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.2, 0.8]);
        assert_eq!(timeline.taps.tap_indicator_positions.len(), 2);
        assert_eq!(timeline.clock.duration_seconds, 9.0);
        let input = &world.resource::<EditorInputRes>().0;
        assert_eq!(input.cursor, [6.0, 2.0, 1.0]);
        assert_eq!(input.selected_block_index, None);
    }

    #[test]
    fn waveform_handler_system_updates_current_app_timing_state() {
        use crate::events::async_results::WaveformResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name = Some("Level1".to_string());
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Started { window_size: 128 },
        )));
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Chunk {
                start_peak: 1,
                peaks: vec![0.25, 0.75],
                sample_rate: 44_100,
                window_size: 128,
            },
        )));
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "Level1".to_string(),
            WaveformLoadMessage::Finished {
                sample_rate: 44_100,
                window_size: 128,
                total_peaks: 3,
                cache_key: None,
                bytes: None,
            },
        )));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(waveform_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.waveform_samples, vec![0.0, 0.25, 0.75]);
        assert_eq!(timing.waveform_sample_rate, 44_100);
        assert_eq!(timing.waveform_window_size, 128);
        assert!(!timing.waveform_loading);
        assert!(timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("Level1", "audio.mp3");
        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .waveform(&source_key)
                .is_some_and(|entry| entry.complete)
        );
    }

    #[test]
    fn waveform_handler_system_keeps_stale_results_out_of_app_timing_state() {
        use crate::events::async_results::WaveformResultEvent;
        let mut world = build_world();
        world.resource_mut::<SessionStateRes>().editor_level_name =
            Some("CurrentLevel".to_string());
        {
            let mut timing = world.resource_mut::<EditorTimingRes>();
            timing.0.waveform_samples = vec![1.0];
            timing.0.waveform_sample_rate = 22_050;
            timing.0.waveform_complete = true;
        }
        world.write_message(WaveformResultEvent::from_result((
            "audio.mp3".to_string(),
            "OldLevel".to_string(),
            WaveformLoadMessage::Cached {
                samples: vec![0.25, 0.75],
                sample_rate: 44_100,
                window_size: 256,
                bytes: None,
            },
        )));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(waveform_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.waveform_samples, vec![1.0]);
        assert_eq!(timing.waveform_sample_rate, 22_050);
        assert!(timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("OldLevel", "audio.mp3");
        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .contains_waveform(&source_key)
        );
    }

    #[test]
    fn audio_import_handler_system_runs_with_cancellation() {
        // The audio-import handler should be a no-op when the
        // event's `bytes` is `None` (cancellation).
        use crate::events::async_results::AudioImportResultEvent;
        let mut world = build_world();
        world.write_message(AudioImportResultEvent::new("level1.ogg".to_string(), None));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(audio_import_handler_system);
        schedule.run(&mut world);

        assert!(
            world
                .resource::<EditorAudioCacheRes>()
                .bytes_for_source("level1.ogg")
                .is_none()
        );
    }

    #[test]
    fn audio_import_handler_system_resets_app_waveform_presentation() {
        use crate::events::async_results::AudioImportResultEvent;
        let mut world = build_world();
        {
            let mut timing = world.resource_mut::<EditorTimingRes>();
            timing.0.waveform_samples = vec![0.5];
            timing.0.waveform_sample_rate = 44_100;
            timing.0.waveform_complete = true;
        }
        world.write_message(AudioImportResultEvent::new(
            "imported.mp3".to_string(),
            Some(vec![1, 2, 3, 4]),
        ));

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(audio_import_handler_system);
        schedule.run(&mut world);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert!(timing.waveform_samples.is_empty());
        assert_eq!(timing.waveform_sample_rate, 0);
        assert_eq!(
            timing.waveform_window_size,
            eg_audio::service::WAVEFORM_WINDOW
        );
        assert!(timing.waveform_loading);
        assert!(!timing.waveform_complete);

        let source_key = eg_audio::audio::runtime_asset_source_key("Untitled", "audio.mp3");
        assert_eq!(
            world
                .resource::<EditorAudioCacheRes>()
                .bytes_for_source(&source_key),
            Some(vec![1, 2, 3, 4])
        );
    }

    // ----- helpers for the handler-system tests -----

    fn test_auth_session() -> eg_types::AuthSession {
        use eg_types::{AuthSession, AuthSessionTokens, AuthUser};
        AuthSession {
            session: AuthSessionTokens {
                access_token: String::new(),
                refresh_token: String::new(),
                expires_at: Some(0),
                token_type: String::new(),
            },
            user: AuthUser {
                id: String::new(),
                email: None,
            },
            profile: None,
        }
    }
}
