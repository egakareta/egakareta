use crate::game::{simulate_timeline_state, BASE_PLAYER_SPEED};
use crate::types::{LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata};

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

pub(crate) struct EditorSessionInit {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) music: MusicMetadata,
    pub(crate) tap_times: Vec<f32>,
    pub(crate) timeline_time_seconds: f32,
    pub(crate) timeline_duration_seconds: f32,
    pub(crate) cursor: [i32; 3],
    pub(crate) camera_pan: [f32; 2],
}

pub(crate) fn editor_session_init_from_metadata(
    metadata: Option<LevelMetadata>,
) -> EditorSessionInit {
    let (
        objects,
        spawn,
        music,
        mut tap_times,
        mut timeline_time_seconds,
        timeline_duration_seconds,
        legacy_taps,
        legacy_timeline_step,
    ) = if let Some(metadata) = metadata {
        (
            metadata.objects,
            metadata.spawn,
            metadata.music,
            metadata.tap_times,
            metadata.timeline_time_seconds,
            metadata.timeline_duration_seconds,
            metadata.legacy_taps,
            metadata.legacy_timeline_step,
        )
    } else {
        (
            Vec::new(),
            SpawnMetadata::default(),
            MusicMetadata::default(),
            Vec::new(),
            0.0,
            16.0,
            Vec::new(),
            0,
        )
    };

    if tap_times.is_empty() && !legacy_taps.is_empty() {
        let seconds_per_step = 1.0 / BASE_PLAYER_SPEED.max(0.1);
        tap_times = legacy_taps
            .iter()
            .copied()
            .map(|step| step as f32 * seconds_per_step)
            .collect();
    }

    if timeline_time_seconds <= 0.0 && legacy_timeline_step > 0 {
        let seconds_per_step = 1.0 / BASE_PLAYER_SPEED.max(0.1);
        timeline_time_seconds = legacy_timeline_step as f32 * seconds_per_step;
    }

    timeline_time_seconds = timeline_time_seconds.clamp(0.0, timeline_duration_seconds.max(0.1));

    tap_times.retain(|tap| tap.is_finite() && *tap >= 0.0);
    tap_times.sort_by(f32::total_cmp);
    let cursor = cursor_from_objects(&objects);
    let camera_pan = camera_pan_from_cursor(cursor);

    EditorSessionInit {
        objects,
        spawn,
        music,
        tap_times,
        timeline_time_seconds,
        timeline_duration_seconds: timeline_duration_seconds.max(0.1),
        cursor,
        camera_pan,
    }
}

fn cursor_from_objects(objects: &[LevelObject]) -> [i32; 3] {
    if let Some(first) = objects.first() {
        [
            first.position[0].round() as i32,
            first.position[1].round() as i32,
            first.position[2].round() as i32,
        ]
    } else {
        [0, 0, 0]
    }
}

fn camera_pan_from_cursor(cursor: [i32; 3]) -> [f32; 2] {
    [cursor[0] as f32 + 0.5, cursor[1] as f32 + 0.5]
}

pub(crate) fn move_cursor_xy(cursor: &mut [i32; 3], dx: i32, dy: i32, bounds: i32) {
    cursor[0] = (cursor[0] + dx).clamp(-bounds, bounds);
    cursor[1] = (cursor[1] + dy).clamp(-bounds, bounds);
}

