use crate::types::LevelObject;

pub(crate) fn create_block_at_cursor(cursor: [f32; 3], block_id: &str) -> LevelObject {
    LevelObject {
        position: cursor,
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        roundness: 0.18,
        block_id: block_id.to_string(),
        color_tint: [1.0, 1.0, 1.0],
    }
}

pub(crate) fn remove_topmost_block_at_cursor(
    objects: &mut Vec<LevelObject>,
    cursor: [f32; 3],
) -> bool {
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

    if let Some(index) = top_index {
        objects.remove(index);
        true
    } else {
        false
    }
}
