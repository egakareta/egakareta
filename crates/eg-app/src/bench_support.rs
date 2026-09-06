/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Feature-gated helpers for external benchmark targets.

use std::{
    hint::black_box,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use eg_game::{TimelineSimulationRuntime, trigger_transformed_objects_at_time};
use eg_mesh::{
    BlockInstance, TransformTriggerMarker, build_block_geometry, build_block_instance_batches,
    build_block_instance_for_object, build_chunked_block_mesh, build_trail_vertices_with_alpha,
    build_transform_trigger_marker_vertices,
};
use eg_state::state::{
    EditorInputRes, EditorSimulationInputsRes, EditorTimelineRes, SessionStateRes,
    collect_editor_objects_from_world, enqueue_platform_input, replace_editor_objects,
};

static STATE_SCHEDULE_DIAGNOSTIC_CALLS: AtomicUsize = AtomicUsize::new(0);
use eg_types::{
    AppPhase, Direction, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
    TimedTrigger, TriggerTarget,
};

/// Warmed headless editor state for measuring a real placement frame.
pub struct EditorPlacementBenchmarkState {
    app: eg_state::state::test_support::TestApp,
    expected_object_count: usize,
}

impl EditorPlacementBenchmarkState {
    /// Builds and warms an editor scene with the requested authored object count.
    pub fn new(object_count: usize) -> Self {
        let mut app = eg_state::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(bevy_state::prelude::State::new(eg_types::AppPhase::Editor));
        replace_editor_objects(app.world_mut(), benchmark_objects(object_count));
        app.world_mut().resource_mut::<EditorInputRes>().0.mode = eg_types::EditorMode::Tapping;
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .pointer_screen = Some([640.0, 360.0]);
        enqueue_platform_input(
            app.world_mut(),
            eg_state::commands::InputEvent::PointerMoved { x: 640.0, y: 360.0 },
        );
        app.tick_state();
        for _ in 0..1024 {
            if app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .snapshot_cache_complete
            {
                break;
            }
            app.tick_state();
        }
        assert!(
            app.world()
                .resource::<EditorTimelineRes>()
                .0
                .snapshot_cache_complete
        );
        app.world_mut().resource_mut::<EditorInputRes>().0.mode = eg_types::EditorMode::Place;
        enqueue_platform_input(
            app.world_mut(),
            eg_state::commands::InputEvent::PointerMoved { x: 640.0, y: 360.0 },
        );
        app.tick_state();
        app.world_mut().clear_trackers();

        Self {
            app,
            expected_object_count: object_count,
        }
    }

    /// Places one block through the platform-input and editor schedules.
    pub fn place_one(&mut self) -> BenchmarkWorkSummary {
        enqueue_platform_input(
            self.app.world_mut(),
            eg_state::commands::InputEvent::MouseButton {
                button: 0,
                pressed: true,
            },
        );
        self.app.tick_state();
        BenchmarkWorkSummary {
            object_count: self.expected_object_count + 1,
            vertex_count: 0,
            draw_count: 0,
        }
    }

    /// Runs one warmed editor frame without new input.
    pub fn idle_frame(&mut self) -> BenchmarkWorkSummary {
        self.app.tick_state();
        self.object_count_summary()
    }

    /// Runs only the state schedule for one warmed editor frame.
    pub fn state_schedule_frame(&mut self) -> BenchmarkWorkSummary {
        let before_revision = self
            .app
            .world()
            .resource::<EditorSimulationInputsRes>()
            .0
            .object_revision;
        assert!(self.app.run_state_schedule());
        let after_revision = self
            .app
            .world()
            .resource::<EditorSimulationInputsRes>()
            .0
            .object_revision;
        if STATE_SCHEDULE_DIAGNOSTIC_CALLS.fetch_add(1, Ordering::Relaxed) < 3 {
            eprintln!(
                "state schedule objects={} input revision {} -> {}",
                self.expected_object_count, before_revision, after_revision
            );
        }
        self.object_count_summary()
    }

    fn object_count_summary(&mut self) -> BenchmarkWorkSummary {
        BenchmarkWorkSummary {
            object_count: self.expected_object_count,
            vertex_count: 0,
            draw_count: 0,
        }
    }
}

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

/// Cached decode of the real `testing` level (`assets/levels/testing`).
///
/// The level holds ~19k authored objects with real-world block variety and no
/// triggers, so benchmarks built on it measure full-scene costs (per-edit
/// clones, reconciliation, extraction scans, timeline invalidation) instead
/// of synthetic-grid costs.
static TESTING_LEVEL_OBJECTS: OnceLock<Vec<LevelObject>> = OnceLock::new();

/// Returns the testing level's authored objects, decoded once and shared.
pub fn testing_level_objects() -> &'static [LevelObject] {
    TESTING_LEVEL_OBJECTS.get_or_init(|| {
        eg_level::load_builtin_level_metadata("testing")
            .expect("testing level must exist in assets/levels/testing/metadata.egb")
            .objects
    })
}

