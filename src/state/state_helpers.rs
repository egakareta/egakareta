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

    pub(super) fn selection_contains(&self, index: usize) -> bool {
        self.editor.selection_contains(index)
    }

    pub(super) fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.editor.selected_group_bounds()
    }

    pub(super) fn reset_playing_camera_defaults(&mut self) {
        self.editor.camera.playing_rotation = -45.0f32.to_radians();
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
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
        self.editor.runtime.interaction.clipboard = None;
    }

    pub(super) fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.session.editor_level_name = Some(level_name);
        self.session.playtesting_editor = false;
        self.session.playtest_audio_start_seconds = None;
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
        self.editor.camera.editor_rotation = -45.0f32.to_radians();
        self.editor.camera.editor_pitch = 45.0f32.to_radians();
        self.editor.camera.editor_zoom = 1.0;
        self.editor.camera.editor_target_z = 0.0;
        self.gameplay.state = GameState::new();
        self.render.meshes.trail.clear();
    }

    pub(super) fn enter_menu_phase(&mut self) {
        self.session.playtesting_editor = false;
        self.session.playtest_audio_start_seconds = None;
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

    /// Enters the level select screen from the menu.
    pub(crate) fn enter_level_select(&mut self) {
        if self.phase == AppPhase::Menu {
            // Initialize level select with the same selected level as menu
            self.level_select.state.selected_level = self.menu.state.selected_level;
            self.phase = AppPhase::LevelSelect;
            // Load the level metadata for the selected level
            self.load_level_select_level();
        }
    }

    /// Loads the level objects and preview camera for the currently selected level.
    pub(crate) fn load_level_select_level(&mut self) {
        let level_index = self.level_select.state.selected_level;
        let level_name = self.menu.state.levels.get(level_index).cloned();

        if let Some(name) = level_name {
            // Load the level metadata
            if let Some(metadata) = self.load_level_metadata(&name) {
                // Store the preview camera
                self.level_select.state.preview_camera = metadata.preview_camera;

                // Load the level objects into gameplay state for rendering
                self.gameplay.state.objects = metadata.objects;
                self.gameplay.state.rebuild_behavior_cache();

                // Rebuild the block vertices for rendering
                self.rebuild_block_vertices();
            } else {
                self.level_select.state.preview_camera = None;
            }
        }
    }

    /// Exits the level select screen back to the menu.
    pub(crate) fn exit_level_select(&mut self) {
        if self.phase == AppPhase::LevelSelect {
            // Sync the selected level back to menu
            self.menu.state.selected_level = self.level_select.state.selected_level;
            // Clear level select state
            self.level_select.state.preview_camera = None;
            self.gameplay.state.objects.clear();
            self.gameplay.state.rebuild_behavior_cache();
            self.rebuild_block_vertices();
            self.phase = AppPhase::Menu;
        }
    }

    /// Selects the next level in the level select screen.
    pub(crate) fn level_select_next_level(&mut self) {
        if self.phase == AppPhase::LevelSelect {
            let level_count = self.menu.state.levels.len();
            if level_count > 0 {
                self.level_select.state.selected_level =
                    (self.level_select.state.selected_level + 1) % level_count;
                // Reload the level data for the new selection
                self.load_level_select_level();
            }
        }
    }

    /// Selects the previous level in the level select screen.
    pub(crate) fn level_select_prev_level(&mut self) {
        if self.phase == AppPhase::LevelSelect {
            let level_count = self.menu.state.levels.len();
            if level_count > 0 {
                if self.level_select.state.selected_level == 0 {
                    self.level_select.state.selected_level = level_count - 1;
                } else {
                    self.level_select.state.selected_level -= 1;
                }
                // Reload the level data for the new selection
                self.load_level_select_level();
            }
        }
    }

    /// Starts playing the selected level from the level select screen.
    pub(crate) fn level_select_play(&mut self) {
        if self.phase == AppPhase::LevelSelect {
            let selected = self.level_select.state.selected_level;
            self.exit_level_select();
            self.start_level(selected);
        }
    }
}
