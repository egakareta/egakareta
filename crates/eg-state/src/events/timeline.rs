/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Timeline / playback / tap events.
//!
//! Phase 3.16 carves out the pure timeline-setting messages that only
//! mutate App-world resource fields and already have matching legacy
//! setters/getters:
//! - `ShiftTimeline` and `SetTimelineTime` write
//!   `EditorTimelineRes.0.clock.time_seconds`
//! - `SetTimelineDuration` writes
//!   `EditorTimelineRes.0.clock.{duration_seconds,time_seconds}`
//! - `SetSelectedTap` writes
//!   `EditorTimelineRes.0.taps.selected_index`
//! - `SetSelectedTapTime` reorders
//!   `EditorTimelineRes.0.taps.{tap_times,tap_indicator_positions}`,
//!   updates `EditorTimelineRes.0.clock.time_seconds`, and moves
//!   `EditorInputRes.0.cursor`
//! - `RemoveTap` and `RemoveTapAt` remove from
//!   `EditorTimelineRes.0.taps.{tap_times,tap_indicator_positions}`
//!   and adjust `selected_index`
//! - `ToggleTimelinePlayback` toggles
//!   `EditorTimelineRes.0.playback.playing`, clears playback seek state,
//!   and swaps `EditorInputRes.0.mode` through `EditorRuntimeRes`
//! - `SetPlaybackSpeed`, `SetWaveformZoom`, and `SetWaveformScroll`
//!   write `EditorTimingRes.0.{playback_speed,waveform_zoom,waveform_scroll}`
//!
//! `Playtest` records an App-world gameplay transition seed; the exclusive
//! gameplay reset system applies the world replacement atomically.

use bevy_app::App;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::{NextState, State};
use eg_editor_domain::{
    EditorPlaytestTransitionParams, add_tap_with_indicator, build_editor_playtest_transition,
    derive_timeline_elapsed_seconds, remove_tap_with_indicator,
};
use eg_game::{LevelMeta, TimelineSimulationRuntime};
use eg_types::{AppPhase, EditorMode};

use crate::state::{
    AudioBackendSystemParam, EditorAudioCacheRes, EditorCameraRes, EditorInputRes,
    EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes, GameplayResetRes,
    GameplayResetSeed, RuntimeAudioPreloadRes, SessionStateRes,
    start_audio_source_at_seconds_for_resources,
};

#[derive(SystemParam)]
pub struct TimelineSeekResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: Res<'w, EditorInputRes>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static crate::state::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<crate::state::EditorAuthoredObject>,
    >,
    spawn: Res<'w, EditorSpawnRes>,
    camera: ResMut<'w, EditorCameraRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
}

/// Toggle timeline playback.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleTimelinePlayback;

/// Shift the timeline cursor by `delta_seconds`.
#[derive(Message, Debug, Clone, Copy)]
pub struct ShiftTimeline(pub f32);

/// Set the timeline cursor to an absolute time in seconds.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimelineTime(pub f32);

/// Set the total duration of the timeline (seconds).
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimelineDuration(pub f32);

/// Add a tap at the current timeline position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddTap;

/// Remove a tap at the current timeline position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RemoveTap;

/// Remove a tap at a specific timeline position.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemoveTapAt(pub f32);

/// Select or deselect a tap by index.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSelectedTap(pub Option<usize>);

/// Update the selected tap's timestamp.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSelectedTapTime(pub f32);

/// Remove all taps from the level.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ClearTaps;

/// Update the playback speed multiplier.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPlaybackSpeed(pub f32);

/// Set waveform view zoom.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetWaveformZoom(pub f32);

/// Set waveform view scroll offset.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetWaveformScroll(pub f32);

/// Start playtesting from the current editor state.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Playtest;

pub fn register_events(app: &mut App) {
    app.add_message::<ToggleTimelinePlayback>()
        .add_message::<ShiftTimeline>()
        .add_message::<SetTimelineTime>()
        .add_message::<SetTimelineDuration>()
        .add_message::<AddTap>()
        .add_message::<RemoveTap>()
        .add_message::<RemoveTapAt>()
        .add_message::<SetSelectedTap>()
        .add_message::<SetSelectedTapTime>()
        .add_message::<ClearTaps>()
        .add_message::<SetPlaybackSpeed>()
        .add_message::<SetWaveformZoom>()
        .add_message::<SetWaveformScroll>()
        .add_message::<Playtest>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

#[derive(SystemParam)]
pub struct TimelinePlaybackResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: ResMut<'w, EditorInputRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    audio: AudioBackendSystemParam<'w>,
    editor_audio: Res<'w, EditorAudioCacheRes>,
    runtime_preload: Res<'w, RuntimeAudioPreloadRes>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static crate::state::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<crate::state::EditorAuthoredObject>,
    >,
    spawn: Res<'w, EditorSpawnRes>,
    session: Res<'w, SessionStateRes>,
}

