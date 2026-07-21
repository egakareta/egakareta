/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

#[cfg(test)]
use eg_editor_domain::add_tap_with_indicator;
use eg_editor_domain::{
    TapDivisionPreview, TapDivisionPreviewRange, TimelineNearSearch,
    derive_timeline_time_for_world_target_near_time, derive_timing_division_tap_previews,
    timeline_axis_aligned_segment_split_fraction, timeline_turn_corner_position,
};
#[cfg(test)]
use eg_editor_domain::{
    derive_tap_indicator_positions, remove_topmost_block_at_cursor, snap_cell_to_step,
    toggle_spawn_direction, topmost_block_index_at_cursor,
};
#[cfg(test)]
use eg_types::{LevelObject, TriggerTarget};

#[cfg(test)]
use super::EditorSubsystem;

#[cfg(test)]
fn object_ids_for_removed_indices(
    objects: &[LevelObject],
    removed_indices: &[usize],
) -> Vec<String> {
    let mut removed_indices = removed_indices
        .iter()
        .copied()
        .filter(|index| *index < objects.len())
        .collect::<Vec<_>>();
    removed_indices.sort_unstable();
    removed_indices.dedup();

    objects
        .iter()
        .enumerate()
        .filter(|(index, object)| {
            removed_indices.binary_search(index).is_ok()
                && object.trigger.is_none()
                && !object.id.is_empty()
        })
        .map(|(_, object)| object.id.clone())
        .collect()
}

#[cfg(test)]
fn dereference_removed_transform_trigger_targets(
    objects: &mut [LevelObject],
    removed_object_ids: &[String],
) {
    if removed_object_ids.is_empty() {
        return;
    }

    let mut removed_object_ids = removed_object_ids.to_vec();
    removed_object_ids.sort_unstable();
    removed_object_ids.dedup();

    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        if !trigger.transform {
            continue;
        }
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };

        object_ids.retain(|object_id| removed_object_ids.binary_search(object_id).is_err());
        object_ids.sort_unstable();
        object_ids.dedup();
    }
}

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
pub struct TapPathPick {
    pub indicator_position: [f32; 3],
    pub time_seconds: f32,
}

