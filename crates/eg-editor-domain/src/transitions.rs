/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::timeline::derive_timeline_state;
use eg_types::{DEFAULT_CAMERA_PITCH, DEFAULT_CAMERA_YAW, Direction, LevelObject, SpawnMetadata};

pub struct EditorPlaytestTransition {
    pub objects: Vec<LevelObject>,
    pub sky_color: [f32; 3],
    pub spawn_position: [f32; 3],
    pub spawn_direction: Direction,
    pub spawn_speed: f32,
    pub spawn_vertical_velocity: f32,
    pub spawn_is_grounded: bool,
    pub playtest_audio_start_seconds: f32,
    pub level_duration_seconds: f32,
    pub playing_level_name: Option<String>,
    pub camera_rotation: f32,
    pub camera_pitch: f32,
}

pub struct EditorPlaytestTransitionParams<'a> {
    pub objects: &'a [LevelObject],
    pub level_name: Option<&'a str>,
    pub spawn: SpawnMetadata,
    pub sky_color: [f32; 3],
    pub tap_times: &'a [f32],
    pub timeline_seconds: (f32, f32),
}

// TODO: FIX THIS STUPID TRANSITION SHIT
pub fn build_editor_playtest_transition(
    params: EditorPlaytestTransitionParams<'_>,
) -> EditorPlaytestTransition {
    let EditorPlaytestTransitionParams {
        objects,
        level_name,
        spawn,
        sky_color,
        tap_times,
        timeline_seconds,
    } = params;
    let (timeline_time_seconds, timeline_duration_seconds) = timeline_seconds;
    let state = derive_timeline_state(
        spawn.position,
        spawn.direction,
        tap_times,
        timeline_time_seconds,
        objects,
    );

    EditorPlaytestTransition {
        objects: objects.to_vec(),
        sky_color,
        spawn_position: state.position,
        spawn_direction: state.direction,
        spawn_speed: state.speed,
        spawn_vertical_velocity: state.vertical_velocity,
        spawn_is_grounded: state.is_grounded,
        playtest_audio_start_seconds: state.elapsed_seconds,
        level_duration_seconds: timeline_duration_seconds,
        playing_level_name: level_name.map(|name| name.to_string()),
        camera_rotation: DEFAULT_CAMERA_YAW,
        camera_pitch: DEFAULT_CAMERA_PITCH,
    }
}
