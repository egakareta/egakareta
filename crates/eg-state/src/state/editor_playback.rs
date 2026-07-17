/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-world editor timeline playback.

use bevy_ecs::prelude::*;
use eg_blocks::BlockEffectColorMode;
use eg_editor_domain::derive_timeline_elapsed_seconds;
use eg_game::{ConsumedObjectEvent, TimelineSimulationRuntime};
use eg_types::{AppPhase, Direction, EditorMode, LevelObject, TriggerTarget};

use super::audio_state::start_audio_source_at_seconds_for_resources;
use super::editor_timeline::EditorTimelineState;
use super::runtime::BlockVisualEffect;
use super::{
    AppPhaseRes, AudioBackendRes, AudioBackendSystemParam, EditorAudioCacheRes, EditorDirtyFlags,
    EditorInputRes, EditorObjectsRes, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes,
    EditorTimingRes, FrameTimeRes, RuntimeAudioPreloadRes, SessionStateRes,
};

const METRONOME_MAJOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_major.mp3"
));
const METRONOME_MINOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_minor.mp3"
));

/// Advance editor playback from App-owned clock/audio state and publish only
/// presentation invalidations for the compatibility renderer.
pub(crate) fn update_editor_timeline_playback_system(
    phase: Res<AppPhaseRes>,
    frame_time: Res<FrameTimeRes>,
    mut input: ResMut<EditorInputRes>,
    objects: Res<EditorObjectsRes>,
    spawn: Res<EditorSpawnRes>,
    mut timeline: ResMut<EditorTimelineRes>,
    timing: Res<EditorTimingRes>,
    mut runtime_state: ResMut<EditorRuntimeRes>,
    audio: AudioBackendSystemParam<'_>,
    editor_audio: Res<EditorAudioCacheRes>,
    runtime_preload: Res<RuntimeAudioPreloadRes>,
    session: Res<SessionStateRes>,
) {
    if phase.current != AppPhase::Editor || !timeline.0.playback.playing {
        return;
    }

    puffin::profile_scope!("EditorTimelinePlayback");
    let mode = effective_playback_mode(&input, &runtime_state, &timeline);
    let simulate_preview = mode != EditorMode::Timing;

    if let Some(target_time) = take_ready_pending_seek(&mut timeline.0, frame_time.delta_seconds) {
        audio.stop();
        let start_seconds =
            rebuild_playback_runtime(mode, target_time, &objects.objects, &spawn.0, &mut timeline);
        start_editor_audio(
            &session,
            start_seconds,
            &audio,
            &editor_audio,
            &runtime_preload,
        );
    }

    let audio_time = audio
        .playback_time_seconds()
        .unwrap_or(timeline.0.clock.time_seconds);
    let clamped_time = audio_time.max(0.0).min(timeline.0.clock.duration_seconds);
    let old_time = timeline.0.clock.time_seconds;

    if (clamped_time - old_time).abs() > 1e-4 {
        timeline.0.clock.time_seconds = clamped_time;

        if simulate_preview && has_timeline_preview_triggers(&objects.objects) {
            runtime_state
                .0
                .dirty
                .merge(EditorDirtyFlags::timeline_trigger_preview_changed());
        }

        if simulate_preview {
            let (position, direction, events) =
                advance_simulation_preview(clamped_time, &objects.objects, &spawn.0, &mut timeline);
            let consumed_objects = !events.is_empty();
            queue_block_effects(events, &mut runtime_state);
            if consumed_objects {
                runtime_state
                    .0
                    .dirty
                    .merge(EditorDirtyFlags::block_mesh_and_hitboxes_changed());
            }
            timeline.0.preview.position = position;
            timeline.0.preview.direction = direction;
        } else {
            timeline.0.playback.runtime = None;
            play_metronome_for_crossed_beat(
                old_time,
                clamped_time,
                &timing.0.timing_points,
                &audio,
            );
        }
    }

    if clamped_time >= timeline.0.clock.duration_seconds || !audio.is_playing() {
        stop_editor_playback(
            &mut input,
            &mut runtime_state,
            &mut timeline,
            &audio,
            simulate_preview,
        );
    }
}

fn take_ready_pending_seek(
    timeline: &mut EditorTimelineState,
    frame_delta_seconds: f32,
) -> Option<f32> {
    let Some(target_time) = timeline.playback.pending_seek_time_seconds else {
        timeline.playback.seek_resync_cooldown_seconds = 0.0;
        return None;
    };

    let cooldown = (timeline.playback.seek_resync_cooldown_seconds - frame_delta_seconds).max(0.0);
    timeline.playback.seek_resync_cooldown_seconds = cooldown;
    if cooldown > 0.0 {
        return None;
    }

    timeline.playback.pending_seek_time_seconds = None;
    timeline.playback.seek_resync_cooldown_seconds = 0.0;
    Some(target_time)
}
fn effective_playback_mode(
    input: &EditorInputRes,
    runtime: &EditorRuntimeRes,
    timeline: &EditorTimelineRes,
) -> EditorMode {
    if timeline.0.playback.playing && input.0.mode == EditorMode::Null {
        runtime.0.interaction.last_mode.unwrap_or(input.0.mode)
    } else {
        input.0.mode
    }
}

fn has_timeline_preview_triggers(objects: &[LevelObject]) -> bool {
    objects.iter().any(|object| {
        let Some(trigger) = object.trigger.as_ref() else {
            return false;
        };
        (!matches!(trigger.target, TriggerTarget::Camera) && trigger.transform)
            || (matches!(trigger.target, TriggerTarget::Camera) && trigger.camera.is_some())
    })
}