/// Warmed headless editor state over the real testing level.
///
/// Each benchmark builds a fresh state per sample (via `iter_batched` setup)
/// so every measured operation runs against the same ~19k-object scene with
/// settled change detection. Per-sample setup (selection, mode, clipboard)
/// happens in the unmeasured setup closure; only the message write plus the
/// frame (`tick_state`, i.e. state schedule + render extraction) is measured.
pub struct TestingLevelState {
    app: eg_state::state::test_support::TestApp,
    base_count: usize,
    spin: u64,
}

impl Default for TestingLevelState {
    fn default() -> Self {
        Self::new()
    }
}

impl TestingLevelState {
    /// Loads the testing level into a fresh editor and settles caches.
    pub fn new() -> Self {
        let objects = testing_level_objects().to_vec();
        let base_count = objects.len();
        let mut app = eg_state::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(bevy_state::prelude::State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        app.world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_level_name = Some("testing".to_string());
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = EditorMode::Place;
            input.0.pointer_screen = Some([640.0, 360.0]);
        }
        for _ in 0..3 {
            app.tick_state();
        }
        app.world_mut().clear_trackers();
        Self {
            app,
            base_count,
            spin: 0,
        }
    }

    fn next_spin(&mut self) -> u64 {
        let spin = self.spin;
        self.spin = spin.wrapping_add(1);
        spin
    }

    /// Number of authored objects in the untouched testing level.
    pub fn base_count(&self) -> usize {
        self.base_count
    }

    fn summary(&self, object_count: usize) -> BenchmarkWorkSummary {
        BenchmarkWorkSummary {
            object_count,
            vertex_count: 0,
            draw_count: 0,
        }
    }

    /// Overwrites the block selection (unmeasured setup helper).
    pub fn set_selection(&mut self, indices: &[usize]) {
        let mut input = self.app.world_mut().resource_mut::<EditorInputRes>();
        input.0.selected_block_index = indices.first().copied();
        input.0.selected_block_indices = indices.to_vec();
        input.0.hovered_block_index = indices.first().copied();
    }

    /// Reads one authored object by order index (for building update payloads).
    pub fn object_at(&mut self, index: usize) -> LevelObject {
        collect_editor_objects_from_world(self.app.world_mut())
            .into_iter()
            .nth(index)
            .expect("testing level must contain the indexed object")
    }

    /// Finds the first authored object carrying a trigger.
    pub fn first_trigger_object(&mut self) -> Option<(usize, LevelObject)> {
        collect_editor_objects_from_world(self.app.world_mut())
            .into_iter()
            .enumerate()
            .find(|(_, object)| object.trigger.is_some())
    }

