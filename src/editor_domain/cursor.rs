/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::triggers::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};
use crate::types::{LevelObject, TRANSFORM_TRIGGER_BLOCK_ID};

pub(crate) fn create_block_at_cursor(
    cursor: [f32; 3],
    block_id: &str,
    default_size: [f32; 3],
    rotation_degrees: [f32; 3],
) -> LevelObject {
    let trigger = (block_id == TRANSFORM_TRIGGER_BLOCK_ID).then(|| TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 1.0,
        easing: TimedTriggerEasing::EaseInOut,
        target: TimedTriggerTarget::Objects {
            object_ids: Vec::new(),
        },
        action: TimedTriggerAction::TransformObjects {
            position: cursor,
            rotation_degrees,
            size: default_size,
        },
    });

    LevelObject {
        position: cursor,
        size: default_size,
        rotation_degrees,
        block_id: block_id.to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger,
    }
}

pub(crate) fn topmost_block_index_at_cursor(
    objects: &[LevelObject],
    cursor: [f32; 3],
) -> Option<usize> {
    let mut top_index: Option<usize> = None;
    let mut top_height = f32::NEG_INFINITY;

    for (index, obj) in objects.iter().enumerate() {
        let occupies_x =
            cursor[0] + 0.5 >= obj.position[0] && cursor[0] + 0.5 <= obj.position[0] + obj.size[0];
        let occupies_z =
            cursor[2] + 0.5 >= obj.position[2] && cursor[2] + 0.5 <= obj.position[2] + obj.size[2];
        if occupies_x && occupies_z {
            let top = obj.position[1] + obj.size[1];
            if top > top_height {
                top_height = top;
                top_index = Some(index);
            }
        }
    }

    top_index
}

pub(crate) fn remove_topmost_block_at_cursor(
    objects: &mut Vec<LevelObject>,
    cursor: [f32; 3],
) -> bool {
    let Some(index) = topmost_block_index_at_cursor(objects, cursor) else {
        return false;
    };

    objects.remove(index);
    true
}
