/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::state::{test_support::TestApp, with_editor_interaction_context};

#[test]
fn test_editor_snap_override_with_ctrl() {
    let mut app = TestApp::minimal();
    // 1. Initially enabled
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_to_grid = true;
        editor.config.snap_rotation = true;
        editor.input.ctrl_held = false;
    });

    with_editor_interaction_context(app.world_mut(), |editor| {
        assert!(editor.effective_snap_to_grid());
        assert!(editor.effective_snap_rotation());
    });

    // 2. Press Ctrl -> should disable effective snapping
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.ctrl_held = true;
    });

    with_editor_interaction_context(app.world_mut(), |editor| {
        assert!(!editor.effective_snap_to_grid());
        assert!(!editor.effective_snap_rotation());
    });

    // 3. Release Ctrl -> should re-enable effective snapping
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.ctrl_held = false;
    });
    with_editor_interaction_context(app.world_mut(), |editor| {
        assert!(editor.effective_snap_to_grid());
        assert!(editor.effective_snap_rotation());
    });

    // 4. Disable setting manually -> effective should be false even without Ctrl
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_to_grid = false;
        assert!(!editor.effective_snap_to_grid());
        editor.input.ctrl_held = true;
        assert!(!editor.effective_snap_to_grid());
    });
}

#[test]
fn test_editor_cursor_snapping_respects_ctrl_override() {
    let mut app = TestApp::minimal();
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_to_grid = true;
        editor.config.snap_step = 1.0;
        editor.input.ctrl_held = false;
    });

    assert!(with_editor_interaction_context(app.world_mut(), |editor| {
        editor.effective_snap_to_grid()
    }));

    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.ctrl_held = true;
    });
    assert!(!with_editor_interaction_context(
        app.world_mut(),
        |editor| editor.effective_snap_to_grid()
    ));
}
