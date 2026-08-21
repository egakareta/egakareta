/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_blocks::BlockEffect;
use eg_types::LevelObject;

use super::editor_interaction::EditorInteractionState;

#[derive(Default)]
pub struct EditorGizmoState {
    pub rebuild_accumulator: f32,
    pub last_pan: [f32; 2],
    pub last_target_z: f32,
    pub last_rotation: f32,
    pub last_pitch: f32,
}

#[derive(Default)]
pub struct EditorRuntimeState {
    pub pending_block_effects: Vec<BlockVisualEffect>,
    pub gizmo: EditorGizmoState,
    pub drag_heavy_rebuild_accumulator: f32,
    pub interaction: EditorInteractionState,
}

#[derive(Clone)]
pub struct BlockVisualEffect {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 3],
    pub effect: BlockEffect,
    pub age_seconds: f32,
}

impl BlockVisualEffect {
    pub fn from_object(object: &LevelObject, effect: BlockEffect, color: [f32; 3]) -> Self {
        Self {
            position: object.position,
            size: object.size,
            color,
            effect,
            age_seconds: 0.0,
        }
    }
}
