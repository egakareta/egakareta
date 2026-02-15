use crate::types::LevelObject;

pub(crate) fn create_menu_scene() -> Vec<LevelObject> {
    let mut objects = Vec::new();

    // Create a base platform
    for x in -5..6 {
        for y in -5..6 {
            let height = if (x * x + y * y) < 8 {
                0.0
            } else if (x + y) % 2 == 0 {
                -1.0
            } else {
                -2.0
            };

            objects.push(LevelObject {
                position: [x as f32 * 2.0, y as f32 * 2.0, height],
                size: [2.0, 2.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/grass".to_string(),
            });
        }
    }

    objects
}
