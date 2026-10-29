/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use egakareta_lib::bench_support::{
    EditorPlacementBenchmarkState, TestingLevelState, TimelineScrubBenchmarkState,
    advance_timeline_preview, rebuild_chunked_dense_block_mesh, rebuild_full_block_mesh,
    rebuild_instanced_block_batch, rebuild_transform_marker_sidecars,
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

fn editor_placement_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_primary_click_placement");
    for object_count in [256, 1_024, 4_096] {
        group.bench_function(
            format!("state_schedule_frame_in_{object_count}_objects"),
            |b| {
                b.iter_batched(
                    || EditorPlacementBenchmarkState::new(object_count),
                    |mut state| state.state_schedule_frame(),
                    BatchSize::SmallInput,
                )
            },
        );
        group.bench_function(format!("idle_frame_in_{object_count}_objects"), |b| {
            b.iter_batched(
                || EditorPlacementBenchmarkState::new(object_count),
                |mut state| state.idle_frame(),
                BatchSize::SmallInput,
            )
        });
        group.bench_function(format!("place_one_in_{object_count}_objects"), |b| {
            b.iter_batched(
                || EditorPlacementBenchmarkState::new(object_count),
                |mut state| state.place_one(),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
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

/// Realistic editor benchmarks over the `testing` level (~19k objects).
///
/// Two measurement patterns are used. Most benchmarks reuse one warmed state
/// per benchmark function (`b.iter`): the operation either leaves the scene
/// unchanged (selection, hover, idle, tabs, escape, scrub) or drifts it by a
/// negligible amount (single placements grow it <2% over the whole run), so
/// every sample measures a full headless frame (`tick_state`: state schedule
/// + render extraction) with zero per-sample setup.
///
/// One-shot operations (music play/stop, playtest enter/exit, hitbox toggle,
/// bulk remove) rebuild a fresh state per sample (`iter_batched`) and return
/// it from the routine so the ~18ms `World` teardown is not timed.
fn testing_level_select_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_select");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);
    group.bench_function("select_one_extract", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&[7]);
        b.iter(|| state.idle_tick())
    });
    group.bench_function("select_100_extract", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&(100..200).collect::<Vec<_>>());
        b.iter(|| state.idle_tick())
    });
    group.bench_function("hover_pick", |b| {
        let mut state = TestingLevelState::new();
        state.hover_pick(600.0, 300.0);
        b.iter(|| state.hover_cycle())
    });
    group.finish();
}

fn testing_level_edit_single_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_edit_single");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);
    group.bench_function("place_one_stamp", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.stamp_place(black_box(640.0), black_box(360.0)))
    });
    group.bench_function("remove_one", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&[5]);
        b.iter(|| state.remove_selected())
    });
    group.bench_function("move_one_update", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&[7]);
        b.iter(|| state.move_one_alternate())
    });
    group.finish();
}

fn testing_level_edit_bulk_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_edit_bulk");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(30);
    group.bench_function("place_10_bulk", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.bulk_stamp_place_10())
    });
    group.bench_function("drag_100", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&(0..100).collect::<Vec<_>>());
        b.iter(|| state.drag_selection_alternate())
    });
    group.bench_function("duplicate_10", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&(0..10).collect::<Vec<_>>());
        b.iter(|| state.duplicate_selected())
    });
    group.bench_function("paste_10", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&(0..10).collect::<Vec<_>>());
        state.copy_selection();
        b.iter(|| state.paste())
    });
    group.finish();
}

fn testing_level_trigger_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_trigger");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(30);
    group.bench_function("add_transform_trigger", |b| {
        let mut state = TestingLevelState::new();
        state.set_selection(&(0..5).collect::<Vec<_>>());
        b.iter(|| state.add_transform_trigger())
    });
    group.bench_function("update_trigger_props", |b| {
        let mut state = TestingLevelState::new();
        state.seed_one_trigger();
        b.iter(|| state.trigger_nudge_alternate())
    });
    group.bench_function("add_camera_trigger", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.add_camera_trigger())
    });
    group.finish();
}

