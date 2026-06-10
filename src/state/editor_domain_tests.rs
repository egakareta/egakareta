/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Pure domain tests for `EditorSubsystem`.
//!
//! These tests exercise editor logic **without GPU, audio, or rendering**.
//! They use `EditorSubsystem::new_test()` which requires no wgpu adapter.

use super::EditorSubsystem;
use crate::test_utils::{assert_approx_eq, stone};
use crate::types::SpawnDirection;

fn assert_vec3_approx_eq(actual: [f32; 3], expected: [f32; 3], eps: f32) {
    assert_approx_eq(actual[0], expected[0], eps);
    assert_approx_eq(actual[1], expected[1], eps);
    assert_approx_eq(actual[2], expected[2], eps);
}

// ── Nudge ──────────────────────────────────────────────────────────────

#[test]
fn nudge_moves_selected_blocks_by_world_delta() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(3.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(0);
    editor.ui.selected_block_indices = vec![0];

    assert!(editor.nudge_selected(2.0, -1.0));
    assert_vec3_approx_eq(editor.objects[0].position, [2.0, 0.0, -1.0], 1e-6);
    assert_vec3_approx_eq(editor.objects[1].position, [3.0, 0.0, 0.0], 1e-6);
}

#[test]
fn nudge_returns_false_when_nothing_selected() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));

    assert!(!editor.nudge_selected(1.0, 0.0));
    assert_vec3_approx_eq(editor.objects[0].position, [0.0, 0.0, 0.0], 1e-6);
}

#[test]
fn nudge_updates_cursor_to_selected_block_position() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(1.0, 2.0, 3.0));
    editor.ui.selected_block_index = Some(0);

    editor.nudge_selected(5.0, -2.0);
    assert_vec3_approx_eq(editor.ui.cursor, [6.0, 2.0, 1.0], 1e-6);
}

// ── Snap to grid ───────────────────────────────────────────────────────

#[test]
fn snap_moves_blocks_to_nearest_grid_cell() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_step = 1.0;
    editor.objects.push(stone(0.3, 0.0, 0.7));
    editor.ui.selected_block_index = Some(0);

    assert!(editor.snap_selected_blocks_to_grid());
    assert_vec3_approx_eq(editor.objects[0].position, [0.0, 0.0, 1.0], 1e-6);
}

#[test]
fn snap_respects_custom_step_size() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_step = 2.0;
    editor.objects.push(stone(1.3, 0.0, 3.8));
    editor.ui.selected_block_index = Some(0);

    assert!(editor.snap_selected_blocks_to_grid());
    assert_vec3_approx_eq(editor.objects[0].position, [2.0, 0.0, 4.0], 1e-6);
}

#[test]
fn snap_returns_false_when_already_snapped() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_step = 1.0;
    editor.objects.push(stone(3.0, 0.0, 5.0));
    editor.ui.selected_block_index = Some(0);

    assert!(!editor.snap_selected_blocks_to_grid());
}

// ── Remove selected ────────────────────────────────────────────────────

#[test]
fn remove_selected_deletes_marked_blocks() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(1.0, 0.0, 0.0));
    editor.objects.push(stone(2.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(1);
    editor.ui.selected_block_indices = vec![1];

    assert!(editor.remove_selected());
    assert_eq!(editor.objects.len(), 2);
    assert_vec3_approx_eq(editor.objects[0].position, [0.0, 0.0, 0.0], 1e-6);
    assert_vec3_approx_eq(editor.objects[1].position, [2.0, 0.0, 0.0], 1e-6);
}

#[test]
fn remove_selected_clears_selection() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(0);

    editor.remove_selected();
    assert!(editor.ui.selected_block_index.is_none());
    assert!(editor.ui.selected_block_indices.is_empty());
}

// ── Spawn ──────────────────────────────────────────────────────────────

#[test]
fn set_spawn_here_copies_cursor_position() {
    let mut editor = EditorSubsystem::new_test();
    editor.ui.cursor = [5.0, 3.0, 7.0];

    editor.set_spawn_here();
    assert_vec3_approx_eq(editor.spawn.position, [5.0, 3.0, 7.0], 1e-6);
}

#[test]
fn rotate_spawn_direction_toggles_between_forward_and_right() {
    let mut editor = EditorSubsystem::new_test();
    assert_eq!(editor.spawn.direction, SpawnDirection::Forward);

    editor.rotate_spawn_direction();
    assert_eq!(editor.spawn.direction, SpawnDirection::Right);

    editor.rotate_spawn_direction();
    assert_eq!(editor.spawn.direction, SpawnDirection::Forward);
}

// ── Selection helpers ──────────────────────────────────────────────────

#[test]
fn clear_block_selection_resets_all_indices() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(1.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(0);
    editor.ui.selected_block_indices = vec![0, 1];

    editor.clear_block_selection();
    assert!(editor.ui.selected_block_index.is_none());
    assert!(editor.ui.selected_block_indices.is_empty());
}

