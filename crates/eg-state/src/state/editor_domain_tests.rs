/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Domain tests for the App-owned editor resources.

use bevy_state::prelude::State;
use eg_types::{AppPhase, Direction, EditorMode, LevelObject};

use super::{
    EditorConfigRes, EditorInputRes, EditorSpawnRes, collect_editor_objects_from_world,
    replace_editor_objects, test_support::TestApp, with_editor_interaction_context,
};
use crate::events::AppMessageWorldExt;

fn assert_vec3_approx_eq(actual: [f32; 3], expected: [f32; 3], eps: f32) {
    approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = eps);
    approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = eps);
    approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = eps);
}

// ── Snap to grid ───────────────────────────────────────────────────────

#[test]
fn snap_moves_blocks_to_nearest_grid_cell() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(
        app.world_mut(),
        vec![LevelObject {
            position: [0.3, 0.0, 0.7],
            ..LevelObject::default()
        }],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_step = 1.0;
        editor.input.selected_block_index = Some(0);
    });

    app.world_mut()
        .write_app_message(crate::events::selection::SnapSelectionToGrid);
    app.tick_state();

    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [0.0, 0.0, 1.0], 1e-6);
}

#[test]
fn snap_respects_custom_step_size() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(
        app.world_mut(),
        vec![LevelObject {
            position: [1.3, 0.0, 3.8],
            ..LevelObject::default()
        }],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_step = 2.0;
        editor.input.selected_block_index = Some(0);
    });

    app.world_mut()
        .write_app_message(crate::events::selection::SnapSelectionToGrid);
    app.tick_state();

    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [2.0, 0.0, 4.0], 1e-6);
}

#[test]
fn snap_returns_false_when_already_snapped() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(
        app.world_mut(),
        vec![LevelObject {
            position: [3.0, 0.0, 5.0],
            ..LevelObject::default()
        }],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_step = 1.0;
        editor.input.selected_block_index = Some(0);
    });
    let before = collect_editor_objects_from_world(app.world_mut());

    app.world_mut()
        .write_app_message(crate::events::selection::SnapSelectionToGrid);
    app.tick_state();

    assert_eq!(collect_editor_objects_from_world(app.world_mut()), before);
}

// ── Remove selected ────────────────────────────────────────────────────

#[test]
fn remove_selected_deletes_marked_blocks() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(
        app.world_mut(),
        vec![
            LevelObject::default(),
            LevelObject {
                position: [1.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                position: [2.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.selected_block_index = Some(1);
        editor.input.selected_block_indices = vec![1];
    });

    app.world_mut()
        .write_app_message(crate::events::block::RemoveBlock);
    app.tick_state();

    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_eq!(objects.len(), 2);
    assert_vec3_approx_eq(objects[0].position, [0.0, 0.0, 0.0], 1e-6);
    assert_vec3_approx_eq(objects[1].position, [2.0, 0.0, 0.0], 1e-6);
}

#[test]
fn remove_selected_clears_selection() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.selected_block_index = Some(0);
    });

    app.world_mut()
        .write_app_message(crate::events::block::RemoveBlock);
    app.tick_state();

    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.selected_block_index.is_none());
    assert!(input.selected_block_indices.is_empty());
}

// ── Spawn ──────────────────────────────────────────────────────────────

#[test]
fn set_spawn_here_copies_cursor_position() {
    let mut app = TestApp::minimal();
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.cursor = [5.0, 3.0, 7.0];
    });

    app.world_mut()
        .write_app_message(crate::events::spawn::SetSpawnHere);
    app.tick_state();

    assert_vec3_approx_eq(
        app.world().resource::<EditorSpawnRes>().0.position,
        [5.0, 3.0, 7.0],
        1e-6,
    );
}

#[test]
fn rotate_spawn_direction_toggles_between_forward_and_right() {
    let mut app = TestApp::minimal();
    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        Direction::Forward
    );

    app.world_mut()
        .write_app_message(crate::events::spawn::RotateSpawnDirection);
    app.tick_state();
    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        Direction::Right
    );

    app.world_mut()
        .write_app_message(crate::events::spawn::RotateSpawnDirection);
    app.tick_state();
    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        Direction::Forward
    );
}

// ── Selection helpers ──────────────────────────────────────────────────

#[test]
fn clear_block_selection_resets_all_indices() {
    let mut app = TestApp::minimal();
    replace_editor_objects(
        app.world_mut(),
        vec![
            LevelObject::default(),
            LevelObject {
                position: [1.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0, 1];
        editor.clear_block_selection();
    });

    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.selected_block_index.is_none());
    assert!(input.selected_block_indices.is_empty());
}

