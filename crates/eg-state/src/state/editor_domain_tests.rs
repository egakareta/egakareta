/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Focused editor-domain and App-world tests.
//!
//! These tests exercise editor logic **without GPU, audio, or rendering**.
//! They use pure helpers or [`TestApp`] with authored ECS objects.

use bevy::state::prelude::State;
use eg_editor_domain::{normalize_selected_indices, nudge_selected_objects};
use eg_types::{AppPhase, Direction, LevelObject};

use super::{
    EditorConfigRes, EditorInputRes, EditorRuntimeRes, EditorSpawnRes,
    collect_editor_objects_from_world, effective_snap_enabled, replace_editor_objects,
    test_support::TestApp,
};
use crate::events::{
    block::RemoveBlock,
    editor_mode::SetEditorMode,
    selection::SnapSelectionToGrid,
    snap::SetSnapStep,
    spawn::{RotateSpawnDirection, SetSpawnHere},
};

fn assert_vec3_approx_eq(actual: [f32; 3], expected: [f32; 3], eps: f32) {
    approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = eps);
    approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = eps);
    approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = eps);
}

fn snap_test_app(objects: Vec<LevelObject>, selected_indices: Vec<usize>, step: f32) -> TestApp {
    let mut app = TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    replace_editor_objects(app.world_mut(), objects);
    {
        let mut input = app.world_mut().resource_mut::<EditorInputRes>();
        input.0.selected_block_index = selected_indices.first().copied();
        input.0.selected_block_indices = selected_indices;
    }
    app.world_mut()
        .resource_mut::<EditorConfigRes>()
        .0
        .snap_step = step;
    app
}

// ── Nudge ──────────────────────────────────────────────────────────────

#[test]
fn nudge_moves_selected_blocks_by_world_delta() {
    let mut objects = vec![
        LevelObject::default(),
        LevelObject {
            position: [3.0, 0.0, 0.0],
            ..LevelObject::default()
        },
    ];

    assert_eq!(
        nudge_selected_objects(&mut objects, &[0], Some(0), 2.0, -1.0),
        Some([2.0, 0.0, -1.0])
    );
    assert_vec3_approx_eq(objects[0].position, [2.0, 0.0, -1.0], 1e-6);
    assert_vec3_approx_eq(objects[1].position, [3.0, 0.0, 0.0], 1e-6);
}

#[test]
fn nudge_returns_false_when_nothing_selected() {
    let mut objects = vec![LevelObject::default()];

    assert!(nudge_selected_objects(&mut objects, &[], None, 1.0, 0.0).is_none());
    assert_vec3_approx_eq(objects[0].position, [0.0, 0.0, 0.0], 1e-6);
}

#[test]
fn nudge_updates_cursor_to_selected_block_position() {
    let mut objects = vec![LevelObject {
        position: [1.0, 2.0, 3.0],
        ..LevelObject::default()
    }];

    let cursor = nudge_selected_objects(&mut objects, &[0], Some(0), 5.0, -2.0)
        .expect("selection moves a cursor");
    assert_vec3_approx_eq(cursor, [6.0, 2.0, 1.0], 1e-6);
}

// ── Snap to grid ───────────────────────────────────────────────────────

#[test]
fn snap_moves_blocks_to_nearest_grid_cell() {
    let mut app = snap_test_app(
        vec![LevelObject {
            position: [0.3, 0.0, 0.7],
            ..LevelObject::default()
        }],
        vec![0],
        1.0,
    );

    app.world_mut().write_message(SnapSelectionToGrid);
    app.tick();
    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [0.0, 0.0, 1.0], 1e-6);
}

#[test]
fn snap_respects_custom_step_size() {
    let mut app = snap_test_app(
        vec![LevelObject {
            position: [1.3, 0.0, 3.8],
            ..LevelObject::default()
        }],
        vec![0],
        2.0,
    );

    app.world_mut().write_message(SnapSelectionToGrid);
    app.tick();
    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [2.0, 0.0, 4.0], 1e-6);
}

