use glam::Vec3;

use super::{EditorDirtyFlags, EditorSubsystem, GizmoAxis, GizmoDragKind, PerfStage, State};
use crate::editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds};
use crate::mesh::{
    build_block_vertices, build_block_vertices_from_refs, build_editor_cursor_vertices,
    build_editor_gizmo_vertices, build_editor_hover_outline_vertices,
    build_editor_preview_player_vertices, build_editor_selection_outline_vertices,
    build_spawn_marker_vertices, build_tap_indicator_vertices, GizmoPart,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::{AppPhase, EditorMode, SpawnDirection};

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
        derive_timeline_elapsed_seconds(
            self.spawn.position,
            self.spawn.direction,
            &self.timeline.taps.tap_times,
            time_seconds,
            &self.objects,
        )
    }

    pub(crate) fn topmost_block_index_at_cursor(&self, cursor: [f32; 3]) -> Option<usize> {
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.objects.iter().enumerate() {
            let occupies_x = cursor[0] + 0.5 >= obj.position[0]
                && cursor[0] + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_y = cursor[1] + 0.5 >= obj.position[1]
                && cursor[1] + 0.5 <= obj.position[1] + obj.size[1];
            if occupies_x && occupies_y {
                let top = obj.position[2] + obj.size[2];
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
            let gizmo_started_at = PlatformInstant::now();
            self.rebuild_editor_gizmo_vertices();
            self.perf_record(PerfStage::DirtyRebuildSelectionOverlays, gizmo_started_at);

            let hover_started_at = PlatformInstant::now();
            self.rebuild_editor_hover_outline_vertices();
            self.perf_record(PerfStage::DirtyRebuildSelectionOverlays, hover_started_at);

            let outline_started_at = PlatformInstant::now();
            self.rebuild_editor_selection_outline_vertices();
            self.perf_record(PerfStage::DirtyRebuildSelectionOverlays, outline_started_at);
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

    pub(super) fn apply_spawn_to_game(&mut self, position: [f32; 3], direction: SpawnDirection) {
        self.gameplay.state.apply_spawn(position, direction);
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

        let bounds = self.editor.ui.bounds as f32;
        if !self.editor.timeline.playback.playing {
            self.editor.ui.cursor = [
                position[0].round(),
                position[1].round(),
                position[2].round(),
            ];
            self.editor.ui.cursor[0] = self.editor.ui.cursor[0].clamp(-bounds, bounds);
            self.editor.ui.cursor[1] = self.editor.ui.cursor[1].clamp(-bounds, bounds);
            self.editor.ui.cursor[2] = self.editor.ui.cursor[2].max(0.0);

            self.rebuild_editor_cursor_vertices();
        }

        let max_pan = bounds;
        self.editor.camera.editor_pan[0] = (position[0] + 0.5).clamp(-max_pan, max_pan);
        self.editor.camera.editor_pan[1] = (position[1] + 0.5).clamp(-max_pan, max_pan);

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
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Select {
            self.render.meshes.editor_hover_outline.clear();
            return;
        }

        let Some(index) = self
            .editor
            .ui
            .hovered_block_index
            .filter(|index| *index < self.editor.objects.len())
        else {
            self.render.meshes.editor_hover_outline.clear();
            return;
        };

        if self.selection_contains(index) {
            self.render.meshes.editor_hover_outline.clear();
            return;
        }

        let obj = &self.editor.objects[index];
        let vertices = build_editor_hover_outline_vertices(obj.position, obj.size);
        self.render
            .meshes
            .editor_hover_outline
            .replace_with_vertices(
                &self.render.gpu.device,
                "Editor Hover Outline Vertex Buffer",
                &vertices,
            );
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Select {
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
        let axis_lengths = self.editor_gizmo_axis_lengths_world(center, 50.0);
        let axis_width = self.editor_gizmo_axis_width_world(center, 3.0);

        let active_part = if let Some(drag) = &self.editor.runtime.interaction.gizmo_drag {
            match (drag.axis, drag.kind) {
                (GizmoAxis::X, GizmoDragKind::Move) => Some(GizmoPart::MoveX),
                (GizmoAxis::Y, GizmoDragKind::Move) => Some(GizmoPart::MoveY),
                (GizmoAxis::Z, GizmoDragKind::Move) => Some(GizmoPart::MoveZ),
                (GizmoAxis::XNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveXNeg),
                (GizmoAxis::YNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveYNeg),
                (GizmoAxis::ZNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveZNeg),
                (GizmoAxis::X, GizmoDragKind::Resize) => Some(GizmoPart::ResizeX),
                (GizmoAxis::Y, GizmoDragKind::Resize) => Some(GizmoPart::ResizeY),
                (GizmoAxis::Z, GizmoDragKind::Resize) => Some(GizmoPart::ResizeZ),
                (GizmoAxis::XNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeXNeg),
                (GizmoAxis::YNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeYNeg),
                (GizmoAxis::ZNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeZNeg),
            }
        } else {
            None
        };

        let vertices = build_editor_gizmo_vertices(
            bounds_position,
            bounds_size,
            axis_lengths,
            axis_width,
            active_part,
        );
        self.render.meshes.editor_gizmo.replace_with_vertices(
            &self.render.gpu.device,
            "Editor Gizmo Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor.ui.mode != EditorMode::Select {
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
                vertices.extend(build_editor_selection_outline_vertices(
                    obj.position,
                    obj.size,
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
        let mask_build_started_at = PlatformInstant::now();
        let selected_indices = self.selected_block_indices_normalized();
        let mut selected_mask = vec![false; self.editor.objects.len()];
        for index in selected_indices {
            if index < selected_mask.len() {
                selected_mask[index] = true;
            }
        }
        self.perf_record(PerfStage::BlockMeshMaskBuild, mask_build_started_at);

        let static_mesh_started_at = PlatformInstant::now();
        let static_vertices = {
            let mut static_objects = Vec::new();
            for (index, object) in self.editor.objects.iter().enumerate() {
                if !selected_mask[index] {
                    static_objects.push(object);
                }
            }
            build_block_vertices_from_refs(static_objects)
        };

        let selected_mesh_started_at = PlatformInstant::now();
        let selected_vertices = {
            let mut selected_objects = Vec::new();
            for (index, object) in self.editor.objects.iter().enumerate() {
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
        let selected_only_started_at = PlatformInstant::now();
        if self
            .editor
            .selected_mask_cache
            .as_ref()
            .is_none_or(|cache| cache.len() != self.editor.objects.len())
        {
            let selected_indices = self.selected_block_indices_normalized();
            let mut mask = vec![false; self.editor.objects.len()];
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
            for (index, object) in self.editor.objects.iter().enumerate() {
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
        if self.phase != AppPhase::Editor {
            self.render.meshes.editor_preview_player.clear();
            return;
        }

        let position = self.editor.timeline.preview.position;
        let direction = self.editor.timeline.preview.direction;
        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    pub(super) fn rebuild_editor_preview_player_vertices_for_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        let mesh_started_at = PlatformInstant::now();
        self.editor.timeline.preview.position = position;
        self.editor.timeline.preview.direction = direction;

        let tap_times = &self.editor.timeline.taps.tap_times;
        let current_time = self.editor.timeline.clock.time_seconds;
        let is_tapping = if tap_times.is_empty() {
            false
        } else {
            let idx = tap_times.partition_point(|t| *t < current_time - 0.01);
            idx < tap_times.len() && (tap_times[idx] - current_time).abs() <= 0.01
        };
        let preview_origin = [position[0] - 0.5, position[1] - 0.5, position[2]];
        let vertices = build_editor_preview_player_vertices(preview_origin, direction, is_tapping);
        self.render
            .meshes
            .editor_preview_player
            .replace_with_vertices(
                &self.render.gpu.device,
                "Editor Preview Player Vertex Buffer",
                &vertices,
            );
        self.perf_record(PerfStage::PreviewMeshBuild, mesh_started_at);
    }
}
