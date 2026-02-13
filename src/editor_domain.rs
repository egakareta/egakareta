use crate::types::{LevelObject, SpawnDirection};

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
