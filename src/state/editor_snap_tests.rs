use super::State;
use crate::commands::AppCommand;
use crate::types::AppPhase;

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
        });
        state.editor.ui.selected_block_index = Some(0);

        // 1. Nudge with snapping (Ctrl OFF) -> should move by 2.0
        state.editor.ui.ctrl_held = false;
        state.editor_nudge_selected_blocks(1, 0);
        assert_eq!(
            state.editor.objects[0].position[0], 2.0,
            "Nudge should use snap step (2.0) when snapping is active"
        );

        // 2. Nudge without snapping (Ctrl ON) -> should move by 1.0 (default nudge)
        state.editor.ui.ctrl_held = true;
        state.editor_nudge_selected_blocks(1, 0);
        assert_eq!(
            state.editor.objects[0].position[0], 3.0,
            "Nudge should use unit step (1.0) when snapping is overridden by Ctrl"
        );
    });
}
