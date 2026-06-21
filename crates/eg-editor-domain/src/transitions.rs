/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::timeline::derive_timeline_state_with_triggers;
use eg_types::TimedTrigger;
use eg_types::{
    LevelMetadata, LevelObject, SpawnDirection, SpawnMetadata, DEFAULT_PLAY_CAMERA_PITCH,
    DEFAULT_PLAY_CAMERA_ROTATION,
};

pub struct EditorPlaytestTransition {
    pub objects: Vec<LevelObject>,
    pub sky_color: [f32; 3],
    pub spawn_position: [f32; 3],
    pub spawn_direction: SpawnDirection,
    pub spawn_speed: f32,
    pub spawn_vertical_velocity: f32,
    pub spawn_is_grounded: bool,
    pub playtest_audio_start_seconds: f32,
    pub level_duration_seconds: f32,
    pub playing_level_name: Option<String>,
    pub camera_rotation: f32,
    pub camera_pitch: f32,
}

pub struct PlayingLevelTransition {
    pub level_name: String,
    pub objects: Vec<LevelObject>,
    pub sky_color: [f32; 3],
    pub spawn_position: [f32; 3],
    pub spawn_direction: SpawnDirection,
    pub level_duration_seconds: f32,
}

pub fn build_playing_transition_from_metadata(metadata: LevelMetadata) -> PlayingLevelTransition {
    PlayingLevelTransition {
        level_name: metadata.name,
        objects: metadata.objects,
        sky_color: metadata.sky_color,
        spawn_position: metadata.spawn.position,
        spawn_direction: metadata.spawn.direction,
        level_duration_seconds: metadata.timeline_duration_seconds,
    }
}

pub struct EditorPlaytestTransitionParams<'a> {
    pub objects: &'a [LevelObject],
    pub level_name: Option<&'a str>,
    pub spawn: SpawnMetadata,
    pub sky_color: [f32; 3],
    pub tap_times: &'a [f32],
    pub triggers: &'a [TimedTrigger],
    pub timeline_seconds: (f32, f32),
}

pub fn build_editor_playtest_transition(
    params: EditorPlaytestTransitionParams<'_>,
) -> EditorPlaytestTransition {
    let EditorPlaytestTransitionParams {
        objects,
        level_name,
        spawn,
        sky_color,
        tap_times,
        triggers,
        timeline_seconds,
    } = params;
    let (timeline_time_seconds, timeline_duration_seconds) = timeline_seconds;
    let state = derive_timeline_state_with_triggers(
        spawn.position,
        spawn.direction,
        tap_times,
        timeline_time_seconds,
        objects,
        triggers,
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
        camera_rotation: DEFAULT_PLAY_CAMERA_ROTATION,
        camera_pitch: DEFAULT_PLAY_CAMERA_PITCH,
    }
}

pub fn playtest_return_objects(
    playtesting_editor: bool,
    editor_objects: &[LevelObject],
) -> Option<Vec<LevelObject>> {
    if playtesting_editor {
        Some(editor_objects.to_vec())
    } else {
        None
    }
}
