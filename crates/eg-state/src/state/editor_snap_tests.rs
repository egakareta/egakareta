/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[test]
fn test_editor_snap_override_with_ctrl() {
    let mut editor = crate::state::EditorSubsystem::default();
    // 1. Initially enabled
    editor.config.snap_to_grid = true;
    editor.config.snap_rotation = true;
    editor.input.ctrl_held = false;

    assert!(editor.snap_to_grid());
    assert!(editor.snap_rotation());
    assert!(editor.effective_snap_to_grid());
    assert!(editor.effective_snap_rotation());

    // 2. Press Ctrl -> should disable effective snapping
    editor.input.ctrl_held = true;

    assert!(!editor.effective_snap_to_grid());
    assert!(!editor.effective_snap_rotation());

    // 3. Release Ctrl -> should re-enable effective snapping
    editor.input.ctrl_held = false;
    assert!(editor.effective_snap_to_grid());
    assert!(editor.effective_snap_rotation());

    // 4. Disable setting manually -> effective should be false even without Ctrl
    editor.config.snap_to_grid = false;
    assert!(!editor.effective_snap_to_grid());
    editor.input.ctrl_held = true;
    assert!(!editor.effective_snap_to_grid());
}

#[test]
fn test_editor_cursor_snapping_respects_ctrl_override() {
    let mut editor = crate::state::EditorSubsystem::default();
    editor.config.snap_to_grid = true;
    editor.config.snap_step = 1.0;

    editor.input.ctrl_held = false;
    assert!(editor.effective_snap_to_grid());

    editor.input.ctrl_held = true;
    assert!(!editor.effective_snap_to_grid());
}
