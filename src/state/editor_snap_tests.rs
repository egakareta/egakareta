/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use super::State;
use crate::commands::AppCommand;
use crate::types::AppPhase;
use glam::{Vec2, Vec3};

#[test]
fn test_editor_snap_override_with_ctrl() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
        state.phase = AppPhase::Editor;

        // 1. Initially enabled
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_rotation = true;
        state.editor.ui.ctrl_held = false;

        assert!(state.editor.snap_to_grid());
        assert!(state.editor.snap_rotation());
        assert!(state.editor.effective_snap_to_grid());
        assert!(state.editor.effective_snap_rotation());

        // 2. Press Ctrl -> should disable effective snapping
        state.dispatch(AppCommand::EditorSetCtrlHeld(true));

        assert!(
            state.editor.snap_to_grid(),
            "Config setting should remain true"
        );
        assert!(
            state.editor.snap_rotation(),
            "Config setting should remain true"
        );
        assert!(
            !state.editor.effective_snap_to_grid(),
            "Effective snapping should be disabled by Ctrl"
        );
        assert!(
            !state.editor.effective_snap_rotation(),
            "Effective rotation snapping should be disabled by Ctrl"
        );

        // 3. Release Ctrl -> should re-enable effective snapping
        state.dispatch(AppCommand::EditorSetCtrlHeld(false));

        assert!(
            state.editor.effective_snap_to_grid(),
            "Effective snapping should re-enable after releasing Ctrl"
        );
        assert!(
            state.editor.effective_snap_rotation(),
            "Effective rotation snapping should re-enable after releasing Ctrl"
        );

        // 4. Disable setting manually -> effective should be false even without Ctrl
        state.editor.config.snap_to_grid = false;
        assert!(!state.editor.effective_snap_to_grid());

        state.dispatch(AppCommand::EditorSetCtrlHeld(true));
        assert!(!state.editor.effective_snap_to_grid());
    });
}

#[test]
fn test_editor_cursor_snapping_respects_ctrl_override() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
        state.phase = AppPhase::Editor;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 1.0;

        state.editor.ui.ctrl_held = false;

        assert!(state.editor.effective_snap_to_grid());

        state.editor.ui.ctrl_held = true;
        assert!(!state.editor.effective_snap_to_grid());
    });
}

#[test]
fn test_editor_nudge_respects_ctrl_override() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
        state.phase = AppPhase::Editor;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 2.0; // Big snap step

        // Add a block at [0, 0, 0]
        state.editor.objects.push(crate::types::LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            name: String::new(),
            group_path: Vec::new(),
        });
        state.editor.ui.selected_block_index = Some(0);

        // 1. Nudge with snapping (Ctrl OFF) -> should move by snap step magnitude
        let start_x = state.editor.objects[0].position[0];
        state.editor.ui.ctrl_held = false;
        state.editor_nudge_selected_blocks(1, 0);
        let after_first = state.editor.objects[0].position[0];
        let delta_first = after_first - start_x;
        assert_eq!(
            delta_first.abs(),
            2.0,
            "Nudge should use snap step magnitude (2.0) when snapping is active"
        );

        // 2. Nudge without snapping (Ctrl ON) -> should move by default unit magnitude
        state.editor.ui.ctrl_held = true;
        state.editor_nudge_selected_blocks(1, 0);
        let after_second = state.editor.objects[0].position[0];
        let delta_second = after_second - after_first;
        assert_eq!(
            delta_second.abs(),
            1.0,
            "Nudge should use unit step magnitude (1.0) when snapping is overridden by Ctrl"
        );
        assert_eq!(
            delta_first.signum(),
            delta_second.signum(),
            "Repeated nudges with same input should move in the same direction"
        );
    });
}

#[test]
fn test_editor_nudge_left_right_screen_direction_regression() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
        state.phase = AppPhase::Editor;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 1.0;

        state.editor.objects.push(crate::types::LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            name: String::new(),
            group_path: Vec::new(),
        });
        state.editor.ui.selected_block_index = Some(0);

        // Make camera orientation deterministic for screen-space assertions.
        state
            .editor
            .set_editor_camera_orientation(0.0, std::f32::consts::FRAC_PI_4, None);

        let viewport = Vec2::new(1280.0, 720.0);
        let screen_x = |state: &State| {
            state
                .editor
                .world_to_screen_v(Vec3::from_array(state.editor.objects[0].position), viewport)
                .map(|pos| pos.x)
                .expect("Selected object should project to screen")
        };

        let baseline_x = screen_x(&state);
        assert!(
            state.editor_nudge_selected_blocks(1, 0),
            "Right nudge should apply when a block is selected"
        );
        let right_x = screen_x(&state);
        assert!(
            right_x > baseline_x,
            "Nudge right should move the selected block to the screen-right direction"
        );

        state.editor.objects[0].position = [0.0, 0.0, 0.0];
        let baseline_x = screen_x(&state);
        assert!(
            state.editor_nudge_selected_blocks(-1, 0),
            "Left nudge should apply when a block is selected"
        );
        let left_x = screen_x(&state);
        assert!(
            left_x < baseline_x,
            "Nudge left should move the selected block to the screen-left direction"
        );
    });
}
