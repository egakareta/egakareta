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

}

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_game::TimelineSimulationRuntime;
    use eg_types::{AppPhase, Direction, EditorMode, LevelObject, TimingPoint};

    use super::super::{
        EditorInputRes, EditorTimelineRes, collect_editor_objects_from_world,
        replace_editor_objects, test_support::TestApp, with_editor_interaction_context,
    };

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!("object_{:.0}", position[0]),
            position,
            ..LevelObject::default()
        }
    }

    fn editor_app() -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app
    }

    #[test]
    fn add_then_remove_tap_uses_timeline_messages() {
        let mut app = editor_app();
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.duration_seconds = 8.0;
            editor.timeline.clock.time_seconds = 2.0;
            editor.timeline.preview.position = [1.5, 0.0, 2.5];
        });

        app.world_mut()
            .write_message(crate::events::timeline::AddTap);
        app.tick_state();
        let timeline = &app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times.len(), 1);
        assert_eq!(timeline.taps.tap_indicator_positions.len(), 1);

        app.world_mut()
            .write_message(crate::events::timeline::RemoveTap);
        app.tick_state();
        assert!(
            app.world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_times
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_indicator_positions
                .is_empty()
        );
    }

    #[test]
    fn tap_time_after_death_uses_reachable_timeline_time() {
        let mut app = editor_app();
        replace_editor_objects(app.world_mut(), vec![test_block([0.0, 0.0, 4.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.duration_seconds = 8.0;
            editor.timeline.clock.time_seconds = 3.0;
            editor.input.cursor = [0.0, 0.0, 3.0];

            let mut runtime = TimelineSimulationRuntime::new(
                editor.spawn.position,
                editor.spawn.direction,
                &editor.objects,
                &[],
            );
            runtime.advance_to(editor.timeline.clock.time_seconds);
            editor.timeline.preview.position = runtime.position();
        });

        let added_time = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.tap_time_for_indicator_cell(editor.input.cursor)
        });
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
    fn snap_selection_to_grid_snaps_all_selected_blocks_to_configured_step() {
        let mut app = editor_app();
        replace_editor_objects(
            app.world_mut(),
            vec![
                test_block([0.24, 0.26, -0.74]),
                test_block([1.76, 0.0, 2.24]),
                test_block([9.1, 0.0, 9.1]),
            ],
        );
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_step = 0.5;
            editor.input.selected_block_index = Some(0);
            editor.input.selected_block_indices = vec![0, 1];
        });

        app.world_mut()
            .write_message(crate::events::selection::SnapSelectionToGrid);
        app.tick_state();

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
        let mut forward_app = editor_app();
        with_editor_interaction_context(forward_app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Tapping;
            editor.spawn.position = [1.0, 0.0, 0.0];
            editor.spawn.direction = Direction::Forward;
            editor.config.snap_to_grid = true;
            editor.config.snap_step = 2.0;
            editor.timeline.clock.duration_seconds = 4.0;
            editor.set_timeline_time_seconds(0.4);
        });
        forward_app.tick_state();
        let forward_cursor = with_editor_interaction_context(forward_app.world_mut(), |editor| {
            editor.tap_path_cursor_near_world([4.5, 0.0, 3.4])
        });
        assert_eq!(forward_cursor[0], 1.0);
        assert_eq!(forward_cursor[2], 2.0);

        let mut right_app = editor_app();
        with_editor_interaction_context(right_app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Tapping;
            editor.spawn.position = [0.0, 0.0, 1.0];
            editor.spawn.direction = Direction::Right;
            editor.config.snap_to_grid = true;
            editor.config.snap_step = 2.0;
            editor.timeline.clock.duration_seconds = 4.0;
            editor.set_timeline_time_seconds(0.4);
        });
        right_app.tick_state();
        let right_cursor = with_editor_interaction_context(right_app.world_mut(), |editor| {
            editor.tap_path_cursor_near_world([3.4, 0.0, 4.5])
        });
        assert_eq!(right_cursor[0], 2.0);
        assert_eq!(right_cursor[2], 1.0);
    }

    #[test]
    fn tapping_path_cursor_splits_turn_samples_without_diagonal_drift() {
        let mut app = editor_app();
        with_editor_interaction_context(app.world_mut(), |editor| {
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
        });

        let cursor = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.tap_path_cursor_near_world([1.5, 0.0, 1.55])
        });
        assert_eq!(cursor[0], 1.0);
        assert_eq!(cursor[2], 1.0);
    }

    #[test]
    fn timing_division_previews_include_distant_timeline_times() {
        let mut app = editor_app();
        let includes_distant_time = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Tapping;
            editor.timeline.clock.duration_seconds = 60.0;
            editor.timeline.clock.time_seconds = 0.0;
            editor.timing.timing_points = vec![TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];

            editor.timing_division_tap_previews().iter().any(|preview| {
                (preview.time_seconds - 45.0).abs() <= 0.001
                    && (preview.indicator_position[2] - 360.0).abs() < 0.05
            })
        });

        assert!(includes_distant_time);
    }

    #[test]
    fn tap_time_can_use_a_distant_cached_path_position() {
        let mut app = editor_app();
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Tapping;
            editor.config.snap_to_grid = true;
            editor.config.snap_step = 1.0;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.set_timeline_time_seconds(0.1);
        });
        app.tick_state();

        let added_time = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.tap_time_for_indicator_cell([0.0, 0.0, 32.0])
        });
        assert!(
            (added_time - 4.0).abs() < 0.02,
            "expected distant tap near 4.0s, got {added_time}"
        );
    }
}