    /// One idle editor frame with no new input.
    pub fn idle_tick(&mut self) -> BenchmarkWorkSummary {
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Runs only the state schedule (message handlers, caches, playback).
    pub fn tick_state_only(&mut self) -> BenchmarkWorkSummary {
        self.app.run_state_schedule();
        self.summary(self.base_count)
    }

    /// Runs only render extraction (mesh/overlay vertex building, no GPU).
    pub fn tick_render_extract(&mut self) -> BenchmarkWorkSummary {
        let _ = self
            .app
            .world_mut()
            .try_run_schedule(eg_state::state::RenderExtractSchedule);
        self.summary(self.base_count)
    }

    /// Mutable access to the underlying world for custom scenario setup.
    pub fn world_mut(&mut self) -> &mut bevy_ecs::world::World {
        self.app.world_mut()
    }

    /// One pointer-move (hover pick + cursor update) at screen coordinates.
    pub fn hover_pick(&mut self, x: f64, y: f64) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::pointer::PointerMoved { x, y });
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Places one block via stamp-click (mode stays `Place` for repeatability).
    pub fn stamp_place(&mut self, x: f64, y: f64) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .shift_held = true;
        self.app
            .world_mut()
            .write_message(eg_state::events::pointer::PrimaryClick { x, y });
        self.app.tick_state();
        self.summary(self.base_count + 1)
    }

    /// Places ten blocks from ten clicks processed in a single frame.
    pub fn bulk_stamp_place_10(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .shift_held = true;
        for i in 0..10 {
            let x = 600.0 + f64::from(i % 5) * 20.0;
            let y = 320.0 + f64::from(i / 5) * 20.0;
            self.app
                .world_mut()
                .write_message(eg_state::events::pointer::PrimaryClick { x, y });
        }
        self.app.tick_state();
        self.summary(self.base_count + 10)
    }

    /// Removes the currently selected blocks.
    pub fn remove_selected(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::block::RemoveBlock);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Applies an edited object to the current selection.
    pub fn update_selected(&mut self, updated: LevelObject) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::block::UpdateSelectedBlock(updated));
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Snaps the current selection to the grid.
    pub fn snap_selection(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::selection::SnapSelectionToGrid);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Duplicates the current selection in place.
    pub fn duplicate_selected(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::block::DuplicateBlock);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Copies the current selection into the clipboard (setup helper).
    pub fn copy_selection(&mut self) {
        self.app
            .world_mut()
            .write_message(eg_state::events::block::CopyBlock);
        self.app.tick_state();
    }

    /// Pastes the clipboard at the cursor.
    pub fn paste(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::block::PasteBlock);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Adds a transform trigger for the current selection.
    pub fn add_transform_trigger(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::selection::AddTransformTrigger);
        self.app.tick_state();
        self.summary(self.base_count + 1)
    }

    /// Adds a camera trigger at the cursor.
    pub fn add_camera_trigger(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::camera::AddCameraTrigger);
        self.app.tick_state();
        self.summary(self.base_count + 1)
    }

    /// Adds a tap at the current timeline time (caller sets `Tapping` mode).
    pub fn add_tap(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::timeline::AddTap);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Seeks the timeline clock (cold snapshot cache unless warmed).
    pub fn seek_timeline(&mut self, time_seconds: f32) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::timeline::SetTimelineTime(time_seconds));
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Toggles timeline playback (music start when stopped, stop when playing).
    pub fn toggle_playback(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::timeline::ToggleTimelinePlayback);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Enters playtesting (phase transition plus seeded gameplay reset).
    pub fn enter_playtest(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::timeline::Playtest);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Leaves a playtest via Escape (restores authored editor objects).
    pub fn exit_playtest(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::misc::Escape);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Switches the editor tab/mode.
    pub fn switch_mode(&mut self, mode: EditorMode) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::editor_mode::SetEditorMode(mode));
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Undoes the last history entry.
    pub fn undo(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::history::Undo);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Redoes the last undone history entry.
    pub fn redo(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::history::Redo);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Presses Escape in the editor (clears selection/drag state).
    pub fn escape(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::misc::Escape);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Toggles hitbox visualization (on enables the full-scene rebuild).
    pub fn toggle_hitbox(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::misc::ToggleHitboxVisualization);
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Drags the camera by a fixed delta.
    pub fn camera_drag(&mut self, dx: f64, dy: f64) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .write_message(eg_state::events::pointer::CameraDrag { dx, dy });
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Jitters the first `count` objects so a later snap has work to do.
    pub fn jitter_first_objects(&mut self, count: usize, delta: f32) {
        let mut objects = collect_editor_objects_from_world(self.app.world_mut());
        for object in objects.iter_mut().take(count) {
            object.position[0] += delta;
            object.position[1] += 0.5 * delta;
        }
        replace_editor_objects(self.app.world_mut(), objects);
        self.app.tick_state();
        self.app.world_mut().clear_trackers();
    }

    /// Switches to `Tapping` mode without measuring (setup helper).
    pub fn enter_tapping_mode(&mut self) {
        let _ = self.switch_mode(EditorMode::Tapping);
        self.app.world_mut().clear_trackers();
    }

    /// Hovers at cycling coordinates so consecutive frames re-run the pick.
    pub fn hover_cycle(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let x = 600.0 + f64::from((spin % 5) as u32) * 20.0;
        self.hover_pick(x, 340.0)
    }

    /// Moves the selected block between two absolute positions on alternate frames.
    pub fn move_one_alternate(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let mut object = self.object_at(7);
        object.position[0] = if spin.is_multiple_of(2) { 105.0 } else { 106.0 };
        self.set_selection(&[7]);
        self.update_selected(object)
    }

    /// Drags the current selection between two screen positions on alternate frames.
    pub fn drag_selection_alternate(&mut self) -> BenchmarkWorkSummary {
        self.app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        let spin = self.next_spin();
        let x = if spin.is_multiple_of(2) { 620.0 } else { 660.0 };
        self.app
            .world_mut()
            .write_message(eg_state::events::pointer::PointerMoved { x, y: 340.0 });
        self.app.tick_state();
        self.summary(self.base_count)
    }

    /// Nudges the first trigger's fire time between two values on alternate frames.
    pub fn trigger_nudge_alternate(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let (index, mut object) = self
            .first_trigger_object()
            .expect("a trigger must exist; create one in setup");
        let trigger = object.trigger.as_mut().expect("trigger must exist");
        trigger.time_seconds = if spin.is_multiple_of(2) { 1.0 } else { 2.0 };
        trigger.duration_seconds = 2.5;
        self.set_selection(&[index]);
        self.update_selected(object)
    }

    /// Seeks between two timeline times on alternate frames (cold cache path).
    pub fn scrub_alternate(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let target = if spin.is_multiple_of(2) { 4.0 } else { 12.0 };
        self.seek_timeline(black_box(target))
    }

    /// Toggles the Place/Tapping tabs on alternate frames.
    pub fn toggle_tabs_place_tapping(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let mode = if spin.is_multiple_of(2) {
            EditorMode::Tapping
        } else {
            EditorMode::Place
        };
        self.switch_mode(mode)
    }

    /// Toggles the Place/Timing tabs on alternate frames.
    pub fn toggle_tabs_place_timing(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        let mode = if spin.is_multiple_of(2) {
            EditorMode::Timing
        } else {
            EditorMode::Place
        };
        self.switch_mode(mode)
    }

    /// Alternates undo/redo of one seeded placement (setup must place first).
    pub fn undo_redo_roundtrip(&mut self) -> BenchmarkWorkSummary {
        let spin = self.next_spin();
        if spin.is_multiple_of(2) {
            self.undo()
        } else {
            self.redo()
        }
    }

    /// Re-selects 100 blocks and presses Escape in the same frame.
    pub fn escape_with_fresh_selection(&mut self) -> BenchmarkWorkSummary {
        self.set_selection(&(100..200).collect::<Vec<_>>());
        self.escape()
    }

    /// Stamps one block for later undo/redo benchmarks (setup helper).
    pub fn seed_one_placement(&mut self) {
        let _ = self.stamp_place(640.0, 360.0);
        self.app.world_mut().clear_trackers();
    }

    /// Creates one transform trigger for later benchmarks (setup helper).
    pub fn seed_one_trigger(&mut self) {
        self.set_selection(&(0..3).collect::<Vec<_>>());
        let _ = self.add_transform_trigger();
        self.app.world_mut().clear_trackers();
    }
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

