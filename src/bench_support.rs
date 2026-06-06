/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Feature-gated helpers for external benchmark targets.

use std::hint::black_box;

use crate::game::{trigger_transformed_objects_at_time, TimelineSimulationRuntime};
use crate::mesh::build_block_geometry;
use crate::types::{
    LevelObject, SpawnDirection, TimedTrigger, TimedTriggerAction, TimedTriggerEasing,
    TimedTriggerTarget,
};

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
                action: match trigger_index % 3 {
                    0 => TimedTriggerAction::MoveTo {
                        position: [
                            (trigger_index % 64) as f32 * 0.25,
                            0.5 + (trigger_index % 5) as f32,
                            (trigger_index % 48) as f32 * 0.25,
                        ],
                    },
                    1 => TimedTriggerAction::RotateTo {
                        rotation_degrees: [0.0, 90.0 + (trigger_index % 8) as f32 * 15.0, 0.0],
                    },
                    _ => TimedTriggerAction::ScaleTo {
                        size: [1.0, 0.5 + (trigger_index % 6) as f32 * 0.25, 1.0],
                    },
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
