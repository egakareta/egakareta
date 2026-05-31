/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::Vec2;

use super::{EditorDirtyFlags, EditorSubsystem, State};
use crate::editor_domain::{
    add_tap_with_indicator, build_editor_playtest_transition, derive_tap_indicator_positions,
    derive_timeline_time_for_world_target_near_time, derive_timing_division_tap_previews,
    playtest_return_objects, remove_topmost_block_at_cursor, toggle_spawn_direction,
    TapDivisionPreview, TapDivisionPreviewRange, TimelineNearSearch,
};
use crate::game::{GameState, TimelineSimulationRuntime};
use crate::types::{AppPhase, EditorMode};

fn distance_sq(left: [f32; 3], right: [f32; 3]) -> f32 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    let dz = left[2] - right[2];
    dx * dx + dy * dy + dz * dz
}

fn closest_point_on_segment(start: [f32; 3], end: [f32; 3], target: [f32; 3]) -> ([f32; 3], f32) {
    let segment = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let segment_length_sq =
        segment[0] * segment[0] + segment[1] * segment[1] + segment[2] * segment[2];
    if segment_length_sq <= f32::EPSILON {
        return (start, 0.0);
    }

    let target_offset = [
        target[0] - start[0],
        target[1] - start[1],
        target[2] - start[2],
    ];
    let alpha = ((target_offset[0] * segment[0]
        + target_offset[1] * segment[1]
        + target_offset[2] * segment[2])
        / segment_length_sq)
        .clamp(0.0, 1.0);

    (
        [
            start[0] + segment[0] * alpha,
            start[1] + segment[1] * alpha,
            start[2] + segment[2] * alpha,
        ],
        alpha,
    )
}

#[derive(Clone, Copy)]
enum PathAxis {
    X,
    Z,
}

