/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use crate::commands::AppCommand;
use crate::test_utils::{editor_test, stone};
use glam::{Vec2, Vec3};

editor_test!(test_editor_snap_override_with_ctrl, |state| {
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

    assert!(!state.editor.effective_snap_to_grid());
    assert!(!state.editor.effective_snap_rotation());

    // 3. Release Ctrl -> should re-enable effective snapping
    state.dispatch(AppCommand::EditorSetCtrlHeld(false));
    assert!(state.editor.effective_snap_to_grid());
    assert!(state.editor.effective_snap_rotation());

    // 4. Disable setting manually -> effective should be false even without Ctrl
    state.editor.config.snap_to_grid = false;
    assert!(!state.editor.effective_snap_to_grid());
    state.dispatch(AppCommand::EditorSetCtrlHeld(true));
    assert!(!state.editor.effective_snap_to_grid());
});

editor_test!(
    test_editor_cursor_snapping_respects_ctrl_override,
    |state| {
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 1.0;

        state.editor.ui.ctrl_held = false;
        assert!(state.editor.effective_snap_to_grid());

        state.editor.ui.ctrl_held = true;
        assert!(!state.editor.effective_snap_to_grid());
    }
);

editor_test!(test_editor_nudge_respects_ctrl_override, |state| {
    state.editor.config.snap_to_grid = true;
    state.editor.config.snap_step = 2.0;

    state.editor.objects.push(stone(0.0, 0.0, 0.0));
    state.editor.ui.selected_block_index = Some(0);

    let start_x = state.editor.objects[0].position[0];
    state.editor.ui.ctrl_held = false;
    state.editor_nudge_selected_blocks(1, 0);
    let after_first = state.editor.objects[0].position[0];
    assert_eq!(
        (after_first - start_x).abs(),
        2.0,
        "Should use snap step when snapping active"
    );

    state.editor.ui.ctrl_held = true;
    state.editor_nudge_selected_blocks(1, 0);
    let after_second = state.editor.objects[0].position[0];
    assert_eq!(
        (after_second - after_first).abs(),
        1.0,
        "Should use unit step when Ctrl overrides snap"
    );
});

#[test]
fn test_editor_nudge_left_right_screen_direction_regression() {
    pollster::block_on(async {
        let mut state = State::new_test().await;
        state.phase = crate::types::AppPhase::Editor;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 1.0;

        state.editor.objects.push(stone(0.0, 0.0, 0.0));
        state.editor.ui.selected_block_index = Some(0);

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
        assert!(state.editor_nudge_selected_blocks(1, 0));
        assert!(
            screen_x(&state) > baseline_x,
            "Right nudge should move screen-right"
        );

        state.editor.objects[0].position = [0.0, 0.0, 0.0];
        let baseline_x = screen_x(&state);
        assert!(state.editor_nudge_selected_blocks(-1, 0));
        assert!(
            screen_x(&state) < baseline_x,
            "Left nudge should move screen-left"
        );
    });
}