/// Builds the complete compact instanced representation for a synthetic editor level.
pub fn rebuild_instanced_block_batch(object_count: usize) -> BenchmarkWorkSummary {
    let objects = benchmark_objects(object_count);
    let result = build_block_instance_batches(black_box(&objects));
    let vertex_count = result
        .batches
        .iter()
        .map(|batch| batch.template.vertex_count())
        .sum::<usize>()
        + result.instance_count() * std::mem::size_of::<BlockInstance>();
    BenchmarkWorkSummary {
        object_count: result.instance_count(),
        vertex_count,
        draw_count: result.batches.len(),
    }
}

/// Builds one changed editor instance record without rebuilding the level mesh.
pub fn update_one_instanced_block(
    object_count: usize,
    changed_index: usize,
) -> BenchmarkWorkSummary {
    let object_count = black_box(object_count);
    if object_count == 0 {
        return BenchmarkWorkSummary {
            object_count: 0,
            vertex_count: 0,
            draw_count: 0,
        };
    }
    let side = (object_count as f32).sqrt().ceil() as usize;
    let object = benchmark_object_with_side(changed_index % object_count, side.max(1));
    let instance = build_block_instance_for_object(black_box(&object), None);
    black_box(instance);
    BenchmarkWorkSummary {
        object_count: 1,
        vertex_count: std::mem::size_of::<BlockInstance>(),
        draw_count: 1,
    }
}