fn segment_path_axis(start: [f32; 3], end: [f32; 3]) -> Option<PathAxis> {
    let dx = (end[0] - start[0]).abs();
    let dz = (end[2] - start[2]).abs();
    if dx <= f32::EPSILON && dz <= f32::EPSILON {
        None
    } else if dx > dz {
        Some(PathAxis::X)
    } else {
        Some(PathAxis::Z)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TapPathPick {
    pub(crate) indicator_position: [f32; 3],
    pub(crate) time_seconds: f32,
}

impl EditorSubsystem {
    fn sync_tap_indicators_to_spawn(&mut self) {
        let selected_index = self.timeline.taps.selected_index;
        self.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.spawn.position,
            self.spawn.direction,
            &self.timeline.taps.tap_times,
            &self.objects,
        );
        self.set_selected_tap_index(selected_index);
    }

    pub(crate) fn timing_division_tap_previews(&mut self) -> &[TapDivisionPreview] {
        let duration_seconds = self.timeline.clock.duration_seconds.max(0.0);
        let window = (0.0, duration_seconds);
        let cache_is_current = self.timeline.tap_division_preview_cache_revision
            == self.timeline.simulation_revision
            && self.timeline.tap_division_preview_cache_timing_revision == self.timing.revision
            && (self.timeline.tap_division_preview_cache_duration_seconds - duration_seconds).abs()
                <= f32::EPSILON
            && (self.timeline.tap_division_preview_cache_window.0 - window.0).abs() <= f32::EPSILON
            && (self.timeline.tap_division_preview_cache_window.1 - window.1).abs() <= f32::EPSILON;

        if !cache_is_current {
            self.timeline.tap_division_preview_cache = derive_timing_division_tap_previews(
                self.spawn.position,
                self.spawn.direction,
                &self.timeline.taps.tap_times,
                &self.timing.timing_points,
                duration_seconds,
                TapDivisionPreviewRange {
                    start_seconds: window.0,
                    end_seconds: window.1,
                },
                &self.objects,
            );
            self.timeline.tap_division_preview_cache_revision = self.timeline.simulation_revision;
            self.timeline.tap_division_preview_cache_timing_revision = self.timing.revision;
            self.timeline.tap_division_preview_cache_duration_seconds = duration_seconds;
            self.timeline.tap_division_preview_cache_window = window;
        }

        &self.timeline.tap_division_preview_cache
    }

    pub(crate) fn tap_path_pick_near_world(&self, target_world: [f32; 3]) -> Option<TapPathPick> {
        let cache = &self.timeline.snapshot_cache;
        if cache.is_empty()
            || self.timeline.snapshot_cache_revision != self.timeline.simulation_revision
        {
            return None;
        }

        let step_seconds = self.timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
        if cache.len() == 1 {
            return Some(TapPathPick {
                indicator_position: self.tap_indicator_position_from_world(cache[0].position),
                time_seconds: 0.0,
            });
        }

        let mut best_position = cache[0].position;
        let mut best_index = 0;
        let mut best_alpha = 0.0;
        let mut best_axis = None;
        let mut best_distance_sq = f32::INFINITY;

        for index in 0..cache.len().saturating_sub(1) {
            let previous = cache[index].position;
            let current = cache[index + 1].position;
            let (candidate, alpha) = closest_point_on_segment(previous, current, target_world);
            let candidate_distance_sq = distance_sq(candidate, target_world);
            if candidate_distance_sq < best_distance_sq {
                best_distance_sq = candidate_distance_sq;
                best_position = candidate;
                best_index = index;
                best_alpha = alpha;
                best_axis = segment_path_axis(previous, current);
            }
        }

        let mut indicator_position = self.tap_indicator_position_from_world(best_position);
        let mut time_seconds = (best_index as f32 + best_alpha) * step_seconds;
        if self.effective_snap_to_grid() {
            let snap_step = self.config.snap_step.max(0.05);
            match best_axis {
                Some(PathAxis::X) => {
                    indicator_position[0] = (indicator_position[0] / snap_step).floor() * snap_step;
                    let snapped_world_x = indicator_position[0] + 0.5;
                    let previous = cache[best_index].position;
                    let current = cache[best_index + 1].position;
                    let segment_x = current[0] - previous[0];
                    if segment_x.abs() > f32::EPSILON {
                        best_alpha = ((snapped_world_x - previous[0]) / segment_x).clamp(0.0, 1.0);
                        time_seconds = (best_index as f32 + best_alpha) * step_seconds;
                    }
                }
                Some(PathAxis::Z) => {
                    indicator_position[2] = (indicator_position[2] / snap_step).floor() * snap_step;
                    let snapped_world_z = indicator_position[2] + 0.5;
                    let previous = cache[best_index].position;
                    let current = cache[best_index + 1].position;
                    let segment_z = current[2] - previous[2];
                    if segment_z.abs() > f32::EPSILON {
                        best_alpha = ((snapped_world_z - previous[2]) / segment_z).clamp(0.0, 1.0);
                        time_seconds = (best_index as f32 + best_alpha) * step_seconds;
                    }
                }
                None => {}
            }
        }

        Some(TapPathPick {
            indicator_position,
            time_seconds: time_seconds.clamp(0.0, self.timeline.clock.duration_seconds.max(0.0)),
        })
    }

    pub(crate) fn tap_path_cursor_near_world(&self, target_world: [f32; 3]) -> [f32; 3] {
        self.tap_path_pick_near_world(target_world)
            .map(|pick| pick.indicator_position)
            .unwrap_or_else(|| {
                self.tap_indicator_position_from_world(self.timeline.preview.position)
            })
    }

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
            self.adjust_selected_tap_after_removal(remove_index);

            self.invalidate_samples_from(removed_time);
            return (Some(removed_time), false);
        }

        let duration_seconds = self.timeline.clock.duration_seconds.max(0.0);
        let seed_time = self
            .timeline_elapsed_seconds(self.timeline.clock.time_seconds)
            .clamp(0.0, duration_seconds);
        let target_world = [
            indicator_cell[0] + 0.5,
            indicator_cell[1],
            indicator_cell[2] + 0.5,
        ];
        let preview_cell = self.tap_indicator_position_from_world(self.timeline.preview.position);
        let derived_time = if (preview_cell[0] - indicator_cell[0]).abs() <= 0.001
            && (preview_cell[1] - indicator_cell[1]).abs() <= 0.001
            && (preview_cell[2] - indicator_cell[2]).abs() <= 0.001
        {
            seed_time
        } else {
            self.tap_path_pick_near_world(target_world)
                .map(|pick| pick.time_seconds)
                .unwrap_or_else(|| {
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
                })
        };

        let selected_index = add_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            derived_time,
            indicator_cell,
        );
        self.set_selected_tap_index(selected_index);
        self.invalidate_samples_from(derived_time);
        (Some(derived_time), true)
    }

    pub(crate) fn nudge_selected(&mut self, world_dx: f32, world_dz: f32) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        for index in &selected_indices {
            if let Some(obj) = self.objects.get_mut(*index) {
                obj.position[0] += world_dx;
                obj.position[2] += world_dz;
            }
        }

        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .or_else(|| selected_indices.first().copied())
        {
            if let Some(obj) = self.objects.get(index) {
                self.ui.cursor = [obj.position[0], obj.position[1].max(0.0), obj.position[2]];
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
            self.clear_block_selection();
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
        self.sync_tap_indicators_to_spawn();
        self.invalidate_samples();
    }

    pub(crate) fn rotate_spawn_direction(&mut self) {
        self.spawn.direction = toggle_spawn_direction(self.spawn.direction);
        self.sync_tap_indicators_to_spawn();
        self.invalidate_samples();
    }

    pub(crate) fn move_cursor(&mut self, dx: i32, dy: i32) {
        let step = if self.effective_snap_to_grid() {
            self.config.snap_step.max(0.05)
        } else {
            1.0
        };
        self.ui.cursor[0] += dx as f32 * step;
        self.ui.cursor[2] += dy as f32 * step;
    }
}

