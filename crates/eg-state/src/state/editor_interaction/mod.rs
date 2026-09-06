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

use eg_blocks::BlockId;
use eg_mesh::{GizmoAxis, GizmoDragKind};
use eg_types::{EditorMode, EditorTapDivisionPick, LevelObject, SpawnMetadata};
pub(crate) use picking::EditorPickSpatialIndex;

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
    pub rotation: [f32; 3],
}

#[derive(Clone)]
pub struct EditorHistorySnapshot {
    pub objects: Vec<LevelObject>,
    pub(crate) object_change: Option<EditorHistoryObjectChange>,
    pub selected_block_index: Option<usize>,
    pub selected_block_indices: Vec<usize>,
    pub cursor: [f32; 3],
    pub selected_block_id: BlockId,
    pub selected_block_rotation_degrees: [f32; 3],
    pub recent_block_ids: Vec<BlockId>,
    pub spawn: SpawnMetadata,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub tap_times: Vec<f32>,
    pub tap_indicator_positions: Vec<[f32; 3]>,
    pub selected_tap_index: Option<usize>,
    pub timing_points: Vec<eg_types::TimingPoint>,
    pub selected_trigger_index: Option<usize>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum EditorHistoryObjectChange {
    RemovePlaced {
        object_id: String,
        index: usize,
    },
    /// Undo removes the listed added IDs (duplicate/paste/add-trigger bulk).
    /// `indices` are informational (base positions at record time); removal
    /// is by ID with index fallback for robustness.
    RemoveAdded {
        object_ids: Vec<String>,
        indices: Vec<usize>,
    },
    /// Undo reinserts previously removed objects and restores trigger target
    /// lists that were dereferenced at record time.
    ReinsertRemoved {
        removed: Vec<(usize, LevelObject)>,
        repaired_triggers: Vec<(usize, LevelObject)>,
    },
    /// Undo restores one object at `index` to `before`.
    RestoreOne {
        index: usize,
        before: LevelObject,
    },
    /// Undo restores several objects at their indices to `before` states.
    RestoreMany {
        before: Vec<(usize, LevelObject)>,
    },
    /// Undo removes added trigger IDs and restores pre-existing triggers
    /// whose targets were replaced at record time.
    AddTriggers {
        added_ids: Vec<String>,
        added_indices: Vec<usize>,
        modified_before: Vec<(usize, LevelObject)>,
    },
    /// Redo reinserts previously removed objects at their indices.
    ReinsertPlaced {
        objects: Vec<(usize, LevelObject)>,
    },
    /// Redo of a remove: removes by ID and strips those IDs from
    /// transform-trigger targets (mirrors record-time dereference).
    RemoveAndDereference {
        object_ids: Vec<String>,
    },
    /// Redo of trigger creation: reinserts trigger objects and re-strips
    /// their targets from same-time triggers.
    ReaddTriggers {
        objects: Vec<(usize, LevelObject)>,
    },
}

#[derive(Clone)]
pub struct EditorClipboard {
    pub objects: Vec<LevelObject>,
    pub anchor: [f32; 3],
}

#[derive(Default)]
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
