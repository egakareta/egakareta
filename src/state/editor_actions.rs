use super::*;

impl State {
    pub fn editor_remove_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        let selected_indices = self.selected_block_indices_normalized();
        if !selected_indices.is_empty() {
            for index in selected_indices.into_iter().rev() {
                if index < self.editor_objects.len() {
                    self.editor_objects.remove(index);
                }
            }
            self.editor_selected_block_index = None;
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            return;
        }

        remove_topmost_block_at_cursor(&mut self.editor_objects, self.editor.cursor);

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub fn editor_playtest(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.stop_audio();

        let transition = build_editor_playtest_transition(
            &self.editor_objects,
            self.editor_level_name.as_deref(),
            self.editor_spawn.clone(),
            &self.editor_tap_steps,
            self.editor_timeline_step,
        );

        self.enter_playing_phase(transition.playing_level_name, true);
        self.game = GameState::new();
        self.game.objects = transition.objects;
        self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        self.playing_camera_rotation = transition.camera_rotation;
        self.playing_camera_pitch = transition.camera_pitch;
        self.editor_right_dragging = false;
        self.rebuild_block_vertices();
    }

    pub fn editor_set_spawn_here(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        let cursor = self.editor.cursor;
        self.editor_spawn.position = [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32];

        self.sync_editor_objects();
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.editor_spawn.direction = toggle_spawn_direction(self.editor_spawn.direction);
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn back_to_menu(&mut self) {
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.playtesting_editor, &self.editor_objects)
        {
            self.playtesting_editor = false;
            self.phase = AppPhase::Editor;
            self.game = GameState::new();
            self.game.objects = objects;
            self.rebuild_block_vertices();
            return;
        }

        self.enter_menu_phase();

        self.game = GameState::new();
        self.game.objects = create_menu_scene();
        self.rebuild_block_vertices();
        self.trail_vertex_count = 0;
    }

    pub(super) fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        move_cursor_xy(&mut self.editor.cursor, dx, dy, self.editor.bounds);
        self.rebuild_editor_cursor_vertices();
    }
}
