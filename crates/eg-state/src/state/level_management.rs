/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_types::{Direction, LevelMetadata, SpawnMetadata};
use glam::Vec3;

pub(crate) fn menu_preview_camera_for_metadata(metadata: &LevelMetadata) -> ([f32; 3], [f32; 3]) {
    metadata
        .menu_preview_camera
        .as_ref()
        .map(|preview| (preview.position, preview.target))
        .unwrap_or_else(|| auto_menu_preview_camera_from_spawn(&metadata.spawn))
}

fn auto_menu_preview_camera_from_spawn(spawn: &SpawnMetadata) -> ([f32; 3], [f32; 3]) {
    // Tuned to keep a broad, readable menu composition:
    // - target a little ahead/above the spawn so the route direction is visible
    // - place camera behind and to the side with elevated height for coverage
    const PREVIEW_TARGET_FORWARD_DISTANCE: f32 = 10.0;
    const PREVIEW_TARGET_HEIGHT_OFFSET: f32 = 1.0;
    const PREVIEW_CAMERA_BACK_DISTANCE: f32 = 20.0;
    const PREVIEW_CAMERA_SIDE_OFFSET: f32 = 12.0;
    const PREVIEW_CAMERA_HEIGHT_OFFSET: f32 = 14.0;

    let spawn_position = Vec3::from_array(spawn.position);
    let forward = match spawn.direction {
        Direction::Forward => Vec3::new(0.0, 0.0, 1.0),
        Direction::Right => Vec3::new(1.0, 0.0, 0.0),
    };
    let side = Vec3::new(forward.z, 0.0, -forward.x);
    let target = spawn_position
        + forward * PREVIEW_TARGET_FORWARD_DISTANCE
        + Vec3::new(0.0, PREVIEW_TARGET_HEIGHT_OFFSET, 0.0);
    let eye = spawn_position - forward * PREVIEW_CAMERA_BACK_DISTANCE
        + side * PREVIEW_CAMERA_SIDE_OFFSET
        + Vec3::new(0.0, PREVIEW_CAMERA_HEIGHT_OFFSET, 0.0);
    (eye.to_array(), target.to_array())
}

#[cfg(test)]
mod tests {
    use super::{auto_menu_preview_camera_from_spawn, menu_preview_camera_for_metadata};
    use eg_types::{
        BasicLevelMetadata, Direction, EditorStateParams, LevelMetadata,
        LevelPreviewCameraMetadata, MusicMetadata, SpawnMetadata,
    };

    #[test]
    fn menu_preview_camera_for_metadata_prefers_manual_camera() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            basic: BasicLevelMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: LevelMetadata::default().sky_color,
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            menu_preview_camera: Some(LevelPreviewCameraMetadata {
                position: [1.0, 2.0, 3.0],
                target: [4.0, 5.0, 6.0],
            }),
            objects: Vec::new(),
        });

        assert_eq!(
            menu_preview_camera_for_metadata(&metadata),
            ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])
        );
    }

    #[test]
    fn auto_menu_preview_camera_positions_follow_spawn_direction() {
        let (forward_eye, forward_target) = auto_menu_preview_camera_from_spawn(&SpawnMetadata {
            position: [2.0, 3.0, 4.0],
            direction: Direction::Forward,
        });
        assert_eq!(forward_eye, [14.0, 17.0, -16.0]);
        assert_eq!(forward_target, [2.0, 4.0, 14.0]);

        let (right_eye, right_target) = auto_menu_preview_camera_from_spawn(&SpawnMetadata {
            position: [2.0, 3.0, 4.0],
            direction: Direction::Right,
        });
        assert_eq!(right_eye, [-18.0, 17.0, -8.0]);
        assert_eq!(right_target, [12.0, 4.0, 4.0]);
    }
}
