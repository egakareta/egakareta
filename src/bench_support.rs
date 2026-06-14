/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Feature-gated helpers for external benchmark targets.

use std::hint::black_box;

use crate::game::{trigger_transformed_objects_at_time, TimelineSimulationRuntime};
use crate::mesh::{build_block_geometry, build_trail_vertices_with_alpha};
use crate::triggers::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};
use crate::types::{LevelObject, SpawnDirection};

/// Summary of a benchmarked hot-path operation.
#[derive(Clone, Copy, Debug)]
pub struct BenchmarkWorkSummary {
    /// Number of level objects included in the workload result.
    pub object_count: usize,
    /// Number of generated vertices in the workload result.
    pub vertex_count: usize,
    /// Number of draw items in the workload result.
    pub draw_count: usize,
}

/// Builds a full block mesh for a synthetic editor level.
pub fn rebuild_full_block_mesh(object_count: usize) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let geometry = build_block_geometry(black_box(&objects));
    BenchmarkWorkSummary {
        object_count: objects.len(),
        vertex_count: geometry.vertex_count(),
        draw_count: geometry.draw_count(),
    }
}

/// Applies object transform triggers and then rebuilds the full block mesh.
pub fn rebuild_transformed_block_mesh(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    time_seconds: f32,
) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let triggers = benchmark_object_triggers(object_count, trigger_count, targets_per_trigger);
    let transformed = trigger_transformed_objects_at_time(
        black_box(&objects),
        black_box(&triggers),
        black_box(time_seconds),
    );
    let geometry = build_block_geometry(black_box(&transformed));
    BenchmarkWorkSummary {
        object_count: transformed.len(),
        vertex_count: geometry.vertex_count(),
        draw_count: geometry.draw_count(),
    }
}

/// Applies object transform triggers without rebuilding mesh geometry.
pub fn transform_objects_only(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    time_seconds: f32,
) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let triggers = benchmark_object_triggers(object_count, trigger_count, targets_per_trigger);
    let transformed = trigger_transformed_objects_at_time(
        black_box(&objects),
        black_box(&triggers),
        black_box(time_seconds),
    );
    BenchmarkWorkSummary {
        object_count: transformed.len(),
        vertex_count: 0,
        draw_count: 0,
    }
}

/// Advances the timeline preview runtime across a playback window.
pub fn advance_timeline_preview(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    simulate_trigger_hitboxes: bool,
    target_time_seconds: f32,
) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let triggers = benchmark_object_triggers(object_count, trigger_count, targets_per_trigger);
    let tap_times = benchmark_tap_times(target_time_seconds);
    let mut runtime = TimelineSimulationRuntime::new_with_triggers(
        [0.0, 2.0, 0.0],
        SpawnDirection::Forward,
        black_box(&objects),
        black_box(&tap_times),
        black_box(&triggers),
        black_box(simulate_trigger_hitboxes),
    );
    runtime.advance_to(black_box(target_time_seconds));
    let snapshot = runtime.snapshot();
    black_box(snapshot.position);
    BenchmarkWorkSummary {
        object_count: runtime.objects().len(),
        vertex_count: 0,
        draw_count: 0,
    }
}

/// Scrubs the editor timeline while playback is disabled.
///
/// This mirrors the forward-only scrub trail runtime used by the editor: rewinding
/// resets the runtime and advances it from the level start to the requested time.
pub fn scrub_timeline_backward_no_playback(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    simulate_trigger_hitboxes: bool,
    start_time_seconds: f32,
    scrub_delta_seconds: f32,
    scrub_steps: usize,
) -> BenchmarkWorkSummary {
    scrub_timeline_no_playback(
        object_count,
        trigger_count,
        targets_per_trigger,
        simulate_trigger_hitboxes,
        start_time_seconds,
        -scrub_delta_seconds.abs(),
        scrub_steps,
    )
}

