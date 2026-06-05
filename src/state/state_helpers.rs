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

    fn normalized_block_selection_indices(&self) -> Vec<usize> {
        let object_count = self.objects.len();
        let mut indices: Vec<usize> = self
            .ui
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < object_count)
            .collect();

        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < object_count)
        {
            indices.push(index);
        }

        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub(crate) fn selected_indices_normalized(&self) -> Vec<usize> {
        self.normalized_block_selection_indices()
    }

    pub(crate) fn normalize_block_selection(&mut self) {
        let indices = self.normalized_block_selection_indices();
        self.ui.selected_block_index = self
            .ui
            .selected_block_index
            .filter(|index| indices.binary_search(index).is_ok())
            .or_else(|| indices.first().copied());
        self.ui.selected_block_indices = indices;

        if self
            .ui
            .hovered_block_index
            .is_some_and(|index| index >= self.objects.len())
        {
            self.ui.hovered_block_index = None;
        }
        self.selected_mask_cache = None;
    }

    pub(crate) fn clear_block_selection(&mut self) {
        let had_selection = self.ui.selected_block_index.is_some()
            || !self.ui.selected_block_indices.is_empty()
            || self.ui.hovered_block_index.is_some();
        self.ui.selected_block_index = None;
        self.ui.selected_block_indices.clear();
        self.ui.hovered_block_index = None;
        if had_selection {
            self.selected_mask_cache = None;
        }
    }

    pub(crate) fn replace_block_selection(&mut self, indices: Vec<usize>) {
        self.ui.selected_block_index = indices.first().copied();
        self.ui.selected_block_indices = indices;
        self.normalize_block_selection();
    }

    pub(crate) fn add_block_to_selection(&mut self, index: usize) {
        self.ui.selected_block_indices.push(index);
        self.normalize_block_selection();
    }

    pub(crate) fn remove_block_from_selection(&mut self, index: usize) {
        self.ui
            .selected_block_indices
            .retain(|selected| *selected != index);
        if self.ui.selected_block_index == Some(index) {
            self.ui.selected_block_index = None;
        }
        self.normalize_block_selection();
        if self.ui.selected_block_indices.is_empty() {
            self.clear_block_selection();
        }
    }

    pub(crate) fn sync_primary_selection_from_indices(&mut self) {
        self.normalize_block_selection();
    }

    pub(crate) fn selection_contains(&self, index: usize) -> bool {
        self.ui.selected_block_indices.contains(&index)
            || self.ui.selected_block_index == Some(index)
    }

    pub(crate) fn selected_mask_for_len(&self, len: usize) -> Vec<bool> {
        let mut selected_mask = vec![false; len];
        for index in self.selected_indices_normalized() {
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
        self.session.game_paused = false;
        self.session.playing_level_name = level_name;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_sky_color = crate::types::default_sky_color();
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.session.practice_mode_enabled = false;
        self.session.practice_checkpoints.clear();
        self.gameplay.death_sfx_played = false;
        self.rebuild_practice_checkpoint_vertices();
        self.clear_pending_gameplay_inputs();
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
        self.editor.runtime.interaction.clipboard = None;
    }

    pub(super) fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.session.editor_level_name = Some(level_name);
        self.session.editor_creator_metadata = crate::types::LevelCreatorMetadata::default();
        self.session.editor_sky_color = crate::types::default_sky_color();
        self.session.playtesting_editor = false;
        self.session.game_paused = false;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_sky_color = crate::types::default_sky_color();
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.session.practice_mode_enabled = false;
        self.session.practice_checkpoints.clear();
        self.gameplay.death_sfx_played = false;
        self.rebuild_practice_checkpoint_vertices();
        self.clear_pending_gameplay_inputs();
        self.session.editor_menu_preview_camera = None;
        self.editor.ui.right_dragging = false;
        self.editor.ui.mode = EditorMode::Null;
        self.editor.clear_block_selection();
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

    #[cfg(test)]
    pub(crate) fn enter_editor_phase_for_test(&mut self, level_name: impl Into<String>) {
        self.enter_editor_phase(level_name.into());
    }

    pub(super) fn enter_menu_phase(&mut self) {
        self.session.playtesting_editor = false;
        self.session.game_paused = false;
        self.session.playtest_audio_start_seconds = None;
        self.session.playing_sky_color = crate::types::default_sky_color();
        self.session.playing_trigger_hitboxes = false;
        self.session.playing_trigger_base_objects = None;
        self.session.practice_mode_enabled = false;
        self.session.practice_checkpoints.clear();
        self.gameplay.death_sfx_played = false;
        self.rebuild_practice_checkpoint_vertices();
        self.clear_pending_gameplay_inputs();
        self.session.editor_level_name = None;
        self.session.editor_creator_metadata = crate::types::LevelCreatorMetadata::default();
        self.session.editor_sky_color = crate::types::default_sky_color();
        self.editor.clear_block_selection();
        self.editor.ui.marquee_start_screen = None;
        self.editor.ui.marquee_current_screen = None;
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        self.session.playing_level_name = None;
        self.editor.ui.right_dragging = false;
        self.clear_editor_pan_keys();
        self.menu.state.preview_level_index = None;
        self.phase = AppPhase::Menu;
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::types::LevelObject;

    fn block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn normalize_block_selection_filters_invalid_indices_and_preserves_valid_primary() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![
                block([0.0, 0.0, 0.0]),
                block([1.0, 0.0, 0.0]),
                block([2.0, 0.0, 0.0]),
            ];
            state.editor.ui.selected_block_index = Some(1);
            state.editor.ui.selected_block_indices = vec![2, 99, 2];
            state.editor.ui.hovered_block_index = Some(42);
            state.editor.selected_mask_cache = Some(vec![false, true, true]);

            state.editor.normalize_block_selection();

            assert_eq!(state.editor.ui.selected_block_index, Some(1));
            assert_eq!(state.editor.ui.selected_block_indices, vec![1, 2]);
            assert_eq!(state.editor.ui.hovered_block_index, None);
            assert!(state.editor.selected_mask_cache.is_none());
        });
    }
}
