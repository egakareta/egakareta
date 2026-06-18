/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
mod camera;
mod cursor;
mod gizmo;
mod picking;
mod selection;

use eg_mesh::{GizmoAxis, GizmoDragKind};
use eg_types::{EditorMode, EditorTapDivisionPick, LevelObject, SpawnMetadata};

#[derive(Clone)]
pub struct EditorGizmoDrag {
    pub axis: GizmoAxis,
    pub kind: GizmoDragKind,
    pub start_mouse: [f64; 2],
    pub start_center_screen: [f32; 2],
    pub start_center_world: [f32; 3],
    pub start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone)]
pub struct EditorBlockDrag {
    pub start_mouse: [f64; 2],
    pub start_center_world: [f32; 3],
    pub start_drag_world: [f32; 3],
    pub start_blocks: Vec<EditorDragBlockStart>,
    pub start_cursor: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct EditorDragBlockStart {
    pub index: usize,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub rotation_degrees: [f32; 3],
}

#[derive(Clone)]
pub struct EditorHistorySnapshot {
    pub objects: Vec<LevelObject>,
    pub selected_block_index: Option<usize>,
    pub selected_block_indices: Vec<usize>,
    pub cursor: [f32; 3],
    pub selected_block_id: String,
    pub selected_block_rotation_degrees: [f32; 3],
    pub recent_block_ids: Vec<String>,
    pub spawn: SpawnMetadata,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub tap_times: Vec<f32>,
    pub tap_indicator_positions: Vec<[f32; 3]>,
    pub selected_tap_index: Option<usize>,
    pub timing_points: Vec<eg_types::TimingPoint>,
    pub selected_trigger_index: Option<usize>,
    pub simulate_trigger_hitboxes: bool,
}

#[derive(Clone)]
pub struct EditorClipboard {
    pub objects: Vec<LevelObject>,
    pub anchor: [f32; 3],
}

pub struct EditorInteractionState {
    pub gizmo_drag: Option<EditorGizmoDrag>,
    pub hovered_gizmo: Option<(GizmoDragKind, GizmoAxis)>,
    pub hovered_tap_index: Option<usize>,
    pub hovered_tap_division: Option<EditorTapDivisionPick>,
    pub pending_tap_click: Option<EditorPendingTapClick>,
    pub block_drag: Option<EditorBlockDrag>,
    pub clipboard: Option<EditorClipboard>,
    pub last_mode: Option<EditorMode>,
}

#[derive(Clone, Copy)]
pub struct EditorPendingTapClick {
    pub screen: [f64; 2],
    pub pick: EditorTapDivisionPick,
}

impl EditorInteractionState {
    pub fn new() -> Self {
        Self {
            gizmo_drag: None,
            hovered_gizmo: None,
            hovered_tap_index: None,
            hovered_tap_division: None,
            pending_tap_click: None,
            block_drag: None,
            clipboard: None,
            last_mode: None,
        }
    }
}

impl Default for EditorInteractionState {
    fn default() -> Self {
        Self::new()
    }
}
