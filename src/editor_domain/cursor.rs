use crate::types::LevelObject;

#[allow(dead_code)]
pub(crate) fn move_cursor_xy(cursor: &mut [f32; 3], dx: i32, dy: i32, bounds: i32) {
    cursor[0] = (cursor[0] + dx as f32).clamp(-bounds as f32, bounds as f32);
    cursor[1] = (cursor[1] + dy as f32).clamp(-bounds as f32, bounds as f32);
}

pub(crate) fn create_block_at_cursor(cursor: [f32; 3], block_id: &str) -> LevelObject {
    LevelObject {
        position: cursor,
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: block_id.to_string(),
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
        let occupies_y =
            cursor[1] + 0.5 >= obj.position[1] && cursor[1] + 0.5 <= obj.position[1] + obj.size[1];
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
