use crate::editor_domain::timeline::derive_timeline_position;
use crate::types::{LevelMetadata, LevelObject, SpawnDirection, SpawnMetadata};

pub(crate) struct EditorPlaytestTransition {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn_position: [f32; 3],
    pub(crate) spawn_direction: SpawnDirection,
    pub(crate) playing_level_name: Option<String>,
    pub(crate) camera_rotation: f32,
    pub(crate) camera_pitch: f32,
}

pub(crate) struct PlayingLevelTransition {
    pub(crate) level_name: String,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn_position: [f32; 3],
    pub(crate) spawn_direction: SpawnDirection,
}

pub(crate) fn build_playing_transition_from_metadata(
    metadata: LevelMetadata,
) -> PlayingLevelTransition {
    PlayingLevelTransition {
        level_name: metadata.name,
        objects: metadata.objects,
        spawn_position: metadata.spawn.position,
        spawn_direction: metadata.spawn.direction,
    }
}

pub(crate) fn build_editor_playtest_transition(
    editor_objects: &[LevelObject],
    editor_level_name: Option<&str>,
    editor_spawn: SpawnMetadata,
    tap_times: &[f32],
    timeline_time_seconds: f32,
) -> EditorPlaytestTransition {
    let (spawn_position, spawn_direction) = derive_timeline_position(
        editor_spawn.position,
        editor_spawn.direction,
        tap_times,
        timeline_time_seconds,
        editor_objects,
    );

    EditorPlaytestTransition {
        objects: editor_objects.to_vec(),
        spawn_position,
        spawn_direction,
        playing_level_name: editor_level_name.map(|name| name.to_string()),
        camera_rotation: -45.0f32.to_radians(),
        camera_pitch: 45.0f32.to_radians(),
    }
}

pub(crate) fn playtest_return_objects(
    playtesting_editor: bool,
    editor_objects: &[LevelObject],
) -> Option<Vec<LevelObject>> {
    if playtesting_editor {
        Some(editor_objects.to_vec())
    } else {
        None
    }
}