fn rebuild_playback_runtime(
    mode: EditorMode,
    timeline_time: f32,
    objects: &[LevelObject],
    spawn: &eg_types::SpawnMetadata,
    timeline: &mut EditorTimelineRes,
) -> f32 {
    if mode == EditorMode::Timing {
        timeline.0.playback.runtime = None;
        return derive_timeline_elapsed_seconds(
            spawn.position,
            spawn.direction,
            &timeline.0.taps.tap_times,
            timeline_time,
            objects,
        );
    }

    let mut runtime = TimelineSimulationRuntime::new(
        spawn.position,
        spawn.direction,
        objects,
        &timeline.0.taps.tap_times,
    );
    runtime.advance_to(timeline_time);
    let start_seconds = runtime.elapsed_seconds();
    timeline.0.playback.runtime = Some(runtime);
    start_seconds
}

fn advance_simulation_preview(
    target_time: f32,
    objects: &[LevelObject],
    spawn: &eg_types::SpawnMetadata,
    timeline: &mut EditorTimelineRes,
) -> ([f32; 3], Direction, Vec<ConsumedObjectEvent>) {
    let runtime = timeline
        .0
        .playback
        .runtime
        .take()
        .filter(|runtime| target_time + 1e-6 >= runtime.elapsed_seconds());
    let mut runtime = runtime.unwrap_or_else(|| {
        TimelineSimulationRuntime::new(
            spawn.position,
            spawn.direction,
            objects,
            &timeline.0.taps.tap_times,
        )
    });
    runtime.advance_to(target_time);
    let events = runtime.take_consumed_object_events();
    let snapshot = runtime.snapshot();
    let preview = (snapshot.position, snapshot.direction, events);
    timeline.0.playback.runtime = Some(runtime);
    preview
}

fn queue_block_effects(events: Vec<ConsumedObjectEvent>, runtime: &mut EditorRuntimeRes) {
    for event in events {
        let block = eg_blocks::BlockDefinition::get(&event.object.block_id);
        let Some(effect) = block.animation.on_consumed else {
            continue;
        };
        let color = match effect.color_mode {
            BlockEffectColorMode::Tint => event.object.color_tint,
            BlockEffectColorMode::Render => [
                block.render.color_top[0],
                block.render.color_top[1],
                block.render.color_top[2],
            ],
        };
        runtime
            .0
            .pending_block_effects
            .push(BlockVisualEffect::from_object(&event.object, effect, color));
    }
}

fn play_metronome_for_crossed_beat(
    old_time: f32,
    new_time: f32,
    timing_points: &[eg_types::TimingPoint],
    audio: &AudioBackendRes,
) {
    if new_time <= old_time {
        return;
    }

    let Some(timing_point) = timing_points
        .iter()
        .rev()
        .find(|point| point.time_seconds <= new_time)
    else {
        return;
    };
    if timing_point.bpm <= 0.0 {
        return;
    }

    let beat_duration = 60.0 / timing_point.bpm;
    let old_beat = if old_time < timing_point.time_seconds {
        -1
    } else {
        ((old_time - timing_point.time_seconds) / beat_duration).floor() as i64
    };
    let new_beat = ((new_time - timing_point.time_seconds) / beat_duration).floor() as i64;
    if new_beat <= old_beat {
        return;
    }

    let is_downbeat = new_beat % (timing_point.time_signature_numerator.max(1) as i64) == 0;
    audio.play_sfx(if is_downbeat {
        METRONOME_MAJOR_SFX
    } else {
        METRONOME_MINOR_SFX
    });
}

fn start_editor_audio(
    session: &SessionStateRes,
    start_seconds: f32,
    audio: &AudioBackendRes,
    editor_audio: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
) {
    let level_name = session.editor_level_name.as_deref().unwrap_or("Untitled");
    start_audio_source_at_seconds_for_resources(
        true,
        false,
        level_name,
        start_seconds,
        audio,
        editor_audio,
        runtime_preload,
    );
}

fn stop_editor_playback(
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &mut EditorTimelineRes,
    audio: &AudioBackendRes,
    simulate_preview: bool,
) {
    timeline.0.playback.playing = false;
    timeline.0.playback.runtime = None;
    timeline.0.playback.pending_seek_time_seconds = None;
    timeline.0.playback.seek_resync_cooldown_seconds = 0.0;
    input.0.mode = runtime
        .0
        .interaction
        .last_mode
        .take()
        .unwrap_or(EditorMode::Place);
    runtime
        .0
        .dirty
        .merge(EditorDirtyFlags::playback_mode_changed());
    if simulate_preview {
        runtime
            .0
            .dirty
            .merge(EditorDirtyFlags::block_mesh_and_hitboxes_changed());
    }
    audio.stop();
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_seek_without_target_resets_cooldown() {
        let mut timeline = EditorTimelineState::default();
        timeline.playback.seek_resync_cooldown_seconds = 0.75;

        assert_eq!(take_ready_pending_seek(&mut timeline, 0.1), None);
        assert_eq!(timeline.playback.seek_resync_cooldown_seconds, 0.0);
    }

    #[test]
    fn pending_seek_waits_for_cooldown_then_returns_target() {
        let mut timeline = EditorTimelineState::default();
        timeline.playback.pending_seek_time_seconds = Some(2.0);
        timeline.playback.seek_resync_cooldown_seconds = 0.2;

        assert_eq!(take_ready_pending_seek(&mut timeline, 0.05), None);
        assert_eq!(timeline.playback.pending_seek_time_seconds, Some(2.0));
        assert!(timeline.playback.seek_resync_cooldown_seconds > 0.0);

        assert_eq!(take_ready_pending_seek(&mut timeline, 1.0), Some(2.0));
        assert_eq!(timeline.playback.pending_seek_time_seconds, None);
        assert_eq!(timeline.playback.seek_resync_cooldown_seconds, 0.0);
    }
}
