/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::Vec3;

use super::{EditorDirtyFlags, EditorSubsystem, PerfStage, State};
use crate::editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds_with_triggers};
use crate::game::trigger_transformed_objects_at_time;
use crate::mesh::{
    append_editor_hover_outline_vertices, append_editor_hover_proxy_vertices, build_block_vertices,
    build_block_vertices_from_refs, build_camera_trigger_marker_vertices,
    build_editor_cursor_vertices, build_editor_gizmo_vertices,
    build_editor_selection_outline_vertices, build_spawn_marker_vertices,
    build_tap_indicator_vertices, GizmoParams, FAST_HOVER_VERTICES_PER_BLOCK,
    OUTLINE_VERTICES_PER_BLOCK,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::{AppPhase, EditorMode, GizmoPart, LevelObject, SpawnDirection};

const HOVER_DETAIL_BUDGET_MIN_BLOCKS: usize = 8;
const HOVER_DETAIL_BUDGET_MAX_BLOCKS: usize = 96;
const HOVER_DETAIL_BUDGET_STEP_BLOCKS: usize = 8;
const HOVER_DETAIL_TARGET_MS: f32 = 1.4;

impl EditorSubsystem {
    pub(crate) fn mark_dirty(&mut self, dirty: EditorDirtyFlags) {
        self.runtime.dirty.merge(dirty);
    }

    pub(crate) fn sync_objects(&mut self) {
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.selected_block_index {
            if index >= self.objects.len() {
                self.ui.selected_block_index = None;
            }
        }
        self.ui
            .selected_block_indices
            .retain(|index| *index < self.objects.len());
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.hovered_block_index {
            if index >= self.objects.len() {
                self.ui.hovered_block_index = None;
            }
        }
        self.invalidate_samples();
        self.selected_mask_cache = None;
        self.invalidate_marquee_projection_cache();
        self.mark_dirty(EditorDirtyFlags::from_object_sync());
    }

    pub(crate) fn sync_objects_for_drag(&mut self) {
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.selected_block_index {
            if index >= self.objects.len() {
                self.ui.selected_block_index = None;
            }
        }
        self.ui
            .selected_block_indices
            .retain(|index| *index < self.objects.len());
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.hovered_block_index {
            if index >= self.objects.len() {
                self.ui.hovered_block_index = None;
            }
        }
        self.invalidate_marquee_projection_cache();
        self.mark_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn sync_objects_after_drag_release(&mut self) {
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.selected_block_index {
            if index >= self.objects.len() {
                self.ui.selected_block_index = None;
            }
        }
        self.ui
            .selected_block_indices
            .retain(|index| *index < self.objects.len());
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.ui.hovered_block_index {
            if index >= self.objects.len() {
                self.ui.hovered_block_index = None;
            }
        }
        self.invalidate_samples();
        self.invalidate_marquee_projection_cache();
        self.mark_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_selection_overlays: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn add_block_at_cursor(&mut self) {
        self.record_history_state();
        self.objects.push(create_block_at_cursor(
            self.ui.cursor,
            &self.config.selected_block_id,
        ));
        self.ui.selected_block_index = None;
        self.ui.selected_block_indices.clear();
        self.ui.hovered_block_index = None;
        self.sync_objects();
    }

    pub(crate) fn timeline_elapsed_seconds(&self, time_seconds: f32) -> f32 {
        derive_timeline_elapsed_seconds_with_triggers(
            self.spawn.position,
            self.spawn.direction,
            &self.timeline.taps.tap_times,
            time_seconds,
            &self.objects,
            self.triggers(),
            self.simulate_trigger_hitboxes(),
        )
    }

    pub(crate) fn topmost_block_index_at_cursor(&self, cursor: [f32; 3]) -> Option<usize> {
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.objects.iter().enumerate() {
            let occupies_x = cursor[0] + 0.5 >= obj.position[0]
                && cursor[0] + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_z = cursor[2] + 0.5 >= obj.position[2]
                && cursor[2] + 0.5 <= obj.position[2] + obj.size[2];
            if occupies_x && occupies_z {
                let top = obj.position[1] + obj.size[1];
                if top > top_height {
                    top_height = top;
                    top_index = Some(index);
                }
            }
        }

        top_index
    }
}

impl State {
    pub(super) fn mark_editor_dirty(&mut self, dirty: EditorDirtyFlags) {
        self.editor.mark_dirty(dirty);
    }

    fn editor_runtime_objects_for_render(&self) -> Option<Vec<LevelObject>> {
        if self.phase != AppPhase::Editor
            || !self.editor.timeline.playback.playing
            || self.editor.ui.mode == EditorMode::Timing
            || !self.editor.has_object_transform_triggers()
        {
            return None;
        }

        Some(trigger_transformed_objects_at_time(
            &self.editor.objects,
            self.editor.triggers(),
            self.editor.timeline.clock.time_seconds,
        ))
    }

    pub(super) fn process_editor_dirty(&mut self, frame_dt: f32) {
        let pending = self.editor.runtime.dirty;
        if !pending.any() {
            self.editor.runtime.drag_heavy_rebuild_accumulator = 0.0;
            return;
        }

        let is_dragging = self.editor.runtime.interaction.gizmo_drag.is_some()
            || self.editor.runtime.interaction.block_drag.is_some();

        const DRAG_HEAVY_REBUILD_INTERVAL_SECONDS: f32 = 1.0 / 60.0;

        if is_dragging {
            self.editor.runtime.drag_heavy_rebuild_accumulator += frame_dt.max(0.0);
        } else {
            self.editor.runtime.drag_heavy_rebuild_accumulator =
                DRAG_HEAVY_REBUILD_INTERVAL_SECONDS;
        }

        let allow_heavy_during_drag = !is_dragging
            || self.editor.runtime.drag_heavy_rebuild_accumulator
                >= DRAG_HEAVY_REBUILD_INTERVAL_SECONDS;

        let mut dirty = pending;
        if is_dragging && !allow_heavy_during_drag {
            dirty.sync_game_objects = false;
            dirty.rebuild_block_mesh = false;
        }

        if !dirty.any() {
            return;
        }

        if dirty.sync_game_objects {
            self.editor.runtime.dirty.sync_game_objects = false;
            self.perf_record(PerfStage::DirtySyncGameObjects, PlatformInstant::now());
        }
        if dirty.rebuild_block_mesh {
            self.editor.runtime.dirty.rebuild_block_mesh = false;
        }
        if dirty.rebuild_selection_overlays {
            self.editor.runtime.dirty.rebuild_selection_overlays = false;
        }
        if dirty.rebuild_tap_indicators {
            self.editor.runtime.dirty.rebuild_tap_indicators = false;
        }
        if dirty.rebuild_preview_player {
            self.editor.runtime.dirty.rebuild_preview_player = false;
        }
        if dirty.rebuild_cursor {
            self.editor.runtime.dirty.rebuild_cursor = false;
        }

        if dirty.rebuild_block_mesh {
            let block_mesh_started_at = PlatformInstant::now();
            if self.phase == AppPhase::Editor && is_dragging {
                self.rebuild_editor_selected_block_vertices();
            } else {
                self.rebuild_block_vertices();
            }
            self.perf_record(PerfStage::DirtyRebuildBlockMesh, block_mesh_started_at);
        }

        if dirty.rebuild_selection_overlays {
            let overlays_started_at = PlatformInstant::now();
            let gizmo_started_at = PlatformInstant::now();
            self.rebuild_editor_gizmo_vertices();
            self.perf_record(PerfStage::DirtySelectionOverlayGizmo, gizmo_started_at);

            let camera_triggers_started_at = PlatformInstant::now();
            self.rebuild_camera_trigger_marker_vertices();
            self.perf_record(
                PerfStage::DirtySelectionOverlayCameraTriggers,
                camera_triggers_started_at,
            );

            let hover_started_at = PlatformInstant::now();
            self.rebuild_editor_hover_outline_vertices();
            self.perf_record(PerfStage::DirtySelectionOverlayHover, hover_started_at);

            let outline_started_at = PlatformInstant::now();
            self.rebuild_editor_selection_outline_vertices();
            self.perf_record(
                PerfStage::DirtySelectionOverlaySelection,
                outline_started_at,
            );

            self.perf_record(
                PerfStage::DirtyRebuildSelectionOverlays,
                overlays_started_at,
            );
        }

        if dirty.rebuild_tap_indicators {
            let tap_indicators_started_at = PlatformInstant::now();
            self.rebuild_tap_indicator_vertices();
            self.perf_record(
                PerfStage::DirtyRebuildTapIndicators,
                tap_indicators_started_at,
            );
        }

        if dirty.rebuild_preview_player {
            let preview_started_at = PlatformInstant::now();
            self.rebuild_editor_preview_player_vertices();
            self.perf_record(PerfStage::DirtyRebuildPreviewPlayer, preview_started_at);
        }

        if dirty.rebuild_cursor {
            let cursor_started_at = PlatformInstant::now();
            self.rebuild_editor_cursor_vertices();
            self.perf_record(PerfStage::DirtyRebuildCursor, cursor_started_at);
        }

        if dirty.sync_game_objects || dirty.rebuild_block_mesh {
            self.editor.runtime.drag_heavy_rebuild_accumulator = 0.0;
        }
    }

    pub(super) fn place_editor_block(&mut self) {
        self.editor.add_block_at_cursor();
        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn sync_editor_objects(&mut self) {
        self.editor.sync_objects();
    }

    pub(super) fn sync_editor_objects_after_drag_release(&mut self) {
        self.editor.sync_objects_after_drag_release();
    }

    pub(super) fn apply_spawn_to_game(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
        speed: Option<f32>,
    ) {
        self.gameplay.state.apply_spawn(position, direction);
        if let Some(speed) = speed {
            self.gameplay.state.speed = speed;
        }
    }

    pub(super) fn apply_spawn_exact_to_game(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
        speed: Option<f32>,
    ) {
        self.gameplay.state.apply_spawn_exact(position, direction);
        if let Some(speed) = speed {
            self.gameplay.state.speed = speed;
        }
    }

    pub(super) fn editor_timeline_elapsed_seconds(&self, time_seconds: f32) -> f32 {
        self.editor.timeline_elapsed_seconds(time_seconds)
    }

    pub(super) fn apply_editor_timeline_preview_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        self.editor.timeline.preview.position = position;
        self.editor.timeline.preview.direction = direction;

        if !self.editor.timeline.playback.playing {
            self.editor.ui.cursor = [
                position[0].round(),
                position[1].round(),
                position[2].round(),
            ];
            self.editor.ui.cursor[1] = self.editor.ui.cursor[1].max(0.0);

            self.rebuild_editor_cursor_vertices();
        }

        self.editor.camera.editor_pan[0] = position[0] + 0.5;
        self.editor.camera.editor_pan[1] = position[2] + 0.5;
        self.editor.camera.editor_target_z = position[1];

        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    pub(super) fn rebuild_editor_cursor_vertices(&mut self) {
        let vertices = build_editor_cursor_vertices(self.editor.ui.cursor);
        self.render.meshes.editor_cursor.replace_with_vertices(
            &self.render.gpu.device,
            "Editor Cursor Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_hover_outline_vertices(&mut self) {
        let total_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor || !self.editor.ui.mode.is_selection_mode() {
            self.render.meshes.editor_hover_outline.clear();
            self.perf_record(PerfStage::HoverOutlineTotal, total_started_at);
            return;
        }

        let object_count = self.editor.objects.len();
        let mut indices_to_outline = Vec::new();
        let hovered_outlined = self
            .editor
            .ui
            .hovered_block_index
            .filter(|index| *index < self.editor.objects.len())
            .filter(|index| !self.editor.selection_contains(*index));

        if let Some(index) = hovered_outlined {
            indices_to_outline.push(index);
        }

        let marquee_active = self
            .editor
            .marquee_selection_rect_screen()
            .is_some_and(|(_, _, active)| active);

        if marquee_active {
            let marquee_hits_started_at = PlatformInstant::now();
            let selected_mask = self.editor.selected_mask_for_len(object_count);
            let viewport = glam::Vec2::new(
                self.render.gpu.config.width as f32,
                self.render.gpu.config.height as f32,
            );
            for hit in self.editor.marquee_overlapping_blocks(viewport) {
                if selected_mask[hit] || Some(hit) == hovered_outlined {
                    continue;
                }
                indices_to_outline.push(hit);
            }
            self.perf_record(PerfStage::HoverOutlineMarqueeHits, marquee_hits_started_at);
        }

        if indices_to_outline.is_empty() {
            self.render.meshes.editor_hover_outline.clear();
            self.perf_record(PerfStage::HoverOutlineTotal, total_started_at);
            return;
        }

        let target_pixels = if self.editor.ui.left_mouse_down {
            6.0
        } else {
            3.0
        };
        let line_width = indices_to_outline
            .first()
            .and_then(|index| self.editor.objects.get(*index))
            .map(|obj| {
                let center = glam::Vec3::new(
                    obj.position[0] + obj.size[0] * 0.5,
                    obj.position[1] + obj.size[1] * 0.5,
                    obj.position[2] + obj.size[2] * 0.5,
                );
                self.editor_gizmo_axis_width_world(center, target_pixels)
            })
            .unwrap_or(0.06);

        if marquee_active {
            let last_hover_ms =
                self.editor.perf.profiler.stats[PerfStage::HoverOutlineTotal.as_index()].last_ms;
            let budget = &mut self.editor.runtime.interaction.hover_detail_budget_blocks;
            if last_hover_ms > HOVER_DETAIL_TARGET_MS {
                *budget = budget
                    .saturating_sub(HOVER_DETAIL_BUDGET_STEP_BLOCKS)
                    .max(HOVER_DETAIL_BUDGET_MIN_BLOCKS);
            } else if last_hover_ms < HOVER_DETAIL_TARGET_MS * 0.55 {
                *budget =
                    (*budget + HOVER_DETAIL_BUDGET_STEP_BLOCKS).min(HOVER_DETAIL_BUDGET_MAX_BLOCKS);
            }
        }

        let detail_budget = if marquee_active {
            self.editor
                .runtime
                .interaction
                .hover_detail_budget_blocks
                .min(indices_to_outline.len())
        } else {
            indices_to_outline.len()
        };

        let vertex_build_started_at = PlatformInstant::now();
        let mut all_vertices = if marquee_active {
            Vec::with_capacity(
                indices_to_outline
                    .len()
                    .saturating_mul(FAST_HOVER_VERTICES_PER_BLOCK)
                    .saturating_add(detail_budget.saturating_mul(OUTLINE_VERTICES_PER_BLOCK)),
            )
        } else {
            Vec::with_capacity(
                indices_to_outline
                    .len()
                    .saturating_mul(OUTLINE_VERTICES_PER_BLOCK),
            )
        };

        if marquee_active {
            for &index in &indices_to_outline {
                let obj = &self.editor.objects[index];
                append_editor_hover_proxy_vertices(
                    &mut all_vertices,
                    obj.position,
                    obj.size,
                    line_width,
                );
            }

            let mut cursor =
                self.editor.runtime.interaction.hover_detail_cursor % indices_to_outline.len();
            let mut hovered_in_detail = false;
            for _ in 0..detail_budget {
                let index = indices_to_outline[cursor];
                let obj = &self.editor.objects[index];
                append_editor_hover_outline_vertices(
                    &mut all_vertices,
                    obj.position,
                    obj.size,
                    line_width,
                );
                hovered_in_detail |= Some(index) == hovered_outlined;
                cursor += 1;
                if cursor >= indices_to_outline.len() {
                    cursor = 0;
                }
            }

            if let Some(index) = hovered_outlined.filter(|_| !hovered_in_detail) {
                if let Some(obj) = self.editor.objects.get(index) {
                    append_editor_hover_outline_vertices(
                        &mut all_vertices,
                        obj.position,
                        obj.size,
                        line_width,
                    );
                }
            }

            self.editor.runtime.interaction.hover_detail_cursor = cursor;
        } else {
            self.editor.runtime.interaction.hover_detail_cursor = 0;
            for index in indices_to_outline {
                let obj = &self.editor.objects[index];
                append_editor_hover_outline_vertices(
                    &mut all_vertices,
                    obj.position,
                    obj.size,
                    line_width,
                );
            }
        }
        self.perf_record(PerfStage::HoverOutlineVertexBuild, vertex_build_started_at);

        let upload_started_at = PlatformInstant::now();
        self.render
            .meshes
            .editor_hover_outline
            .write_streaming_vertices(&self.render.gpu.queue, &all_vertices);
        self.perf_record(PerfStage::HoverOutlineUpload, upload_started_at);
        self.perf_record(PerfStage::HoverOutlineTotal, total_started_at);
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        let mode = self.editor.ui.mode;
        if self.phase != AppPhase::Editor || !mode.shows_gizmo() {
            self.render.meshes.editor_gizmo.clear();
            return;
        }

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            self.render.meshes.editor_gizmo.clear();
            return;
        };

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.editor_gizmo_axis_lengths_world(center, 100.0);
        let axis_width = self.editor_gizmo_axis_width_world(center, 4.5);
        let resize_radius = self.editor_gizmo_axis_width_world(center, 8.5);
        let resize_offsets = self.editor_gizmo_axis_lengths_world(center, 9.0);

        let dragged_part = self
            .editor
            .runtime
            .interaction
            .gizmo_drag
            .as_ref()
            .map(|drag| GizmoPart::from_axis_kind(drag.axis, drag.kind));

        let hovered_part = self
            .editor
            .runtime
            .interaction
            .hovered_gizmo
            .map(|(kind, axis)| GizmoPart::from_axis_kind(axis, kind));

        let rotation_degrees = self
            .editor
            .selected_indices_normalized()
            .first()
            .and_then(|&index| self.editor.objects.get(index))
            .map(|obj| obj.rotation_degrees)
            .unwrap_or([0.0, 0.0, 0.0]);

        let vertices = build_editor_gizmo_vertices(GizmoParams {
            position: bounds_position,
            size: bounds_size,
            rotation_degrees,
            axis_lengths,
            axis_width,
            resize_radius,
            resize_offsets,
            show_move_handles: mode.shows_move_gizmo(),
            show_scale_handles: mode.shows_scale_gizmo(),
            show_rotate_handles: mode.shows_rotate_gizmo(),
            hovered_part,
            dragged_part,
        });
        self.render.meshes.editor_gizmo.replace_with_vertices(
            &self.render.gpu.device,
            "Editor Gizmo Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        let total_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor || !self.editor.ui.mode.is_selection_mode() {
            self.render.meshes.editor_selection_outline.clear();
            self.perf_record(PerfStage::SelectionOutlineTotal, total_started_at);
            return;
        }

        let indices_started_at = PlatformInstant::now();
        let selected_indices = self.selected_block_indices_normalized();
        self.perf_record(PerfStage::SelectionOutlineIndices, indices_started_at);
        if selected_indices.is_empty() {
            self.render.meshes.editor_selection_outline.clear();
            self.perf_record(PerfStage::SelectionOutlineTotal, total_started_at);
            return;
        }

        let line_width = selected_indices
            .first()
            .and_then(|index| self.editor.objects.get(*index))
            .map(|obj| {
                let center = glam::Vec3::new(
                    obj.position[0] + obj.size[0] * 0.5,
                    obj.position[1] + obj.size[1] * 0.5,
                    obj.position[2] + obj.size[2] * 0.5,
                );
                self.editor_gizmo_axis_width_world(center, 2.0)
            })
            .unwrap_or(0.06);

        let vertex_build_started_at = PlatformInstant::now();
        let mut vertices = Vec::new();
        for index in selected_indices {
            if let Some(obj) = self.editor.objects.get(index) {
                vertices.extend(build_editor_selection_outline_vertices(
                    obj.position,
                    obj.size,
                    line_width,
                ));
            }
        }
        self.perf_record(
            PerfStage::SelectionOutlineVertexBuild,
            vertex_build_started_at,
        );

        let upload_started_at = PlatformInstant::now();
        self.render
            .meshes
            .editor_selection_outline
            .replace_with_vertices(
                &self.render.gpu.device,
                "Editor Selection Outline Vertex Buffer",
                &vertices,
            );
        self.perf_record(PerfStage::SelectionOutlineUpload, upload_started_at);
        self.perf_record(PerfStage::SelectionOutlineTotal, total_started_at);
    }

    pub(super) fn rebuild_spawn_marker_vertices(&mut self) {
        let vertices = build_spawn_marker_vertices(
            self.editor.spawn.position,
            matches!(self.editor.spawn.direction, SpawnDirection::Right),
        );
        self.render.meshes.spawn_marker.replace_with_vertices(
            &self.render.gpu.device,
            "Spawn Marker Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_camera_trigger_marker_vertices(&mut self) {
        if self.phase != AppPhase::Editor {
            self.render.meshes.camera_trigger_markers.clear();
            return;
        }

        let markers = self.editor.camera_trigger_markers();
        if markers.is_empty() {
            self.render.meshes.camera_trigger_markers.clear();
            return;
        }

        let camera_triggers = markers
            .iter()
            .map(|(_, camera_trigger)| camera_trigger.clone())
            .collect::<Vec<_>>();
        let selected_camera_trigger_index =
            self.editor
                .selected_trigger_index()
                .and_then(|selected_trigger_index| {
                    markers
                        .iter()
                        .position(|(trigger_index, _)| *trigger_index == selected_trigger_index)
                });

        let current_camera_eye = if self.editor_is_playing() {
            let (e, _) = self.editor_preview_camera_view();
            Some(Vec3::from_array(e))
        } else {
            let target = self.editor.editor_camera_target();
            let offset = self.editor_camera_offset();
            Some(target + offset)
        };

        let vertices = build_camera_trigger_marker_vertices(
            &camera_triggers,
            selected_camera_trigger_index,
            current_camera_eye,
        );
        self.render
            .meshes
            .camera_trigger_markers
            .replace_with_vertices(
                &self.render.gpu.device,
                "Camera Trigger Marker Vertex Buffer",
                &vertices,
            );
    }

    pub(super) fn rebuild_block_vertices(&mut self) {
        let perf_started_at = PlatformInstant::now();
        if self.phase == AppPhase::Editor {
            self.rebuild_editor_block_vertices_split();
        } else {
            let vertices = build_block_vertices(&self.gameplay.state.objects);
            self.render.meshes.blocks.replace_with_vertices(
                &self.render.gpu.device,
                "Block Vertex Buffer",
                &vertices,
            );
            self.render.meshes.blocks_static.clear();
            self.render.meshes.blocks_selected.clear();
        }
        self.perf_record(PerfStage::BlockMeshRebuild, perf_started_at);
    }

    fn rebuild_editor_block_vertices_split(&mut self) {
        let object_source = self
            .editor_runtime_objects_for_render()
            .unwrap_or_else(|| self.editor.objects.clone());

        let mask_build_started_at = PlatformInstant::now();
        let selected_indices = self.selected_block_indices_normalized();
        let mut selected_mask = vec![false; object_source.len()];
        for index in selected_indices {
            if index < selected_mask.len() {
                selected_mask[index] = true;
            }
        }
        self.perf_record(PerfStage::BlockMeshMaskBuild, mask_build_started_at);

        let static_mesh_started_at = PlatformInstant::now();
        let static_vertices = {
            let mut static_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if !selected_mask[index] {
                    static_objects.push(object);
                }
            }
            build_block_vertices_from_refs(static_objects)
        };

        let selected_mesh_started_at = PlatformInstant::now();
        let selected_vertices = {
            let mut selected_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if selected_mask[index] {
                    selected_objects.push(object);
                }
            }
            build_block_vertices_from_refs(selected_objects)
        };

        self.perf_record(PerfStage::BlockMeshSplitStatic, static_mesh_started_at);
        self.perf_record(PerfStage::BlockMeshSplitSelected, selected_mesh_started_at);

        let upload_static_started_at = PlatformInstant::now();
        self.render.meshes.blocks_static.replace_with_vertices(
            &self.render.gpu.device,
            "Block Static Vertex Buffer",
            &static_vertices,
        );
        self.perf_record(PerfStage::BlockMeshUploadStatic, upload_static_started_at);

        let upload_selected_started_at = PlatformInstant::now();
        self.render.meshes.blocks_selected.replace_with_vertices(
            &self.render.gpu.device,
            "Block Selected Vertex Buffer",
            &selected_vertices,
        );
        self.perf_record(
            PerfStage::BlockMeshUploadSelected,
            upload_selected_started_at,
        );
        self.render.meshes.blocks.clear();
    }

    fn rebuild_editor_selected_block_vertices(&mut self) {
        let object_source = self
            .editor_runtime_objects_for_render()
            .unwrap_or_else(|| self.editor.objects.clone());

        let selected_only_started_at = PlatformInstant::now();
        if self
            .editor
            .selected_mask_cache
            .as_ref()
            .is_none_or(|cache| cache.len() != object_source.len())
        {
            let selected_indices = self.selected_block_indices_normalized();
            let mut mask = vec![false; object_source.len()];
            for index in selected_indices {
                if index < mask.len() {
                    mask[index] = true;
                }
            }
            self.editor.selected_mask_cache = Some(mask);
        }

        let Some(selected_mask) = self.editor.selected_mask_cache.as_ref() else {
            self.render.meshes.blocks_selected.clear();
            self.perf_record(PerfStage::BlockMeshSelectedOnly, selected_only_started_at);
            return;
        };

        let selected_build_started_at = PlatformInstant::now();
        let selected_vertices = {
            let mut selected_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if selected_mask[index] {
                    selected_objects.push(object);
                }
            }

            build_block_vertices_from_refs(selected_objects)
        };
        self.perf_record(
            PerfStage::BlockMeshSelectedOnlyBuild,
            selected_build_started_at,
        );

        let selected_upload_started_at = PlatformInstant::now();
        self.render.meshes.blocks_selected.replace_with_vertices(
            &self.render.gpu.device,
            "Block Selected Vertex Buffer",
            &selected_vertices,
        );
        self.perf_record(
            PerfStage::BlockMeshSelectedOnlyUpload,
            selected_upload_started_at,
        );
        self.render.meshes.blocks.clear();
        self.perf_record(PerfStage::BlockMeshSelectedOnly, selected_only_started_at);
    }

    pub(super) fn rebuild_tap_indicator_vertices(&mut self) {
        let perf_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor {
            self.render.meshes.tap_indicators.clear();
            self.perf_record(PerfStage::TapIndicatorMeshRebuild, perf_started_at);
            return;
        }

        // Build unique sorted positions without a full clone when possible
        let positions = &self.editor.timeline.taps.tap_indicator_positions;
        let unique_positions: Vec<[f32; 3]> = if positions.len() <= 1 {
            positions.clone()
        } else {
            let mut sorted = positions.clone();
            sorted.sort_unstable_by(|a, b| {
                a[0].total_cmp(&b[0])
                    .then(a[1].total_cmp(&b[1]))
                    .then(a[2].total_cmp(&b[2]))
            });
            sorted.dedup();
            sorted
        };

        if unique_positions.is_empty() {
            self.render.meshes.tap_indicators.clear();
            self.perf_record(PerfStage::TapIndicatorMeshRebuild, perf_started_at);
            return;
        }

        let vertices = build_tap_indicator_vertices(&unique_positions);
        self.render.meshes.tap_indicators.replace_with_vertices(
            &self.render.gpu.device,
            "Tap Indicator Vertex Buffer",
            &vertices,
        );
        self.perf_record(PerfStage::TapIndicatorMeshRebuild, perf_started_at);
    }

    pub(super) fn rebuild_editor_preview_player_vertices(&mut self) {
        self.render.meshes.editor_preview_player.clear();
    }

    pub(super) fn rebuild_editor_preview_player_vertices_for_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        self.editor.timeline.preview.position = position;
        self.editor.timeline.preview.direction = direction;
        self.render.meshes.editor_preview_player.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::types::{
        AppPhase, EditorMode, LevelObject, TimedTrigger, TimedTriggerAction, TimedTriggerEasing,
        TimedTriggerTarget,
    };

    fn block(position: [f32; 3], size: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size,
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    fn object_move_trigger() -> TimedTrigger {
        TimedTrigger {
            time_seconds: 0.0,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Object { object_id: 0 },
            action: TimedTriggerAction::MoveTo {
                position: [2.0, 0.0, 0.0],
            },
        }
    }

    fn camera_trigger() -> TimedTrigger {
        TimedTrigger {
            time_seconds: 0.0,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Camera,
            action: TimedTriggerAction::CameraPose {
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [0.0, 0.0, 0.0],
                rotation: 0.0,
                pitch: 0.0,
            },
        }
    }

    #[test]
    fn topmost_block_index_at_cursor_prefers_highest_overlapping_block() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![
                block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                block([0.0, 1.0, 0.0], [1.0, 2.0, 1.0]),
                block([2.0, 0.0, 2.0], [1.0, 1.0, 1.0]),
            ];

            assert_eq!(
                state.editor.topmost_block_index_at_cursor([0.0, 0.0, 0.0]),
                Some(1)
            );
            assert_eq!(
                state.editor.topmost_block_index_at_cursor([4.0, 0.0, 4.0]),
                None
            );
        });
    }

    #[test]
    fn sync_objects_filters_invalid_selection_and_marks_full_dirty() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
            state.editor.ui.selected_block_index = Some(99);
            state.editor.ui.selected_block_indices = vec![0, 99];
            state.editor.ui.hovered_block_index = Some(42);
            state.editor.selected_mask_cache = Some(vec![true]);

            state.editor.sync_objects();

            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert_eq!(state.editor.ui.hovered_block_index, None);
            assert!(state.editor.runtime.dirty.sync_game_objects);
            assert!(state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
            assert!(state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(state.editor.runtime.dirty.rebuild_preview_player);
            assert!(state.editor.runtime.dirty.rebuild_cursor);
            assert!(state.editor.selected_mask_cache.is_none());
        });
    }

    #[test]
    fn sync_objects_for_drag_and_after_release_set_expected_dirty_flags() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();

            state.editor.sync_objects_for_drag();
            assert!(state.editor.runtime.dirty.sync_game_objects);
            assert!(state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
            assert!(!state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(!state.editor.runtime.dirty.rebuild_preview_player);
            assert!(!state.editor.runtime.dirty.rebuild_cursor);

            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.editor.sync_objects_after_drag_release();
            assert!(state.editor.runtime.dirty.sync_game_objects);
            assert!(!state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
            assert!(!state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(state.editor.runtime.dirty.rebuild_preview_player);
            assert!(state.editor.runtime.dirty.rebuild_cursor);
        });
    }

    #[test]
    fn editor_runtime_objects_for_render_requires_editor_playback_and_transform_triggers() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
            state.editor.set_triggers(vec![object_move_trigger()]);

            state.phase = AppPhase::Menu;
            assert!(state.editor_runtime_objects_for_render().is_none());

            state.phase = AppPhase::Editor;
            state.editor.timeline.playback.playing = false;
            assert!(state.editor_runtime_objects_for_render().is_none());

            state.editor.timeline.playback.playing = true;
            state.editor.ui.mode = EditorMode::Timing;
            assert!(state.editor_runtime_objects_for_render().is_none());

            state.editor.ui.mode = EditorMode::Place;
            let transformed = state
                .editor_runtime_objects_for_render()
                .expect("expected transformed render objects");
            assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn process_editor_dirty_handles_idle_drag_and_full_rebuild_paths() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];

            state.editor.runtime.drag_heavy_rebuild_accumulator = 3.0;
            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.process_editor_dirty(0.01);
            assert_eq!(state.editor.runtime.drag_heavy_rebuild_accumulator, 0.0);

            state.editor.runtime.interaction.block_drag = Some(super::super::EditorBlockDrag {
                start_mouse: [0.0, 0.0],
                start_center_screen: [0.0, 0.0],
                start_center_world: [0.0, 0.0, 0.0],
                start_blocks: Vec::new(),
            });
            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::from_object_sync();
            state.process_editor_dirty(0.001);
            assert!(state.editor.runtime.dirty.sync_game_objects);
            assert!(state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(!state.editor.runtime.dirty.rebuild_selection_overlays);
            assert!(!state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(!state.editor.runtime.dirty.rebuild_preview_player);
            assert!(!state.editor.runtime.dirty.rebuild_cursor);

            state.editor.runtime.interaction.block_drag = None;
            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::from_object_sync();
            state.process_editor_dirty(0.02);
            assert!(!state.editor.runtime.dirty.any());
        });
    }

    #[test]
    fn camera_trigger_and_tap_indicator_meshes_clear_or_build_by_phase_and_data() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Menu;
            state.rebuild_camera_trigger_marker_vertices();
            assert!(state
                .render
                .meshes
                .camera_trigger_markers
                .draw_data()
                .is_none());

            state.phase = AppPhase::Editor;
            state.editor.set_triggers(Vec::new());
            state.rebuild_camera_trigger_marker_vertices();
            assert!(state
                .render
                .meshes
                .camera_trigger_markers
                .draw_data()
                .is_none());

            state.editor.set_triggers(vec![camera_trigger()]);
            state.rebuild_camera_trigger_marker_vertices();
            assert!(state
                .render
                .meshes
                .camera_trigger_markers
                .draw_data()
                .is_some());

            state.phase = AppPhase::Menu;
            state.rebuild_tap_indicator_vertices();
            assert!(state.render.meshes.tap_indicators.draw_data().is_none());

            state.phase = AppPhase::Editor;
            state.editor.timeline.taps.tap_indicator_positions =
                vec![[1.0, 0.0, 1.0], [1.0, 0.0, 1.0]];
            state.rebuild_tap_indicator_vertices();
            assert!(state.render.meshes.tap_indicators.draw_data().is_some());
        });
    }

    #[test]
    fn rebuild_block_vertices_switches_between_editor_split_and_playing_mesh() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.objects = vec![
                block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                block([2.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.phase = AppPhase::Editor;
            state.rebuild_block_vertices();
            assert!(state.render.meshes.blocks.draw_data().is_none());
            assert!(state.render.meshes.blocks_selected.draw_data().is_some());
            assert!(state.render.meshes.blocks_static.draw_data().is_some());

            state.phase = AppPhase::Playing;
            state.gameplay.state.objects = state.editor.objects.clone();
            state.rebuild_block_vertices();
            assert!(state.render.meshes.blocks.draw_data().is_some());
            assert!(state.render.meshes.blocks_selected.draw_data().is_none());
            assert!(state.render.meshes.blocks_static.draw_data().is_none());
        });
    }

    #[test]
    fn apply_editor_timeline_preview_state_updates_cursor_only_when_not_playing() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.cursor = [0.0, 0.0, 0.0];

            state.editor.timeline.playback.playing = false;
            state.apply_editor_timeline_preview_state(
                [1.2, 2.1, 3.8],
                crate::types::SpawnDirection::Right,
            );
            assert_eq!(state.editor.ui.cursor, [1.0, 2.0, 4.0]);
            assert_eq!(state.editor.camera.editor_pan, [1.7, 4.3]);
            assert_eq!(state.editor.camera.editor_target_z, 2.1);

            state.editor.timeline.playback.playing = true;
            state.editor.ui.cursor = [9.0, 9.0, 9.0];
            state.apply_editor_timeline_preview_state(
                [4.2, 1.0, 2.2],
                crate::types::SpawnDirection::Forward,
            );
            assert_eq!(state.editor.ui.cursor, [9.0, 9.0, 9.0]);
            assert_eq!(state.editor.camera.editor_pan, [4.7, 2.7]);
            assert_eq!(state.editor.camera.editor_target_z, 1.0);
        });
    }
}
