use super::*;

impl State {
    pub(super) fn clear_editor_pan_keys(&mut self) {
        self.editor.pan_up_held = false;
        self.editor.pan_down_held = false;
        self.editor.pan_left_held = false;
        self.editor.pan_right_held = false;
        self.editor.shift_held = false;
        self.editor.ctrl_held = false;
    }

    pub(super) fn selected_block_indices_normalized(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .editor
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < self.editor_objects.len())
            .collect();

        if indices.is_empty() {
            if let Some(index) = self
                .editor
                .selected_block_index
                .filter(|index| *index < self.editor_objects.len())
            {
                indices.push(index);
            }
        }

        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub(super) fn sync_primary_selection_from_indices(&mut self) {
        let indices = self.selected_block_indices_normalized();
        self.editor.selected_block_index = indices.first().copied();
        self.editor.selected_block_indices = indices;
    }

    pub(super) fn selection_contains(&self, index: usize) -> bool {
        self.editor.selected_block_indices.contains(&index)
            || self.editor.selected_block_index == Some(index)
    }

    pub(super) fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let indices = self.selected_block_indices_normalized();
        let first = *indices.first()?;
        let first_obj = self.editor_objects.get(first)?;
        let mut min = first_obj.position;
        let mut max = [
            first_obj.position[0] + first_obj.size[0],
            first_obj.position[1] + first_obj.size[1],
            first_obj.position[2] + first_obj.size[2],
        ];

        for index in indices.into_iter().skip(1) {
            if let Some(obj) = self.editor_objects.get(index) {
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

    pub(super) fn reset_playing_camera_defaults(&mut self) {
        self.playing_camera_rotation = -45.0f32.to_radians();
        self.playing_camera_pitch = 45.0f32.to_radians();
    }

    pub(super) fn enter_playing_phase(
        &mut self,
        level_name: Option<String>,
        playtesting_editor: bool,
    ) {
        self.phase = AppPhase::Playing;
        self.editor_session.playtesting_editor = playtesting_editor;
        self.editor_session.playing_level_name = level_name;
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
    }

    pub(super) fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.editor_session.editor_level_name = Some(level_name);
        self.editor_session.playtesting_editor = false;
        self.editor.right_dragging = false;
        self.editor.mode = EditorMode::Place;
        self.editor.selected_block_index = None;
        self.editor.selected_block_indices.clear();
        self.editor.hovered_block_index = None;
        self.editor_interaction.gizmo_drag = None;
        self.editor_interaction.block_drag = None;
        self.editor_history.undo.clear();
        self.editor_history.redo.clear();
        self.clear_editor_pan_keys();
        self.editor_camera_rotation = -45.0f32.to_radians();
        self.editor_camera_pitch = 45.0f32.to_radians();
        self.editor_zoom = 1.0;
        self.game = GameState::new();
        self.meshes.trail.clear();
    }

    pub(super) fn enter_menu_phase(&mut self) {
        self.editor_session.playtesting_editor = false;
        self.editor_session.editor_level_name = None;
        self.editor.selected_block_index = None;
        self.editor.selected_block_indices.clear();
        self.editor.hovered_block_index = None;
        self.editor_interaction.gizmo_drag = None;
        self.editor_interaction.block_drag = None;
        self.editor_session.playing_level_name = None;
        self.editor.right_dragging = false;
        self.clear_editor_pan_keys();
        self.phase = AppPhase::Menu;
    }
}
