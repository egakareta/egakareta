/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::game::{
    simulate_timeline_state, simulate_timeline_state_with_triggers, TimelineSimulationRuntime,
};
use crate::types::{LevelObject, SpawnDirection, TimedTrigger, TimingPoint};

use super::TAP_EPSILON_SECONDS;

pub(crate) const MAX_TIMING_DIVISION_TAP_PREVIEWS: usize = 2048;

fn lerp_position(start: [f32; 3], end: [f32; 3], alpha: f32) -> [f32; 3] {
    [
        start[0] + (end[0] - start[0]) * alpha,
        start[1] + (end[1] - start[1]) * alpha,
        start[2] + (end[2] - start[2]) * alpha,
    ]
}

fn horizontal_distance(start: [f32; 3], end: [f32; 3]) -> f32 {
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    (dx * dx + dz * dz).sqrt()
}

pub(crate) fn timeline_turn_corner_position(
    previous_position: [f32; 3],
    previous_direction: SpawnDirection,
    current_position: [f32; 3],
    current_direction: SpawnDirection,
) -> Option<[f32; 3]> {
    if previous_direction == current_direction {
        return None;
    }

    match (previous_direction, current_direction) {
        (SpawnDirection::Forward, SpawnDirection::Right) => Some([
            previous_position[0],
            current_position[1],
            current_position[2],
        ]),
        (SpawnDirection::Right, SpawnDirection::Forward) => Some([
            current_position[0],
            current_position[1],
            previous_position[2],
        ]),
        _ => None,
    }
}

pub(crate) fn timeline_axis_aligned_segment_split_fraction(
    previous_position: [f32; 3],
    corner_position: [f32; 3],
    current_position: [f32; 3],
) -> f32 {
    let previous_length = horizontal_distance(previous_position, corner_position);
    let current_length = horizontal_distance(corner_position, current_position);
    let total_length = previous_length + current_length;
    if total_length <= f32::EPSILON {
        0.5
    } else {
        (previous_length / total_length).clamp(0.0, 1.0)
    }
}