/// Rebuilds every transform-trigger marker sidecar for a timeline sample.
pub fn rebuild_transform_marker_sidecars(
    marker_count: usize,
    time_seconds: f32,
) -> BenchmarkWorkSummary {
    let markers = benchmark_transform_markers(marker_count);
    let vertices = build_transform_trigger_marker_vertices(black_box(&markers), time_seconds);
    BenchmarkWorkSummary {
        object_count: markers.len(),
        vertex_count: vertices.len(),
        draw_count: markers.len(),
    }
}

/// Rebuilds only one keyed transform-trigger marker sidecar.
pub fn update_one_transform_marker_sidecar(
    marker_count: usize,
    changed_index: usize,
    time_seconds: f32,
) -> BenchmarkWorkSummary {
    let marker_count = black_box(marker_count);
    if marker_count == 0 {
        return BenchmarkWorkSummary {
            object_count: 0,
            vertex_count: 0,
            draw_count: 0,
        };
    }
    let marker = benchmark_transform_marker(changed_index % marker_count);
    let vertices = build_transform_trigger_marker_vertices(
        std::slice::from_ref(black_box(&marker)),
        time_seconds,
    );
    BenchmarkWorkSummary {
        object_count: 1,
        vertex_count: vertices.len(),
        draw_count: 1,
    }
}

