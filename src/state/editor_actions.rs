use super::*;

impl State {
    pub(super) fn editor_add_tap_at_pointer_position(&mut self) {
        let total_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Place {
            return;
        }

        if let Some(pointer) = self.editor_pointer_screen {
            self.update_editor_cursor_from_screen(pointer[0], pointer[1]);
        }

        let target = [
            self.editor.cursor[0] + 0.5,
            self.editor.cursor[1] + 0.5,
            self.editor.cursor[2],
        ];

        let indicator_cell = self.editor.cursor;

        if let Some(remove_index) = self
            .editor_tap_indicator_positions
            .iter()
            .enumerate()
            .filter(|(_, position)| {
                ((*position)[0] - indicator_cell[0]).abs() < 0.01
                    && ((*position)[1] - indicator_cell[1]).abs() < 0.01
                    && ((*position)[2] - indicator_cell[2]).abs() < 0.01
            })
            .min_by(|(left_index, _), (right_index, _)| {
                let left_time = self
                    .editor_tap_times
                    .get(*left_index)
                    .copied()
                    .unwrap_or(self.editor_timeline_time_seconds);
                let right_time = self
                    .editor_tap_times
                    .get(*right_index)
                    .copied()
                    .unwrap_or(self.editor_timeline_time_seconds);
                let left_distance = (left_time - self.editor_timeline_time_seconds).abs();
                let right_distance = (right_time - self.editor_timeline_time_seconds).abs();
                f32::total_cmp(&left_distance, &right_distance)
            })
            .map(|(index, _)| index)
        {
            self.record_editor_history_state();
            let removed_time = self
                .editor_tap_times
                .get(remove_index)
                .copied()
                .unwrap_or(self.editor_timeline_time_seconds);

            if remove_index < self.editor_tap_times.len() {
                self.editor_tap_times.remove(remove_index);
            }
            if remove_index < self.editor_tap_indicator_positions.len() {
                self.editor_tap_indicator_positions.remove(remove_index);
            }

            self.invalidate_editor_timeline_samples_from(removed_time);
            let (position, direction) =
                self.editor_timeline_position(self.editor_timeline_time_seconds);
            self.rebuild_editor_preview_player_vertices_for_state(position, direction);
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_tap_indicators: true,
                ..EditorDirtyFlags::default()
            });
            self.perf_record(PerfStage::TTapToggleTotal, total_started_at);
            return;
        }

        self.ensure_editor_timeline_samples();
        let solve_started_at = PlatformInstant::now();
        let derived_time = self
            .nearest_editor_timeline_sample_time_for_target(target)
            .unwrap_or(self.editor_timeline_time_seconds)
            .clamp(0.0, self.editor_timeline_duration_seconds.max(0.0));
        self.perf_record(PerfStage::TTapSolve, solve_started_at);
        self.record_editor_history_state();
        add_tap_with_indicator(
            &mut self.editor_tap_times,
            &mut self.editor_tap_indicator_positions,
            derived_time,
            indicator_cell,
        );
        self.invalidate_editor_timeline_samples_from(derived_time);
        let (position, direction) =
            self.editor_timeline_position(self.editor_timeline_time_seconds);
        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
        self.perf_record(PerfStage::TTapToggleTotal, total_started_at);
    }

    pub(super) fn resync_editor_timeline_playback_audio(&mut self) {
        if self.phase != AppPhase::Editor || !self.editor_timeline_playing {
            return;
        }

        self.stop_audio();
        self.editor_timeline_playback_runtime = Some(TimelineSimulationRuntime::new(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_objects,
            &self.editor_tap_times,
        ));
        if let Some(runtime) = self.editor_timeline_playback_runtime.as_mut() {
            runtime.advance_to(self.editor_timeline_time_seconds);
        }

        let metadata = self.current_editor_metadata();
        let level_name = self
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let start_seconds = self.editor_timeline_elapsed_seconds(self.editor_timeline_time_seconds);
        self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
    }

    pub(super) fn editor_shift_timeline_time(&mut self, delta_seconds: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let next_time = (self.editor_timeline_time_seconds + delta_seconds)
            .clamp(0.0, self.editor_timeline_duration_seconds);
        if (next_time - self.editor_timeline_time_seconds).abs() > f32::EPSILON {
            self.set_editor_timeline_time_seconds(next_time);
        }
    }

    pub(super) fn editor_nudge_selected_blocks(&mut self, dx: i32, dy: i32) -> bool {
        if self.phase != AppPhase::Editor {
            return false;
        }

        if dx == 0 && dy == 0 {
            return false;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        self.record_editor_history_state();

        let nudge_step = if self.editor_snap_to_grid {
            self.editor_snap_step.max(0.05)
        } else {
            1.0
        };

        let (camera_right_xy, camera_up_xy) = self.editor_camera_axes_xy();
        let nearest_cardinal = |axis: Vec2| -> [i32; 2] {
            let candidates = [
                (Vec2::new(1.0, 0.0), [1, 0]),
                (Vec2::new(-1.0, 0.0), [-1, 0]),
                (Vec2::new(0.0, 1.0), [0, 1]),
                (Vec2::new(0.0, -1.0), [0, -1]),
            ];

            let mut best = candidates[0];
            let mut best_dot = axis.dot(best.0);
            for candidate in candidates.into_iter().skip(1) {
                let candidate_dot = axis.dot(candidate.0);
                if candidate_dot > best_dot {
                    best = candidate;
                    best_dot = candidate_dot;
                }
            }

            best.1
        };

        let right_world = nearest_cardinal(camera_right_xy);
        let up_world = nearest_cardinal(camera_up_xy);
        let world_dx = right_world[0] * dx + up_world[0] * dy;
        let world_dy = right_world[1] * dx + up_world[1] * dy;

        let (world_dx, world_dy) = if world_dx.abs() > world_dy.abs() {
            (world_dx.signum(), 0)
        } else if world_dy.abs() > world_dx.abs() {
            (0, world_dy.signum())
        } else if world_dx != 0 {
            (world_dx.signum(), 0)
        } else {
            (0, world_dy.signum())
        };

        for index in &selected_indices {
            if let Some(obj) = self.editor_objects.get_mut(*index) {
                obj.position[0] += world_dx as f32 * nudge_step;
                obj.position[1] += world_dy as f32 * nudge_step;
            }
        }

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| selected_indices.contains(index))
            .or_else(|| selected_indices.first().copied())
        {
            if let Some(obj) = self.editor_objects.get(index) {
                let bounds = self.editor.bounds;
                self.editor.cursor = [
                    obj.position[0].clamp(-bounds as f32, bounds as f32),
                    obj.position[1].clamp(-bounds as f32, bounds as f32),
                    obj.position[2].max(0.0),
                ];
            }
        }

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        true
    }

    pub(super) fn toggle_editor_timeline_playback(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor_timeline_playing = !self.editor_timeline_playing;

        if self.editor_timeline_playing {
            self.editor_timeline_playback_runtime = Some(TimelineSimulationRuntime::new(
                self.editor_spawn.position,
                self.editor_spawn.direction,
                &self.editor_objects,
                &self.editor_tap_times,
            ));
            if let Some(runtime) = self.editor_timeline_playback_runtime.as_mut() {
                runtime.advance_to(self.editor_timeline_time_seconds);
            }

            let metadata = self.current_editor_metadata();
            let level_name = self
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let start_seconds =
                self.editor_timeline_elapsed_seconds(self.editor_timeline_time_seconds);
            self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
            return;
        }

        self.editor_timeline_playback_runtime = None;
        self.stop_audio();
        self.refresh_editor_timeline_position();
    }

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

        self.editor_timeline_playing = false;
        self.editor_timeline_playback_runtime = None;
        self.stop_audio();

        let transition = build_editor_playtest_transition(
            &self.editor_objects,
            self.editor_level_name.as_deref(),
            self.editor_spawn.clone(),
            &self.editor_tap_times,
            self.editor_timeline_time_seconds,
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
        self.editor_spawn.position = cursor;
        self.invalidate_editor_timeline_samples();

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
        self.invalidate_editor_timeline_samples();
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn back_to_menu(&mut self) {
        self.editor_timeline_playing = false;
        self.editor_timeline_playback_runtime = None;
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.playtesting_editor, &self.editor_objects)
        {
            self.playtesting_editor = false;
            self.phase = AppPhase::Editor;
            self.editor_timeline_playing = false;
            self.editor_timeline_playback_runtime = None;
            self.game = GameState::new();
            self.game.objects = objects;
            self.rebuild_block_vertices();
            return;
        }

        self.enter_menu_phase();

        self.game = GameState::new();
        self.game.objects = create_menu_scene();
        self.rebuild_block_vertices();
        self.meshes.trail.clear();
    }

    pub(super) fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        let step = if self.editor_snap_to_grid {
            self.editor_snap_step.max(0.05)
        } else {
            1.0
        };
        self.editor.cursor[0] = (self.editor.cursor[0] + dx as f32 * step)
            .clamp(-self.editor.bounds as f32, self.editor.bounds as f32);
        self.editor.cursor[1] = (self.editor.cursor[1] + dy as f32 * step)
            .clamp(-self.editor.bounds as f32, self.editor.bounds as f32);
        self.rebuild_editor_cursor_vertices();
    }
}
