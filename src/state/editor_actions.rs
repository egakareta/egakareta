use glam::Vec2;

use super::{EditorDirtyFlags, EditorSubsystem, PerfStage, State};
use crate::editor_domain::{
    add_tap_with_indicator, build_editor_playtest_transition,
    derive_timeline_time_for_world_target_near_time, playtest_return_objects,
    remove_topmost_block_at_cursor, toggle_spawn_direction, TimelineNearSearch,
};
use crate::game::{create_menu_scene, GameState, TimelineSimulationRuntime};
use crate::platform::state_host::PlatformInstant;
use crate::types::{AppPhase, EditorMode};

impl EditorSubsystem {
    pub(crate) fn toggle_tap_at_cursor(&mut self) -> (Option<f32>, bool) {
        let indicator_cell = self.ui.cursor;

        if let Some(remove_index) = self
            .timeline
            .taps
            .tap_indicator_positions
            .iter()
            .enumerate()
            .filter(|(_, position)| {
                ((*position)[0] - indicator_cell[0]).abs() < 0.01
                    && ((*position)[1] - indicator_cell[1]).abs() < 0.01
                    && ((*position)[2] - indicator_cell[2]).abs() < 0.01
            })
            .min_by(|(left_index, _), (right_index, _)| {
                let left_time = self
                    .timeline
                    .taps
                    .tap_times
                    .get(*left_index)
                    .copied()
                    .unwrap_or(self.timeline.clock.time_seconds);
                let right_time = self
                    .timeline
                    .taps
                    .tap_times
                    .get(*right_index)
                    .copied()
                    .unwrap_or(self.timeline.clock.time_seconds);
                let left_distance = (left_time - self.timeline.clock.time_seconds).abs();
                let right_distance = (right_time - self.timeline.clock.time_seconds).abs();
                f32::total_cmp(&left_distance, &right_distance)
            })
            .map(|(index, _)| index)
        {
            let removed_time = self
                .timeline
                .taps
                .tap_times
                .get(remove_index)
                .copied()
                .unwrap_or(self.timeline.clock.time_seconds);

            if remove_index < self.timeline.taps.tap_times.len() {
                self.timeline.taps.tap_times.remove(remove_index);
            }
            if remove_index < self.timeline.taps.tap_indicator_positions.len() {
                self.timeline
                    .taps
                    .tap_indicator_positions
                    .remove(remove_index);
            }

            self.invalidate_samples_from(removed_time);
            return (Some(removed_time), false);
        }

        let duration_seconds = self.timeline.clock.duration_seconds.max(0.0);
        let seed_time = self
            .timeline
            .clock
            .time_seconds
            .clamp(0.0, duration_seconds);
        let target_world = [
            indicator_cell[0] + 0.5,
            indicator_cell[1] + 0.5,
            indicator_cell[2],
        ];
        let preview_cell = self.tap_indicator_position_from_world(self.timeline.preview.position);
        let derived_time = if (preview_cell[0] - indicator_cell[0]).abs() <= 0.001
            && (preview_cell[1] - indicator_cell[1]).abs() <= 0.001
            && (preview_cell[2] - indicator_cell[2]).abs() <= 0.001
        {
            seed_time
        } else {
            derive_timeline_time_for_world_target_near_time(
                self.spawn.position,
                self.spawn.direction,
                &self.timeline.taps.tap_times,
                duration_seconds,
                &self.objects,
                target_world,
                TimelineNearSearch {
                    seed_time,
                    window_seconds: 1.5,
                },
            )
            .clamp(0.0, duration_seconds)
        };

        add_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            derived_time,
            indicator_cell,
        );
        self.invalidate_samples_from(derived_time);
        (Some(derived_time), true)
    }

    pub(crate) fn shift_timeline_time(&mut self, delta_seconds: f32) -> bool {
        let next_time = (self.timeline.clock.time_seconds + delta_seconds)
            .clamp(0.0, self.timeline.clock.duration_seconds);
        if (next_time - self.timeline.clock.time_seconds).abs() > f32::EPSILON {
            self.timeline.clock.time_seconds = next_time;
            true
        } else {
            false
        }
    }

    pub(crate) fn nudge_selected(&mut self, world_dx: f32, world_dy: f32) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        for index in &selected_indices {
            if let Some(obj) = self.objects.get_mut(*index) {
                obj.position[0] += world_dx;
                obj.position[1] += world_dy;
            }
        }

        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .or_else(|| selected_indices.first().copied())
        {
            if let Some(obj) = self.objects.get(index) {
                let bounds = self.ui.bounds as f32;
                self.ui.cursor = [
                    obj.position[0].clamp(-bounds, bounds),
                    obj.position[1].clamp(-bounds, bounds),
                    obj.position[2].max(0.0),
                ];
            }
        }

        self.invalidate_samples();
        true
    }

    pub(crate) fn remove_selected(&mut self) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if !selected_indices.is_empty() {
            for index in selected_indices.into_iter().rev() {
                if index < self.objects.len() {
                    self.objects.remove(index);
                }
            }
            self.ui.selected_block_index = None;
            self.ui.selected_block_indices.clear();
            self.ui.hovered_block_index = None;
            self.invalidate_samples();
            return true;
        }

        if remove_topmost_block_at_cursor(&mut self.objects, self.ui.cursor) {
            self.invalidate_samples();
            return true;
        }

        false
    }

    pub(crate) fn set_spawn_here(&mut self) {
        self.spawn.position = self.ui.cursor;
        self.invalidate_samples();
    }

    pub(crate) fn rotate_spawn_direction(&mut self) {
        self.spawn.direction = toggle_spawn_direction(self.spawn.direction);
        self.invalidate_samples();
    }

    pub(crate) fn move_cursor(&mut self, dx: i32, dy: i32) {
        let step = if self.config.snap_to_grid {
            self.config.snap_step.max(0.05)
        } else {
            1.0
        };
        self.ui.cursor[0] = (self.ui.cursor[0] + dx as f32 * step)
            .clamp(-self.ui.bounds as f32, self.ui.bounds as f32);
        self.ui.cursor[1] = (self.ui.cursor[1] + dy as f32 * step)
            .clamp(-self.ui.bounds as f32, self.ui.bounds as f32);
    }
}