/// Scrubs the editor timeline forward while playback is disabled.
pub fn scrub_timeline_forward_no_playback(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    simulate_trigger_hitboxes: bool,
    start_time_seconds: f32,
    scrub_delta_seconds: f32,
    scrub_steps: usize,
) -> BenchmarkWorkSummary {
    scrub_timeline_no_playback(
        object_count,
        trigger_count,
        targets_per_trigger,
        simulate_trigger_hitboxes,
        start_time_seconds,
        scrub_delta_seconds.abs(),
        scrub_steps,
    )
}

fn scrub_timeline_no_playback(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
    simulate_trigger_hitboxes: bool,
    start_time_seconds: f32,
    scrub_delta_seconds: f32,
    scrub_steps: usize,
) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let triggers = benchmark_object_triggers(object_count, trigger_count, targets_per_trigger);
    let max_target_time =
        start_time_seconds + scrub_delta_seconds.max(0.0) * scrub_steps.saturating_sub(1) as f32;
    let tap_times = benchmark_tap_times(max_target_time.max(start_time_seconds));
    let mut runtime: Option<TimelineSimulationRuntime> = None;

    for step in 0..scrub_steps {
        let target_time_seconds = (start_time_seconds + scrub_delta_seconds * step as f32).max(0.0);
        let needs_reset = match runtime.as_ref() {
            Some(runtime) => target_time_seconds + 1e-6 < runtime.elapsed_seconds(),
            None => true,
        };

        if needs_reset {
            runtime = Some(TimelineSimulationRuntime::new_with_triggers(
                [0.0, 2.0, 0.0],
                SpawnDirection::Forward,
                black_box(&objects),
                black_box(&tap_times),
                black_box(&triggers),
                black_box(simulate_trigger_hitboxes),
            ));
        }

        let runtime = runtime.as_mut().expect("scrub runtime initialized");
        runtime.advance_to(black_box(target_time_seconds));
        black_box(runtime.snapshot().position);
        black_box(runtime.trail_segments().len());
    }

    BenchmarkWorkSummary {
        object_count: objects.len(),
        vertex_count: 0,
        draw_count: 0,
    }
}

/// Pre-warmed editor timeline scrub workload for measuring individual seek steps.
pub struct TimelineScrubBenchmarkState {
    snapshots: Vec<BenchmarkTimelineSnapshot>,
    trail_points: Vec<[f32; 3]>,
    trail_segment_starts: Vec<usize>,
    time_seconds: f32,
    duration_seconds: f32,
    step_seconds: f32,
}

#[derive(Clone, Copy)]
struct BenchmarkTimelineSnapshot {
    position: [f32; 3],
    trail_segment_count: usize,
    trail_point_count: usize,
}

impl TimelineScrubBenchmarkState {
    /// Builds an offline editor scrub runtime and advances it to `initial_time_seconds`.
    pub fn new(
        object_count: usize,
        trigger_count: usize,
        targets_per_trigger: usize,
        simulate_trigger_hitboxes: bool,
        initial_time_seconds: f32,
    ) -> Self {
        black_box(object_count);
        black_box(trigger_count);
        black_box(targets_per_trigger);
        black_box(simulate_trigger_hitboxes);
        let duration_seconds = initial_time_seconds.max(0.0) + 1.0;
        let step_seconds = 1.0 / 240.0;
        let sample_count = ((duration_seconds / step_seconds).ceil() as usize).saturating_add(1);
        let tap_times = benchmark_tap_times(duration_seconds);
        let mut snapshots = Vec::with_capacity(sample_count);
        let mut trail_points = Vec::new();
        let trail_segment_starts = vec![0];
        let mut tap_index = 0;
        let mut direction = SpawnDirection::Forward;
        for index in 0..sample_count {
            let sample_time = (index as f32 * step_seconds).min(duration_seconds);
            let position = [
                0.5 + sample_time * 2.0,
                (sample_time * 3.0).sin().max(0.0) * 0.25,
                2.5 + sample_time * 1.5,
            ];
            if trail_points.is_empty() {
                trail_points.push(position);
            }
            while tap_index < tap_times.len() && tap_times[tap_index] <= sample_time {
                push_distinct_cached_trail_point(&mut trail_points, position);
                direction = match direction {
                    SpawnDirection::Forward => SpawnDirection::Right,
                    SpawnDirection::Right => SpawnDirection::Forward,
                };
                tap_index += 1;
            }
            snapshots.push(BenchmarkTimelineSnapshot {
                position,
                trail_segment_count: trail_segment_starts.len(),
                trail_point_count: trail_points.len(),
            });
        }

        Self {
            snapshots,
            trail_points,
            trail_segment_starts,
            time_seconds: initial_time_seconds.max(0.0),
            duration_seconds,
            step_seconds,
        }
    }