#[test]
fn snap_returns_false_when_already_snapped() {
    let mut app = snap_test_app(
        vec![LevelObject {
            position: [3.0, 0.0, 5.0],
            ..LevelObject::default()
        }],
        vec![0],
        1.0,
    );

    app.world_mut().write_message(SnapSelectionToGrid);
    app.tick();
    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [3.0, 0.0, 5.0], 1e-6);
}

// ── Remove selected ────────────────────────────────────────────────────

#[test]
fn remove_selected_deletes_marked_blocks() {
    let mut app = snap_test_app(
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
        vec![1],
        1.0,
    );

    app.world_mut().write_message(RemoveBlock);
    app.tick();
    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_eq!(objects.len(), 2);
    assert_vec3_approx_eq(objects[0].position, [0.0, 0.0, 0.0], 1e-6);
    assert_vec3_approx_eq(objects[1].position, [2.0, 0.0, 0.0], 1e-6);
}

#[test]
fn remove_selected_clears_selection() {
    let mut app = snap_test_app(vec![LevelObject::default()], vec![0], 1.0);

    app.world_mut().write_message(RemoveBlock);
    app.tick();
    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.selected_block_index.is_none());
    assert!(input.selected_block_indices.is_empty());
}

// ── Spawn ──────────────────────────────────────────────────────────────

#[test]
fn set_spawn_here_copies_cursor_position() {
    let mut app = TestApp::minimal();
    app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [5.0, 3.0, 7.0];

    app.world_mut().write_message(SetSpawnHere);
    app.tick();
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

    app.world_mut().write_message(RotateSpawnDirection);
    app.tick();
    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        Direction::Right
    );

    app.world_mut().write_message(RotateSpawnDirection);
    app.tick();
    assert_eq!(
        app.world().resource::<EditorSpawnRes>().0.direction,
        Direction::Forward
    );
}

// ── Selection helpers ──────────────────────────────────────────────────

#[test]
fn clear_block_selection_resets_all_indices() {
    let mut app = TestApp::minimal();
    {
        let mut input = app.world_mut().resource_mut::<EditorInputRes>();
        input.0.selected_block_index = Some(0);
        input.0.selected_block_indices = vec![0, 1];
    }

    app.world_mut()
        .write_message(SetEditorMode(eg_types::EditorMode::Null));
    app.tick();
    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.selected_block_index.is_none());
    assert!(input.selected_block_indices.is_empty());
}

#[test]
fn selected_indices_normalized_deduplicates() {
    let indices = normalize_selected_indices(&[0, 0, 1], Some(0), 2);
    assert_eq!(indices, vec![0, 1]);
}

// ── Mode ───────────────────────────────────────────────────────────────

#[test]
fn set_mode_clears_drags_and_marquee() {
    let mut app = TestApp::minimal();
    app.world_mut()
        .resource_mut::<EditorRuntimeRes>()
        .0
        .interaction
        .gizmo_drag = Some(super::EditorGizmoDrag {
        axis: eg_mesh::GizmoAxis::X,
        kind: eg_mesh::GizmoDragKind::Move,
        start_mouse: [0.0, 0.0],
        start_center_screen: [0.0, 0.0],
        start_center_world: [0.0, 0.0, 0.0],
        start_blocks: Vec::new(),
    });
    app.world_mut()
        .resource_mut::<EditorInputRes>()
        .0
        .marquee_start_screen = Some([10.0, 10.0]);

    app.world_mut()
        .write_message(SetEditorMode(eg_types::EditorMode::Select));
    app.tick();
    assert!(
        app.world()
            .resource::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag
            .is_none()
    );
    assert!(
        app.world()
            .resource::<EditorInputRes>()
            .0
            .marquee_start_screen
            .is_none()
    );
}

// ── Config ─────────────────────────────────────────────────────────────