impl State {
    pub(super) fn editor_add_tap_at_pointer_position(&mut self) {
        let total_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Place {
            return;
        }

        if let Some(pointer) = self.editor.ui.pointer_screen {
            self.update_editor_cursor_from_screen(pointer[0], pointer[1]);
        }

        let solve_started_at = PlatformInstant::now();
        self.record_editor_history_state();

        let (time, added) = self.editor.toggle_tap_at_cursor();
        if added {
            self.perf_record(PerfStage::TTapSolve, solve_started_at);
        }

        if time.is_some() {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_tap_indicators: true,
                ..EditorDirtyFlags::default()
            });
        }

        self.perf_record(PerfStage::TTapToggleTotal, total_started_at);
    }

    pub(super) fn resync_editor_timeline_playback_audio(&mut self) {
        if self.phase != AppPhase::Editor || !self.editor.timeline.playback.playing {
            return;
        }

        self.stop_audio();
        if self.editor.ui.mode == EditorMode::Timing {
            self.editor.timeline.playback.runtime = None;
        } else {
            self.editor.timeline.playback.runtime = Some(TimelineSimulationRuntime::new(
                self.editor.spawn.position,
                self.editor.spawn.direction,
                &self.editor.objects,
                &self.editor.timeline.taps.tap_times,
            ));
            if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut() {
                runtime.advance_to(self.editor.timeline.clock.time_seconds);
            }
        }

        let metadata = self.current_editor_metadata();
        let level_name = self
            .session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let start_seconds =
            self.editor_timeline_elapsed_seconds(self.editor.timeline.clock.time_seconds);
        self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
    }

    pub(super) fn editor_shift_timeline_time(&mut self, delta_seconds: f32) {
        if self.phase == AppPhase::Editor && self.editor.shift_timeline_time(delta_seconds) {
            let next_time = self.editor.timeline.clock.time_seconds;
            self.set_editor_timeline_time_seconds(next_time);
        }
    }

    pub(super) fn editor_nudge_selected_blocks(&mut self, dx: i32, dy: i32) -> bool {
        if self.phase != AppPhase::Editor || (dx == 0 && dy == 0) {
            return false;
        }

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

        let nudge_step = if self.editor.config.snap_to_grid {
            self.editor.config.snap_step.max(0.05)
        } else {
            1.0
        };

        self.record_editor_history_state();
        if self
            .editor
            .nudge_selected(world_dx as f32 * nudge_step, world_dy as f32 * nudge_step)
        {
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            return true;
        }

        false
    }

    pub(super) fn toggle_editor_timeline_playback(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor.timeline.playback.playing = !self.editor.timeline.playback.playing;

        if self.editor.timeline.playback.playing {
            if self.editor.ui.mode == EditorMode::Timing {
                self.editor.timeline.playback.runtime = None;
            } else {
                self.editor.timeline.playback.runtime = Some(TimelineSimulationRuntime::new(
                    self.editor.spawn.position,
                    self.editor.spawn.direction,
                    &self.editor.objects,
                    &self.editor.timeline.taps.tap_times,
                ));
                if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut() {
                    runtime.advance_to(self.editor.timeline.clock.time_seconds);
                }
            }

            let metadata = self.current_editor_metadata();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let start_seconds =
                self.editor_timeline_elapsed_seconds(self.editor.timeline.clock.time_seconds);
            self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
            return;
        }

        self.editor.timeline.playback.runtime = None;
        self.stop_audio();
    }

    pub fn editor_remove_block(&mut self) {
        if self.phase == AppPhase::Editor {
            self.record_editor_history_state();
            if self.editor.remove_selected() {
                self.sync_editor_objects();
                self.rebuild_editor_cursor_vertices();
            }
        }
    }

    pub fn editor_playtest(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor.timeline.playback.playing = false;
        self.editor.timeline.playback.runtime = None;
        self.stop_audio();

        let transition = build_editor_playtest_transition(
            &self.editor.objects,
            self.session.editor_level_name.as_deref(),
            self.editor.spawn.clone(),
            &self.editor.timeline.taps.tap_times,
            self.editor.timeline.clock.time_seconds,
        );

        self.enter_playing_phase(transition.playing_level_name, true);
        self.session.playtest_audio_start_seconds = Some(transition.playtest_audio_start_seconds);
        self.gameplay.state = GameState::new();
        self.gameplay.state.objects = transition.objects;
        self.gameplay.state.rebuild_behavior_cache();
        self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        self.editor.camera.playing_rotation = transition.camera_rotation;
        self.editor.camera.playing_pitch = transition.camera_pitch;
        self.editor.ui.right_dragging = false;
        self.rebuild_block_vertices();
    }

    pub fn editor_set_spawn_here(&mut self) {
        if self.phase == AppPhase::Editor {
            self.record_editor_history_state();
            self.editor.set_spawn_here();
            self.sync_editor_objects();
            self.rebuild_spawn_marker_vertices();
        }
    }

    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase == AppPhase::Editor {
            self.record_editor_history_state();
            self.editor.rotate_spawn_direction();
            self.rebuild_spawn_marker_vertices();
        }
    }

    pub fn back_to_menu(&mut self) {
        self.editor.timeline.playback.playing = false;
        self.editor.timeline.playback.runtime = None;
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.session.playtesting_editor, &self.editor.objects)
        {
            self.session.playtesting_editor = false;
            self.phase = AppPhase::Editor;
            self.editor.timeline.playback.playing = false;
            self.editor.timeline.playback.runtime = None;
            self.gameplay.state = GameState::new();
            self.gameplay.state.objects = objects;
            self.gameplay.state.rebuild_behavior_cache();
            self.rebuild_block_vertices();
            return;
        }

        self.enter_menu_phase();

        self.gameplay.state = GameState::new();
        self.gameplay.state.objects = create_menu_scene();
        self.gameplay.state.rebuild_behavior_cache();
        self.rebuild_block_vertices();
        self.render.meshes.trail.clear();
    }

    pub(super) fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        if self.phase == AppPhase::Editor {
            self.editor.move_cursor(dx, dy);
            self.rebuild_editor_cursor_vertices();
        }
    }
}