#[test]
fn selected_indices_normalized_deduplicates() {
    let mut app = TestApp::minimal();
    replace_editor_objects(
        app.world_mut(),
        vec![
            LevelObject::default(),
            LevelObject {
                position: [1.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ],
    );
    let indices = with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0, 0, 1];
        editor.selected_indices_normalized()
    });

    assert_eq!(indices, vec![0, 1]);
}

// ── Config ─────────────────────────────────────────────────────────────

#[test]
fn effective_snap_overridden_by_ctrl() {
    let mut app = TestApp::minimal();
    let initially_enabled = with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_to_grid = true;
        editor.input.ctrl_held = false;
        editor.effective_snap_to_grid()
    });
    assert!(initially_enabled);
    assert!(app.world().resource::<EditorConfigRes>().0.snap_to_grid);

    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.ctrl_held = true;
    });
    assert!(!with_editor_interaction_context(
        app.world_mut(),
        |editor| { editor.effective_snap_to_grid() }
    ));
}

#[test]
fn set_snap_step_clamps_minimum() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .write_app_message(crate::events::snap::SetSnapStep(0.001));
    app.tick_state();

    assert!(app.world().resource::<EditorConfigRes>().0.snap_step >= 0.05);
}

// ── Snap edge cases ────────────────────────────────────────────────────

#[test]
fn snap_returns_false_when_nothing_selected() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(
        app.world_mut(),
        vec![LevelObject {
            position: [0.3, 0.0, 0.7],
            ..LevelObject::default()
        }],
    );
    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_step = 1.0;
    });
    let before = collect_editor_objects_from_world(app.world_mut());

    app.world_mut()
        .write_app_message(crate::events::selection::SnapSelectionToGrid);
    app.tick_state();

    assert_eq!(collect_editor_objects_from_world(app.world_mut()), before);
}

// ── Remove edge cases ──────────────────────────────────────────────────

#[test]
fn remove_selected_returns_false_when_nothing_selected_and_cursor_empty() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);
    with_editor_interaction_context(app.world_mut(), |editor| {
        // Cursor far from any block so remove_topmost_block_at_cursor also fails.
        editor.input.cursor = [100.0, 100.0, 100.0];
    });
    let before = collect_editor_objects_from_world(app.world_mut());

    app.world_mut()
        .write_app_message(crate::events::block::RemoveBlock);
    app.tick_state();

    assert_eq!(collect_editor_objects_from_world(app.world_mut()), before);
}

// ── Rotation snap ──────────────────────────────────────────────────────

#[test]
fn effective_snap_rotation_overridden_by_ctrl() {
    let mut app = TestApp::minimal();
    let initially_enabled = with_editor_interaction_context(app.world_mut(), |editor| {
        editor.config.snap_rotation = true;
        editor.input.ctrl_held = false;
        editor.effective_snap_rotation()
    });
    assert!(initially_enabled);
    assert!(app.world().resource::<EditorConfigRes>().0.snap_rotation);

    with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.ctrl_held = true;
    });
    assert!(!with_editor_interaction_context(
        app.world_mut(),
        |editor| { editor.effective_snap_rotation() }
    ));
}

// ── Multi-select dedup ─────────────────────────────────────────────────

#[test]
fn selected_indices_normalized_sorted() {
    let mut app = TestApp::minimal();
    replace_editor_objects(
        app.world_mut(),
        vec![
            LevelObject::default(),
            LevelObject {
                position: [1.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                position: [2.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ],
    );
    let indices = with_editor_interaction_context(app.world_mut(), |editor| {
        editor.input.selected_block_index = Some(2);
        editor.input.selected_block_indices = vec![2, 0, 1];
        editor.selected_indices_normalized()
    });

    assert_eq!(indices, vec![0, 1, 2]);
}

// ── Spawn direction full cycle ─────────────────────────────────────────

#[test]
fn rotate_spawn_direction_full_cycle() {
    let mut app = TestApp::minimal();
    let initial = app.world().resource::<EditorSpawnRes>().0.direction;

    for _ in 0..4 {
        app.world_mut()
            .write_app_message(crate::events::spawn::RotateSpawnDirection);
    }
    app.tick_state();

    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        initial
    );
}

// ── Mode change ────────────────────────────────────────────────────────

#[test]
fn set_mode_updates_mode_field() {
    let mut app = TestApp::minimal();

    app.world_mut()
        .write_app_message(crate::events::editor_mode::SetEditorMode(EditorMode::Move));
    app.tick_state();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        EditorMode::Move
    );

    app.world_mut()
        .write_app_message(crate::events::editor_mode::SetEditorMode(EditorMode::Scale));
    app.tick_state();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        EditorMode::Scale
    );

    app.world_mut()
        .write_app_message(crate::events::editor_mode::SetEditorMode(
            EditorMode::Rotate,
        ));
    app.tick_state();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        EditorMode::Rotate
    );
}
