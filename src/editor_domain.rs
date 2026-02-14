use crate::types::{
    BlockKind, LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata,
};

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
    tap_steps: &[u32],
    timeline_step: u32,
) -> EditorPlaytestTransition {
    let (spawn_position, spawn_direction) = derive_timeline_position(
        editor_spawn.position,
        editor_spawn.direction,
        tap_steps,
        timeline_step,
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
    pub(crate) tap_steps: Vec<u32>,
    pub(crate) timeline_step: u32,
    pub(crate) cursor: [i32; 3],
    pub(crate) camera_pan: [f32; 2],
}

pub(crate) fn editor_session_init_from_metadata(
    metadata: Option<LevelMetadata>,
) -> EditorSessionInit {
    let (objects, spawn, music, mut tap_steps, timeline_step) = if let Some(metadata) = metadata {
        (
            metadata.objects,
            metadata.spawn,
            metadata.music,
            metadata.taps,
            metadata.timeline_step,
        )
    } else {
        (
            Vec::new(),
            SpawnMetadata::default(),
            MusicMetadata::default(),
            Vec::new(),
            0,
        )
    };

    tap_steps.sort_unstable();
    let cursor = cursor_from_objects(&objects);
    let camera_pan = camera_pan_from_cursor(cursor);

    EditorSessionInit {
        objects,
        spawn,
        music,
        tap_steps,
        timeline_step,
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

pub(crate) fn create_block_at_cursor(cursor: [i32; 3], kind: BlockKind) -> LevelObject {
    LevelObject {
        position: [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        kind,
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

pub(crate) fn add_tap_step(tap_steps: &mut Vec<u32>, step: u32) {
    if !tap_steps.contains(&step) {
        tap_steps.push(step);
        tap_steps.sort_unstable();
    }
}

pub(crate) fn remove_tap_step(tap_steps: &mut Vec<u32>, step: u32) {
    tap_steps.retain(|tap| *tap != step);
}

pub(crate) fn clear_tap_steps(tap_steps: &mut Vec<u32>) {
    tap_steps.clear();
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
    tap_steps: &[u32],
    step: u32,
    objects: &[LevelObject],
) -> ([f32; 3], SpawnDirection) {
    let mut position = spawn;
    let mut direction = direction;
    let mut tap_index = 0;

    while tap_index < tap_steps.len() && tap_steps[tap_index] == 0 {
        direction = toggle_spawn_direction(direction);
        tap_index += 1;
    }

    const SNAP_DISTANCE: f32 = 0.3;
    let mut current_step = 0;
    for _ in 0..step {
        match direction {
            SpawnDirection::Forward => position[1] += 1.0,
            SpawnDirection::Right => position[0] += 1.0,
        }
        current_step += 1;

        let top = top_surface_height_at(
            objects,
            position[0],
            position[1],
            position[2] + SNAP_DISTANCE,
        );
        position[2] = top;

        while tap_index < tap_steps.len() && tap_steps[tap_index] == current_step {
            direction = toggle_spawn_direction(direction);
            tap_index += 1;
        }
    }

    (position, direction)
}

fn top_surface_height_at(objects: &[LevelObject], x: f32, y: f32, max_z: f32) -> f32 {
    const GROUND_PLANE_HEIGHT: f32 = 0.0;
    let mut top_surface: Option<f32> = None;
    for obj in objects {
        if object_xy_contains(obj, x, y) {
            let top = obj.position[2] + obj.size[2];
            if top <= max_z {
                top_surface = Some(match top_surface {
                    Some(existing) => existing.max(top),
                    None => top,
                });
            }
        }
    }

    top_surface.unwrap_or(GROUND_PLANE_HEIGHT)
}

fn object_xy_contains(obj: &LevelObject, x: f32, y: f32) -> bool {
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
    ];
    let local = rotate_point_around_center_2d([x, y], center, -obj.rotation_degrees.to_radians());
    local[0] >= obj.position[0]
        && local[0] < obj.position[0] + obj.size[0]
        && local[1] >= obj.position[1]
        && local[1] < obj.position[1] + obj.size[1]
}

fn rotate_point_around_center_2d(point: [f32; 2], center: [f32; 2], radians: f32) -> [f32; 2] {
    let sin = radians.sin();
    let cos = radians.cos();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    [
        center[0] + (dx * cos - dy * sin),
        center[1] + (dx * sin + dy * cos),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        add_tap_step, build_editor_playtest_transition, build_playing_transition_from_metadata,
        clear_tap_steps, create_block_at_cursor, editor_session_init_from_metadata, move_cursor_xy,
        playtest_return_objects, remove_tap_step, remove_topmost_block_at_cursor,
    };
    use crate::types::{BlockKind, LevelMetadata, LevelObject, MusicMetadata, SpawnMetadata};

    #[test]
    fn keeps_tap_steps_unique_and_sorted() {
        let mut taps = vec![4, 1];
        add_tap_step(&mut taps, 3);
        add_tap_step(&mut taps, 1);
        assert_eq!(taps, vec![1, 3, 4]);
    }

    #[test]
    fn can_remove_and_clear_tap_steps() {
        let mut taps = vec![1, 2, 3];
        remove_tap_step(&mut taps, 2);
        assert_eq!(taps, vec![1, 3]);
        clear_tap_steps(&mut taps);
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
        let block = create_block_at_cursor([1, 2, 3], BlockKind::Grass);
        assert_eq!(block.position, [1.0, 2.0, 3.0]);
        assert_eq!(block.size, [1.0, 1.0, 1.0]);
        assert!(matches!(block.kind, BlockKind::Grass));
    }

    #[test]
    fn removes_topmost_block_at_cursor_cell() {
        let mut objects = vec![
            LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                kind: BlockKind::Standard,
            },
            LevelObject {
                position: [0.0, 0.0, 1.0],
                size: [1.0, 1.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                kind: BlockKind::Grass,
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
            taps: vec![8, 2],
            timeline_step: 5,
            objects: vec![LevelObject {
                position: [4.0, 6.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                kind: BlockKind::Standard,
            }],
            extra: serde_json::Map::new(),
        };

        let init = editor_session_init_from_metadata(Some(metadata));
        assert_eq!(init.cursor, [4, 6, 0]);
        assert_eq!(init.camera_pan, [4.5, 6.5]);
        assert_eq!(init.tap_steps, vec![2, 8]);
        assert_eq!(init.timeline_step, 5);
    }

    #[test]
    fn initializes_editor_session_defaults_without_metadata() {
        let init = editor_session_init_from_metadata(None);
        assert_eq!(init.cursor, [0, 0, 0]);
        assert_eq!(init.camera_pan, [0.5, 0.5]);
        assert_eq!(init.timeline_step, 0);
        assert!(init.tap_steps.is_empty());
        assert!(init.objects.is_empty());
    }

    #[test]
    fn builds_editor_playtest_transition() {
        let objects = vec![LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            kind: BlockKind::Standard,
        }];

        let transition = build_editor_playtest_transition(
            &objects,
            Some("Demo"),
            SpawnMetadata::default(),
            &[],
            1,
        );

        assert_eq!(transition.objects.len(), 1);
        assert_eq!(transition.spawn_position, [0.0, 1.0, 0.0]);
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
            kind: BlockKind::Standard,
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
            taps: vec![],
            timeline_step: 0,
            objects: vec![LevelObject {
                position: [1.0, 2.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                kind: BlockKind::Standard,
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

    #[test]
    fn rotated_surface_height_detects_inside_point() {
        let objects = vec![LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 3.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            kind: BlockKind::Standard,
        }];

        let top = super::top_surface_height_at(&objects, 1.0, 0.5, 10.0);
        assert_eq!(top, 3.0);
    }

    #[test]
    fn rotated_surface_height_falls_back_to_ground_when_outside() {
        let objects = vec![LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 2.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            kind: BlockKind::Standard,
        }];

        let top = super::top_surface_height_at(&objects, 2.2, 0.5, 10.0);
        assert_eq!(top, 0.0);
    }
}
