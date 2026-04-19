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

use crate::types::{
    EditorMode, GizmoAxis, GizmoDragKind, LevelObject, SpawnMetadata, TimedTrigger,
};

#[derive(Clone, Copy)]
pub(crate) struct MarqueeScreenBounds {
    pub(crate) min: [f32; 2],
    pub(crate) max: [f32; 2],
}

pub(crate) struct MarqueeProjectionCache {
    pub(crate) valid: bool,
    pub(crate) viewport: [u32; 2],
    pub(crate) camera_pan_bits: [u32; 2],
    pub(crate) camera_target_z_bits: u32,
    pub(crate) camera_rotation_bits: u32,
    pub(crate) camera_pitch_bits: u32,
    pub(crate) object_count: usize,
    pub(crate) bounds: Vec<Option<MarqueeScreenBounds>>,
}

impl MarqueeProjectionCache {
    pub(crate) fn new() -> Self {
        Self {
            valid: false,
            viewport: [0, 0],
            camera_pan_bits: [0, 0],
            camera_target_z_bits: 0,
            camera_rotation_bits: 0,
            camera_pitch_bits: 0,
            object_count: 0,
            bounds: Vec::new(),
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.valid = false;
    }
}

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
    pub(crate) simulate_trigger_hitboxes: bool,
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
    pub(crate) marquee_projection_cache: MarqueeProjectionCache,
    pub(crate) hover_detail_cursor: usize,
    pub(crate) hover_detail_budget_blocks: usize,
}

impl EditorInteractionState {
    pub(crate) fn new() -> Self {
        Self {
            gizmo_drag: None,
            hovered_gizmo: None,
            block_drag: None,
            clipboard: None,
            last_mode: None,
            marquee_projection_cache: MarqueeProjectionCache::new(),
            hover_detail_cursor: 0,
            hover_detail_budget_blocks: 48,
        }
    }
}
