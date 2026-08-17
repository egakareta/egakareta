/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_editor_domain::{
    TapDivisionPreview, TapDivisionPreviewRange, TimelineNearSearch,
    derive_timeline_time_for_world_target_near_time, derive_timing_division_tap_previews,
    timeline_axis_aligned_segment_split_fraction, timeline_turn_corner_position,
};

use super::editor_timeline::EditorTimelineSnapshot;

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

struct TapPathSearch<'a> {
    cache: &'a [EditorTimelineSnapshot],
    snapshot_cache_revision: u64,
    simulation_revision: u64,
    snapshot_cache_step_seconds: f32,
    duration_seconds: f32,
    snap_to_grid: bool,
    snap_step: f32,
}

fn tap_path_pick_near_world(
    search: TapPathSearch<'_>,
    target_world: [f32; 3],
) -> Option<TapPathPick> {
    if search.cache.is_empty() || search.snapshot_cache_revision != search.simulation_revision {
        return None;
    }

    let step_seconds = search.snapshot_cache_step_seconds.max(1.0 / 480.0);
    if search.cache.len() == 1 {
        return Some(TapPathPick {
            indicator_position: [
                search.cache[0].position[0] - 0.5,
                search.cache[0].position[1],
                search.cache[0].position[2] - 0.5,
            ],
            time_seconds: 0.0,
        });
    }

    let mut best_position = search.cache[0].position;
    let mut best_index = 0;
    let mut best_alpha = 0.0;
    let mut best_axis = None;
    let mut best_distance_sq = f32::INFINITY;

    for index in 0..search.cache.len().saturating_sub(1) {
        let previous_snapshot = &search.cache[index];
        let current_snapshot = &search.cache[index + 1];
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

    let mut indicator_position = [
        best_position[0] - 0.5,
        best_position[1],
        best_position[2] - 0.5,
    ];
    let mut time_seconds = (best_index as f32 + best_alpha) * step_seconds;
    if search.snap_to_grid {
        let snap_step = search.snap_step.max(0.05);
        match best_axis {
            Some(PathAxis::X) => {
                indicator_position[0] = (indicator_position[0] / snap_step).floor() * snap_step;
                let snapped_world_x = indicator_position[0] + 0.5;
                let previous = search.cache[best_index].position;
                let current = search.cache[best_index + 1].position;
                let segment_x = current[0] - previous[0];
                if segment_x.abs() > f32::EPSILON {
                    best_alpha = ((snapped_world_x - previous[0]) / segment_x).clamp(0.0, 1.0);
                    time_seconds = (best_index as f32 + best_alpha) * step_seconds;
                }
            }
            Some(PathAxis::Z) => {
                indicator_position[2] = (indicator_position[2] / snap_step).floor() * snap_step;
                let snapped_world_z = indicator_position[2] + 0.5;
                let previous = search.cache[best_index].position;
                let current = search.cache[best_index + 1].position;
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
        time_seconds: time_seconds.clamp(0.0, search.duration_seconds.max(0.0)),
    })
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
        tap_path_pick_near_world(
            TapPathSearch {
                cache: &self.timeline.snapshot_cache,
                snapshot_cache_revision: self.timeline.snapshot_cache_revision,
                simulation_revision: self.timeline.simulation_revision,
                snapshot_cache_step_seconds: self.timeline.snapshot_cache_step_seconds,
                duration_seconds: self.timeline.clock.duration_seconds,
                snap_to_grid: self.effective_snap_to_grid(),
                snap_step: self.config.snap_step,
            },
            target_world,
        )
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
            self.timeline.invalidate_samples();
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
            self.timeline.invalidate_samples();
            self.pick_spatial_index.invalidate();
            self.sync_trigger_selection_from_objects();
            return true;
        }

        if let Some(index) = topmost_block_index_at_cursor(&self.objects, self.input.cursor) {
            let removed_object_ids = object_ids_for_removed_indices(&self.objects, &[index]);
            dereference_removed_transform_trigger_targets(&mut self.objects, &removed_object_ids);
            remove_topmost_block_at_cursor(&mut self.objects, self.input.cursor);
            self.timeline.invalidate_samples();
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
        self.timeline.invalidate_samples();
    }

    #[cfg(test)]
    pub fn rotate_spawn_direction(&mut self) {
        self.spawn.direction = toggle_spawn_direction(self.spawn.direction);
        self.sync_tap_indicators_to_spawn();
        self.timeline.invalidate_samples();
    }

}

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_editor_domain::{
        TapDivisionPreviewRange, TimelineNearSearch, add_tap_with_indicator,
        derive_timeline_elapsed_seconds, derive_timeline_time_for_world_target_near_time,
        derive_timing_division_tap_previews, nudge_selected_objects, remove_tap_with_indicator,
    };
    use eg_types::{
        Direction, EasingDirection, LevelObject, SpawnMetadata, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TimingPoint, TriggerTarget,
    };

    use super::{EditorTimelineSnapshot, TapPathSearch, tap_path_pick_near_world};
    use crate::{
        events::{
            block::RemoveBlock,
            history::{Redo, Undo},
            selection::SnapSelectionToGrid,
            spawn::{RotateSpawnDirection, SetSpawnHere},
        },
        state::{
            EditorConfigRes, EditorInputRes, EditorSpawnRes, EditorTimelineState,
            collect_editor_objects_from_world, replace_editor_objects, test_support::TestApp,
        },
    };

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

    fn remove_block_test_app(objects: Vec<LevelObject>, selected_indices: Vec<usize>) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(eg_types::AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        let mut input = app.world_mut().resource_mut::<EditorInputRes>();
        input.0.selected_block_index = selected_indices.first().copied();
        input.0.selected_block_indices = selected_indices;
        app
    }

    fn pick_timeline_path(
        timeline: &EditorTimelineState,
        snap_to_grid: bool,
        snap_step: f32,
        target_world: [f32; 3],
    ) -> super::TapPathPick {
        tap_path_pick_near_world(
            TapPathSearch {
                cache: &timeline.snapshot_cache,
                snapshot_cache_revision: timeline.snapshot_cache_revision,
                simulation_revision: timeline.simulation_revision,
                snapshot_cache_step_seconds: timeline.snapshot_cache_step_seconds,
                duration_seconds: timeline.clock.duration_seconds,
                snap_to_grid,
                snap_step,
            },
            target_world,
        )
        .expect("timeline samples should yield a path pick")
    }

    #[test]
    fn remove_block_undo_redo_sequence_restores_and_reapplies() {
        let mut app = remove_block_test_app(
            vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])],
            vec![0],
        );

        app.world_mut().write_message(RemoveBlock);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].position, [2.0, 0.0, 0.0]);

        app.world_mut().write_message(Undo);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(objects[1].position, [2.0, 0.0, 0.0]);

        app.world_mut().write_message(Redo);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].position, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn remove_block_dereferences_deleted_transform_trigger_target() {
        let mut objects = vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
        ];
        objects.push(test_transform_trigger(vec!["object_2"]));
        let mut app = remove_block_test_app(objects, vec![1]);

        app.world_mut().write_message(RemoveBlock);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 3);
        assert_eq!(objects[2].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        let triggers = objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .collect::<Vec<_>>();
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
        let mut objects = vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
        ];
        objects.push(test_transform_trigger(vec!["object_4"]));
        let mut app = remove_block_test_app(objects, vec![0]);

        app.world_mut().write_message(RemoveBlock);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects[1].position, [4.0, 0.0, 0.0]);
        let triggers = objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .collect::<Vec<_>>();
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
        let mut app = remove_block_test_app(Vec::new(), Vec::new());
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [4.0, 1.0, 3.0];

        app.world_mut().write_message(SetSpawnHere);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.position,
            [4.0, 1.0, 3.0]
        );

        app.world_mut().write_message(RotateSpawnDirection);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );

        app.world_mut().write_message(Undo);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Forward
        );
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.position,
            [4.0, 1.0, 3.0]
        );

        app.world_mut().write_message(Undo);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.position,
            [0.0, 0.0, 0.0]
        );

        app.world_mut().write_message(Redo);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.position,
            [4.0, 1.0, 3.0]
        );
        app.world_mut().write_message(Redo);
        app.tick();
        assert_eq!(
            app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );
    }

    #[test]
    fn toggle_tap_at_cursor_adds_then_removes_indicator() {
        let mut tap_times = Vec::new();
        let mut indicator_positions = Vec::new();
        let indicator_cell = [1.0, 0.0, 2.0];

        let selected_index = add_tap_with_indicator(
            &mut tap_times,
            &mut indicator_positions,
            2.0,
            indicator_cell,
        );
        assert_eq!(selected_index, Some(0));
        assert_eq!(tap_times, vec![2.0]);
        assert_eq!(indicator_positions, vec![indicator_cell]);

        assert_eq!(
            remove_tap_with_indicator(&mut tap_times, &mut indicator_positions, 2.0),
            Some(0)
        );
        assert!(tap_times.is_empty());
        assert!(indicator_positions.is_empty());
    }

    #[test]
    fn toggle_tap_after_death_uses_reachable_timeline_time() {
        let spawn = SpawnMetadata::default();
        let objects = vec![test_block([0.0, 0.0, 4.0])];
        let seed_time =
            derive_timeline_elapsed_seconds(spawn.position, spawn.direction, &[], 3.0, &objects);
        let added_time = derive_timeline_time_for_world_target_near_time(
            spawn.position,
            spawn.direction,
            &[],
            8.0,
            &objects,
            [0.5, 0.0, 3.5],
            TimelineNearSearch {
                seed_time,
                window_seconds: 1.5,
            },
        );

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
        let mut objects = vec![test_block([0.0, 0.0, 0.0]), test_block([3.0, 0.0, 3.0])];
        let selected_indices = vec![0, 1];

        assert!(
            nudge_selected_objects(&mut objects, &selected_indices, Some(0), 2.0, -1.0,).is_some()
        );
        assert_eq!(objects[0].position, [2.0, 0.0, -1.0]);
        assert_eq!(objects[1].position, [5.0, 0.0, 2.0]);

        assert!(nudge_selected_objects(&mut objects, &[], None, 1.0, 1.0).is_none());

        let mut app = remove_block_test_app(objects, Vec::new());
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [2.0, 0.0, -1.0];
        app.world_mut().write_message(RemoveBlock);
        app.tick();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);

        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [999.0, 0.0, 999.0];
        app.world_mut().write_message(RemoveBlock);
        app.tick();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
    }

    #[test]
    fn snap_selection_to_grid_snaps_all_selected_blocks_to_configured_step() {
        let mut app = remove_block_test_app(
            vec![
                test_block([0.24, 0.26, -0.74]),
                test_block([1.76, 0.0, 2.24]),
                test_block([9.1, 0.0, 9.1]),
            ],
            vec![0, 1],
        );
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_step = 0.5;
        app.world_mut().write_message(SnapSelectionToGrid);
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects[0].position, [0.0, 0.5, -0.5]);
        assert_eq!(objects[1].position, [2.0, 0.0, 2.0]);
        assert_eq!(objects[2].position, [9.1, 0.0, 9.1]);
        assert_eq!(
            app.world().resource::<EditorInputRes>().0.cursor,
            [0.0, 0.5, -0.5]
        );
    }

    #[test]
    fn tapping_path_cursor_snaps_only_along_movement_axis() {
        let forward_spawn = SpawnMetadata {
            position: [1.0, 0.0, 0.0],
            direction: Direction::Forward,
        };
        let mut forward_timeline = EditorTimelineState::default();
        forward_timeline.clock.duration_seconds = 4.0;
        forward_timeline.clock.time_seconds = 0.4;
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut forward_timeline,
            &[],
            &forward_spawn,
        );
        let forward_cursor =
            pick_timeline_path(&forward_timeline, true, 2.0, [4.5, 0.0, 3.4]).indicator_position;
        assert_eq!(forward_cursor[0], 1.0);
        assert_eq!(forward_cursor[2], 2.0);

        let right_spawn = SpawnMetadata {
            position: [0.0, 0.0, 1.0],
            direction: Direction::Right,
        };
        let mut right_timeline = EditorTimelineState::default();
        right_timeline.clock.duration_seconds = 4.0;
        right_timeline.clock.time_seconds = 0.4;
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut right_timeline,
            &[],
            &right_spawn,
        );
        let right_cursor =
            pick_timeline_path(&right_timeline, true, 2.0, [3.4, 0.0, 4.5]).indicator_position;
        assert_eq!(right_cursor[0], 2.0);
        assert_eq!(right_cursor[2], 1.0);
    }

    #[test]
    fn tapping_path_cursor_splits_turn_samples_without_diagonal_drift() {
        let mut timeline = EditorTimelineState::default();
        timeline.clock.duration_seconds = 2.0;
        timeline.snapshot_cache_step_seconds = 1.0;
        timeline.snapshot_cache_revision = timeline.simulation_revision;
        timeline.snapshot_cache = vec![
            EditorTimelineSnapshot {
                position: [1.44, 0.0, 1.5],
                direction: Direction::Right,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
            EditorTimelineSnapshot {
                position: [1.5, 0.0, 1.56],
                direction: Direction::Forward,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
        ];

        let cursor = pick_timeline_path(&timeline, true, 1.0, [1.5, 0.0, 1.55]).indicator_position;

        assert_eq!(cursor[0], 1.0);
        assert_eq!(cursor[2], 1.0);
    }

    #[test]
    fn timing_division_previews_include_distant_timeline_times() {
        let timing_points = vec![TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];

        let previews = derive_timing_division_tap_previews(
            [0.0, 0.0, 0.0],
            Direction::Forward,
            &[],
            &timing_points,
            60.0,
            TapDivisionPreviewRange {
                start_seconds: 0.0,
                end_seconds: 60.0,
            },
            &[],
        );

        assert!(previews.iter().any(|preview| {
            (preview.time_seconds - 45.0).abs() <= 0.001
                && (preview.indicator_position[2] - 360.0).abs() < 0.05
        }));
    }

    #[test]
    fn toggle_tap_at_cursor_can_add_distant_cached_path_tap() {
        let spawn = SpawnMetadata::default();
        let mut timeline = EditorTimelineState::default();
        timeline.clock.duration_seconds = 8.0;
        timeline.clock.time_seconds = 0.1;
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_if_needed(
            &mut timeline,
            &[],
            &spawn,
        );
        let pick = pick_timeline_path(&timeline, true, 1.0, [0.5, 0.0, 32.5]);
        let mut tap_times = Vec::new();
        let mut indicator_positions = Vec::new();
        add_tap_with_indicator(
            &mut tap_times,
            &mut indicator_positions,
            pick.time_seconds,
            pick.indicator_position,
        );

        let added_time = pick.time_seconds;
        assert!(
            (added_time - 4.0).abs() < 0.02,
            "expected distant tap near 4.0s, got {added_time}"
        );
        assert_eq!(tap_times, vec![added_time]);
        assert_eq!(indicator_positions, vec![[0.0, 0.0, 32.0]]);
    }
}