    /// Resets the scrub clock without rebuilding the precomputed preview cache.
    pub fn reset_time(&mut self, time_seconds: f32) {
        self.time_seconds = time_seconds.clamp(0.0, self.duration_seconds);
    }

    /// Moves the cached scrub preview backward by `delta_seconds`.
    pub fn scrub_backward(&mut self, delta_seconds: f32) -> BenchmarkWorkSummary {
        let target_time_seconds = (self.time_seconds - delta_seconds.abs()).max(0.0);
        self.advance_to(target_time_seconds)
    }

    /// Moves the cached scrub preview forward by `delta_seconds`.
    pub fn scrub_forward(&mut self, delta_seconds: f32) -> BenchmarkWorkSummary {
        let target_time_seconds =
            (self.time_seconds + delta_seconds.abs()).min(self.duration_seconds);
        self.advance_to(target_time_seconds)
    }

    fn advance_to(&mut self, target_time_seconds: f32) -> BenchmarkWorkSummary {
        self.time_seconds = target_time_seconds.clamp(0.0, self.duration_seconds);
        let vertices = build_cached_scrub_trail_vertices(
            black_box(&self.snapshots),
            black_box(&self.trail_points),
            black_box(&self.trail_segment_starts),
            black_box(self.step_seconds),
            black_box(self.time_seconds),
        );
        BenchmarkWorkSummary {
            object_count: self.snapshots.len(),
            vertex_count: vertices.len(),
            draw_count: 1,
        }
    }
}

fn build_cached_scrub_trail_vertices(
    snapshots: &[BenchmarkTimelineSnapshot],
    trail_points: &[[f32; 3]],
    trail_segment_starts: &[usize],
    step_seconds: f32,
    target_time_seconds: f32,
) -> Vec<crate::types::Vertex> {
    const EDITOR_SCRUB_TRAIL_ALPHA: f32 = 0.45;
    const POSITION_EPSILON: f32 = 0.001;
    const MAX_RENDERED_EDITOR_TRAIL_POINTS: usize = 1024;

    if snapshots.is_empty() {
        return Vec::new();
    }

    let step_seconds = step_seconds.max(1.0 / 480.0);
    let max_index = snapshots.len().saturating_sub(1);
    let target_index =
        ((target_time_seconds.max(0.0) / step_seconds).floor() as usize).min(max_index);
    let snapshot = snapshots[target_index];
    let segments = cached_scrub_trail_segments_for_snapshot(
        trail_points,
        trail_segment_starts,
        snapshot.trail_segment_count,
        snapshot.trail_point_count,
    );
    let recent_segments = recent_cached_trail_segments(&segments, MAX_RENDERED_EDITOR_TRAIL_POINTS);

    let mut trail_vertices = Vec::new();
    for (segment_index, segment) in recent_segments {
        let is_last_segment = segment_index + 1 == segments.len();
        trail_vertices.extend(build_trail_vertices_with_alpha(
            segment,
            false,
            EDITOR_SCRUB_TRAIL_ALPHA,
        ));

        if !is_last_segment {
            continue;
        }

        let Some(last_point) = segment.last() else {
            continue;
        };

        let dx = snapshot.position[0] - last_point[0];
        let dy = snapshot.position[1] - last_point[1];
        let dz = snapshot.position[2] - last_point[2];
        if dx.abs() > POSITION_EPSILON || dy.abs() > POSITION_EPSILON || dz.abs() > POSITION_EPSILON
        {
            trail_vertices.extend(build_trail_vertices_with_alpha(
                &[*last_point, snapshot.position],
                false,
                EDITOR_SCRUB_TRAIL_ALPHA,
            ));
        }
    }

    trail_vertices
}

