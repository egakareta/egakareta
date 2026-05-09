/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::Vec3;

use super::{EditorDirtyFlags, EditorSubsystem, State};
use crate::editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds_with_triggers};
use crate::game::trigger_transformed_objects_at_time;
use crate::mesh::{
    block_affects_existing_lighting, build_block_vertices,
    build_block_vertices_for_object_with_lighting, build_block_vertices_from_refs,
    build_camera_trigger_marker_vertices, build_editor_cursor_vertices,
    build_editor_gizmo_vertices, build_editor_hover_outline_vertices,
    build_editor_selection_outline_vertices, build_spawn_marker_vertices,
    build_tap_indicator_vertices, GizmoParams,
};
use crate::types::{AppPhase, EditorMode, GizmoPart, LevelObject, SpawnDirection};

fn editor_static_mesh_spare_capacity(vertex_count: usize, object_count: usize) -> u32 {
    let growth_room = vertex_count / 8;
    let object_room = object_count.min(512).saturating_mul(36);
    growth_room.max(object_room).max(4096) as u32
}

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
        self.block_static_vertex_cache.clear();
        self.block_static_vertex_cache_complete_len = None;
        self.runtime.pending_block_mesh_appends.clear();
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
        self.mark_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_selection_overlays: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn add_block_at_cursor(&mut self) {
        let new_block = create_block_at_cursor(self.ui.cursor, &self.config.selected_block_id);
        let can_append_mesh = self.can_append_block_mesh_after_placement(&new_block);

        self.record_history_state();
        let placed_index = self.objects.len();
        self.objects.push(new_block);
        self.ui.selected_block_index = None;
        self.ui.selected_block_indices.clear();
        self.ui.hovered_block_index = None;
        if can_append_mesh {
            self.invalidate_samples();
            self.runtime.pending_block_mesh_appends.push(placed_index);
            self.mark_dirty(EditorDirtyFlags {
                sync_game_objects: true,
                append_block_mesh: true,
                ..EditorDirtyFlags::default()
            });
        } else {
            self.sync_objects();
        }
    }

    fn can_append_block_mesh_after_placement(&self, block: &LevelObject) -> bool {
        self.ui.selected_block_index.is_none()
            && self.ui.selected_block_indices.is_empty()
            && self.block_static_vertex_cache_complete_len == Some(self.objects.len())
            && !(self.timeline.playback.playing && self.has_object_transform_triggers())
            && !block_affects_existing_lighting(&block.block_id)
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
            puffin::profile_scope!("DirtySyncObjects");
            self.editor.runtime.dirty.sync_game_objects = false;
        }
        if dirty.rebuild_block_mesh {
            self.editor.runtime.dirty.rebuild_block_mesh = false;
        }
        if dirty.append_block_mesh {
            self.editor.runtime.dirty.append_block_mesh = false;
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
            puffin::profile_scope!("DirtyBlockMesh");
            self.editor.runtime.pending_block_mesh_appends.clear();
            if self.phase == AppPhase::Editor
                && is_dragging
                && self.editor.selected_mask_cache.is_some()
            {
                self.rebuild_editor_selected_block_vertices();
            } else {
                self.rebuild_block_vertices();
            }
        } else if dirty.append_block_mesh {
            puffin::profile_scope!("DirtyBlockMeshAppend");
            let pending_appends =
                std::mem::take(&mut self.editor.runtime.pending_block_mesh_appends);
            for index in pending_appends {
                if !self.append_editor_static_block_vertices(index) {
                    self.rebuild_block_vertices();
                    break;
                }
            }
        }

        if dirty.rebuild_selection_overlays {
            {
                puffin::profile_scope!("DirtySelectionGizmo");
                self.rebuild_editor_gizmo_vertices();
            }

            {
                puffin::profile_scope!("DirtyCameraTriggerMarkers");
                self.rebuild_camera_trigger_marker_vertices();
            }

            {
                puffin::profile_scope!("DirtyHoverOutline");
                self.rebuild_editor_hover_outline_vertices();
            }

            {
                puffin::profile_scope!("DirtySelectionOutline");
                self.rebuild_editor_selection_outline_vertices();
            }
        }

        if dirty.rebuild_tap_indicators {
            puffin::profile_scope!("DirtyTapIndicators");
            self.rebuild_tap_indicator_vertices();
        }

        if dirty.rebuild_preview_player {
            puffin::profile_scope!("DirtyPreviewPlayer");
            self.rebuild_editor_preview_player_vertices();
        }

        if dirty.rebuild_cursor {
            puffin::profile_scope!("DirtyCursor");
            self.rebuild_editor_cursor_vertices();
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
        if self.phase != AppPhase::Editor || !self.editor.ui.mode.is_selection_mode() {
            self.render.meshes.editor_hover_outline.clear();
            return;
        }

        let object_count = self.editor.objects.len();
        let selected_mask = self.editor.selected_mask_for_len(object_count);

        let mut indices_to_outline = Vec::new();
        let mut outline_mask = vec![false; object_count];

        if let Some(index) = self
            .editor
            .ui
            .hovered_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            if !selected_mask[index] {
                outline_mask[index] = true;
                indices_to_outline.push(index);
            }
        }

        if let Some((_, _, true)) = self.editor.marquee_selection_rect_screen() {
            let viewport = glam::Vec2::new(
                self.render.gpu.config.width as f32,
                self.render.gpu.config.height as f32,
            );
            for hit in self.editor.marquee_overlapping_blocks(viewport) {
                if !selected_mask[hit] && !outline_mask[hit] {
                    outline_mask[hit] = true;
                    indices_to_outline.push(hit);
                }
            }
        }

        if indices_to_outline.is_empty() {
            self.render.meshes.editor_hover_outline.clear();
            return;
        }

        let mut all_vertices = Vec::new();
        for index in indices_to_outline {
            let obj = &self.editor.objects[index];
            let center = glam::Vec3::new(
                obj.position[0] + obj.size[0] * 0.5,
                obj.position[1] + obj.size[1] * 0.5,
                obj.position[2] + obj.size[2] * 0.5,
            );
            let target_pixels = if self.editor.ui.left_mouse_down {
                6.0
            } else {
                3.0
            };
            let line_width = self.editor_gizmo_axis_width_world(center, target_pixels);
            all_vertices.append(&mut build_editor_hover_outline_vertices(
                obj.position,
                obj.size,
                line_width,
            ));
        }

        self.render
            .meshes
            .editor_hover_outline
            .replace_with_vertices(
                &self.render.gpu.device,
                "Editor Hover Outline Vertex Buffer",
                &all_vertices,
            );
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
        if self.phase != AppPhase::Editor || !self.editor.ui.mode.is_selection_mode() {
            self.render.meshes.editor_selection_outline.clear();
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            self.render.meshes.editor_selection_outline.clear();
            return;
        }

        let mut vertices = Vec::new();
        for index in selected_indices {
            if let Some(obj) = self.editor.objects.get(index) {
                let center = glam::Vec3::new(
                    obj.position[0] + obj.size[0] * 0.5,
                    obj.position[1] + obj.size[1] * 0.5,
                    obj.position[2] + obj.size[2] * 0.5,
                );
                let line_width = self.editor_gizmo_axis_width_world(center, 2.0);
                vertices.extend(build_editor_selection_outline_vertices(
                    obj.position,
                    obj.size,
                    line_width,
                ));
            }
        }
        self.render
            .meshes
            .editor_selection_outline
            .replace_with_vertices(
                &self.render.gpu.device,
                "Editor Selection Outline Vertex Buffer",
                &vertices,
            );
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
        puffin::profile_scope!("BlockMeshRebuild");
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
            self.editor.block_static_vertex_cache.clear();
            self.editor.block_static_vertex_cache_complete_len = None;
        }
    }

    fn rebuild_editor_block_vertices_split(&mut self) {
        let runtime_objects = self.editor_runtime_objects_for_render();
        let uses_runtime_objects = runtime_objects.is_some();
        let object_source = runtime_objects.unwrap_or_else(|| self.editor.objects.clone());

        let selected_mask = {
            puffin::profile_scope!("BlockMaskBuild");
            let selected_indices = self.selected_block_indices_normalized();
            let mut selected_mask = vec![false; object_source.len()];
            for index in selected_indices {
                if index < selected_mask.len() {
                    selected_mask[index] = true;
                }
            }
            selected_mask
        };

        let static_vertices = {
            puffin::profile_scope!("BlockMeshSplitStatic");
            let mut static_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if !selected_mask[index] {
                    static_objects.push(object);
                }
            }
            build_block_vertices_from_refs(&static_objects)
        };

        let selected_vertices = {
            puffin::profile_scope!("BlockMeshSplitSelected");
            let mut selected_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if selected_mask[index] {
                    selected_objects.push(object);
                }
            }
            build_block_vertices_from_refs(&selected_objects)
        };

        let has_selected_blocks = selected_mask.iter().any(|selected| *selected);

        self.editor.selected_mask_cache = Some(selected_mask);

        if !uses_runtime_objects && !has_selected_blocks {
            puffin::profile_scope!("BlockMeshUploadStatic");
            self.editor.block_static_vertex_cache = static_vertices;
            self.editor.block_static_vertex_cache_complete_len = Some(object_source.len());
            let spare_capacity = editor_static_mesh_spare_capacity(
                self.editor.block_static_vertex_cache.len(),
                object_source.len(),
            );
            self.render
                .meshes
                .blocks_static
                .replace_with_streaming_vertices(
                    &self.render.gpu.device,
                    &self.render.gpu.queue,
                    "Block Static Vertex Buffer",
                    &self.editor.block_static_vertex_cache,
                    spare_capacity,
                );
        } else {
            puffin::profile_scope!("BlockMeshUploadStatic");
            self.editor.block_static_vertex_cache.clear();
            self.editor.block_static_vertex_cache_complete_len = None;
            self.render.meshes.blocks_static.replace_with_vertices(
                &self.render.gpu.device,
                "Block Static Vertex Buffer",
                &static_vertices,
            );
        }

        {
            puffin::profile_scope!("BlockMeshUploadSelected");
            self.render.meshes.blocks_selected.replace_with_vertices(
                &self.render.gpu.device,
                "Block Selected Vertex Buffer",
                &selected_vertices,
            );
        }
        self.render.meshes.blocks.clear();
    }

    fn append_editor_static_block_vertices(&mut self, index: usize) -> bool {
        if self.phase != AppPhase::Editor
            || self.editor.block_static_vertex_cache_complete_len != Some(index)
            || index >= self.editor.objects.len()
        {
            return false;
        }

        let object = &self.editor.objects[index];
        if block_affects_existing_lighting(&object.block_id) {
            return false;
        }

        let appended_vertices =
            build_block_vertices_for_object_with_lighting(object, &self.editor.objects);
        let previous_vertex_count = self.editor.block_static_vertex_cache.len();
        let appended = self.render.meshes.blocks_static.append_streaming_vertices(
            &self.render.gpu.queue,
            previous_vertex_count,
            &appended_vertices,
        );

        self.editor
            .block_static_vertex_cache
            .extend(appended_vertices.iter().copied());

        if !appended {
            let spare_capacity = editor_static_mesh_spare_capacity(
                self.editor.block_static_vertex_cache.len(),
                self.editor.objects.len(),
            );
            self.render
                .meshes
                .blocks_static
                .replace_with_streaming_vertices(
                    &self.render.gpu.device,
                    &self.render.gpu.queue,
                    "Block Static Vertex Buffer",
                    &self.editor.block_static_vertex_cache,
                    spare_capacity,
                );
        }

        self.editor.block_static_vertex_cache_complete_len = Some(index + 1);
        if let Some(selected_mask) = self.editor.selected_mask_cache.as_mut() {
            if selected_mask.len() == index {
                selected_mask.push(false);
            } else {
                self.editor.selected_mask_cache = Some(vec![false; self.editor.objects.len()]);
            }
        } else {
            self.editor.selected_mask_cache = Some(vec![false; self.editor.objects.len()]);
        }
        self.render.meshes.blocks_selected.clear();
        self.render.meshes.blocks.clear();
        true
    }

    fn rebuild_editor_selected_block_vertices(&mut self) {
        puffin::profile_scope!("BlockMeshSelectedOnly");
        let object_source = self
            .editor_runtime_objects_for_render()
            .unwrap_or_else(|| self.editor.objects.clone());

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
            return;
        };

        let selected_vertices = {
            puffin::profile_scope!("SelectedOnlyBuild");
            let mut selected_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if selected_mask[index] {
                    selected_objects.push(object);
                }
            }

            build_block_vertices_from_refs(&selected_objects)
        };

        {
            puffin::profile_scope!("SelectedOnlyUpload");
            self.render.meshes.blocks_selected.replace_with_vertices(
                &self.render.gpu.device,
                "Block Selected Vertex Buffer",
                &selected_vertices,
            );
        }
        self.render.meshes.blocks.clear();
    }

    pub(super) fn rebuild_tap_indicator_vertices(&mut self) {
        puffin::profile_scope!("TapIndicatorMesh");
        if self.phase != AppPhase::Editor {
            self.render.meshes.tap_indicators.clear();
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
            return;
        }

        let vertices = build_tap_indicator_vertices(&unique_positions);
        self.render.meshes.tap_indicators.replace_with_vertices(
            &self.render.gpu.device,
            "Tap Indicator Vertex Buffer",
            &vertices,
        );
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
    fn placing_plain_block_appends_static_mesh_after_complete_rebuild() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
            state.editor.ui.cursor = [2.0, 0.0, 0.0];

            state.rebuild_block_vertices();
            let before_count = state
                .render
                .meshes
                .blocks_static
                .draw_data()
                .map(|(_, count)| count)
                .unwrap_or_default();
            assert_eq!(state.editor.block_static_vertex_cache_complete_len, Some(1));

            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.editor.add_block_at_cursor();

            assert!(state.editor.runtime.dirty.sync_game_objects);
            assert!(state.editor.runtime.dirty.append_block_mesh);
            assert!(!state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(!state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(!state.editor.runtime.dirty.rebuild_preview_player);

            state.process_editor_dirty(0.02);

            let after_count = state
                .render
                .meshes
                .blocks_static
                .draw_data()
                .map(|(_, count)| count)
                .unwrap_or_default();
            assert!(after_count > before_count);
            assert_eq!(state.editor.block_static_vertex_cache_complete_len, Some(2));
            assert!(!state.editor.runtime.dirty.any());
        });
    }

    #[test]
    fn placing_torch_uses_full_rebuild_because_lighting_changes_existing_blocks() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
            state.editor.ui.cursor = [2.0, 0.0, 0.0];
            state.editor.config.selected_block_id = "core/torch".to_string();

            state.rebuild_block_vertices();
            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.editor.add_block_at_cursor();

            assert!(state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(!state.editor.runtime.dirty.append_block_mesh);
            assert!(state.editor.runtime.pending_block_mesh_appends.is_empty());
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
