use super::*;

impl State {
    pub fn set_editor_pan_up_held(&mut self, held: bool) {
        self.editor_pan_up_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_down_held(&mut self, held: bool) {
        self.editor_pan_down_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_left_held(&mut self, held: bool) {
        self.editor_pan_left_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_right_held(&mut self, held: bool) {
        self.editor_pan_right_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_shift_held(&mut self, held: bool) {
        self.editor_shift_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_ctrl_held(&mut self, held: bool) {
        self.editor_ctrl_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_alt_held(&mut self, held: bool) {
        self.editor_alt_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_block_id(&mut self, block_id: String) {
        self.editor_selected_block_id = crate::block_repository::normalize_block_id(&block_id);
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor_mode = mode;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        if mode == EditorMode::Place {
            self.editor_selected_block_index = None;
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
        }
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor_mode
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor_snap_to_grid
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor_snap_step
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor_snap_to_grid = snap;
        if self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor_snap_step = step.max(0.05);
        if self.editor_snap_to_grid && self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.selected_block_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.editor_objects.get(index).cloned())
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let bounds = self.editor.bounds;
            let snap_step = self.editor_snap_step.max(0.05);
            let next_position = if self.editor_snap_to_grid {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1] / snap_step).round() * snap_step,
                    (position[2].max(0.0) / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.editor_objects[index].position = next_position;
            self.editor.cursor = [
                (next_position[0].floor() as i32).clamp(-bounds, bounds),
                (next_position[1].floor() as i32).clamp(-bounds, bounds),
                (next_position[2].floor() as i32).max(0),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_size(&mut self, size: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let snap_step = self.editor_snap_step.max(0.05);
            let snapped_size = if self.editor_snap_to_grid {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.editor_snap_to_grid {
                snap_step
            } else {
                0.25
            };
            self.editor_objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_id(&mut self, block_id: String) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].block_id =
                crate::block_repository::normalize_block_id(&block_id);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_rotation(&mut self, rotation_degrees: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].rotation_degrees = rotation_degrees;
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_roundness(&mut self, roundness: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].roundness = roundness.max(0.0);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub fn editor_selected_block_id(&self) -> &str {
        &self.editor_selected_block_id
    }

    pub fn editor_timeline_step(&self) -> u32 {
        self.editor_timeline_step
    }

    pub fn editor_timeline_length(&self) -> u32 {
        self.editor_timeline_length
    }

    pub fn editor_tap_steps(&self) -> &[u32] {
        &self.editor_tap_steps
    }

    pub fn set_editor_timeline_step(&mut self, step: u32) {
        self.record_editor_history_state();
        let max_step = self.editor_timeline_length.saturating_sub(1);
        self.editor_timeline_step = step.min(max_step);
        self.refresh_editor_timeline_position();
    }

    pub fn set_editor_timeline_length(&mut self, length: u32) {
        self.record_editor_history_state();
        let length = length.max(1);
        let max_step = length.saturating_sub(1);
        self.editor_timeline_length = length;
        self.editor_timeline_step = self.editor_timeline_step.min(max_step);
        self.editor_tap_steps.retain(|step| *step < length);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        add_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        remove_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        clear_tap_steps(&mut self.editor_tap_steps);
        self.refresh_editor_timeline_position();
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor_timeline_position(self.editor_timeline_step)
    }
}
