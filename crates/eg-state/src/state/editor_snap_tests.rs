/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[test]
fn test_editor_snap_override_with_ctrl() {
    // 1. Initially enabled
    let snap_to_grid = true;
    let snap_rotation = true;
    let ctrl_held = false;

    assert!(snap_to_grid);
    assert!(snap_rotation);
    assert!(crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
    assert!(crate::state::effective_snap_enabled(
        snap_rotation,
        ctrl_held
    ));

    // 2. Press Ctrl -> should disable effective snapping
    let ctrl_held = true;

    assert!(!crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
    assert!(!crate::state::effective_snap_enabled(
        snap_rotation,
        ctrl_held
    ));

    // 3. Release Ctrl -> should re-enable effective snapping
    let ctrl_held = false;
    assert!(crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
    assert!(crate::state::effective_snap_enabled(
        snap_rotation,
        ctrl_held
    ));

    // 4. Disable setting manually -> effective should be false even without Ctrl
    let snap_to_grid = false;
    assert!(!crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
    let ctrl_held = true;
    assert!(!crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
}

#[test]
fn test_editor_cursor_snapping_respects_ctrl_override() {
    let snap_to_grid = true;

    let ctrl_held = false;
    assert!(crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));

    let ctrl_held = true;
    assert!(!crate::state::effective_snap_enabled(
        snap_to_grid,
        ctrl_held
    ));
}
