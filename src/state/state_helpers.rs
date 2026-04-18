/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{EditorSubsystem, State};
use crate::game::GameState;
use crate::types::{AppPhase, EditorMode};

impl EditorSubsystem {
    pub(crate) fn clear_pan_keys(&mut self) {
        self.ui.pan_up_held = false;
        self.ui.pan_down_held = false;
        self.ui.pan_left_held = false;
        self.ui.pan_right_held = false;
        self.ui.shift_held = false;
        self.ui.ctrl_held = false;
    }

    pub(crate) fn selected_indices_normalized(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .ui
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < self.objects.len())
            .collect();

        if indices.is_empty() {
            if let Some(index) = self
                .ui
                .selected_block_index
                .filter(|index| *index < self.objects.len())
            {
                indices.push(index);
            }
        }

        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub(crate) fn sync_primary_selection_from_indices(&mut self) {
        let indices = self.selected_indices_normalized();
        self.ui.selected_block_index = indices.first().copied();
        self.ui.selected_block_indices = indices;
    }

    pub(crate) fn selection_contains(&self, index: usize) -> bool {
        self.ui.selected_block_indices.contains(&index)
            || self.ui.selected_block_index == Some(index)
    }

    pub(crate) fn selected_mask_for_len(&self, len: usize) -> Vec<bool> {
        let mut selected_mask = vec![false; len];
        for index in self.ui.selected_block_indices.iter().copied() {
            if index < len {
                selected_mask[index] = true;
            }
        }
        if let Some(index) = self.ui.selected_block_index {
            if index < len {
                selected_mask[index] = true;
            }
        }
        selected_mask
    }

    pub(crate) fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let indices = self.selected_indices_normalized();
        let first = *indices.first()?;
        let first_obj = self.objects.get(first)?;
        let mut min = first_obj.position;
        let mut max = [
            first_obj.position[0] + first_obj.size[0],
            first_obj.position[1] + first_obj.size[1],
            first_obj.position[2] + first_obj.size[2],
        ];

        for index in indices.into_iter().skip(1) {
            if let Some(obj) = self.objects.get(index) {
                min[0] = min[0].min(obj.position[0]);
                min[1] = min[1].min(obj.position[1]);
                min[2] = min[2].min(obj.position[2]);
                max[0] = max[0].max(obj.position[0] + obj.size[0]);
                max[1] = max[1].max(obj.position[1] + obj.size[1]);
                max[2] = max[2].max(obj.position[2] + obj.size[2]);
            }
        }

        Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
    }
}

impl State {
    pub(super) fn clear_editor_pan_keys(&mut self) {
        self.editor.clear_pan_keys();
    }

    pub(super) fn selected_block_indices_normalized(&self) -> Vec<usize> {
        self.editor.selected_indices_normalized()
    }

    pub(super) fn sync_primary_selection_from_indices(&mut self) {
        self.editor.sync_primary_selection_from_indices();
    }

    pub(super) fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.editor.selected_group_bounds()
    }

    pub(super) fn reset_playing_camera_defaults(&mut self) {
        self.editor.camera.playing_rotation = 45.0f32.to_radians();
        self.editor.camera.playing_pitch = 45.0f32.to_radians();
    }

    pub(super) fn enter_playing_phase(
        &mut self,
        level_name: Option<String>,
        playtesting_editor: bool,
    ) {
        self.phase = AppPhase::Playing;
        self.session.playtesting_editor = playtesting_editor;
        self.session.playing_level_name = level_name;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
        self.editor.runtime.interaction.clipboard = None;
    }

    pub(super) fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.session.editor_level_name = Some(level_name);
        self.session.playtesting_editor = false;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.editor.ui.right_dragging = false;
        self.editor.ui.mode = EditorMode::Place;
        self.editor.ui.selected_block_index = None;
        self.editor.ui.selected_block_indices.clear();
        self.editor.ui.hovered_block_index = None;
        self.editor.ui.marquee_start_screen = None;
        self.editor.ui.marquee_current_screen = None;
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        self.editor.runtime.history.undo.clear();
        self.editor.runtime.history.redo.clear();
        self.clear_editor_pan_keys();
        self.editor.camera.editor_rotation = 45.0f32.to_radians();
        self.editor.camera.editor_pitch = 45.0f32.to_radians();
        self.editor.camera.editor_target_z = 0.0;
        self.gameplay.state = GameState::new();
        self.render.meshes.trail.clear();
    }

    pub(super) fn enter_menu_phase(&mut self) {
        self.session.playtesting_editor = false;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.session.editor_level_name = None;
        self.editor.ui.selected_block_index = None;
        self.editor.ui.selected_block_indices.clear();
        self.editor.ui.hovered_block_index = None;
        self.editor.ui.marquee_start_screen = None;
        self.editor.ui.marquee_current_screen = None;
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        self.session.playing_level_name = None;
        self.editor.ui.right_dragging = false;
        self.clear_editor_pan_keys();
        self.phase = AppPhase::Menu;
    }
}
