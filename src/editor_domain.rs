use crate::types::{BlockKind, LevelObject, SpawnDirection};

pub(crate) fn move_cursor_xy(cursor: &mut [i32; 3], dx: i32, dy: i32, bounds: i32) {
    cursor[0] = (cursor[0] + dx).clamp(-bounds, bounds);
    cursor[1] = (cursor[1] + dy).clamp(-bounds, bounds);
}

pub(crate) fn create_block_at_cursor(cursor: [i32; 3], kind: BlockKind) -> LevelObject {
    LevelObject {
        position: [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32],
        size: [1.0, 1.0, 1.0],
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
        let o_min_x = obj.position[0];
        let o_max_x = obj.position[0] + obj.size[0];
        let o_min_y = obj.position[1];
        let o_max_y = obj.position[1] + obj.size[1];

        if x >= o_min_x && x < o_max_x && y >= o_min_y && y < o_max_y {
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

#[cfg(test)]
mod tests {
    use super::{
        add_tap_step, clear_tap_steps, create_block_at_cursor, move_cursor_xy, remove_tap_step,
        remove_topmost_block_at_cursor,
    };
    use crate::types::{BlockKind, LevelObject};

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
                kind: BlockKind::Standard,
            },
            LevelObject {
                position: [0.0, 0.0, 1.0],
                size: [1.0, 1.0, 2.0],
                kind: BlockKind::Grass,
            },
        ];

        let removed = remove_topmost_block_at_cursor(&mut objects, [0, 0, 0]);
        assert!(removed);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].position, [0.0, 0.0, 0.0]);
    }
}