#[test]
fn effective_snap_overridden_by_ctrl() {
    let snap_to_grid = true;
    assert!(effective_snap_enabled(snap_to_grid, false));

    assert!(!effective_snap_enabled(snap_to_grid, true));
}

#[test]
fn set_snap_step_clamps_minimum() {
    let mut app = TestApp::minimal();
    app.world_mut().write_message(SetSnapStep(0.001));
    app.tick();
    assert!(
        app.world().resource::<EditorConfigRes>().0.snap_step >= 0.05,
        "Snap step should be clamped to at least 0.05"
    );
}

// ── Nudge edge cases ───────────────────────────────────────────────────

#[test]
fn nudge_moves_all_selected_blocks() {
    let mut objects = vec![
        LevelObject::default(),
        LevelObject {
            position: [1.0, 0.0, 0.0],
            ..LevelObject::default()
        },
        LevelObject {
            position: [2.0, 0.0, 0.0],
            ..LevelObject::default()
        },
    ];

    assert!(nudge_selected_objects(&mut objects, &[0, 2], Some(0), 3.0, -1.0).is_some());
    assert_vec3_approx_eq(objects[0].position, [3.0, 0.0, -1.0], 1e-6);
    assert_vec3_approx_eq(objects[1].position, [1.0, 0.0, 0.0], 1e-6);
    assert_vec3_approx_eq(objects[2].position, [5.0, 0.0, -1.0], 1e-6);
}

// ── Snap edge cases ────────────────────────────────────────────────────

#[test]
fn snap_returns_false_when_nothing_selected() {
    let mut app = snap_test_app(
        vec![LevelObject {
            position: [0.3, 0.0, 0.7],
            ..LevelObject::default()
        }],
        Vec::new(),
        1.0,
    );

    app.world_mut().write_message(SnapSelectionToGrid);
    app.tick();
    let objects = collect_editor_objects_from_world(app.world_mut());
    assert_vec3_approx_eq(objects[0].position, [0.3, 0.0, 0.7], 1e-6);
}

// ── Remove edge cases ──────────────────────────────────────────────────

#[test]
fn remove_selected_returns_false_when_nothing_selected_and_cursor_empty() {
    let mut app = snap_test_app(vec![LevelObject::default()], Vec::new(), 1.0);
    // Cursor far from any block so remove_topmost_block_at_cursor also fails
    app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [100.0, 100.0, 100.0];

    app.world_mut().write_message(RemoveBlock);
    app.tick();
    assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
}

// ── Rotation snap ──────────────────────────────────────────────────────

#[test]
fn effective_snap_rotation_overridden_by_ctrl() {
    let snap_rotation = true;
    assert!(effective_snap_enabled(snap_rotation, false));

    assert!(!effective_snap_enabled(snap_rotation, true));
}

// ── Multi-select dedup ─────────────────────────────────────────────────

#[test]
fn selected_indices_normalized_sorted() {
    let indices = normalize_selected_indices(&[2, 0, 1], Some(2), 3);
    assert_eq!(indices, vec![0, 1, 2]);
}

// ── Spawn direction full cycle ─────────────────────────────────────────

#[test]
fn rotate_spawn_direction_full_cycle() {
    let mut app = TestApp::minimal();
    let initial = app.world().resource::<EditorSpawnRes>().0.direction;

    // Rotate through all directions and return to initial
    for _ in 0..4 {
        app.world_mut().write_message(RotateSpawnDirection);
        app.tick();
    }
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
        .write_message(SetEditorMode(eg_types::EditorMode::Move));
    app.tick();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        eg_types::EditorMode::Move
    );

    app.world_mut()
        .write_message(SetEditorMode(eg_types::EditorMode::Scale));
    app.tick();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        eg_types::EditorMode::Scale
    );

    app.world_mut()
        .write_message(SetEditorMode(eg_types::EditorMode::Rotate));
    app.tick();
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        eg_types::EditorMode::Rotate
    );
}
