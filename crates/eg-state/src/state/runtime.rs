/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::editor_interaction::EditorInteractionState;
use super::history::EditorHistoryState;
use eg_platform::state_host::PlatformInstant;
use eg_types::{LevelObject, LineUniform};

#[derive(Clone, Copy, Default, Debug)]
pub struct EditorDirtyFlags {
    pub sync_game_objects: bool,
    pub rebuild_block_mesh: bool,
    pub append_block_mesh: bool,
    pub rebuild_selection_overlays: bool,
    pub rebuild_tap_indicators: bool,
    pub rebuild_transform_trigger_markers: bool,
    pub rebuild_preview_player: bool,
    pub rebuild_cursor: bool,
    pub rebuild_hitbox_visualization: bool,
}

impl EditorDirtyFlags {
    pub fn from_object_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            append_block_mesh: false,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_transform_trigger_markers: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            rebuild_hitbox_visualization: true,
        }
    }

    pub fn camera_changed() -> Self {
        Self {
            rebuild_selection_overlays: true,
            rebuild_cursor: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            ..Self::default()
        }
    }

    pub fn block_mesh_changed() -> Self {
        Self {
            rebuild_block_mesh: true,
            ..Self::default()
        }
    }

    pub fn selection_overlay_changed() -> Self {
        Self {
            rebuild_selection_overlays: true,
            ..Self::default()
        }
    }

    pub fn selection_cursor_changed(rebuild_cursor: bool) -> Self {
        Self {
            rebuild_selection_overlays: true,
            rebuild_cursor,
            ..Self::default()
        }
    }

    pub fn tap_timeline_changed() -> Self {
        Self {
            rebuild_tap_indicators: true,
            ..Self::default()
        }
    }

    pub fn tap_preview_changed(rebuild_cursor: bool) -> Self {
        Self {
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor,
            ..Self::default()
        }
    }

    pub fn playback_mode_changed() -> Self {
        Self {
            rebuild_block_mesh: true,
            rebuild_tap_indicators: true,
            ..Self::default()
        }
    }

    pub fn trigger_markers_changed() -> Self {
        Self {
            rebuild_transform_trigger_markers: true,
            ..Self::default()
        }
    }

    pub fn trigger_selection_changed() -> Self {
        Self {
            rebuild_selection_overlays: true,
            rebuild_transform_trigger_markers: true,
            ..Self::default()
        }
    }

    pub fn hitbox_visualization_changed() -> Self {
        Self {
            rebuild_hitbox_visualization: true,
            ..Self::default()
        }
    }

    pub fn block_mesh_and_hitboxes_changed() -> Self {
        Self {
            rebuild_block_mesh: true,
            rebuild_hitbox_visualization: true,
            ..Self::default()
        }
    }

    pub fn timeline_trigger_preview_changed() -> Self {
        Self {
            rebuild_block_mesh: true,
            rebuild_hitbox_visualization: true,
            rebuild_transform_trigger_markers: true,
            ..Self::default()
        }
    }

    pub fn trigger_object_added() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_hitbox_visualization: true,
            ..Self::default()
        }
    }

    pub fn simulate_trigger_hitboxes_changed() -> Self {
        Self {
            rebuild_preview_player: true,
            rebuild_block_mesh: true,
            rebuild_hitbox_visualization: true,
            ..Self::default()
        }
    }

    pub fn drag_object_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            ..Self::default()
        }
    }

    pub fn drag_release_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_selection_overlays: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            ..Self::default()
        }
    }

    pub fn appended_block(select_placed: bool, transform_trigger: bool) -> Self {
        Self {
            sync_game_objects: true,
            append_block_mesh: true,
            rebuild_selection_overlays: select_placed,
            rebuild_transform_trigger_markers: transform_trigger,
            ..Self::default()
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.sync_game_objects |= other.sync_game_objects;
        self.rebuild_block_mesh |= other.rebuild_block_mesh;
        self.append_block_mesh |= other.append_block_mesh;
        self.rebuild_selection_overlays |= other.rebuild_selection_overlays;
        self.rebuild_tap_indicators |= other.rebuild_tap_indicators;
        self.rebuild_transform_trigger_markers |= other.rebuild_transform_trigger_markers;
        self.rebuild_preview_player |= other.rebuild_preview_player;
        self.rebuild_cursor |= other.rebuild_cursor;
        self.rebuild_hitbox_visualization |= other.rebuild_hitbox_visualization;
    }

    pub fn any(self) -> bool {
        self.sync_game_objects
            || self.rebuild_block_mesh
            || self.append_block_mesh
            || self.rebuild_selection_overlays
            || self.rebuild_tap_indicators
            || self.rebuild_transform_trigger_markers
            || self.rebuild_preview_player
            || self.rebuild_cursor
            || self.rebuild_hitbox_visualization
    }
}

pub struct EditorGizmoState {
    pub rebuild_accumulator: f32,
    pub last_pan: [f32; 2],
    pub last_target_z: f32,
    pub last_rotation: f32,
    pub last_pitch: f32,
}

pub struct EditorRuntimeState {
    pub dirty: EditorDirtyFlags,
    pub pending_block_mesh_appends: Vec<usize>,
    pub gizmo: EditorGizmoState,
    pub drag_heavy_rebuild_accumulator: f32,
    pub interaction: EditorInteractionState,
    pub history: EditorHistoryState,
}

pub struct EditorFrameState {
    pub last_frame: PlatformInstant,
    pub accumulator: f32,
}

pub struct GemShatterEffect {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub color_tint: [f32; 3],
    pub age_seconds: f32,
}

impl GemShatterEffect {
    pub fn from_object(object: &LevelObject) -> Self {
        Self {
            position: object.position,
            size: object.size,
            color_tint: object.color_tint,
            age_seconds: 0.0,
        }
    }
}

pub struct PlayerRenderState {
    pub line_uniform: LineUniform,
    pub gem_shatter_effects: Vec<GemShatterEffect>,
}

pub struct FrameRuntimeState {
    pub editor: EditorFrameState,
    pub player_render: PlayerRenderState,
    pub global_time_seconds: f32,
}