pub(crate) fn create_block_at_cursor(cursor: [i32; 3], block_id: &str) -> LevelObject {
    LevelObject {
        position: [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: block_id.to_string(),
    }
}

pub(crate) fn remove_topmost_block_at_cursor(
    objects: &mut Vec<LevelObject>,
    cursor: [i32; 3],
) -> bool {
    let mut top_index: Option<usize> = None;
    let mut top_height = f32::NEG_INFINITY;

    for (index, obj) in objects.iter().enumerate() {
        let occupies_x = cursor[0] as f32 + 0.5 >= obj.position[0]
            && cursor[0] as f32 + 0.5 <= obj.position[0] + obj.size[0];
        let occupies_y = cursor[1] as f32 + 0.5 >= obj.position[1]
            && cursor[1] as f32 + 0.5 <= obj.position[1] + obj.size[1];
        if occupies_x && occupies_y {
            let top = obj.position[2] + obj.size[2];
            if top > top_height {
                top_height = top;
                top_index = Some(index);
            }
        }
    }

    if let Some(index) = top_index {
        objects.remove(index);
        true
    } else {
        false
    }
}

pub(crate) fn add_tap_time(tap_times: &mut Vec<f32>, time_seconds: f32) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    let clamped_time = time_seconds.max(0.0);
    if !tap_times
        .iter()
        .any(|existing| (existing - clamped_time).abs() <= TAP_EPSILON_SECONDS)
    {
        tap_times.push(clamped_time);
        tap_times.sort_by(f32::total_cmp);
    }
}

pub(crate) fn remove_tap_time(tap_times: &mut Vec<f32>, time_seconds: f32) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    tap_times.retain(|tap| (tap - time_seconds).abs() > TAP_EPSILON_SECONDS);
}

pub(crate) fn clear_tap_times(tap_times: &mut Vec<f32>) {
    tap_times.clear();
}

pub(crate) fn toggle_spawn_direction(direction: SpawnDirection) -> SpawnDirection {
    match direction {
        SpawnDirection::Forward => SpawnDirection::Right,
        SpawnDirection::Right => SpawnDirection::Forward,
    }
}

pub(crate) fn derive_timeline_position(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> ([f32; 3], SpawnDirection) {
    let state = derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects);
    (state.position, state.direction)
}

pub(crate) fn derive_timeline_elapsed_seconds(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> f32 {
    derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects)
        .elapsed_seconds
}

pub(crate) struct TimelineState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
    pub(crate) elapsed_seconds: f32,
}

