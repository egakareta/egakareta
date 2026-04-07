/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::LevelObject;

pub(crate) const MENU_LEVEL_SELECT_BLOCK_INDEX: usize = 0;
pub(crate) const MENU_EDITOR_BLOCK_INDEX: usize = 1;
pub(crate) const MENU_CHARACTER_CUSTOMIZATION_BLOCK_INDEX: usize = 2;

pub(crate) fn create_menu_scene() -> Vec<LevelObject> {
    vec![
        LevelObject {
            position: [-8.0, 0.0, -2.0],
            size: [4.0, 4.0, 4.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.14,
            block_id: "core/stone".to_string(),
            color_tint: [0.82, 0.82, 0.86],
        },
        LevelObject {
            position: [-2.0, 0.0, -2.0],
            size: [4.0, 4.0, 4.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.14,
            block_id: "core/stone".to_string(),
            color_tint: [0.82, 0.82, 0.86],
        },
        LevelObject {
            position: [4.0, 0.0, -2.0],
            size: [4.0, 4.0, 4.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.14,
            block_id: "core/stone".to_string(),
            color_tint: [0.82, 0.82, 0.86],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::create_menu_scene;

    #[test]
    fn menu_scene_uses_three_stone_option_blocks() {
        let objects = create_menu_scene();

        assert_eq!(objects.len(), 3);
        assert!(objects.iter().all(|object| object.block_id == "core/stone"));
    }
}