fn testing_level_timeline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_timeline");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(30);
    group.bench_function("add_tap", |b| {
        let mut state = TestingLevelState::new();
        state.enter_tapping_mode();
        b.iter(|| state.add_tap())
    });
    group.bench_function("scrub_cold_alt", |b| {
        let mut state = TestingLevelState::new();
        state.enter_tapping_mode();
        b.iter(|| state.scrub_alternate())
    });
    group.finish();
}

fn testing_level_tabs_chrome_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_tabs_chrome");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(50);
    group.bench_function("tab_place_tapping", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.toggle_tabs_place_tapping())
    });
    group.bench_function("tab_place_timing", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.toggle_tabs_place_timing())
    });
    group.bench_function("undo_redo_roundtrip", |b| {
        let mut state = TestingLevelState::new();
        state.seed_one_placement();
        b.iter(|| state.undo_redo_roundtrip())
    });
    group.bench_function("escape_clear_selection", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.escape_with_fresh_selection())
    });
    group.bench_function("camera_drag", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.camera_drag(black_box(5.0), black_box(-3.0)))
    });
    group.bench_function("idle", |b| {
        let mut state = TestingLevelState::new();
        b.iter(|| state.idle_tick())
    });
    group.finish();
}

/// One-shot operations: fresh state per sample, state returned so teardown
/// stays outside the measurement.
fn testing_level_oneshot_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_oneshot");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(15);
    group.bench_function("music_play", |b| {
        b.iter_batched(
            TestingLevelState::new,
            |mut state| {
                let summary = state.toggle_playback();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("music_stop", |b| {
        b.iter_batched(
            || {
                let mut state = TestingLevelState::new();
                state.toggle_playback();
                state
            },
            |mut state| {
                let summary = state.toggle_playback();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("playtest_enter", |b| {
        b.iter_batched(
            TestingLevelState::new,
            |mut state| {
                let summary = state.enter_playtest();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("playtest_exit", |b| {
        b.iter_batched(
            || {
                let mut state = TestingLevelState::new();
                state.enter_playtest();
                state
            },
            |mut state| {
                let summary = state.exit_playtest();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("remove_100", |b| {
        b.iter_batched(
            || {
                let mut state = TestingLevelState::new();
                state.set_selection(&(0..100).collect::<Vec<_>>());
                state
            },
            |mut state| {
                let summary = state.remove_selected();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("snap_100", |b| {
        b.iter_batched(
            || {
                let mut state = TestingLevelState::new();
                state.set_selection(&(0..100).collect::<Vec<_>>());
                state.jitter_first_objects(100, 0.37);
                state
            },
            |mut state| {
                let summary = state.snap_selection();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn testing_level_hitbox_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("testing_level_hitbox");
    group.sample_size(10);
    group.bench_function("toggle_hitbox_on", |b| {
        b.iter_batched(
            TestingLevelState::new,
            |mut state| {
                let summary = state.toggle_hitbox();
                (summary, state)
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(
    perf_hot_paths,
    editor_mesh_rebuild_benchmarks,
    retained_editor_benchmarks,
    editor_placement_benchmarks,
    testing_level_select_benchmarks,
    testing_level_edit_single_benchmarks,
    testing_level_edit_bulk_benchmarks,
    testing_level_trigger_benchmarks,
    testing_level_timeline_benchmarks,
    testing_level_tabs_chrome_benchmarks,
    testing_level_oneshot_benchmarks,
    testing_level_hitbox_benchmarks,
    chunked_greedy_mesh_benchmarks,
    trigger_transform_benchmarks,
    timeline_preview_benchmarks,
    editor_timeline_scrub_benchmarks
);
criterion_main!(perf_hot_paths);
