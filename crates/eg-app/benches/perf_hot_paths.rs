/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use egakareta_lib::bench_support::{
    TimelineScrubBenchmarkState, advance_timeline_preview, rebuild_chunked_dense_block_mesh,
    rebuild_full_block_mesh, rebuild_instanced_block_batch, rebuild_transform_marker_sidecars,
    rebuild_transformed_block_mesh, transform_objects_only, update_one_instanced_block,
    update_one_transform_marker_sidecar,
};

fn editor_mesh_rebuild_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_full_block_mesh_rebuild");
    for object_count in [256, 1_024, 4_096] {
        group.bench_function(format!("{object_count}_stone_blocks"), |b| {
            b.iter(|| rebuild_full_block_mesh(black_box(object_count)))
        });
    }
    group.finish();
}

fn retained_editor_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_retained_instance_updates");
    for object_count in [256, 1_024, 4_096] {
        group.bench_function(format!("full_instanced_{object_count}_objects"), |b| {
            b.iter(|| rebuild_instanced_block_batch(black_box(object_count)))
        });
        group.bench_function(format!("single_update_in_{object_count}_objects"), |b| {
            b.iter(|| update_one_instanced_block(black_box(object_count), black_box(17)))
        });
    }
    group.finish();

    let mut marker_group = c.benchmark_group("editor_retained_marker_updates");
    for marker_count in [16, 64, 256] {
        marker_group.bench_function(format!("full_{marker_count}_markers"), |b| {
            b.iter(|| rebuild_transform_marker_sidecars(black_box(marker_count), black_box(2.75)))
        });
        marker_group.bench_function(format!("single_update_in_{marker_count}_markers"), |b| {
            b.iter(|| {
                update_one_transform_marker_sidecar(
                    black_box(marker_count),
                    black_box(7),
                    black_box(2.75),
                )
            })
        });
    }
    marker_group.finish();
}

fn chunked_greedy_mesh_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunked_greedy_dense_block_mesh");
    for object_count in [4_096, 16_384, 65_536] {
        group.bench_function(format!("{object_count}_dense_stone_blocks"), |b| {
            b.iter(|| rebuild_chunked_dense_block_mesh(black_box(object_count)))
        });
    }
    group.finish();
}

fn trigger_transform_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_transform_triggers");
    for (object_count, trigger_count, targets_per_trigger) in [(1_024, 64, 8), (4_096, 256, 16)] {
        group.bench_function(
            format!("transform_only_{object_count}_objects_{trigger_count}_triggers"),
            |b| {
                b.iter(|| {
                    transform_objects_only(
                        black_box(object_count),
                        black_box(trigger_count),
                        black_box(targets_per_trigger),
                        black_box(2.75),
                    )
                })
            },
        );
        group.bench_function(
            format!("transform_then_rebuild_{object_count}_objects_{trigger_count}_triggers"),
            |b| {
                b.iter(|| {
                    rebuild_transformed_block_mesh(
                        black_box(object_count),
                        black_box(trigger_count),
                        black_box(targets_per_trigger),
                        black_box(2.75),
                    )
                })
            },
        );
    }
    group.finish();
}

fn timeline_preview_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_preview_runtime");
    for simulate_trigger_hitboxes in [false, true] {
        group.bench_function(
            format!("advance_4s_hitboxes_{simulate_trigger_hitboxes}"),
            |b| {
                b.iter_batched(
                    || (1_024, 96, 8, simulate_trigger_hitboxes, 4.0),
                    |(object_count, trigger_count, targets_per_trigger, hitboxes, target_time)| {
                        advance_timeline_preview(
                            black_box(object_count),
                            black_box(trigger_count),
                            black_box(targets_per_trigger),
                            black_box(hitboxes),
                            black_box(target_time),
                        )
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn editor_timeline_scrub_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_timeline_scrub_no_playback");
    for simulate_trigger_hitboxes in [false, true] {
        group.bench_function(
            format!(
                "single_step_backward_from_21s_4096_objects_hitboxes_{simulate_trigger_hitboxes}"
            ),
            |b| {
                let mut state = TimelineScrubBenchmarkState::new(
                    black_box(4_096),
                    black_box(256),
                    black_box(16),
                    black_box(simulate_trigger_hitboxes),
                    black_box(21.5),
                );
                b.iter(|| {
                    state.reset_time(black_box(21.5));
                    state.scrub_backward(black_box(0.1))
                })
            },
        );
        group.bench_function(
            format!(
                "single_step_forward_from_21s_4096_objects_hitboxes_{simulate_trigger_hitboxes}"
            ),
            |b| {
                let mut state = TimelineScrubBenchmarkState::new(
                    black_box(4_096),
                    black_box(256),
                    black_box(16),
                    black_box(simulate_trigger_hitboxes),
                    black_box(21.5),
                );
                b.iter(|| {
                    state.reset_time(black_box(21.5));
                    state.scrub_forward(black_box(0.1))
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    perf_hot_paths,
    editor_mesh_rebuild_benchmarks,
    retained_editor_benchmarks,
    chunked_greedy_mesh_benchmarks,
    trigger_transform_benchmarks,
    timeline_preview_benchmarks,
    editor_timeline_scrub_benchmarks
);
criterion_main!(perf_hot_paths);
