mod camera;
mod cursor;
mod gizmo;
mod picking;
mod selection;

use crate::types::{
    EditorMode, GizmoAxis, GizmoDragKind, LevelObject, SpawnMetadata, TimedTrigger,
};

#[derive(Clone)]
pub(crate) struct EditorGizmoDrag {
    pub(crate) axis: GizmoAxis,
    pub(crate) kind: GizmoDragKind,
    pub(crate) start_mouse: [f64; 2],
    pub(crate) start_center_screen: [f32; 2],
    pub(crate) start_center_world: [f32; 3],
    pub(crate) start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone)]
pub(crate) struct EditorBlockDrag {
    pub(crate) start_mouse: [f64; 2],
    pub(crate) start_center_screen: [f32; 2],
    pub(crate) start_center_world: [f32; 3],
    pub(crate) start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorDragBlockStart {
    pub(crate) index: usize,
    pub(crate) position: [f32; 3],
    pub(crate) size: [f32; 3],
    pub(crate) rotation_degrees: [f32; 3],
}

#[derive(Clone)]
pub(crate) struct EditorHistorySnapshot {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) selected_block_index: Option<usize>,
    pub(crate) selected_block_indices: Vec<usize>,
    pub(crate) cursor: [f32; 3],
    pub(crate) selected_block_id: String,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) timeline_time_seconds: f32,
    pub(crate) timeline_duration_seconds: f32,
    pub(crate) tap_times: Vec<f32>,
    pub(crate) tap_indicator_positions: Vec<[f32; 3]>,
    pub(crate) timing_points: Vec<crate::types::TimingPoint>,
    pub(crate) triggers: Vec<TimedTrigger>,
    pub(crate) selected_trigger_index: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct EditorClipboard {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) anchor: [f32; 3],
}

pub(crate) struct EditorInteractionState {
    pub(crate) gizmo_drag: Option<EditorGizmoDrag>,
    pub(crate) hovered_gizmo: Option<(GizmoDragKind, GizmoAxis)>,
    pub(crate) block_drag: Option<EditorBlockDrag>,
    pub(crate) clipboard: Option<EditorClipboard>,
    pub(crate) last_mode: Option<EditorMode>,
}

impl EditorInteractionState {
    pub(crate) fn new() -> Self {
        Self {
            gizmo_drag: None,
            hovered_gizmo: None,
            block_drag: None,
            clipboard: None,
            last_mode: None,
        }
    }
}
