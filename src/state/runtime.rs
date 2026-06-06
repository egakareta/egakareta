/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::editor_interaction::EditorInteractionState;
use super::history::EditorHistoryState;
use crate::platform::state_host::PlatformInstant;
use crate::types::{EditorMode, LevelObject, LineUniform};

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct EditorDirtyFlags {
    pub(crate) sync_game_objects: bool,
    pub(crate) rebuild_block_mesh: bool,
    pub(crate) append_block_mesh: bool,
    pub(crate) rebuild_selection_overlays: bool,
    pub(crate) rebuild_tap_indicators: bool,
    pub(crate) rebuild_preview_player: bool,
    pub(crate) rebuild_cursor: bool,
    pub(crate) rebuild_hitbox_visualization: bool,
}

impl EditorDirtyFlags {
    pub(crate) fn from_object_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            append_block_mesh: false,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            rebuild_hitbox_visualization: true,
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.sync_game_objects |= other.sync_game_objects;
        self.rebuild_block_mesh |= other.rebuild_block_mesh;
        self.append_block_mesh |= other.append_block_mesh;
        self.rebuild_selection_overlays |= other.rebuild_selection_overlays;
        self.rebuild_tap_indicators |= other.rebuild_tap_indicators;
        self.rebuild_preview_player |= other.rebuild_preview_player;
        self.rebuild_cursor |= other.rebuild_cursor;
        self.rebuild_hitbox_visualization |= other.rebuild_hitbox_visualization;
    }

    pub(crate) fn any(self) -> bool {
        self.sync_game_objects
            || self.rebuild_block_mesh
            || self.append_block_mesh
            || self.rebuild_selection_overlays
            || self.rebuild_tap_indicators
            || self.rebuild_preview_player
            || self.rebuild_cursor
            || self.rebuild_hitbox_visualization
    }
}

pub(crate) struct EditorGizmoState {
    pub(crate) rebuild_accumulator: f32,
    pub(crate) last_pan: [f32; 2],
    pub(crate) last_target_z: f32,
    pub(crate) last_rotation: f32,
    pub(crate) last_pitch: f32,
}

pub(crate) struct EditorRuntimeState {
    pub(crate) dirty: EditorDirtyFlags,
    pub(crate) pending_block_mesh_appends: Vec<usize>,
    pub(crate) gizmo: EditorGizmoState,
    pub(crate) drag_heavy_rebuild_accumulator: f32,
    pub(crate) interaction: EditorInteractionState,
    pub(crate) history: EditorHistoryState,
    pub(crate) transform_trigger_capture: Option<EditorTransformTriggerCapture>,
}

pub(crate) struct EditorTransformTriggerCapture {
    pub(crate) time_seconds: f32,
    pub(crate) original_objects: Vec<(usize, LevelObject)>,
    pub(crate) previous_mode: EditorMode,
}

pub(crate) struct EditorFrameState {
    pub(crate) last_frame: PlatformInstant,
    pub(crate) accumulator: f32,
}

pub(crate) struct GemShatterEffect {
    pub(crate) position: [f32; 3],
    pub(crate) size: [f32; 3],
    pub(crate) color_tint: [f32; 3],
    pub(crate) age_seconds: f32,
}

impl GemShatterEffect {
    pub(crate) fn from_object(object: &LevelObject) -> Self {
        Self {
            position: object.position,
            size: object.size,
            color_tint: object.color_tint,
            age_seconds: 0.0,
        }
    }
}

pub(crate) struct PlayerRenderState {
    pub(crate) line_uniform: LineUniform,
    pub(crate) gem_shatter_effects: Vec<GemShatterEffect>,
}

pub(crate) struct FrameRuntimeState {
    pub(crate) editor: EditorFrameState,
    pub(crate) player_render: PlayerRenderState,
    pub(crate) global_time_seconds: f32,
}