pub(crate) fn interpolate_timeline_sample_positions(
    previous_position: [f32; 3],
    previous_direction: SpawnDirection,
    current_position: [f32; 3],
    current_direction: SpawnDirection,
    alpha: f32,
) -> [f32; 3] {
    let alpha = alpha.clamp(0.0, 1.0);
    let Some(corner_position) = timeline_turn_corner_position(
        previous_position,
        previous_direction,
        current_position,
        current_direction,
    ) else {
        return lerp_position(previous_position, current_position, alpha);
    };

    let split_fraction = timeline_axis_aligned_segment_split_fraction(
        previous_position,
        corner_position,
        current_position,
    );
    if alpha <= split_fraction {
        let local_alpha = if split_fraction <= f32::EPSILON {
            0.0
        } else {
            alpha / split_fraction
        };
        lerp_position(previous_position, corner_position, local_alpha)
    } else {
        let remaining_fraction = 1.0 - split_fraction;
        let local_alpha = if remaining_fraction <= f32::EPSILON {
            1.0
        } else {
            (alpha - split_fraction) / remaining_fraction
        };
        lerp_position(corner_position, current_position, local_alpha)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TapDivisionPreview {
    pub(crate) time_seconds: f32,
    pub(crate) indicator_position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TapDivisionPreviewRange {
    pub(crate) start_seconds: f32,
    pub(crate) end_seconds: f32,
}

#[cfg(test)]
pub(crate) fn derive_timeline_position(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> ([f32; 3], SpawnDirection) {
    let state = derive_timeline_state_with_triggers(
        spawn,
        direction,
        tap_times,
        timeline_time_seconds,
        objects,
        &[],
        false,
    );
    (state.position, state.direction)
}

pub(crate) fn derive_timeline_elapsed_seconds_with_triggers(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
    triggers: &[TimedTrigger],
    simulate_trigger_hitboxes: bool,
) -> f32 {
    derive_timeline_state_with_triggers(
        spawn,
        direction,
        tap_times,
        timeline_time_seconds,
        objects,
        triggers,
        simulate_trigger_hitboxes,
    )
    .elapsed_seconds
}

pub(crate) fn derive_tap_indicator_positions(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    objects: &[LevelObject],
) -> Vec<[f32; 3]> {
    let mut sorted_taps: Vec<f32> = tap_times
        .iter()
        .copied()
        .filter(|tap| tap.is_finite() && *tap >= 0.0)
        .collect();
    sorted_taps.sort_by(f32::total_cmp);

    let mut runtime = TimelineSimulationRuntime::new(spawn, direction, objects, &sorted_taps);
    let mut positions = Vec::with_capacity(sorted_taps.len());
    for tap_time in sorted_taps {
        runtime.advance_to(tap_time);
        let snapshot = runtime.snapshot();
        positions.push([
            snapshot.position[0] - 0.5,
            snapshot.position[1],
            snapshot.position[2] - 0.5,
        ]);
    }

    positions.sort_unstable_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then(a[1].total_cmp(&b[1]))
            .then(a[2].total_cmp(&b[2]))
    });
    positions.dedup_by(|a, b| {
        (a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001 && (a[2] - b[2]).abs() < 0.001
    });
    positions
}

pub(crate) fn derive_timing_division_tap_previews(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timing_points: &[TimingPoint],
    duration_seconds: f32,
    preview_range: TapDivisionPreviewRange,
    objects: &[LevelObject],
) -> Vec<TapDivisionPreview> {
    let duration = if duration_seconds.is_finite() {
        duration_seconds.max(0.0)
    } else {
        0.0
    };
    if duration <= 0.0 || timing_points.is_empty() {
        return Vec::new();
    }

    let preview_start = if preview_range.start_seconds.is_finite() {
        preview_range.start_seconds.clamp(0.0, duration)
    } else {
        0.0
    };
    let preview_end = if preview_range.end_seconds.is_finite() {
        preview_range.end_seconds.clamp(preview_start, duration)
    } else {
        duration
    };
    if preview_end <= preview_start {
        return Vec::new();
    }

    let mut sorted_timing_points: Vec<TimingPoint> = timing_points
        .iter()
        .filter(|point| point.time_seconds.is_finite() && point.bpm.is_finite() && point.bpm > 0.0)
        .cloned()
        .collect();
    sorted_timing_points.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    if sorted_timing_points.is_empty() {
        return Vec::new();
    }

    let mut division_times = Vec::new();
    for (point_index, point) in sorted_timing_points.iter().enumerate() {
        if division_times.len() >= MAX_TIMING_DIVISION_TAP_PREVIEWS {
            break;
        }

        let start_time = point.time_seconds.clamp(0.0, duration);
        let end_time = sorted_timing_points
            .get(point_index + 1)
            .map(|next| next.time_seconds.clamp(0.0, duration))
            .unwrap_or(duration);
        if end_time < start_time {
            continue;
        }

        let segment_start = start_time.max(preview_start);
        let segment_end = end_time.min(preview_end);
        if segment_end < segment_start {
            continue;
        }

        let beat_duration = 60.0 / point.bpm;
        if !beat_duration.is_finite() || beat_duration <= 0.0 {
            continue;
        }

        let mut beat = ((segment_start - start_time) / beat_duration)
            .ceil()
            .max(0.0) as u32;
        let mut time = start_time + beat as f32 * beat_duration;
        while time <= end_time + TAP_EPSILON_SECONDS {
            let clamped_time = time.clamp(0.0, duration);
            if clamped_time > segment_end + TAP_EPSILON_SECONDS {
                break;
            }
            if !tap_times
                .iter()
                .any(|tap| (*tap - clamped_time).abs() <= TAP_EPSILON_SECONDS)
            {
                division_times.push(clamped_time);
                if division_times.len() >= MAX_TIMING_DIVISION_TAP_PREVIEWS {
                    break;
                }
            }

            beat += 1;
            if beat > 10000 {
                break;
            }
            time = start_time + beat as f32 * beat_duration;
        }
    }

    division_times.sort_by(f32::total_cmp);
    division_times.dedup_by(|left, right| (*left - *right).abs() <= TAP_EPSILON_SECONDS);

    let mut runtime = TimelineSimulationRuntime::new(spawn, direction, objects, tap_times);
    let mut previews = Vec::with_capacity(division_times.len());
    for time_seconds in division_times {
        runtime.advance_to(time_seconds);
        if runtime.game_over() && runtime.elapsed_seconds() + TAP_EPSILON_SECONDS < time_seconds {
            break;
        }

        let snapshot = runtime.snapshot();
        previews.push(TapDivisionPreview {
            time_seconds,
            indicator_position: [
                snapshot.position[0] - 0.5,
                snapshot.position[1],
                snapshot.position[2] - 0.5,
            ],
        });
    }

    previews
}

#[cfg(test)]
pub(crate) fn derive_timeline_time_for_world_target(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    duration_seconds: f32,
    objects: &[LevelObject],
    target: [f32; 3],
) -> f32 {
    let duration = duration_seconds.max(0.0);
    if duration <= 0.0 {
        return 0.0;
    }

    let last_tap_time = tap_times
        .iter()
        .copied()
        .filter(|tap| tap.is_finite() && *tap >= 0.0)
        .fold(0.0_f32, f32::max)
        .min(duration);

    fn distance_sq(position: [f32; 3], target: [f32; 3]) -> f32 {
        let dx = position[0] - target[0];
        let dy = position[1] - target[1];
        let dz = position[2] - target[2];
        dx * dx + dy * dy + dz * dz
    }

    const SOLVE_COARSE_DT: f32 = 1.0 / 90.0;
    const SOLVE_FINE_DT: f32 = 1.0 / 150.0;

    let sample_best_time =
        |start_time: f32, end_time: f32, samples: usize, sim_dt: f32| -> (f32, f32) {
            let mut runtime = TimelineSimulationRuntime::new_with_dt(
                spawn, direction, objects, tap_times, sim_dt,
            );
            runtime.advance_to(start_time);

            let mut best_time = start_time;
            let mut best_distance_sq = f32::INFINITY;
            let step = if samples == 0 {
                0.0
            } else {
                (end_time - start_time) / samples as f32
            };

            for index in 0..=samples {
                let t = (start_time + step * index as f32).clamp(start_time, end_time);
                runtime.advance_to(t);
                let snapshot = runtime.snapshot();
                let current_distance_sq = distance_sq(snapshot.position, target);
                if current_distance_sq < best_distance_sq {
                    best_distance_sq = current_distance_sq;
                    best_time = t;
                    if best_distance_sq <= 1e-6 {
                        break;
                    }
                }
            }

            (best_time, best_distance_sq)
        };

    let solve_in_range = |range_start: f32, range_end: f32| -> (f32, f32) {
        let range_width = (range_end - range_start).max(0.0);
        let coarse_samples = ((range_width * 28.0).clamp(48.0, 240.0)) as usize;
        let (mut refined_time, mut best_distance_sq) =
            sample_best_time(range_start, range_end, coarse_samples, SOLVE_COARSE_DT);

        const CLOSE_ENOUGH_SQ: f32 = 0.20 * 0.20;
        if best_distance_sq <= CLOSE_ENOUGH_SQ {
            return (refined_time.clamp(range_start, range_end), best_distance_sq);
        }

        for (window_seconds, refinement_samples) in [(0.6_f32, 80_usize), (0.15_f32, 48_usize)] {
            if best_distance_sq <= 1e-6 {
                break;
            }

            let window = window_seconds.min(duration.max(0.01));
            let start_time = (refined_time - window).max(range_start);
            let end_time = (refined_time + window).min(range_end);

            let (local_best_time, local_best_distance_sq) =
                sample_best_time(start_time, end_time, refinement_samples, SOLVE_FINE_DT);
            refined_time = local_best_time;
            best_distance_sq = local_best_distance_sq;
        }

        (refined_time.clamp(range_start, range_end), best_distance_sq)
    };

    let (local_time, local_distance_sq) = solve_in_range(last_tap_time, duration);
    const ACCEPT_LOCAL_DISTANCE_SQ: f32 = 1.5 * 1.5;
    if local_distance_sq <= ACCEPT_LOCAL_DISTANCE_SQ || last_tap_time <= 1e-6 {
        return local_time;
    }

    let (full_time, _) = solve_in_range(0.0, duration);
    full_time
}

pub(crate) fn derive_timeline_time_for_world_target_near_time(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    duration_seconds: f32,
    objects: &[LevelObject],
    target: [f32; 3],
    search: TimelineNearSearch,
) -> f32 {
    let duration = if duration_seconds.is_finite() {
        duration_seconds.max(0.0)
    } else {
        0.0
    };
    if duration <= 0.0 {
        return 0.0;
    }

    const COARSE_SIMULATION_DT: f32 = 1.0 / 90.0;
    const FINE_SIMULATION_DT: f32 = 1.0 / 240.0;

    let requested_seed_time = if search.seed_time.is_finite() {
        search.seed_time.clamp(0.0, duration)
    } else {
        0.0
    };
    let seed_time = {
        let mut runtime = TimelineSimulationRuntime::new_with_dt(
            spawn,
            direction,
            objects,
            tap_times,
            FINE_SIMULATION_DT,
        );
        runtime.advance_to(requested_seed_time);
        runtime.elapsed_seconds().clamp(0.0, duration)
    };
    let search_window = if search.window_seconds.is_finite() {
        search.window_seconds.max(0.01)
    } else {
        0.01
    };
    let range_start = (seed_time - search_window).clamp(0.0, duration);
    let range_end = (seed_time + search_window).clamp(0.0, duration);
    if range_end <= range_start {
        return range_start;
    }

    fn distance_sq(position: [f32; 3], target: [f32; 3]) -> f32 {
        let dx = position[0] - target[0];
        let dy = position[1] - target[1];
        let dz = position[2] - target[2];
        dx * dx + dy * dy + dz * dz
    }

    fn segment_closest_time(
        previous_time: f32,
        previous_position: [f32; 3],
        current_time: f32,
        current_position: [f32; 3],
        target: [f32; 3],
    ) -> Option<(f32, f32)> {
        if current_time <= previous_time {
            return None;
        }

        let segment = [
            current_position[0] - previous_position[0],
            current_position[1] - previous_position[1],
            current_position[2] - previous_position[2],
        ];
        let segment_length_sq =
            segment[0] * segment[0] + segment[1] * segment[1] + segment[2] * segment[2];
        if segment_length_sq <= 1e-8 {
            return None;
        }

        let target_offset = [
            target[0] - previous_position[0],
            target[1] - previous_position[1],
            target[2] - previous_position[2],
        ];
        let projected_fraction = ((target_offset[0] * segment[0]
            + target_offset[1] * segment[1]
            + target_offset[2] * segment[2])
            / segment_length_sq)
            .clamp(0.0, 1.0);
        let closest_position = [
            previous_position[0] + segment[0] * projected_fraction,
            previous_position[1] + segment[1] * projected_fraction,
            previous_position[2] + segment[2] * projected_fraction,
        ];
        let closest_time = previous_time + (current_time - previous_time) * projected_fraction;

        Some((closest_time, distance_sq(closest_position, target)))
    }

    let sample_best_time =
        |start_time: f32, end_time: f32, samples: usize, simulation_dt: f32| -> (f32, f32) {
            let mut runtime = TimelineSimulationRuntime::new_with_dt(
                spawn,
                direction,
                objects,
                tap_times,
                simulation_dt,
            );
            runtime.advance_to(start_time);
            let mut previous_time = start_time;
            let mut previous_snapshot = runtime.snapshot();

            let mut best_time = start_time;
            let mut best_distance_sq = distance_sq(previous_snapshot.position, target);
            if runtime.game_over() {
                return (
                    runtime.elapsed_seconds().clamp(0.0, duration),
                    best_distance_sq,
                );
            }

            let step = if samples == 0 {
                0.0
            } else {
                (end_time - start_time) / samples as f32
            };

            for index in 1..=samples {
                let sample_time = (start_time + step * index as f32).clamp(start_time, end_time);
                runtime.advance_to(sample_time);
                let snapshot = runtime.snapshot();
                if runtime.game_over() {
                    break;
                }

                if previous_snapshot.direction == snapshot.direction
                    && (previous_snapshot.speed - snapshot.speed).abs() <= 1e-4
                {
                    if let Some((candidate_time, candidate_distance_sq)) = segment_closest_time(
                        previous_time,
                        previous_snapshot.position,
                        sample_time,
                        snapshot.position,
                        target,
                    ) {
                        if candidate_distance_sq < best_distance_sq {
                            best_distance_sq = candidate_distance_sq;
                            best_time = candidate_time;
                        }
                    }
                }

                let current_distance_sq = distance_sq(snapshot.position, target);
                if current_distance_sq < best_distance_sq {
                    best_distance_sq = current_distance_sq;
                    best_time = sample_time;
                }

                previous_time = sample_time;
                previous_snapshot = snapshot;

                if best_distance_sq <= 1e-6 {
                    break;
                }
            }

            (best_time, best_distance_sq)
        };

    let range_width = (range_end - range_start).max(0.0);
    let coarse_samples = ((range_width * 28.0).clamp(24.0, 96.0)) as usize;
    let (mut refined_time, best_distance_sq) =
        sample_best_time(range_start, range_end, coarse_samples, COARSE_SIMULATION_DT);
    refined_time = refined_time.clamp(range_start, range_end);

    let refinement_window = (range_width * 0.16).clamp(0.08, 0.28);
    let refinement_samples = ((range_width * 64.0).clamp(48.0, 128.0)) as usize;
    if best_distance_sq > 1e-6 {
        let start_time = (refined_time - refinement_window).max(range_start);
        let end_time = (refined_time + refinement_window).min(range_end);
        let (local_best_time, _local_best_distance_sq) =
            sample_best_time(start_time, end_time, refinement_samples, FINE_SIMULATION_DT);
        refined_time = local_best_time;
    }

    refined_time.clamp(range_start, range_end)
}

#[derive(Clone, Copy)]
pub(crate) struct TimelineNearSearch {
    pub(crate) seed_time: f32,
    pub(crate) window_seconds: f32,
}

pub(crate) struct TimelineState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
    pub(crate) elapsed_seconds: f32,
    pub(crate) speed: f32,
}

pub(crate) fn derive_timeline_state_with_triggers(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
    triggers: &[TimedTrigger],
    simulate_trigger_hitboxes: bool,
) -> TimelineState {
    let simulated = if triggers.is_empty() {
        simulate_timeline_state(spawn, direction, objects, tap_times, timeline_time_seconds)
    } else {
        simulate_timeline_state_with_triggers(
            spawn,
            direction,
            objects,
            tap_times,
            triggers,
            simulate_trigger_hitboxes,
            timeline_time_seconds,
        )
    };

    TimelineState {
        position: simulated.position,
        direction: simulated.direction,
        elapsed_seconds: simulated.elapsed_seconds,
        speed: simulated.speed,
    }
}