impl State {
    pub(crate) fn refresh_editor_after_tap_change(&mut self, cursor_override: Option<[f32; 3]>) {
        self.editor.sync_tap_indicators_to_spawn();
        if let (Some(cursor), Some(selected_index)) =
            (cursor_override, self.editor.timeline.taps.selected_index)
        {
            if let Some(position) = self
                .editor
                .timeline
                .taps
                .tap_indicator_positions
                .get_mut(selected_index)
            {
                *position = cursor;
            }
        }
        let current_time = self.editor.timeline.clock.time_seconds;
        self.set_editor_timeline_time_seconds_preserving_editor_camera(current_time);
        self.resync_editor_timeline_playback_audio();
        if let Some(cursor) = cursor_override {
            self.editor.ui.cursor = cursor;
            self.rebuild_editor_cursor_vertices();
        }
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor: cursor_override.is_some(),
            ..EditorDirtyFlags::default()
        });
    }

    pub(super) fn editor_add_tap_at_pointer_position(&mut self) {
        puffin::profile_scope!("TKeyToggle");
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Tapping {
            return;
        }

        if let Some(pointer) = self.editor.ui.pointer_screen {
            self.update_editor_cursor_from_screen(pointer[0], pointer[1]);
        }

        self.record_editor_history_state();

        let (time, _added) = {
            puffin::profile_scope!("TKeySolve");
            self.editor.toggle_tap_at_cursor()
        };

        if time.is_some() {
            self.refresh_editor_after_tap_change(Some(self.editor.ui.cursor));
        }
    }

    pub(super) fn editor_select_tap_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Tapping {
            return false;
        }

        let viewport_size = glam::Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        let Some(pick) = self.editor.pick_from_screen(x, y, viewport_size) else {
            return false;
        };

        if let Some((selected_index, selected_time_seconds, selected_position)) =
            self.editor.selected_tap()
        {
            if self.editor.timeline.taps.selected_index == Some(selected_index) {
                let selected_screen = self.editor.world_to_screen_v(
                    glam::Vec3::new(
                        selected_position[0] + 0.5,
                        selected_position[1] + 0.1,
                        selected_position[2] + 0.5,
                    ),
                    viewport_size,
                );
                if let Some(selected_screen) = selected_screen {
                    let dx = selected_screen.x - x as f32;
                    let dy = selected_screen.y - y as f32;
                    if dx * dx + dy * dy <= 24.0 * 24.0 {
                        self.editor_remove_tap_at(selected_time_seconds);
                        return true;
                    }
                }
            }
        }

        let Some(tap_index) = pick.hit_tap_index else {
            if let Some(division) = pick.hit_tap_division {
                self.record_editor_history_state();
                let selected_index = add_tap_with_indicator(
                    &mut self.editor.timeline.taps.tap_times,
                    &mut self.editor.timeline.taps.tap_indicator_positions,
                    division.time_seconds,
                    division.indicator_position,
                );
                self.editor.set_selected_tap_index(selected_index);
                self.editor.ui.cursor = division.indicator_position;
                self.editor.invalidate_samples_from(division.time_seconds);
                self.set_editor_timeline_time_seconds_preserving_editor_camera(
                    division.time_seconds,
                );
                self.refresh_editor_after_tap_change(Some(division.indicator_position));
                return true;
            }
            return false;
        };
        let Some(time_seconds) = self.editor.timeline.taps.tap_times.get(tap_index).copied() else {
            return false;
        };

        self.editor.set_selected_tap_index(Some(tap_index));
        self.editor.runtime.interaction.hovered_tap_index = Some(tap_index);
        self.set_editor_timeline_time_seconds_preserving_editor_camera(time_seconds);
        self.editor.ui.cursor = pick.cursor;
        self.rebuild_tap_indicator_vertices();
        self.rebuild_editor_cursor_vertices();
        true
    }

    pub(super) fn resync_editor_timeline_playback_audio(&mut self) {
        if self.phase != AppPhase::Editor || !self.editor.timeline.playback.playing {
            return;
        }

        {
            puffin::profile_scope!("SeekAudioStop");
            self.stop_audio();
        }

        {
            puffin::profile_scope!("SeekRuntimeBuild");
            if self.editor_is_effectively_timing_mode() {
                self.editor.timeline.playback.runtime = None;
            } else {
                self.editor.timeline.playback.runtime =
                    Some(TimelineSimulationRuntime::new_with_triggers(
                        self.editor.spawn.position,
                        self.editor.spawn.direction,
                        &self.editor.objects,
                        &self.editor.timeline.taps.tap_times,
                        self.editor.triggers(),
                        self.editor.simulate_trigger_hitboxes(),
                    ));
                if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut() {
                    runtime.advance_to(self.editor.timeline.clock.time_seconds);
                }
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

        {
            puffin::profile_scope!("SeekAudioStart");
            self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
        }
    }

    pub(super) fn editor_shift_timeline_time(&mut self, delta_seconds: f32) {
        if self.phase == AppPhase::Editor {
            let current_time = self.editor.timeline.clock.time_seconds;
            let next_time = (current_time + delta_seconds)
                .clamp(0.0, self.editor.timeline.clock.duration_seconds);
            if (next_time - current_time).abs() > f32::EPSILON {
                self.set_editor_timeline_time_seconds(next_time);
            }
        }
    }

    pub(super) fn set_editor_playback_effective_mode(&mut self, mode: EditorMode) {
        if self.phase != AppPhase::Editor || !self.editor.timeline.playback.playing {
            self.set_editor_mode(mode);
            return;
        }

        let old_mode = self.editor_effective_mode_for_playback();
        self.editor.runtime.interaction.last_mode = Some(mode);
        self.set_editor_mode(EditorMode::Null);

        if mode == EditorMode::Timing {
            self.editor.timeline.playback.runtime = None;
        } else if self.editor.timeline.playback.runtime.is_none() {
            self.editor.timeline.playback.runtime =
                Some(TimelineSimulationRuntime::new_with_triggers(
                    self.editor.spawn.position,
                    self.editor.spawn.direction,
                    &self.editor.objects,
                    &self.editor.timeline.taps.tap_times,
                    self.editor.triggers(),
                    self.editor.simulate_trigger_hitboxes(),
                ));
            if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut() {
                runtime.advance_to(self.editor.timeline.clock.time_seconds);
            }
        }

        if old_mode != mode && self.editor.has_object_transform_triggers() {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_block_mesh: true,
                ..EditorDirtyFlags::default()
            });
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

        let right_world = nearest_cardinal(-camera_right_xy);
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

        let nudge_step = if self.editor.effective_snap_to_grid() {
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
            let last_mode = self.editor.ui.mode;
            self.editor.runtime.interaction.last_mode = Some(last_mode);
            self.editor.set_mode(EditorMode::Null);
            self.editor.timeline.playback.pending_seek_time_seconds = None;
            self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;

            if self.editor.has_object_transform_triggers() {
                self.mark_editor_dirty(EditorDirtyFlags {
                    rebuild_block_mesh: true,
                    rebuild_tap_indicators: true,
                    ..EditorDirtyFlags::default()
                });
            } else {
                self.mark_editor_dirty(EditorDirtyFlags {
                    rebuild_tap_indicators: true,
                    ..EditorDirtyFlags::default()
                });
            }
            if last_mode == EditorMode::Timing {
                self.editor.timeline.playback.runtime = None;
            } else {
                self.editor.timeline.playback.runtime =
                    Some(TimelineSimulationRuntime::new_with_triggers(
                        self.editor.spawn.position,
                        self.editor.spawn.direction,
                        &self.editor.objects,
                        &self.editor.timeline.taps.tap_times,
                        self.editor.triggers(),
                        self.editor.simulate_trigger_hitboxes(),
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
        self.editor.timeline.playback.pending_seek_time_seconds = None;
        self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
        if let Some(last_mode) = self.editor.runtime.interaction.last_mode.take() {
            self.editor.set_mode(last_mode);
        } else {
            self.editor.set_mode(EditorMode::Place);
        }

        if self.editor.has_object_transform_triggers() {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_block_mesh: true,
                rebuild_tap_indicators: true,
                ..EditorDirtyFlags::default()
            });
        } else {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_tap_indicators: true,
                ..EditorDirtyFlags::default()
            });
        }
        self.stop_audio();
    }

    /// Removes the currently selected block from the editor.
    ///
    /// This action is recorded in the editor's history for undo/redo support.
    /// If a block is successfully removed, the editor's visual objects and
    /// cursor vertices are synchronized to reflect the change.
    pub fn editor_remove_block(&mut self) {
        if self.phase != AppPhase::Editor
            || self.editor_effective_mode_for_playback() == EditorMode::Tapping
        {
            return;
        }

        self.record_editor_history_state();
        if self.editor.remove_selected() {
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
        }
    }

    /// Transitions the application from the editor phase to the playing phase for playtesting.
    ///
    /// This method captures the current state of the editor (blocks, taps, spawn point)
    /// and initializes the gameplay state with these parameters. It also manages
    /// audio stopping and restarting at the correct point in the timeline.
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
            self.editor.triggers(),
            self.editor.simulate_trigger_hitboxes(),
            self.editor.timeline.clock.time_seconds,
        );

        let music_source = self.session.editor_music_metadata.source.clone();
        let metadata = self.current_editor_metadata();
        let level_name = transition
            .playing_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        if let Some(level_name) = transition.playing_level_name.as_deref() {
            self.preload_runtime_audio(level_name, &music_source);
        }
        self.warmup_audio_at_seconds(
            &level_name,
            &metadata,
            transition.playtest_audio_start_seconds,
        );

        self.enter_playing_phase(transition.playing_level_name, true);
        self.session.playtest_audio_start_seconds = Some(transition.playtest_audio_start_seconds);
        self.gameplay.state = GameState::new();
        self.gameplay.state.objects = transition.objects;
        self.gameplay.state.rebuild_behavior_cache();
        self.session.playing_trigger_hitboxes = self.editor.simulate_trigger_hitboxes();
        self.session.playing_trigger_base_objects = Some(self.gameplay.state.objects.clone());
        self.apply_spawn_exact_to_game(
            transition.spawn_position,
            transition.spawn_direction,
            Some(transition.spawn_speed),
        );
        self.gameplay.state.elapsed_seconds = transition.playtest_audio_start_seconds;
        self.editor.camera.playing_rotation = transition.camera_rotation;
        self.editor.camera.playing_pitch = transition.camera_pitch;
        self.editor.ui.right_dragging = false;
        self.rebuild_block_vertices();
    }

    /// Sets the player's spawn point to the current editor cursor position.
    ///
    /// The change is recorded in the editor's history and the spawn marker
    /// vertices are rebuilt to reflect the new position.
    pub fn editor_set_spawn_here(&mut self) {
        if self.phase == AppPhase::Editor {
            self.record_editor_history_state();
            self.editor.set_spawn_here();
            self.sync_editor_objects();
            self.rebuild_spawn_marker_vertices();
        }
    }

    /// Rotates the player's starting direction at the spawn point.
    ///
    /// Each call cycles through available spawn directions. The change is
    /// recorded in history and the spawn marker's visual representation is updated.
    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase == AppPhase::Editor {
            self.record_editor_history_state();
            self.editor.rotate_spawn_direction();
            self.rebuild_spawn_marker_vertices();
        }
    }

    /// Transitions the application back to the menu or editor from a playtest session.
    ///
    /// This stops any active gameplay audio and restores the editor state if the
    /// session originated from the editor.
    pub fn back_to_menu(&mut self) {
        self.editor.timeline.playback.playing = false;
        self.editor.timeline.playback.runtime = None;
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.session.playtesting_editor, &self.editor.objects)
        {
            self.session.playtesting_editor = false;
            self.session.playing_trigger_hitboxes = false;
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
        self.refresh_menu_level_preview_if_needed();
        self.render.meshes.trail.clear();
    }

    pub(super) fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        if self.phase == AppPhase::Editor {
            self.editor.move_cursor(dx, dy);
            self.rebuild_editor_cursor_vertices();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::editor_domain::derive_tap_indicator_positions;
    use crate::game::TimelineSimulationRuntime;
    use crate::types::{AppPhase, EditorMode, LevelObject, SpawnDirection, TimingPoint};

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn remove_block_undo_redo_sequence_restores_and_reapplies() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.editor_remove_block();
            assert_eq!(state.editor.objects.len(), 1);
            assert_eq!(state.editor.objects[0].position, [2.0, 0.0, 0.0]);

            state.editor_undo();
            assert_eq!(state.editor.objects.len(), 2);
            assert_eq!(state.editor.objects[0].position, [0.0, 0.0, 0.0]);
            assert_eq!(state.editor.objects[1].position, [2.0, 0.0, 0.0]);

            state.editor_redo();
            assert_eq!(state.editor.objects.len(), 1);
            assert_eq!(state.editor.objects[0].position, [2.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn remove_block_is_ignored_in_tapping_mode() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.editor_remove_block();

            assert_eq!(state.editor.objects.len(), 2);
            assert_eq!(state.editor.objects[0].position, [0.0, 0.0, 0.0]);
            assert_eq!(state.editor.objects[1].position, [2.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn spawn_set_and_rotate_support_two_step_undo_redo() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects.clear();
            state.editor.spawn.position = [0.0, 0.0, 0.0];
            state.editor.spawn.direction = SpawnDirection::Forward;
            state.editor.timeline.taps.tap_times = vec![1.0 / crate::game::BASE_PLAYER_SPEED];
            state.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.timeline.taps.tap_times,
                &state.editor.objects,
            );
            state.editor.ui.cursor = [4.0, 1.0, 3.0];

            state.editor_set_spawn_here();
            assert_eq!(state.editor.spawn.position, [4.0, 1.0, 3.0]);
            state.editor_rotate_spawn_direction();
            assert_eq!(state.editor.spawn.direction, SpawnDirection::Right);

            state.editor_undo();
            assert_eq!(state.editor.spawn.direction, SpawnDirection::Forward);
            assert_eq!(state.editor.spawn.position, [4.0, 1.0, 3.0]);

            state.editor_undo();
            assert_eq!(state.editor.spawn.direction, SpawnDirection::Forward);
            assert_eq!(state.editor.spawn.position, [0.0, 0.0, 0.0]);

            state.editor_redo();
            assert_eq!(state.editor.spawn.position, [4.0, 1.0, 3.0]);
            state.editor_redo();
            assert_eq!(state.editor.spawn.direction, SpawnDirection::Right);

            let expected_indicators = derive_tap_indicator_positions(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.timeline.taps.tap_times,
                &state.editor.objects,
            );
            assert_eq!(
                state.editor.timeline.taps.tap_indicator_positions,
                expected_indicators
            );
        });
    }

    #[test]
    fn toggle_tap_at_cursor_adds_then_removes_indicator() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.timeline.clock.time_seconds = 2.0;
            state.editor.ui.cursor = [1.0, 0.0, 2.0];
            state.editor.timeline.preview.position = [1.5, 0.0, 2.5];

            let (added_time, added) = state.editor.toggle_tap_at_cursor();
            assert!(added);
            assert!(added_time.is_some());
            assert_eq!(state.editor.timeline.taps.tap_times.len(), 1);
            assert_eq!(state.editor.timeline.taps.tap_indicator_positions.len(), 1);

            let (removed_time, added_again) = state.editor.toggle_tap_at_cursor();
            assert!(!added_again);
            assert!(removed_time.is_some());
            assert!(state.editor.timeline.taps.tap_times.is_empty());
            assert!(state
                .editor
                .timeline
                .taps
                .tap_indicator_positions
                .is_empty());
        });
    }

    #[test]
    fn toggle_tap_after_death_uses_reachable_timeline_time() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.timeline.clock.time_seconds = 3.0;
            state.editor.objects = vec![test_block([0.0, 0.0, 4.0])];
            state.editor.ui.cursor = [0.0, 0.0, 3.0];

            let mut runtime = TimelineSimulationRuntime::new(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.objects,
                &[],
            );
            runtime.advance_to(state.editor.timeline.clock.time_seconds);
            state.editor.timeline.preview.position = runtime.position();

            let (added_time, added) = state.editor.toggle_tap_at_cursor();

            assert!(added);
            let added_time = added_time.expect("tap should be added");
            assert!(
                (added_time - 0.375).abs() < 0.03,
                "expected tap near reachable target time, got {added_time}"
            );
            assert!(
                added_time < 1.0,
                "tap should not inherit the post-death editor clock time"
            );
        });
    }

    #[test]
    fn nudge_and_remove_selected_cover_selected_and_cursor_fallback_paths() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([3.0, 0.0, 3.0])];
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.selected_block_index = Some(0);

            assert!(state.editor.nudge_selected(2.0, -1.0));
            assert_eq!(state.editor.objects[0].position, [2.0, 0.0, -1.0]);
            assert_eq!(state.editor.objects[1].position, [5.0, 0.0, 2.0]);

            state.editor.ui.selected_block_indices.clear();
            state.editor.ui.selected_block_index = None;
            assert!(!state.editor.nudge_selected(1.0, 1.0));

            state.editor.ui.cursor = [2.0, 0.0, -1.0];
            assert!(state.editor.remove_selected());
            assert_eq!(state.editor.objects.len(), 1);

            state.editor.ui.cursor = [999.0, 0.0, 999.0];
            assert!(!state.editor.remove_selected());
        });
    }

    #[test]
    fn move_cursor_uses_snap_configuration() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.cursor = [0.0, 0.0, 0.0];

            state.editor.config.snap_to_grid = true;
            state.editor.config.snap_step = 0.25;
            state.editor.move_cursor(2, -1);
            assert_eq!(state.editor.ui.cursor, [0.5, 0.0, -0.25]);

            state.editor.config.snap_to_grid = false;
            state.editor.move_cursor(1, 1);
            assert_eq!(state.editor.ui.cursor, [1.5, 0.0, 0.75]);
        });
    }

    #[test]
    fn tapping_path_cursor_snaps_only_along_movement_axis() {
        pollster::block_on(async {
            let mut forward_state = State::new_test().await;
            forward_state.phase = AppPhase::Editor;
            forward_state.editor.ui.mode = EditorMode::Tapping;
            forward_state.editor.spawn.position = [1.0, 0.0, 0.0];
            forward_state.editor.spawn.direction = SpawnDirection::Forward;
            forward_state.editor.config.snap_to_grid = true;
            forward_state.editor.config.snap_step = 2.0;
            forward_state.editor.timeline.clock.duration_seconds = 4.0;
            forward_state.set_editor_timeline_time_seconds(0.4);

            let forward_cursor = forward_state
                .editor
                .tap_path_cursor_near_world([4.5, 0.0, 3.4]);
            assert_eq!(forward_cursor[0], 1.0);
            assert_eq!(forward_cursor[2], 2.0);

            let mut right_state = State::new_test().await;
            right_state.phase = AppPhase::Editor;
            right_state.editor.ui.mode = EditorMode::Tapping;
            right_state.editor.spawn.position = [0.0, 0.0, 1.0];
            right_state.editor.spawn.direction = SpawnDirection::Right;
            right_state.editor.config.snap_to_grid = true;
            right_state.editor.config.snap_step = 2.0;
            right_state.editor.timeline.clock.duration_seconds = 4.0;
            right_state.set_editor_timeline_time_seconds(0.4);

            let right_cursor = right_state
                .editor
                .tap_path_cursor_near_world([3.4, 0.0, 4.5]);
            assert_eq!(right_cursor[0], 2.0);
            assert_eq!(right_cursor[2], 1.0);
        });
    }

    #[test]
    fn timing_division_previews_include_distant_timeline_times() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.timeline.clock.duration_seconds = 60.0;
            state.editor.timeline.clock.time_seconds = 0.0;
            state.editor.timing.timing_points = vec![TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];

            let previews = state.editor.timing_division_tap_previews();

            assert!(previews.iter().any(|preview| {
                (preview.time_seconds - 45.0).abs() <= 0.001
                    && (preview.indicator_position[2] - 360.0).abs() < 0.05
            }));
        });
    }

    #[test]
    fn toggle_tap_at_cursor_can_add_distant_cached_path_tap() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.config.snap_to_grid = true;
            state.editor.config.snap_step = 1.0;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.set_editor_timeline_time_seconds(0.1);
            state.editor.ui.cursor = [0.0, 0.0, 32.0];

            let (added_time, added) = state.editor.toggle_tap_at_cursor();

            assert!(added);
            let added_time = added_time.expect("tap should be added");
            assert!(
                (added_time - 4.0).abs() < 0.02,
                "expected distant tap near 4.0s, got {added_time}"
            );
            assert_eq!(state.editor.timeline.taps.tap_times, vec![added_time]);
            assert_eq!(
                state.editor.timeline.taps.tap_indicator_positions,
                vec![[0.0, 0.0, 32.0]]
            );
        });
    }

    #[test]
    fn editor_shift_timeline_time_clamps_and_ignores_non_editor_phase() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.timeline.clock.time_seconds = 1.0;
            state.editor.timeline.clock.duration_seconds = 2.0;

            state.phase = AppPhase::Menu;
            state.editor_shift_timeline_time(0.75);
            assert_eq!(state.editor.timeline.clock.time_seconds, 1.0);

            state.phase = AppPhase::Editor;
            state.editor_shift_timeline_time(5.0);
            assert_eq!(state.editor.timeline.clock.time_seconds, 2.0);
            state.editor_shift_timeline_time(-10.0);
            assert_eq!(state.editor.timeline.clock.time_seconds, 0.0);
        });
    }

    #[test]
    fn toggle_timeline_playback_switches_modes_and_restores_previous_mode() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.enter_editor_phase("PlaybackModeToggle".to_string());
            state.editor.set_mode(EditorMode::Place);
            state.editor.timeline.playback.playing = false;

            state.toggle_editor_timeline_playback();
            assert!(state.editor.timeline.playback.playing);
            assert_eq!(state.editor.ui.mode, EditorMode::Null);

            state.toggle_editor_timeline_playback();
            assert!(!state.editor.timeline.playback.playing);
            assert_eq!(state.editor.ui.mode, EditorMode::Place);
        });
    }

    #[test]
    fn editor_nudge_selected_blocks_honors_guards_and_moves_when_possible() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);

            state.phase = AppPhase::Menu;
            assert!(!state.editor_nudge_selected_blocks(1, 0));

            state.phase = AppPhase::Editor;
            assert!(!state.editor_nudge_selected_blocks(0, 0));

            let before = state.editor.objects[0].position;
            assert!(state.editor_nudge_selected_blocks(1, 0));
            let after = state.editor.objects[0].position;
            assert_ne!(before, after);
        });
    }
}