super::editor_context_methods! {
    #[cfg(test)]
    pub fn sync_tap_indicators_to_spawn(&mut self) {
        let selected_index = self.timeline.taps.selected_index;
        self.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.spawn.position,
            self.spawn.direction,
            &self.timeline.taps.tap_times,
            &self.objects,
        );
        self.set_selected_tap_index(selected_index);
    }

    pub fn timing_division_tap_previews(&mut self) -> &[TapDivisionPreview] {
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

    pub fn tap_path_pick_near_world(&self, target_world: [f32; 3]) -> Option<TapPathPick> {
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
            let previous_snapshot = &cache[index];
            let current_snapshot = &cache[index + 1];
            let previous = previous_snapshot.position;
            let current = current_snapshot.position;

            let mut consider_candidate =
                |segment_start: [f32; 3],
                 segment_end: [f32; 3],
                 segment_start_alpha: f32,
                 segment_alpha_width: f32| {
                    let (candidate, alpha) =
                        closest_point_on_segment(segment_start, segment_end, target_world);
                    let segment_alpha =
                        (segment_start_alpha + alpha * segment_alpha_width).clamp(0.0, 1.0);
                    let candidate_distance_sq = distance_sq(candidate, target_world);
                    if candidate_distance_sq < best_distance_sq {
                        best_distance_sq = candidate_distance_sq;
                        best_position = candidate;
                        best_index = index;
                        best_alpha = segment_alpha;
                        best_axis = segment_path_axis(segment_start, segment_end);
                    }
                };

            if let Some(corner) = timeline_turn_corner_position(
                previous,
                previous_snapshot.direction,
                current,
                current_snapshot.direction,
            ) {
                let split_fraction =
                    timeline_axis_aligned_segment_split_fraction(previous, corner, current);
                consider_candidate(previous, corner, 0.0, split_fraction);
                consider_candidate(corner, current, split_fraction, 1.0 - split_fraction);
                continue;
            }

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

    pub fn tap_path_cursor_near_world(&self, target_world: [f32; 3]) -> [f32; 3] {
        self.tap_path_pick_near_world(target_world)
            .map(|pick| pick.indicator_position)
            .unwrap_or_else(|| {
                self.tap_indicator_position_from_world(self.timeline.preview.position)
            })
    }

    pub fn tap_time_for_indicator_cell(&self, indicator_cell: [f32; 3]) -> f32 {
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
        if (preview_cell[0] - indicator_cell[0]).abs() <= 0.001
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
        }
    }

    #[cfg(test)]
    pub fn toggle_tap_at_cursor(&mut self) -> (Option<f32>, bool) {
        let indicator_cell = self.input.cursor;

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

        let derived_time = self.tap_time_for_indicator_cell(indicator_cell);

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

    #[cfg(test)]
    pub fn nudge_selected(&mut self, world_dx: f32, world_dz: f32) -> bool {
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
            .input
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .or_else(|| selected_indices.first().copied())
            && let Some(obj) = self.objects.get(index)
        {
            self.input.cursor = [obj.position[0], obj.position[1].max(0.0), obj.position[2]];
        }

        self.invalidate_samples();
        true
    }

    #[cfg(test)]
    pub fn snap_selected_blocks_to_grid(&mut self) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let step = self.config.snap_step.max(0.05);
        let mut changed = false;
        for index in &selected_indices {
            if let Some(obj) = self.objects.get_mut(*index) {
                let snapped = snap_cell_to_step(obj.position, step);
                if obj.position != snapped {
                    obj.position = snapped;
                    changed = true;
                }
            }
        }

        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .or_else(|| selected_indices.first().copied())
            && let Some(obj) = self.objects.get(index)
        {
            self.input.cursor = [obj.position[0], obj.position[1].max(0.0), obj.position[2]];
        }

        if changed {
            self.invalidate_samples();
        }
        changed
    }

    #[cfg(test)]
    pub fn remove_selected(&mut self) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if !selected_indices.is_empty() {
            let removed_object_ids =
                object_ids_for_removed_indices(&self.objects, &selected_indices);
            dereference_removed_transform_trigger_targets(&mut self.objects, &removed_object_ids);
            for index in selected_indices.into_iter().rev() {
                if index < self.objects.len() {
                    self.objects.remove(index);
                }
            }
            self.clear_block_selection();
            self.invalidate_samples();
            self.pick_spatial_index.invalidate();
            self.sync_trigger_selection_from_objects();
            return true;
        }

        if let Some(index) = topmost_block_index_at_cursor(&self.objects, self.input.cursor) {
            let removed_object_ids = object_ids_for_removed_indices(&self.objects, &[index]);
            dereference_removed_transform_trigger_targets(&mut self.objects, &removed_object_ids);
            remove_topmost_block_at_cursor(&mut self.objects, self.input.cursor);
            self.invalidate_samples();
            self.pick_spatial_index.invalidate();
            self.sync_trigger_selection_from_objects();
            return true;
        }

        false
    }

    #[cfg(test)]
    pub fn set_spawn_here(&mut self) {
        self.spawn.position = self.input.cursor;
        self.sync_tap_indicators_to_spawn();
        self.invalidate_samples();
    }

    #[cfg(test)]
    pub fn rotate_spawn_direction(&mut self) {
        self.spawn.direction = toggle_spawn_direction(self.spawn.direction);
        self.sync_tap_indicators_to_spawn();
        self.invalidate_samples();
    }

    #[cfg(test)]
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        let step = if self.effective_snap_to_grid() {
            self.config.snap_step.max(0.05)
        } else {
            1.0
        };
        self.input.cursor[0] += dx as f32 * step;
        self.input.cursor[2] += dy as f32 * step;
    }
}

#[cfg(test)]
mod tests {
    use eg_editor_domain::derive_tap_indicator_positions;
    use eg_game::TimelineSimulationRuntime;
    use eg_types::{
        Direction, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TimingPoint, TriggerTarget,
    };