/// Builds a chunked greedy block mesh for a dense voxel field.
pub fn rebuild_chunked_dense_block_mesh(object_count: usize) -> BenchmarkWorkSummary {
    let objects = dense_voxel_objects(object_count);
    let mesh = build_chunked_block_mesh(black_box(&objects));
    let vertex_count = mesh
        .chunks
        .iter()
        .map(|chunk| {
            chunk.greedy_geometry.vertex_count()
                + chunk.fallback.fallback_geometry.vertex_count()
                + chunk
                    .fallback
                    .batches
                    .iter()
                    .map(|batch| batch.template.vertex_count())
                    .sum::<usize>()
        })
        .sum();
    let draw_count = mesh
        .chunks
        .iter()
        .map(|chunk| {
            chunk.greedy_geometry.draw_count()
                + chunk.fallback.fallback_geometry.draw_count()
                + chunk.fallback.batches.len()
        })
        .sum();
    BenchmarkWorkSummary {
        object_count: objects.len(),
        vertex_count,
        draw_count,
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
    let _ = black_box((
        trigger_count,
        targets_per_trigger,
        simulate_trigger_hitboxes,
    ));
    let tap_times = benchmark_tap_times(target_time_seconds);
    let mut runtime = TimelineSimulationRuntime::new(
        [0.0, 2.0, 0.0],
        Direction::Forward,
        black_box(&objects),
        black_box(&tap_times),
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
    let _ = black_box((
        trigger_count,
        targets_per_trigger,
        simulate_trigger_hitboxes,
    ));
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
            runtime = Some(TimelineSimulationRuntime::new(
                [0.0, 2.0, 0.0],
                Direction::Forward,
                black_box(&objects),
                black_box(&tap_times),
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
        let mut direction = Direction::Forward;
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
                    Direction::Forward => Direction::Right,
                    Direction::Right => Direction::Forward,
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
) -> Vec<eg_mesh::Vertex> {
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

/// Builds a synthetic list of plain editor blocks for benchmark workloads.
pub fn benchmark_objects(object_count: usize) -> Vec<LevelObject> {
    let side = (object_count as f32).sqrt().ceil() as usize;
    (0..object_count)
        .map(|index| benchmark_object_with_side(index, side))
        .collect()
}

fn benchmark_object_with_side(index: usize, side: usize) -> LevelObject {
    let x = (index % side) as f32;
    let z = (index / side) as f32;
    LevelObject {
        id: format!("object_{index}"),
        position: [x, 0.0, z],
        size: [1.0, 1.0 + ((index % 3) as f32 * 0.25), 1.0],
        rotation: [0.0, ((index % 4) as f32) * 15.0, 0.0],
        ..LevelObject::new("core/stone")
    }
}

/// Builds grid-aligned full-cube blocks for greedy meshing benchmarks.
pub fn dense_voxel_objects(object_count: usize) -> Vec<LevelObject> {
    let side = (object_count as f32).sqrt().ceil() as usize;
    (0..object_count)
        .map(|index| {
            let x = (index % side) as f32;
            let z = (index / side) as f32;
            LevelObject {
                id: format!("object_{index}"),
                position: [x, 0.0, z],
                ..LevelObject::new("core/stone")
            }
        })
        .collect()
}

/// Builds synthetic transform triggers for benchmark workloads.
pub fn benchmark_object_triggers(
    object_count: usize,
    trigger_count: usize,
    targets_per_trigger: usize,
) -> Vec<LevelObject> {
    if object_count == 0 || trigger_count == 0 || targets_per_trigger == 0 {
        return Vec::new();
    }

    (0..trigger_count)
        .map(|trigger_index| {
            let object_ids = (0..targets_per_trigger)
                .map(|target_offset| {
                    let object_index = (trigger_index * 17 + target_offset * 31) % object_count;
                    format!("object_{object_index}")
                })
                .collect();
            let trigger_time = (trigger_index % 32) as f32 * 0.125;
            LevelObject {
                id: format!("trigger_{trigger_index}"),
                position: [
                    (trigger_index % 64) as f32 * 0.25,
                    0.5 + (trigger_index % 5) as f32,
                    (trigger_index % 48) as f32 * 0.25,
                ],
                rotation: [0.0, 90.0 + (trigger_index % 8) as f32 * 15.0, 0.0],
                size: [1.0, 0.5 + (trigger_index % 6) as f32 * 0.25, 1.0],
                trigger: Some(TimedTrigger {
                    time_seconds: trigger_time,
                    duration_seconds: 1.5,
                    easing: match trigger_index % 4 {
                        0 => EasingDirection::Linear,
                        1 => EasingDirection::EaseIn,
                        2 => EasingDirection::EaseOut,
                        _ => EasingDirection::EaseInOut,
                    },
                    target: TriggerTarget::Objects { object_ids },
                    transform: true,
                    camera: None,
                }),
                ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
            }
        })
        .collect()
}

fn benchmark_transform_markers(marker_count: usize) -> Vec<TransformTriggerMarker> {
    (0..marker_count).map(benchmark_transform_marker).collect()
}

fn benchmark_transform_marker(index: usize) -> TransformTriggerMarker {
    let position = [index as f32 * 0.5, 0.0, index as f32 * 0.25];
    TransformTriggerMarker {
        source_position: Some(position),
        source_size: Some([1.0, 1.0, 1.0]),
        target_position: [position[0] + 2.0, 0.5, position[2] + 1.0],
        target_rotation_degrees: [0.0, (index % 8) as f32 * 45.0, 0.0],
        target_size: [1.0, 1.0, 1.0],
        time_seconds: (index % 32) as f32 * 0.125,
        duration_seconds: 1.5,
        is_selected: index == 0,
    }
}

/// Builds evenly spaced tap times up to `target_time_seconds`.
pub fn benchmark_tap_times(target_time_seconds: f32) -> Vec<f32> {
    let tap_count = (target_time_seconds.max(0.0) * 2.0).ceil() as usize;
    (0..tap_count)
        .map(|index| 0.25 + index as f32 * 0.5)
        .collect()
}
