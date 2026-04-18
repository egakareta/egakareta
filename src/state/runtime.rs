/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::editor_interaction::EditorInteractionState;
use super::history::EditorHistoryState;
use crate::platform::state_host::PlatformInstant;
use crate::types::LineUniform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockMeshOperation {
    FullRebuild,
    AppendObject,
    RemoveObjects,
    UpdateObjects,
    SelectionPartitionOnly,
    SelectedOnly,
}

#[derive(Default)]
pub(crate) struct EditorMeshOperationState {
    pub(crate) pending: Option<BlockMeshOperation>,
    pub(crate) changed_indices: Vec<usize>,
}

impl EditorMeshOperationState {
    pub(crate) fn request(&mut self, operation: BlockMeshOperation, changed_indices: &[usize]) {
        self.pending = Some(match (self.pending, operation) {
            (_, BlockMeshOperation::FullRebuild) => BlockMeshOperation::FullRebuild,
            (None, next) => next,
            (Some(BlockMeshOperation::FullRebuild), _) => BlockMeshOperation::FullRebuild,
            (Some(current), next) if current == next => current,
            _ => BlockMeshOperation::FullRebuild,
        });

        if self.pending == Some(BlockMeshOperation::FullRebuild) {
            self.changed_indices.clear();
        } else {
            self.changed_indices.extend(changed_indices.iter().copied());
        }
    }

    pub(crate) fn take(&mut self) -> Option<(BlockMeshOperation, Vec<usize>)> {
        let operation = self.pending.take()?;
        let mut changed_indices = std::mem::take(&mut self.changed_indices);
        changed_indices.sort_unstable();
        changed_indices.dedup();
        Some((operation, changed_indices))
    }

    pub(crate) fn clear(&mut self) {
        self.pending = None;
        self.changed_indices.clear();
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct EditorDirtyFlags {
    pub(crate) sync_game_objects: bool,
    pub(crate) rebuild_block_mesh: bool,
    pub(crate) rebuild_selection_overlays: bool,
    pub(crate) rebuild_tap_indicators: bool,
    pub(crate) rebuild_preview_player: bool,
    pub(crate) rebuild_cursor: bool,
}

impl EditorDirtyFlags {
    pub(crate) fn from_object_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.sync_game_objects |= other.sync_game_objects;
        self.rebuild_block_mesh |= other.rebuild_block_mesh;
        self.rebuild_selection_overlays |= other.rebuild_selection_overlays;
        self.rebuild_tap_indicators |= other.rebuild_tap_indicators;
        self.rebuild_preview_player |= other.rebuild_preview_player;
        self.rebuild_cursor |= other.rebuild_cursor;
    }

    pub(crate) fn any(self) -> bool {
        self.sync_game_objects
            || self.rebuild_block_mesh
            || self.rebuild_selection_overlays
            || self.rebuild_tap_indicators
            || self.rebuild_preview_player
            || self.rebuild_cursor
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
    pub(crate) block_mesh_operation: EditorMeshOperationState,
    pub(crate) static_chunk_keys: Vec<[i32; 3]>,
    pub(crate) gizmo: EditorGizmoState,
    pub(crate) drag_heavy_rebuild_accumulator: f32,
    pub(crate) interaction: EditorInteractionState,
    pub(crate) history: EditorHistoryState,
}

pub(crate) struct EditorFrameState {
    pub(crate) last_frame: PlatformInstant,
    pub(crate) accumulator: f32,
}

pub(crate) struct PlayerRenderState {
    pub(crate) line_uniform: LineUniform,
}

pub(crate) struct FrameRuntimeState {
    pub(crate) editor: EditorFrameState,
    pub(crate) player_render: PlayerRenderState,
    pub(crate) global_time_seconds: f32,
}
