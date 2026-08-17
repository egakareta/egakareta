/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_block_geometry::hitbox_cuboids;
use eg_game::physics::object_xz_contains;
use eg_types::{
    EasingDirection, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
};

pub fn create_block_at_cursor(
    cursor: [f32; 3],
    block_id: &str,
    default_size: [f32; 3],
    rotation: [f32; 3],
) -> LevelObject {
    let trigger = (block_id == TRANSFORM_TRIGGER_BLOCK_ID).then(|| TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 1.0,
        easing: EasingDirection::EaseInOut,
        target: TriggerTarget::Objects {
            object_ids: Vec::new(),
        },
        transform: true,
        ..TimedTrigger::default()
    });

    LevelObject {
        position: cursor,
        size: default_size,
        rotation,
        trigger,
        ..LevelObject::new(block_id)
    }
}

pub fn topmost_block_index_at_cursor(objects: &[LevelObject], cursor: [f32; 3]) -> Option<usize> {
    let mut top_index: Option<usize> = None;
    let mut top_height = f32::NEG_INFINITY;

    for (index, obj) in objects.iter().enumerate() {
        if object_xz_contains(obj, cursor[0] + 0.5, cursor[2] + 0.5) {
            let top = hitbox_cuboids(obj)
                .into_iter()
                .filter(|cuboid| cuboid.max[1].is_finite())
                .map(|cuboid| cuboid.max[1])
                .fold(f32::NEG_INFINITY, f32::max);
            if top > top_height {
                top_height = top;
                top_index = Some(index);
            }
        }
    }

    top_index
}

pub fn remove_topmost_block_at_cursor(objects: &mut Vec<LevelObject>, cursor: [f32; 3]) -> bool {
    let Some(index) = topmost_block_index_at_cursor(objects, cursor) else {
        return false;
    };

    objects.remove(index);
    true
}

/// Move every selected object in the horizontal editor plane and return the
/// resulting cursor position for the selected primary object.
///
/// Invalid and duplicate selection indices are ignored so callers can pass
/// raw UI selection data without moving an object more than once.
pub fn nudge_selected_objects(
    objects: &mut [LevelObject],
    selected_indices: &[usize],
    selected_primary: Option<usize>,
    world_dx: f32,
    world_dz: f32,
) -> Option<[f32; 3]> {
    let mut selected_indices = selected_indices
        .iter()
        .copied()
        .filter(|index| *index < objects.len())
        .collect::<Vec<_>>();
    selected_indices.sort_unstable();
    selected_indices.dedup();

    if selected_indices.is_empty() {
        return None;
    }

    for index in &selected_indices {
        let object = &mut objects[*index];
        object.position[0] += world_dx;
        object.position[2] += world_dz;
    }

    let primary = selected_primary
        .filter(|index| selected_indices.binary_search(index).is_ok())
        .unwrap_or(selected_indices[0]);
    let object = &objects[primary];
    Some([
        object.position[0],
        object.position[1].max(0.0),
        object.position[2],
    ])
}

/// Normalize a raw editor block selection against the authored object count.
///
/// The valid primary selection is included even when the UI's multi-select
/// list omitted it; output is stable, sorted, and duplicate-free.
pub fn normalize_selected_indices(
    selected_indices: &[usize],
    selected_primary: Option<usize>,
    object_count: usize,
) -> Vec<usize> {
    let mut indices = selected_indices
        .iter()
        .copied()
        .filter(|index| *index < object_count)
        .collect::<Vec<_>>();
    if let Some(index) = selected_primary.filter(|index| *index < object_count) {
        indices.push(index);
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}
