/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::editor_command::EditorCommand;
use crate::test_utils::editor_test;

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
    state.dispatch(AppCommand::Editor(EditorCommand::SetCtrlHeld(true)));

    assert!(!state.editor.effective_snap_to_grid());
    assert!(!state.editor.effective_snap_rotation());

    // 3. Release Ctrl -> should re-enable effective snapping
    state.dispatch(AppCommand::Editor(EditorCommand::SetCtrlHeld(false)));
    assert!(state.editor.effective_snap_to_grid());
    assert!(state.editor.effective_snap_rotation());

    // 4. Disable setting manually -> effective should be false even without Ctrl
    state.editor.config.snap_to_grid = false;
    assert!(!state.editor.effective_snap_to_grid());
    state.dispatch(AppCommand::Editor(EditorCommand::SetCtrlHeld(true)));
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
