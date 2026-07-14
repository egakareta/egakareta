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
use bevy_ecs::prelude::*;
use eg_editor_domain::{
    EditorPlaytestTransitionParams, add_tap_with_indicator, build_editor_playtest_transition,
    remove_tap_with_indicator,
};
use eg_types::{AppPhase, EditorMode};

use crate::state::{
    AppPhaseRes, EditorCameraRes, EditorInputRes, EditorObjectsRes, EditorRuntimeRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, GameplayResetRes, GameplayResetSeed,
    SessionStateRes,
};

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

pub fn handle_toggle_timeline_playback(
    mut events: MessageReader<ToggleTimelinePlayback>,
    phase: Res<AppPhaseRes>,
    mut input: ResMut<EditorInputRes>,
    mut runtime: ResMut<EditorRuntimeRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Editor {
            continue;
        }

        if timeline.0.playback.playing {
            stop_editor_timeline_playback(&mut input, &mut runtime, &mut timeline);
        } else {
            start_editor_timeline_playback(&mut input, &mut runtime, &mut timeline);
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

fn start_editor_timeline_playback(
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &mut EditorTimelineRes,
) {
    timeline.0.playback.playing = true;
    timeline.0.playback.runtime = None;
    timeline.0.playback.pending_seek_time_seconds = None;
    timeline.0.playback.seek_resync_cooldown_seconds = 0.0;
    runtime.0.interaction.last_mode = Some(input.0.mode);
    input.0.mode = EditorMode::Null;
}

pub fn handle_shift_timeline(
    mut events: MessageReader<ShiftTimeline>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        let current_time = timeline.0.clock.time_seconds;
        let next_time = (current_time + event.0).clamp(0.0, timeline.0.clock.duration_seconds);
        timeline.0.clock.time_seconds = next_time;
    }
}

pub fn handle_set_timeline_time(
    mut events: MessageReader<SetTimelineTime>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        timeline.0.clock.time_seconds = event.0.clamp(0.0, timeline.0.clock.duration_seconds);
    }
}

pub fn handle_set_timeline_duration(
    mut events: MessageReader<SetTimelineDuration>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        timeline.0.clock.duration_seconds = event.0.max(0.1);
        timeline.0.clock.time_seconds = timeline
            .0
            .clock
            .time_seconds
            .min(timeline.0.clock.duration_seconds);
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
    phase: Res<AppPhaseRes>,
    mut input: ResMut<EditorInputRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    for event in events.read() {
        if phase.current != AppPhase::Editor || !event.0.is_finite() {
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

pub fn handle_playtest(
    mut events: MessageReader<Playtest>,
    mut phase: ResMut<AppPhaseRes>,
    objects: Res<EditorObjectsRes>,
    spawn: Res<EditorSpawnRes>,
    mut timeline: ResMut<EditorTimelineRes>,
    mut session: ResMut<SessionStateRes>,
    mut camera: ResMut<EditorCameraRes>,
    mut input: ResMut<EditorInputRes>,
    mut gameplay_reset: ResMut<GameplayResetRes>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Editor {
            continue;
        }

        let transition = build_editor_playtest_transition(EditorPlaytestTransitionParams {
            objects: &objects.objects,
            level_name: session.editor_level_name.as_deref(),
            spawn: spawn.0.clone(),
            sky_color: session.editor_sky_color,
            tap_times: &timeline.0.taps.tap_times,
            timeline_seconds: (
                timeline.0.clock.time_seconds,
                timeline.0.clock.duration_seconds,
            ),
        });

        timeline.0.playback.playing = false;
        timeline.0.playback.runtime = None;
        phase.set(AppPhase::Playing);
        session.game_paused = false;
        session.playtesting_editor = true;
        session.playing_level_name = transition.playing_level_name.clone();
        session.playtest_audio_start_seconds = Some(transition.playtest_audio_start_seconds);
        session.playing_sky_color = transition.sky_color;
        session.playing_trigger_base_objects = Some(transition.objects.clone());
        session.practice_mode_enabled = false;
        session.practice_checkpoints.clear();
        session.practice_checkpoints_dirty = true;
        session.audio_stop_pending = true;
        camera.0.playing_rotation = transition.camera_rotation;
        camera.0.playing_pitch = transition.camera_pitch;
        input.0.right_dragging = false;
        gameplay_reset.request_with_seed(GameplayResetSeed {
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
