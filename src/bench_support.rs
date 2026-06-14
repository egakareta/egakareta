/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Feature-gated helpers for external benchmark targets.

use std::hint::black_box;

use crate::game::{trigger_transformed_objects_at_time, TimelineSimulationRuntime};
use crate::mesh::build_block_geometry;
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
    objects: Vec<LevelObject>,
    tap_times: Vec<f32>,
    triggers: Vec<TimedTrigger>,
    simulate_trigger_hitboxes: bool,
    runtime: TimelineSimulationRuntime,
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
        let objects = benchmark_objects(object_count);
        let triggers = benchmark_object_triggers(object_count, trigger_count, targets_per_trigger);
        let tap_times = benchmark_tap_times(initial_time_seconds + 1.0);
        let mut runtime =
            new_scrub_runtime(&objects, &tap_times, &triggers, simulate_trigger_hitboxes);
        runtime.advance_to(black_box(initial_time_seconds.max(0.0)));
        Self {
            objects,
            tap_times,
            triggers,
            simulate_trigger_hitboxes,
            runtime,
        }
    }

    /// Moves the scrub runtime backward by `delta_seconds`, forcing a rewind reset.
    pub fn scrub_backward(&mut self, delta_seconds: f32) -> BenchmarkWorkSummary {
        let target_time_seconds = (self.runtime.elapsed_seconds() - delta_seconds.abs()).max(0.0);
        self.runtime = new_scrub_runtime(
            &self.objects,
            &self.tap_times,
            &self.triggers,
            self.simulate_trigger_hitboxes,
        );
        self.advance_to(target_time_seconds)
    }

    /// Moves the scrub runtime forward by `delta_seconds`, reusing the runtime.
    pub fn scrub_forward(&mut self, delta_seconds: f32) -> BenchmarkWorkSummary {
        let target_time_seconds = self.runtime.elapsed_seconds() + delta_seconds.abs();
        self.advance_to(target_time_seconds)
    }

    fn advance_to(&mut self, target_time_seconds: f32) -> BenchmarkWorkSummary {
        self.runtime.advance_to(black_box(target_time_seconds));
        black_box(self.runtime.snapshot().position);
        black_box(self.runtime.trail_segments().len());
        BenchmarkWorkSummary {
            object_count: self.runtime.objects().len(),
            vertex_count: 0,
            draw_count: 0,
        }
    }
}

fn new_scrub_runtime(
    objects: &[LevelObject],
    tap_times: &[f32],
    triggers: &[TimedTrigger],
    simulate_trigger_hitboxes: bool,
) -> TimelineSimulationRuntime {
    TimelineSimulationRuntime::new_with_triggers(
        [0.0, 2.0, 0.0],
        SpawnDirection::Forward,
        black_box(objects),
        black_box(tap_times),
        black_box(triggers),
        black_box(simulate_trigger_hitboxes),
    )
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
