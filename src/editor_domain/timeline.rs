use crate::game::{simulate_timeline_state, TimelineSimulationRuntime};
use crate::types::{LevelObject, SpawnDirection};

#[cfg(test)]
pub(crate) fn derive_timeline_position(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> ([f32; 3], SpawnDirection) {
    let state = derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects);
    (state.position, state.direction)
}

pub(crate) fn derive_timeline_elapsed_seconds(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> f32 {
    derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects)
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
            (snapshot.position[0] - 0.5).round(),
            snapshot.position[1].round(),
            (snapshot.position[2] - 0.5).round(),
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
    let duration = duration_seconds.max(0.0);
    if duration <= 0.0 {
        return 0.0;
    }

    let range_start = (search.seed_time - search.window_seconds.max(0.01)).clamp(0.0, duration);
    let range_end = (search.seed_time + search.window_seconds.max(0.01)).clamp(0.0, duration);
    if range_end <= range_start {
        return range_start;
    }

    fn distance_sq(position: [f32; 3], target: [f32; 3]) -> f32 {
        let dx = position[0] - target[0];
        let dy = position[1] - target[1];
        let dz = position[2] - target[2];
        dx * dx + dy * dy + dz * dz
    }

    const COARSE_SIMULATION_DT: f32 = 1.0 / 90.0;
    const FINE_SIMULATION_DT: f32 = 1.0 / 150.0;

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

    let range_width = (range_end - range_start).max(0.0);
    let coarse_samples = ((range_width * 28.0).clamp(24.0, 96.0)) as usize;
    let (mut refined_time, best_distance_sq) =
        sample_best_time(range_start, range_end, coarse_samples, COARSE_SIMULATION_DT);

    const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 0.20 * 0.20;
    if best_distance_sq <= CLOSE_ENOUGH_DISTANCE_SQ {
        return refined_time.clamp(range_start, range_end);
    }

    let refinement_window = (range_width * 0.16).clamp(0.08, 0.28);
    let refinement_samples = ((range_width * 36.0).clamp(28.0, 84.0)) as usize;
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

pub(crate) fn derive_timeline_state(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> TimelineState {
    let simulated =
        simulate_timeline_state(spawn, direction, objects, tap_times, timeline_time_seconds);

    TimelineState {
        position: simulated.position,
        direction: simulated.direction,
        elapsed_seconds: simulated.elapsed_seconds,
        speed: simulated.speed,
    }
}