pub fn handle_toggle_timeline_playback(
    mut events: MessageReader<ToggleTimelinePlayback>,
    mut resources: TimelinePlaybackResources,
) {
    for _ in events.read() {
        if *resources.phase.get() != AppPhase::Editor {
            continue;
        }

        if resources.timeline.0.playback.playing {
            if stop_editor_timeline_playback(
                &mut resources.input,
                &mut resources.runtime,
                &mut resources.timeline,
            ) {
                resources.audio.stop();
            }
        } else {
            start_editor_timeline_playback(&mut resources);
        }
    }
}

pub(crate) fn stop_editor_timeline_playback(
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &mut EditorTimelineRes,
) -> bool {
    if !timeline.0.playback.playing {
        return false;
    }

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
    true
}

fn start_editor_timeline_playback(resources: &mut TimelinePlaybackResources<'_, '_>) {
    let objects = crate::state::collect_editor_object_data(&resources.authored_objects);
    let start_mode = resources.input.0.mode;
    resources.timeline.0.playback.playing = true;
    resources.timeline.0.playback.pending_seek_time_seconds = None;
    resources.timeline.0.playback.seek_resync_cooldown_seconds = 0.0;
    resources.runtime.0.interaction.last_mode = Some(start_mode);
    resources.input.0.mode = EditorMode::Null;

    let start_seconds = if start_mode == EditorMode::Timing {
        resources.timeline.0.playback.runtime = None;
        derive_timeline_elapsed_seconds(
            resources.spawn.0.position,
            resources.spawn.0.direction,
            &resources.timeline.0.taps.tap_times,
            resources.timeline.0.clock.time_seconds,
            &objects,
        )
    } else {
        let mut playback_runtime = TimelineSimulationRuntime::new(
            resources.spawn.0.position,
            resources.spawn.0.direction,
            &objects,
            &resources.timeline.0.taps.tap_times,
        );
        playback_runtime.advance_to(resources.timeline.0.clock.time_seconds);
        let start_seconds = playback_runtime.elapsed_seconds();
        resources.timeline.0.playback.runtime = Some(playback_runtime);
        start_seconds
    };

    let level_name = resources
        .session
        .editor_level_name
        .as_deref()
        .unwrap_or("Untitled");
    start_audio_source_at_seconds_for_resources(
        true,
        false,
        level_name,
        start_seconds,
        &resources.audio,
        &resources.editor_audio,
        &resources.runtime_preload,
    );
}

pub fn handle_shift_timeline(
    mut events: MessageReader<ShiftTimeline>,
    mut resources: TimelineSeekResources,
) {
    for event in events.read() {
        let current_time = resources.timeline.0.clock.time_seconds;
        let next_time =
            (current_time + event.0).clamp(0.0, resources.timeline.0.clock.duration_seconds);
        seek_timeline(&mut resources, next_time);
    }
}

pub fn handle_set_timeline_time(
    mut events: MessageReader<SetTimelineTime>,
    mut resources: TimelineSeekResources,
) {
    for event in events.read() {
        let next_time = event
            .0
            .clamp(0.0, resources.timeline.0.clock.duration_seconds);
        seek_timeline(&mut resources, next_time);
    }
}

pub fn handle_set_timeline_duration(
    mut events: MessageReader<SetTimelineDuration>,
    mut resources: TimelineSeekResources,
) {
    for event in events.read() {
        resources.timeline.0.clock.duration_seconds = event.0.max(0.1);
        resources.timeline.0.invalidate_samples();
        let next_time = resources
            .timeline
            .0
            .clock
            .time_seconds
            .min(resources.timeline.0.clock.duration_seconds);
        seek_timeline(&mut resources, next_time);
    }
}