    use super::EditorSubsystem;

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!("object_{:.0}", position[0]),
            position,
            ..LevelObject::default()
        }
    }

    fn test_transform_trigger(object_ids: Vec<&str>) -> LevelObject {
        let object_ids = object_ids.into_iter().map(str::to_string).collect();
        LevelObject {
            position: [9.0, 0.0, 0.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects { object_ids },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    #[test]
    fn remove_block_undo_redo_sequence_restores_and_reapplies() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])];
        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0];

        editor.record_history_state();
        editor.remove_selected();
        assert_eq!(editor.objects.len(), 1);
        assert_eq!(editor.objects[0].position, [2.0, 0.0, 0.0]);

        editor.undo();
        assert_eq!(editor.objects.len(), 2);
        assert_eq!(editor.objects[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(editor.objects[1].position, [2.0, 0.0, 0.0]);

        editor.redo();
        assert_eq!(editor.objects.len(), 1);
        assert_eq!(editor.objects[0].position, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn remove_block_dereferences_deleted_transform_trigger_target() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
        ];
        editor
            .objects
            .push(test_transform_trigger(vec!["object_2"]));
        editor.input.selected_block_index = Some(1);
        editor.input.selected_block_indices = vec![1];

        editor.remove_selected();

        assert_eq!(editor.objects.len(), 3);
        assert_eq!(editor.objects[2].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        let triggers = editor.trigger_objects();
        let trigger = triggers
            .first()
            .and_then(|object| object.trigger.as_ref())
            .expect("trigger remains");
        assert_eq!(
            trigger.target,
            TriggerTarget::Objects {
                object_ids: Vec::new(),
            }
        );
    }

    #[test]
    fn remove_block_remaps_transform_trigger_targets_after_deleted_block() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
        ];
        editor
            .objects
            .push(test_transform_trigger(vec!["object_4"]));
        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0];

        editor.remove_selected();

        assert_eq!(editor.objects[1].position, [4.0, 0.0, 0.0]);
        let triggers = editor.trigger_objects();
        let trigger = triggers
            .first()
            .and_then(|object| object.trigger.as_ref())
            .expect("trigger remains");
        assert_eq!(
            trigger.target,
            TriggerTarget::Objects {
                object_ids: vec!["object_4".to_string()],
            }
        );
    }

    #[test]
    fn spawn_set_and_rotate_support_two_step_undo_redo() {
        let mut editor = EditorSubsystem::default();
        editor.objects.clear();
        editor.spawn.position = [0.0, 0.0, 0.0];
        editor.spawn.direction = Direction::Forward;
        editor.timeline.taps.tap_times = vec![1.0 / eg_game::BASE_PLAYER_SPEED];
        editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            editor.spawn.position,
            editor.spawn.direction,
            &editor.timeline.taps.tap_times,
            &editor.objects,
        );
        editor.input.cursor = [4.0, 1.0, 3.0];

        editor.record_history_state();
        editor.set_spawn_here();
        assert_eq!(editor.spawn.position, [4.0, 1.0, 3.0]);
        editor.record_history_state();
        editor.rotate_spawn_direction();
        assert_eq!(editor.spawn.direction, Direction::Right);

        editor.undo();
        assert_eq!(editor.spawn.direction, Direction::Forward);
        assert_eq!(editor.spawn.position, [4.0, 1.0, 3.0]);

        editor.undo();
        assert_eq!(editor.spawn.direction, Direction::Forward);
        assert_eq!(editor.spawn.position, [0.0, 0.0, 0.0]);

        editor.redo();
        assert_eq!(editor.spawn.position, [4.0, 1.0, 3.0]);
        editor.redo();
        assert_eq!(editor.spawn.direction, Direction::Right);

        let expected_indicators = derive_tap_indicator_positions(
            editor.spawn.position,
            editor.spawn.direction,
            &editor.timeline.taps.tap_times,
            &editor.objects,
        );
        assert_eq!(
            editor.timeline.taps.tap_indicator_positions,
            expected_indicators
        );
    }

    #[test]
    fn toggle_tap_at_cursor_adds_then_removes_indicator() {
        let mut editor = EditorSubsystem::default();
        editor.timeline.clock.duration_seconds = 8.0;
        editor.timeline.clock.time_seconds = 2.0;
        editor.input.cursor = [1.0, 0.0, 2.0];
        editor.timeline.preview.position = [1.5, 0.0, 2.5];

        let (added_time, added) = editor.toggle_tap_at_cursor();
        assert!(added);
        assert!(added_time.is_some());
        assert_eq!(editor.timeline.taps.tap_times.len(), 1);
        assert_eq!(editor.timeline.taps.tap_indicator_positions.len(), 1);

        let (removed_time, added_again) = editor.toggle_tap_at_cursor();
        assert!(!added_again);
        assert!(removed_time.is_some());
        assert!(editor.timeline.taps.tap_times.is_empty());
        assert!(editor.timeline.taps.tap_indicator_positions.is_empty());
    }

    #[test]
    fn toggle_tap_after_death_uses_reachable_timeline_time() {
        let mut editor = EditorSubsystem::default();
        editor.timeline.clock.duration_seconds = 8.0;
        editor.timeline.clock.time_seconds = 3.0;
        editor.objects = vec![test_block([0.0, 0.0, 4.0])];
        editor.input.cursor = [0.0, 0.0, 3.0];

        let mut runtime = TimelineSimulationRuntime::new(
            editor.spawn.position,
            editor.spawn.direction,
            &editor.objects,
            &[],
        );
        runtime.advance_to(editor.timeline.clock.time_seconds);
        editor.timeline.preview.position = runtime.position();

        let (added_time, added) = editor.toggle_tap_at_cursor();

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
    }

    #[test]
    fn nudge_and_remove_selected_cover_selected_and_cursor_fallback_paths() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([3.0, 0.0, 3.0])];
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.selected_block_index = Some(0);

        assert!(editor.nudge_selected(2.0, -1.0));
        assert_eq!(editor.objects[0].position, [2.0, 0.0, -1.0]);
        assert_eq!(editor.objects[1].position, [5.0, 0.0, 2.0]);

        editor.input.selected_block_indices.clear();
        editor.input.selected_block_index = None;
        assert!(!editor.nudge_selected(1.0, 1.0));

        editor.input.cursor = [2.0, 0.0, -1.0];
        assert!(editor.remove_selected());
        assert_eq!(editor.objects.len(), 1);

        editor.input.cursor = [999.0, 0.0, 999.0];
        assert!(!editor.remove_selected());
    }

    #[test]
    fn snap_selection_to_grid_snaps_all_selected_blocks_to_configured_step() {
        let mut editor = EditorSubsystem::default();
        editor.config.snap_step = 0.5;
        editor.objects = vec![
            test_block([0.24, 0.26, -0.74]),
            test_block([1.76, 0.0, 2.24]),
            test_block([9.1, 0.0, 9.1]),
        ];
        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0, 1];

        assert!(editor.snap_selected_blocks_to_grid());

        assert_eq!(editor.objects[0].position, [0.0, 0.5, -0.5]);
        assert_eq!(editor.objects[1].position, [2.0, 0.0, 2.0]);
        assert_eq!(editor.objects[2].position, [9.1, 0.0, 9.1]);
        assert_eq!(editor.input.cursor, [0.0, 0.5, -0.5]);
    }

    #[test]
    fn move_cursor_uses_snap_configuration() {
        let mut editor = EditorSubsystem::default();
        editor.input.cursor = [0.0, 0.0, 0.0];

        editor.config.snap_to_grid = true;
        editor.config.snap_step = 0.25;
        editor.move_cursor(2, -1);
        assert_eq!(editor.input.cursor, [0.5, 0.0, -0.25]);

        editor.config.snap_to_grid = false;
        editor.move_cursor(1, 1);
        assert_eq!(editor.input.cursor, [1.5, 0.0, 0.75]);
    }

    #[test]
    fn tapping_path_cursor_snaps_only_along_movement_axis() {
        let mut forward_editor = EditorSubsystem::default();
        forward_editor.input.mode = EditorMode::Tapping;
        forward_editor.spawn.position = [1.0, 0.0, 0.0];
        forward_editor.spawn.direction = Direction::Forward;
        forward_editor.config.snap_to_grid = true;
        forward_editor.config.snap_step = 2.0;
        forward_editor.timeline.clock.duration_seconds = 4.0;
        forward_editor.set_timeline_time_seconds(0.4);
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut forward_editor.timeline,
            &forward_editor.objects,
            &forward_editor.spawn,
        );

        let forward_cursor = forward_editor.tap_path_cursor_near_world([4.5, 0.0, 3.4]);
        assert_eq!(forward_cursor[0], 1.0);
        assert_eq!(forward_cursor[2], 2.0);

        let mut right_editor = EditorSubsystem::default();
        right_editor.input.mode = EditorMode::Tapping;
        right_editor.spawn.position = [0.0, 0.0, 1.0];
        right_editor.spawn.direction = Direction::Right;
        right_editor.config.snap_to_grid = true;
        right_editor.config.snap_step = 2.0;
        right_editor.timeline.clock.duration_seconds = 4.0;
        right_editor.set_timeline_time_seconds(0.4);
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut right_editor.timeline,
            &right_editor.objects,
            &right_editor.spawn,
        );

        let right_cursor = right_editor.tap_path_cursor_near_world([3.4, 0.0, 4.5]);
        assert_eq!(right_cursor[0], 2.0);
        assert_eq!(right_cursor[2], 1.0);
    }

    #[test]
    fn tapping_path_cursor_splits_turn_samples_without_diagonal_drift() {
        let mut editor = EditorSubsystem::default();
        editor.input.mode = EditorMode::Tapping;
        editor.config.snap_to_grid = true;
        editor.config.snap_step = 1.0;
        editor.timeline.clock.duration_seconds = 2.0;
        editor.timeline.snapshot_cache_step_seconds = 1.0;
        editor.timeline.snapshot_cache_revision = editor.timeline.simulation_revision;
        editor.timeline.snapshot_cache = vec![
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [1.44, 0.0, 1.5],
                direction: Direction::Right,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [1.5, 0.0, 1.56],
                direction: Direction::Forward,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
        ];

        let cursor = editor.tap_path_cursor_near_world([1.5, 0.0, 1.55]);

        assert_eq!(cursor[0], 1.0);
        assert_eq!(cursor[2], 1.0);
    }

    #[test]
    fn timing_division_previews_include_distant_timeline_times() {
        let mut editor = EditorSubsystem::default();
        editor.input.mode = EditorMode::Tapping;
        editor.timeline.clock.duration_seconds = 60.0;
        editor.timeline.clock.time_seconds = 0.0;
        editor.timing.timing_points = vec![TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];

        let previews = editor.timing_division_tap_previews();

        assert!(previews.iter().any(|preview| {
            (preview.time_seconds - 45.0).abs() <= 0.001
                && (preview.indicator_position[2] - 360.0).abs() < 0.05
        }));
    }

    #[test]
    fn toggle_tap_at_cursor_can_add_distant_cached_path_tap() {
        let mut editor = EditorSubsystem::default();
        editor.input.mode = EditorMode::Tapping;
        editor.config.snap_to_grid = true;
        editor.config.snap_step = 1.0;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.set_timeline_time_seconds(0.1);
        editor.input.cursor = [0.0, 0.0, 32.0];
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut editor.timeline,
            &editor.objects,
            &editor.spawn,
        );

        let (added_time, added) = editor.toggle_tap_at_cursor();

        assert!(added);
        let added_time = added_time.expect("tap should be added");
        assert!(
            (added_time - 4.0).abs() < 0.02,
            "expected distant tap near 4.0s, got {added_time}"
        );
        assert_eq!(editor.timeline.taps.tap_times, vec![added_time]);
        assert_eq!(
            editor.timeline.taps.tap_indicator_positions,
            vec![[0.0, 0.0, 32.0]]
        );
    }
}