pub(crate) fn derive_timeline_state(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> TimelineState {
    let simulated =
        simulate_timeline_state(spawn, direction, objects, tap_times, timeline_time_seconds);

    TimelineState {
        position: simulated.position,
        direction: simulated.direction,
        elapsed_seconds: simulated.elapsed_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        add_tap_time, build_editor_playtest_transition, build_playing_transition_from_metadata,
        clear_tap_times, create_block_at_cursor, editor_session_init_from_metadata, move_cursor_xy,
        playtest_return_objects, remove_tap_time, remove_topmost_block_at_cursor,
    };
    use crate::types::{LevelMetadata, LevelObject, MusicMetadata, SpawnMetadata};

    #[test]
    fn keeps_tap_times_unique_and_sorted() {
        let mut taps = vec![0.4, 0.1];
        add_tap_time(&mut taps, 0.3);
        add_tap_time(&mut taps, 0.1);
        assert_eq!(taps, vec![0.1, 0.3, 0.4]);
    }

    #[test]
    fn can_remove_and_clear_tap_times() {
        let mut taps = vec![0.1, 0.2, 0.3];
        remove_tap_time(&mut taps, 0.2);
        assert_eq!(taps, vec![0.1, 0.3]);
        clear_tap_times(&mut taps);
        assert!(taps.is_empty());
    }

    #[test]
    fn moves_cursor_within_bounds() {
        let mut cursor = [0, 0, 0];
        move_cursor_xy(&mut cursor, 8, -10, 3);
        assert_eq!(cursor, [3, -3, 0]);
    }

    #[test]
    fn creates_block_at_cursor() {
        let block = create_block_at_cursor([1, 2, 3], "core/grass");
        assert_eq!(block.position, [1.0, 2.0, 3.0]);
        assert_eq!(block.size, [1.0, 1.0, 1.0]);
        assert_eq!(block.block_id, "core/grass");
    }

    #[test]
    fn removes_topmost_block_at_cursor_cell() {
        let mut objects = vec![
            LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
            },
            LevelObject {
                position: [0.0, 0.0, 1.0],
                size: [1.0, 1.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/grass".to_string(),
            },
        ];

        let removed = remove_topmost_block_at_cursor(&mut objects, [0, 0, 0]);
        assert!(removed);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn initializes_editor_session_from_metadata() {
        let metadata = LevelMetadata {
            format_version: 1,
            name: "Test".to_string(),
            music: MusicMetadata {
                source: "audio.mp3".to_string(),
                title: None,
                author: None,
                extra: serde_json::Map::new(),
            },
            spawn: SpawnMetadata {
                position: [2.0, 3.0, 1.0],
                direction: crate::types::SpawnDirection::Right,
            },
            tap_times: vec![0.8, 0.2],
            timeline_time_seconds: 0.5,
            timeline_duration_seconds: 16.0,
            legacy_taps: Vec::new(),
            legacy_timeline_step: 0,
            objects: vec![LevelObject {
                position: [4.0, 6.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
            }],
            extra: serde_json::Map::new(),
        };

        let init = editor_session_init_from_metadata(Some(metadata));
        assert_eq!(init.cursor, [4, 6, 0]);
        assert_eq!(init.camera_pan, [4.5, 6.5]);
        assert_eq!(init.tap_times, vec![0.2, 0.8]);
        assert!((init.timeline_time_seconds - 0.5).abs() <= 1e-6);
    }

    #[test]
    fn initializes_editor_session_defaults_without_metadata() {
        let init = editor_session_init_from_metadata(None);
        assert_eq!(init.cursor, [0, 0, 0]);
        assert_eq!(init.camera_pan, [0.5, 0.5]);
        assert_eq!(init.timeline_time_seconds, 0.0);
        assert!(init.tap_times.is_empty());
        assert!(init.objects.is_empty());
    }

    #[test]
    fn builds_editor_playtest_transition() {
        let objects = Vec::new();

        let transition = build_editor_playtest_transition(
            &objects,
            Some("Demo"),
            SpawnMetadata::default(),
            &[],
            1.0 / crate::game::BASE_PLAYER_SPEED,
        );

        assert!(transition.objects.is_empty());
        assert!((transition.spawn_position[1] - 1.5).abs() < 0.1);
        assert!(matches!(
            transition.spawn_direction,
            crate::types::SpawnDirection::Forward
        ));
        assert_eq!(transition.playing_level_name.as_deref(), Some("Demo"));
    }

    #[test]
    fn returns_objects_only_when_playtesting() {
        let objects = vec![LevelObject {
            position: [1.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        }];

        assert!(playtest_return_objects(true, &objects).is_some());
        assert!(playtest_return_objects(false, &objects).is_none());
    }

    #[test]
    fn builds_playing_transition_from_metadata() {
        let metadata = LevelMetadata {
            format_version: 1,
            name: "Starter".to_string(),
            music: MusicMetadata {
                source: "audio.mp3".to_string(),
                title: None,
                author: None,
                extra: serde_json::Map::new(),
            },
            spawn: SpawnMetadata {
                position: [3.0, 4.0, 1.0],
                direction: crate::types::SpawnDirection::Right,
            },
            tap_times: vec![],
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            legacy_taps: Vec::new(),
            legacy_timeline_step: 0,
            objects: vec![LevelObject {
                position: [1.0, 2.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
            }],
            extra: serde_json::Map::new(),
        };

        let transition = build_playing_transition_from_metadata(metadata);
        assert_eq!(transition.level_name, "Starter");
        assert_eq!(transition.spawn_position, [3.0, 4.0, 1.0]);
        assert!(matches!(
            transition.spawn_direction,
            crate::types::SpawnDirection::Right
        ));
        assert_eq!(transition.objects.len(), 1);
    }
}