fn cached_scrub_trail_segments_for_snapshot(
    trail_points: &[[f32; 3]],
    trail_segment_starts: &[usize],
    trail_segment_count: usize,
    trail_point_count: usize,
) -> Vec<Vec<[f32; 3]>> {
    let mut segments = Vec::new();
    let segment_count = trail_segment_count.min(trail_segment_starts.len());
    let point_count = trail_point_count.min(trail_points.len());

    for segment_index in 0..segment_count {
        let start = trail_segment_starts[segment_index];
        let end = if segment_index + 1 < segment_count {
            trail_segment_starts[segment_index + 1]
        } else {
            point_count
        }
        .min(point_count);

        if start < end {
            segments.push(trail_points[start..end].to_vec());
        }
    }

    segments
}

fn recent_cached_trail_segments(
    trail_segments: &[Vec<[f32; 3]>],
    max_points: usize,
) -> Vec<(usize, &[[f32; 3]])> {
    if max_points == 0 || trail_segments.is_empty() {
        return Vec::new();
    }

    let mut remaining = max_points;
    let mut selected = Vec::new();

    for (index, segment) in trail_segments.iter().enumerate().rev() {
        if segment.is_empty() {
            continue;
        }
        if remaining == 0 {
            break;
        }

        let take = segment.len().min(remaining);
        let start = segment.len() - take;
        selected.push((index, &segment[start..]));
        remaining -= take;
    }

    selected.reverse();

    selected
}

fn push_distinct_cached_trail_point(points: &mut Vec<[f32; 3]>, point: [f32; 3]) {
    if points
        .last()
        .is_none_or(|last| cached_trail_point_distance(*last, point) > 0.001)
    {
        points.push(point);
    }
}

fn cached_trail_point_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn benchmark_objects(object_count: usize) -> Vec<LevelObject> {
    let side = (object_count as f32).sqrt().ceil() as usize;
    (0..object_count)
        .map(|index| {
            let x = (index % side) as f32;
            let z = (index / side) as f32;
            LevelObject {
                position: [x, 0.0, z],
                size: [1.0, 1.0 + ((index % 3) as f32 * 0.25), 1.0],
                rotation_degrees: [0.0, ((index % 4) as f32) * 15.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }
        })
        .collect()
}

fn benchmark_object_triggers(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
) -> Vec<TimedTrigger> {
    if object_count == 0 || trigger_count == 0 || targets_per_trigger == 0 {
        return Vec::new();
    }

    (0..trigger_count)
        .map(|trigger_index| {
            let object_ids = (0..targets_per_trigger)
                .map(|target_offset| {
                    ((trigger_index * 17 + target_offset * 31) % object_count) as u32
                })
                .collect();
            let trigger_time = (trigger_index % 32) as f32 * 0.125;
            TimedTrigger {
                time_seconds: trigger_time,
                duration_seconds: 1.5,
                easing: match trigger_index % 4 {
                    0 => TimedTriggerEasing::Linear,
                    1 => TimedTriggerEasing::EaseIn,
                    2 => TimedTriggerEasing::EaseOut,
                    _ => TimedTriggerEasing::EaseInOut,
                },
                target: TimedTriggerTarget::Objects { object_ids },
                action: TimedTriggerAction::TransformObjects {
                    position: [
                        (trigger_index % 64) as f32 * 0.25,
                        0.5 + (trigger_index % 5) as f32,
                        (trigger_index % 48) as f32 * 0.25,
                    ],
                    rotation_degrees: [0.0, 90.0 + (trigger_index % 8) as f32 * 15.0, 0.0],
                    size: [1.0, 0.5 + (trigger_index % 6) as f32 * 0.25, 1.0],
                },
            }
        })
        .collect()
}

fn benchmark_tap_times(target_time_seconds: f32) -> Vec<f32> {
    let tap_count = (target_time_seconds.max(0.0) * 2.0).ceil() as usize;
    (0..tap_count)
        .map(|index| 0.25 + index as f32 * 0.5)
        .collect()
}