fn seek_timeline(resources: &mut TimelineSeekResources<'_, '_>, time_seconds: f32) {
    resources.timeline.0.clock.time_seconds = time_seconds;
    queue_playback_seek(&mut resources.timeline);

    if *resources.phase.get() != AppPhase::Editor || resources.timeline.0.playback.playing {
        return;
    }

    let (position, direction) = if let Some(cached) =
        resources.timeline.0.cached_state_at_seconds(time_seconds)
    {
        cached
    } else {
        let revision = resources.timeline.0.simulation_revision;
        let mut runtime = resources
            .timeline
            .0
            .scrub_runtime
            .take()
            .filter(|runtime| {
                resources.timeline.0.scrub_runtime_revision == revision
                    && runtime.elapsed_seconds() <= time_seconds + 1e-6
            })
            .unwrap_or_else(|| {
                let objects = crate::state::collect_editor_object_data(&resources.authored_objects);
                TimelineSimulationRuntime::new(
                    resources.spawn.0.position,
                    resources.spawn.0.direction,
                    &objects,
                    &resources.timeline.0.taps.tap_times,
                )
            });
        runtime.advance_to(time_seconds);
        let snapshot = runtime.snapshot();
        resources.timeline.0.scrub_runtime_revision = revision;
        resources.timeline.0.scrub_runtime = Some(runtime);
        (snapshot.position, snapshot.direction)
    };
    resources.timeline.0.preview.position = position;
    resources.timeline.0.preview.direction = direction;

    // Ctrl deliberately keeps the editor view fixed while scrubbing. The
    // default follows the simulated snake, matching the pre-ECS editor.
    if !resources.input.0.ctrl_held {
        resources.camera.0.editor_pan = [position[0] + 0.5, position[2] + 0.5];
        resources.camera.0.editor_target_z = position[1];
    }
}

fn queue_playback_seek(timeline: &mut EditorTimelineRes) {
    if timeline.0.playback.playing {
        timeline.0.playback.pending_seek_time_seconds = Some(timeline.0.clock.time_seconds);
        timeline.0.playback.seek_resync_cooldown_seconds = 0.12;
    }
}

pub fn handle_add_tap(mut events: MessageReader<AddTap>, mut timeline: ResMut<EditorTimelineRes>) {
    for _ in events.read() {
        let time_seconds = timeline.0.clock.time_seconds;
        let indicator_position = tap_indicator_position_from_world(timeline.0.preview.position);
        let selected_index = {
            let taps = &mut timeline.0.taps;
            add_tap_with_indicator(
                &mut taps.tap_times,
                &mut taps.tap_indicator_positions,
                time_seconds,
                indicator_position,
            )
        };
        timeline.0.taps.selected_index = selected_index;
        timeline.0.invalidate_samples();
    }
}

pub fn handle_remove_tap(
    mut events: MessageReader<RemoveTap>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for _ in events.read() {
        let time_seconds = timeline.0.clock.time_seconds;
        remove_tap_at_time(&mut timeline, time_seconds);
    }
}

pub fn handle_remove_tap_at(
    mut events: MessageReader<RemoveTapAt>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        remove_tap_at_time(&mut timeline, event.0);
    }
}

fn remove_tap_at_time(timeline: &mut EditorTimelineRes, time_seconds: f32) {
    if let Some(removed_index) = remove_tap_with_indicator(
        &mut timeline.0.taps.tap_times,
        &mut timeline.0.taps.tap_indicator_positions,
        time_seconds,
    ) {
        timeline.0.taps.selected_index = match timeline.0.taps.selected_index {
            Some(selected_index) if selected_index == removed_index => None,
            Some(selected_index) if selected_index > removed_index => Some(selected_index - 1),
            selected_index => selected_index,
        };
        timeline.0.invalidate_samples();
    }
}

fn tap_indicator_position_from_world(position: [f32; 3]) -> [f32; 3] {
    [position[0] - 0.5, position[1], position[2] - 0.5]
}

pub fn handle_set_selected_tap(
    mut events: MessageReader<SetSelectedTap>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        timeline.0.taps.selected_index = event.0.filter(|index| {
            *index < timeline.0.taps.tap_times.len()
                && *index < timeline.0.taps.tap_indicator_positions.len()
        });
    }
}

pub fn handle_set_selected_tap_time(
    mut events: MessageReader<SetSelectedTapTime>,
    phase: Res<State<AppPhase>>,
    mut input: ResMut<EditorInputRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        if *phase.get() != AppPhase::Editor || !event.0.is_finite() {
            continue;
        }

        let duration_seconds = timeline.0.clock.duration_seconds.max(0.0);
        let next_time = event.0.clamp(0.0, duration_seconds);
        let taps = &mut timeline.0.taps;
        let Some(selected_index) = taps.selected_index else {
            continue;
        };
        if selected_index >= taps.tap_times.len()
            || selected_index >= taps.tap_indicator_positions.len()
        {
            continue;
        }

        let old_time_seconds = taps.tap_times[selected_index];
        if (old_time_seconds - next_time).abs() <= 0.0001 {
            continue;
        }

        let selected_position = taps.tap_indicator_positions[selected_index];
        taps.tap_times.remove(selected_index);
        taps.tap_indicator_positions.remove(selected_index);
        let next_index = taps
            .tap_times
            .partition_point(|time_seconds| *time_seconds < next_time);
        taps.tap_times.insert(next_index, next_time);
        taps.tap_indicator_positions
            .insert(next_index, selected_position);
        taps.selected_index = Some(next_index);
        timeline.0.clock.time_seconds = next_time;
        input.0.cursor = selected_position;
        timeline.0.invalidate_samples();
    }
}