#[test]
fn selected_indices_normalized_deduplicates() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(1.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(0);
    editor.ui.selected_block_indices = vec![0, 0, 1];

    let indices = editor.selected_indices_normalized();
    assert_eq!(indices, vec![0, 1]);
}

// ── Mode ───────────────────────────────────────────────────────────────

#[test]
fn set_mode_clears_drags_and_marquee() {
    let mut editor = EditorSubsystem::new_test();
    editor.runtime.interaction.gizmo_drag = Some(super::EditorGizmoDrag {
        axis: crate::types::GizmoAxis::X,
        kind: crate::types::GizmoDragKind::Move,
        start_mouse: [0.0, 0.0],
        start_center_screen: [0.0, 0.0],
        start_center_world: [0.0, 0.0, 0.0],
        start_blocks: Vec::new(),
    });
    editor.ui.marquee_start_screen = Some([10.0, 10.0]);

    editor.set_mode(crate::types::EditorMode::Select);
    assert!(editor.runtime.interaction.gizmo_drag.is_none());
    assert!(editor.ui.marquee_start_screen.is_none());
}

// ── Config ─────────────────────────────────────────────────────────────

#[test]
fn effective_snap_overridden_by_ctrl() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_to_grid = true;
    editor.ui.ctrl_held = false;
    assert!(editor.effective_snap_to_grid());

    editor.ui.ctrl_held = true;
    assert!(!editor.effective_snap_to_grid());
}

#[test]
fn set_snap_step_clamps_minimum() {
    let mut editor = EditorSubsystem::new_test();
    editor.set_snap_step(0.001);
    assert!(
        editor.snap_step() >= 0.05,
        "Snap step should be clamped to at least 0.05"
    );
}

// ── Nudge edge cases ───────────────────────────────────────────────────

#[test]
fn nudge_moves_all_selected_blocks() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(1.0, 0.0, 0.0));
    editor.objects.push(stone(2.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(0);
    editor.ui.selected_block_indices = vec![0, 2];

    assert!(editor.nudge_selected(3.0, -1.0));
    assert_vec3_approx_eq(editor.objects[0].position, [3.0, 0.0, -1.0], 1e-6);
    assert_vec3_approx_eq(editor.objects[1].position, [1.0, 0.0, 0.0], 1e-6);
    assert_vec3_approx_eq(editor.objects[2].position, [5.0, 0.0, -1.0], 1e-6);
}

// ── Snap edge cases ────────────────────────────────────────────────────

#[test]
fn snap_returns_false_when_nothing_selected() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_step = 1.0;
    editor.objects.push(stone(0.3, 0.0, 0.7));

    assert!(!editor.snap_selected_blocks_to_grid());
}

// ── Remove edge cases ──────────────────────────────────────────────────

#[test]
fn remove_selected_returns_false_when_nothing_selected_and_cursor_empty() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    // Cursor far from any block so remove_topmost_block_at_cursor also fails
    editor.ui.cursor = [100.0, 100.0, 100.0];

    assert!(!editor.remove_selected());
    assert_eq!(editor.objects.len(), 1);
}

// ── Rotation snap ──────────────────────────────────────────────────────

#[test]
fn effective_snap_rotation_overridden_by_ctrl() {
    let mut editor = EditorSubsystem::new_test();
    editor.config.snap_rotation = true;
    editor.ui.ctrl_held = false;
    assert!(editor.effective_snap_rotation());

    editor.ui.ctrl_held = true;
    assert!(!editor.effective_snap_rotation());
}

// ── Multi-select dedup ─────────────────────────────────────────────────

#[test]
fn selected_indices_normalized_sorted() {
    let mut editor = EditorSubsystem::new_test();
    editor.objects.push(stone(0.0, 0.0, 0.0));
    editor.objects.push(stone(1.0, 0.0, 0.0));
    editor.objects.push(stone(2.0, 0.0, 0.0));
    editor.ui.selected_block_index = Some(2);
    editor.ui.selected_block_indices = vec![2, 0, 1];

    let indices = editor.selected_indices_normalized();
    assert_eq!(indices, vec![0, 1, 2]);
}

// ── Spawn direction full cycle ─────────────────────────────────────────

#[test]
fn rotate_spawn_direction_full_cycle() {
    let mut editor = EditorSubsystem::new_test();
    let initial = editor.spawn.direction;

    // Rotate through all directions and return to initial
    for _ in 0..4 {
        editor.rotate_spawn_direction();
    }
    assert_eq!(editor.spawn.direction, initial);
}

// ── Mode change ────────────────────────────────────────────────────────

#[test]
fn set_mode_updates_mode_field() {
    let mut editor = EditorSubsystem::new_test();

    editor.set_mode(crate::types::EditorMode::Move);
    assert_eq!(editor.ui.mode, crate::types::EditorMode::Move);

    editor.set_mode(crate::types::EditorMode::Scale);
    assert_eq!(editor.ui.mode, crate::types::EditorMode::Scale);

    editor.set_mode(crate::types::EditorMode::Rotate);
    assert_eq!(editor.ui.mode, crate::types::EditorMode::Rotate);
}
