/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[tokio::test]
async fn test_editor_snap_override_with_ctrl() {
    let mut state = crate::state::State::new_test().await;
    state.phase = eg_types::AppPhase::Editor;
    // 1. Initially enabled
    state.editor.config.snap_to_grid = true;
    state.editor.config.snap_rotation = true;
    state.editor.input.ctrl_held = false;

    assert!(state.editor.snap_to_grid());
    assert!(state.editor.snap_rotation());
    assert!(state.editor.effective_snap_to_grid());
    assert!(state.editor.effective_snap_rotation());

    // 2. Press Ctrl -> should disable effective snapping
    state.set_editor_ctrl_held(true);

    assert!(!state.editor.effective_snap_to_grid());
    assert!(!state.editor.effective_snap_rotation());

    // 3. Release Ctrl -> should re-enable effective snapping
    state.set_editor_ctrl_held(false);
    assert!(state.editor.effective_snap_to_grid());
    assert!(state.editor.effective_snap_rotation());

    // 4. Disable setting manually -> effective should be false even without Ctrl
    state.editor.config.snap_to_grid = false;
    assert!(!state.editor.effective_snap_to_grid());
    state.set_editor_ctrl_held(true);
    assert!(!state.editor.effective_snap_to_grid());
}

#[tokio::test]
async fn test_editor_cursor_snapping_respects_ctrl_override() {
    let mut state = crate::state::State::new_test().await;
    state.phase = eg_types::AppPhase::Editor;
    state.editor.config.snap_to_grid = true;
    state.editor.config.snap_step = 1.0;

    state.editor.input.ctrl_held = false;
    assert!(state.editor.effective_snap_to_grid());

    state.editor.input.ctrl_held = true;
    assert!(!state.editor.effective_snap_to_grid());
}