pub fn handle_clear_taps(
    mut events: MessageReader<ClearTaps>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for _ in events.read() {
        timeline.0.taps.tap_times.clear();
        timeline.0.taps.tap_indicator_positions.clear();
        timeline.0.taps.selected_index = None;
        timeline.0.invalidate_samples();
    }
}

pub fn handle_set_playback_speed(
    mut events: MessageReader<SetPlaybackSpeed>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        timing.0.playback_speed = event.0.clamp(0.1, 2.0);
    }
}

pub fn handle_set_waveform_zoom(
    mut events: MessageReader<SetWaveformZoom>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        timing.0.waveform_zoom = event.0.clamp(0.1, 10.0);
    }
}

pub fn handle_set_waveform_scroll(
    mut events: MessageReader<SetWaveformScroll>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        timing.0.waveform_scroll = event.0;
    }
}

#[derive(SystemParam)]
pub struct PlaytestResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    next_phase: ResMut<'w, NextState<AppPhase>>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static crate::state::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<crate::state::EditorAuthoredObject>,
    >,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    session: ResMut<'w, SessionStateRes>,
    camera: ResMut<'w, EditorCameraRes>,
    input: ResMut<'w, EditorInputRes>,
    gameplay_reset: ResMut<'w, GameplayResetRes>,
    level_meta: ResMut<'w, LevelMeta>,
}

pub fn handle_playtest(mut events: MessageReader<Playtest>, mut resources: PlaytestResources) {
    for _ in events.read() {
        if *resources.phase.get() != AppPhase::Editor {
            continue;
        }

        let objects = crate::state::collect_editor_object_data(&resources.authored_objects);
        let transition = build_editor_playtest_transition(EditorPlaytestTransitionParams {
            objects: &objects,
            level_name: resources.session.editor_level_name.as_deref(),
            spawn: resources.spawn.0.clone(),
            sky_color: resources.session.editor_sky_color,
            tap_times: &resources.timeline.0.taps.tap_times,
            timeline_seconds: (
                resources.timeline.0.clock.time_seconds,
                resources.timeline.0.clock.duration_seconds,
            ),
        });

        resources.timeline.0.playback.playing = false;
        resources.timeline.0.playback.runtime = None;
        resources.next_phase.set(AppPhase::Playing);
        resources.session.game_paused = false;
        resources.session.playtesting_editor = true;
        resources.session.playing_level_name = transition.playing_level_name.clone();
        resources.session.playtest_audio_start_seconds =
            Some(transition.playtest_audio_start_seconds);
        resources.session.playing_sky_color = transition.sky_color;
        resources.session.playing_trigger_base_objects = Some(transition.objects.clone());
        // Snapshot the editor's authored blocks before gameplay reset despawns
        // them. The playtest-exit handlers (Escape, ToggleEditor, level
        // completion) use this snapshot to repopulate the editor.
        resources.session.playtest_editor_objects = Some(objects.clone());
        resources.session.practice_mode_enabled = false;
        resources.session.practice_checkpoints.clear();
        resources.session.audio_stop_pending = true;
        resources.level_meta.initial_objects = transition.objects.clone();
        resources.level_meta.spawn_position = transition.spawn_position;
        resources.level_meta.spawn_direction = transition.spawn_direction;
        resources.level_meta.level_duration_seconds = transition.level_duration_seconds;
        resources.camera.0.playing_rotation = transition.camera_rotation;
        resources.camera.0.playing_pitch = transition.camera_pitch;
        resources.input.0.right_dragging = false;
        resources
            .gameplay_reset
            .request_with_seed(GameplayResetSeed {
                objects: transition.objects,
                position: transition.spawn_position,
                direction: transition.spawn_direction,
                speed: transition.spawn_speed,
                vertical_velocity: transition.spawn_vertical_velocity,
                is_grounded: transition.spawn_is_grounded,
                elapsed_seconds: transition.playtest_audio_start_seconds,
                level_duration_seconds: transition.level_duration_seconds,
                progress: None,
                preserve_practice_state: false,
            });
    }
}
