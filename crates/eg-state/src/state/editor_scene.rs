/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::borrow::Cow;

use glam::Vec3;

use super::{EditorDirtyFlags, EditorSubsystem, State};
use eg_blocks::{BlockDefinition, BlockRenderProfile};
use eg_editor_domain::tap_time_is_timing_division;
use eg_editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds};
use eg_game::trigger_transformed_objects_at_time;
use eg_mesh::{
    GizmoParams, GizmoPart, MeshGeometry, PracticeCheckpointFlagInstance, TransformTriggerMarker,
    Vertex, build_block_geometry_for_object, build_block_geometry_from_refs,
    build_block_geometry_from_refs_at_time, build_camera_arrow_vertices, build_chunked_block_mesh,
    build_chunked_block_mesh_at_time, build_chunked_block_mesh_from_refs,
    build_chunked_block_mesh_from_refs_at_time, build_colored_tap_indicator_vertices,
    build_editor_cursor_vertices, build_editor_gizmo_vertices,
    build_editor_hitbox_visualization_vertices, build_editor_hover_outline_vertices,
    build_editor_selection_outline_vertices, build_editor_tap_cursor_vertices,
    build_editor_transform_trigger_target_outline_vertices,
    build_practice_checkpoint_flag_geometry, build_spawn_marker_vertices,
    build_tap_division_preview_vertices, build_tap_division_tap_marker_vertices,
    build_transform_trigger_marker_vertices,
};
use eg_render::EditorOutlineInstance;
use eg_types::{
    AppPhase, Direction, EditorMode, EditorPlaceMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
    TimingPoint, TriggerTarget,
};

const SIMPLE_SELECTION_OUTLINE_BLOCK_THRESHOLD: usize = 700;
const MAX_EXPANDED_SELECTED_BLOCK_OBJECTS: usize = 100_000;
const EDITOR_SELECTION_OUTLINE_COLOR: [f32; 4] = [0.098, 0.6, 1.0, 1.0];
const EDITOR_HOVER_OUTLINE_COLOR: [f32; 4] = [0.698, 0.898, 1.0, 1.0];

#[derive(Clone, Copy)]
enum EditorObjectOutlineStyle {
    Selection,
    Hover,
}

impl EditorObjectOutlineStyle {
    fn color(self) -> [f32; 4] {
        match self {
            Self::Selection => EDITOR_SELECTION_OUTLINE_COLOR,
            Self::Hover => EDITOR_HOVER_OUTLINE_COLOR,
        }
    }
}

fn is_camera_trigger_block(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile == BlockRenderProfile::CameraTrigger
}

fn build_camera_trigger_visual_outline_vertices(
    object: &LevelObject,
    color: [f32; 4],
    line_width: f32,
) -> Vec<Vertex> {
    let mut vertices = build_block_geometry_for_object(object).to_triangle_vertices();
    let outline_width_pixels = (line_width * 1.35).max(1.0);
    for vertex in &mut vertices {
        vertex.color = color;
        vertex.color_outline = [
            object.position[0],
            object.position[1],
            object.position[2],
            outline_width_pixels,
        ];
        vertex.render_profile = 3.0;
    }
    vertices
}

fn build_editor_object_outline_vertices(
    object: &LevelObject,
    style: EditorObjectOutlineStyle,
    line_width: f32,
) -> Vec<Vertex> {
    if is_camera_trigger_block(object) {
        return build_camera_trigger_visual_outline_vertices(object, style.color(), line_width);
    }

    match style {
        EditorObjectOutlineStyle::Selection => build_editor_selection_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            line_width,
        ),
        EditorObjectOutlineStyle::Hover => build_editor_hover_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            line_width,
        ),
    }
}

fn append_outline_instance(
    mask_vertices: &mut Vec<Vertex>,
    outline_vertices: &mut Vec<Vertex>,
    instances: &mut Vec<EditorOutlineInstance>,
    object: &LevelObject,
    outline_vertices_for_object: Vec<Vertex>,
) {
    let mask_start = mask_vertices.len() as u32;
    mask_vertices.extend(build_block_geometry_for_object(object).to_triangle_vertices());
    let mask_end = mask_vertices.len() as u32;

    let outline_start = outline_vertices.len() as u32;
    outline_vertices.extend(outline_vertices_for_object);
    let outline_end = outline_vertices.len() as u32;

    instances.push(EditorOutlineInstance {
        mask_vertices: mask_start..mask_end,
        outline_vertices: outline_start..outline_end,
    });
}

fn transform_trigger_target_indices_for_block(
    objects: &[LevelObject],
    trigger_block_index: usize,
) -> Vec<usize> {
    let Some(trigger) = objects
        .get(trigger_block_index)
        .and_then(|object| object.trigger.as_ref())
    else {
        return Vec::new();
    };

    if !trigger.transform {
        return Vec::new();
    }

    let TriggerTarget::Objects { object_ids } = &trigger.target else {
        return Vec::new();
    };

    let mut target_indices = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| object_ids.contains(&object.id).then_some(index))
        .collect::<Vec<_>>();
    target_indices.sort_unstable();
    target_indices.dedup();
    target_indices
}

fn append_transform_trigger_target_outline_instance(
    mask_vertices: &mut Vec<Vertex>,
    outline_vertices: &mut Vec<Vertex>,
    instances: &mut Vec<EditorOutlineInstance>,
    object: &LevelObject,
) {
    append_outline_instance(
        mask_vertices,
        outline_vertices,
        instances,
        object,
        build_editor_transform_trigger_target_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            2.5,
        ),
    );
}

fn append_transform_trigger_target_outline_instances(
    objects: &[LevelObject],
    trigger_block_indices: impl IntoIterator<Item = usize>,
    mask_vertices: &mut Vec<Vertex>,
    outline_vertices: &mut Vec<Vertex>,
    instances: &mut Vec<EditorOutlineInstance>,
) {
    let mut target_indices = trigger_block_indices
        .into_iter()
        .flat_map(|index| transform_trigger_target_indices_for_block(objects, index))
        .collect::<Vec<_>>();
    target_indices.sort_unstable();
    target_indices.dedup();

    for target_index in target_indices {
        if let Some(object) = objects.get(target_index) {
            append_transform_trigger_target_outline_instance(
                mask_vertices,
                outline_vertices,
                instances,
                object,
            );
        }
    }
}

fn append_hovered_transform_trigger_target_outlines(
    objects: &[LevelObject],
    hovered_block_index: Option<usize>,
    stencil_geometry: &mut MeshGeometry,
    outline_vertices: &mut Vec<Vertex>,
) {
    let mut target_indices = hovered_block_index
        .into_iter()
        .flat_map(|index| transform_trigger_target_indices_for_block(objects, index))
        .collect::<Vec<_>>();
    target_indices.sort_unstable();
    target_indices.dedup();

    for target_index in target_indices {
        let Some(object) = objects.get(target_index) else {
            continue;
        };
        stencil_geometry.append_geometry(build_block_geometry_for_object(object));
        outline_vertices.extend(build_editor_transform_trigger_target_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            2.5,
        ));
    }
}

fn transform_trigger_markers_for_triggers(
    objects: &[LevelObject],
    trigger_objects: &[LevelObject],
    selected_trigger_index: Option<usize>,
) -> Vec<TransformTriggerMarker> {
    let mut markers = Vec::new();

    for (trigger_index, trigger_object) in trigger_objects.iter().enumerate() {
        let Some(trigger) = trigger_object.trigger.as_ref() else {
            continue;
        };
        let TriggerTarget::Objects { object_ids } = &trigger.target else {
            continue;
        };
        if !trigger.transform {
            continue;
        }

        let position = trigger_object.position;
        let rotation = trigger_object.rotation;
        let size = trigger_object.size;

        let source_objects = transform_trigger_marker_source_objects(
            objects,
            trigger_objects,
            trigger_index,
            trigger.time_seconds,
        );

        if object_ids.is_empty() {
            markers.push(TransformTriggerMarker {
                source_position: None,
                source_size: None,
                target_position: position,
                target_rotation_degrees: rotation,
                target_size: size,
                time_seconds: trigger.time_seconds,
                duration_seconds: trigger.duration_seconds,
                is_selected: selected_trigger_index == Some(trigger_index),
            });
        } else {
            for object_id in object_ids {
                let Some(source_object) =
                    source_objects.iter().find(|object| object.id == *object_id)
                else {
                    continue;
                };

                markers.push(TransformTriggerMarker {
                    source_position: Some(source_object.position),
                    source_size: Some(source_object.size),
                    target_position: position,
                    target_rotation_degrees: rotation,
                    target_size: size,
                    time_seconds: trigger.time_seconds,
                    duration_seconds: trigger.duration_seconds,
                    is_selected: selected_trigger_index == Some(trigger_index),
                });
            }
        }
    }

    markers
}

fn transform_trigger_marker_source_objects<'a>(
    objects: &'a [LevelObject],
    trigger_objects: &[LevelObject],
    trigger_index: usize,
    time_seconds: f32,
) -> Cow<'a, [LevelObject]> {
    if trigger_index == 0 {
        Cow::Borrowed(objects)
    } else {
        Cow::Owned(trigger_transformed_objects_at_time(
            objects,
            &trigger_objects[..trigger_index],
            time_seconds,
        ))
    }
}

impl EditorSubsystem {
    pub fn mark_dirty(&mut self, dirty: EditorDirtyFlags) {
        self.runtime.dirty.merge(dirty);
    }

    pub fn sync_objects(&mut self) {
        self.normalize_block_selection();
        self.sync_trigger_selection_from_objects();
        self.invalidate_samples();
        self.pick_spatial_index.invalidate();
        self.selected_mask_cache = None;
        self.block_static_vertex_cache.clear();
        self.block_static_vertex_cache_complete_len = None;
        self.runtime.pending_block_mesh_appends.clear();
        self.mark_dirty(EditorDirtyFlags::from_object_sync());
    }

    pub fn sync_objects_for_drag(&mut self) {
        self.normalize_block_selection();
        self.pick_spatial_index.invalidate();
        self.mark_dirty(EditorDirtyFlags::drag_object_sync());
    }

    pub fn sync_objects_after_drag_release(&mut self) {
        self.normalize_block_selection();
        self.invalidate_samples();
        self.pick_spatial_index.invalidate();
        self.mark_dirty(EditorDirtyFlags::drag_release_sync());
    }

    pub fn selected_block_default_size(&self) -> [f32; 3] {
        BlockDefinition::get(&self.config.selected_block_id)
            .render
            .default_size
    }

    pub fn add_block_at_cursor(&mut self, place_mode: EditorPlaceMode) -> usize {
        let new_block = create_block_at_cursor(
            self.input.cursor,
            &self.config.selected_block_id,
            self.selected_block_default_size(),
            self.config.selected_block_rotation_degrees,
        )
        .with_generated_id(&self.objects);
        let placed_transform_trigger = new_block.block_id == TRANSFORM_TRIGGER_BLOCK_ID;
        let can_append_mesh = self.can_append_block_mesh_after_placement();

        self.record_history_state();
        let placed_index = self.objects.len();
        self.objects.push(new_block);
        self.pick_spatial_index.invalidate();
        if place_mode == EditorPlaceMode::SelectPlaced {
            self.replace_block_selection(vec![placed_index]);
            self.input.hovered_block_index = Some(placed_index);
        } else {
            self.clear_block_selection();
        }
        if can_append_mesh {
            self.invalidate_samples();
            self.runtime.pending_block_mesh_appends.push(placed_index);
            self.mark_dirty(EditorDirtyFlags::appended_block(
                place_mode == EditorPlaceMode::SelectPlaced,
                placed_transform_trigger,
            ));
        } else {
            self.sync_objects();
            if placed_transform_trigger {
                self.mark_dirty(EditorDirtyFlags::trigger_markers_changed());
            }
        }
        placed_index
    }

    fn can_append_block_mesh_after_placement(&self) -> bool {
        self.input.selected_block_index.is_none()
            && self.input.selected_block_indices.is_empty()
            && self.block_static_vertex_cache_complete_len == Some(self.objects.len())
            && !(self.timeline.playback.playing && self.has_object_transform_triggers())
    }

    pub fn timeline_elapsed_seconds(&self, time_seconds: f32) -> f32 {
        derive_timeline_elapsed_seconds(
            self.spawn.position,
            self.spawn.direction,
            &self.timeline.taps.tap_times,
            time_seconds,
            &self.objects,
        )
    }

    pub fn topmost_block_index_at_cursor(&self, cursor: [f32; 3]) -> Option<usize> {
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
        if self.phase != AppPhase::Editor || self.editor.input.mode == EditorMode::Timing {
            return None;
        }

        let has_object_transform_triggers = self.editor.has_object_transform_triggers();

        if !self.editor.timeline.playback.playing {
            return has_object_transform_triggers.then(|| {
                trigger_transformed_objects_at_time(
                    &self.editor.objects,
                    &self.editor.trigger_objects(),
                    self.editor.timeline.clock.time_seconds,
                )
            });
        }

        if !has_object_transform_triggers {
            return self
                .editor
                .timeline
                .playback
                .runtime
                .as_ref()
                .map(|runtime| runtime.objects().to_vec());
        }

        Some(trigger_transformed_objects_at_time(
            &self.editor.objects,
            &self.editor.trigger_objects(),
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
        if dirty.rebuild_transform_trigger_markers {
            self.editor.runtime.dirty.rebuild_transform_trigger_markers = false;
        }
        if dirty.rebuild_preview_player {
            self.editor.runtime.dirty.rebuild_preview_player = false;
        }
        if dirty.rebuild_cursor {
            self.editor.runtime.dirty.rebuild_cursor = false;
        }
        if dirty.rebuild_hitbox_visualization {
            self.editor.runtime.dirty.rebuild_hitbox_visualization = false;
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
            puffin::profile_scope!("DirtyBlockMeshAppendChunked");
            self.editor.runtime.pending_block_mesh_appends.clear();
            self.rebuild_block_vertices();
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

        if dirty.rebuild_transform_trigger_markers {
            puffin::profile_scope!("DirtyTransformTriggerMarkers");
            self.rebuild_transform_trigger_marker_vertices();
        }

        if dirty.rebuild_preview_player {
            puffin::profile_scope!("DirtyPreviewPlayer");
            self.rebuild_editor_preview_player_vertices();
        }

        if dirty.rebuild_cursor {
            puffin::profile_scope!("DirtyCursor");
            self.rebuild_editor_cursor_vertices();
        }

        if dirty.rebuild_hitbox_visualization {
            puffin::profile_scope!("DirtyHitboxVisualization");
            self.rebuild_editor_hitbox_visualization_vertices();
        }

        if dirty.sync_game_objects || dirty.rebuild_block_mesh {
            self.editor.runtime.drag_heavy_rebuild_accumulator = 0.0;
        }
    }

    pub(super) fn place_editor_block(&mut self) {
        let place_mode = if self.editor.input.shift_held {
            EditorPlaceMode::Stamp
        } else {
            EditorPlaceMode::SelectPlaced
        };
        self.editor.add_block_at_cursor(place_mode);
        if place_mode == EditorPlaceMode::SelectPlaced {
            self.set_editor_mode(EditorMode::Scale);
        }
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
        direction: Direction,
        speed: Option<f32>,
    ) {
        self.gameplay.state.apply_spawn(position, direction);
        self.gameplay.death_sfx_played = false;
        if let Some(speed) = speed {
            self.gameplay.state.speed = speed;
        }
    }

    pub(super) fn apply_spawn_exact_to_game(
        &mut self,
        position: [f32; 3],
        direction: Direction,
        speed: Option<f32>,
    ) {
        self.gameplay.state.apply_spawn_exact(position, direction);
        self.gameplay.death_sfx_played = false;
        if let Some(speed) = speed {
            self.gameplay.state.speed = speed;
        }
    }

    pub(super) fn editor_timeline_elapsed_seconds(&self, time_seconds: f32) -> f32 {
        self.editor.timeline_elapsed_seconds(time_seconds)
    }

    fn editor_tap_cursor_overlap_index(&self) -> Option<usize> {
        let hovered_index = self.editor.runtime.interaction.hovered_tap_index;
        let selected_index = self.editor.timeline.taps.selected_index;
        hovered_index.or(selected_index).filter(|index| {
            self.editor
                .timeline
                .taps
                .tap_indicator_positions
                .get(*index)
                .is_some_and(|position| positions_match(*position, self.editor.input.cursor))
        })
    }

    pub(super) fn apply_editor_timeline_preview_state(
        &mut self,
        position: [f32; 3],
        direction: Direction,
    ) {
        self.editor.timeline.preview.position = position;
        self.editor.timeline.preview.direction = direction;

        if !self.editor.timeline.playback.playing {
            self.editor.input.cursor = [
                position[0].round(),
                position[1].round(),
                position[2].round(),
            ];
            self.editor.input.cursor[1] = self.editor.input.cursor[1].max(0.0);

            self.rebuild_editor_cursor_vertices();
        }

        self.editor.camera.editor_pan[0] = position[0] + 0.5;
        self.editor.camera.editor_pan[1] = position[2] + 0.5;
        self.editor.camera.editor_target_z = position[1];

        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    pub(super) fn rebuild_editor_cursor_vertices(&mut self) {
        puffin::profile_scope!("EditorCursorMesh");
        let vertices = if self.editor.input.mode == EditorMode::Tapping {
            if self.editor_tap_cursor_overlap_index().is_some() {
                self.render_meshes_mut().editor_cursor.clear();
                return;
            }
            build_editor_tap_cursor_vertices(self.editor.input.cursor)
        } else {
            build_editor_cursor_vertices(
                self.editor.input.cursor,
                self.editor.selected_block_default_size(),
                &self.editor.config.selected_block_id,
                self.editor.config.selected_block_rotation_degrees,
            )
        };
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.editor_cursor.replace_with_vertices(
                device,
                "Editor Cursor Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_editor_hover_outline_vertices(&mut self) {
        puffin::profile_scope!("EditorHoverOutlineMesh");
        if self.phase != AppPhase::Editor || !self.editor.input.mode.is_transform_mode() {
            self.render_meshes_mut().editor_hover_stencil.clear();
            self.render_meshes_mut().editor_hover_outline.clear();
            return;
        }

        let object_count = self.editor.objects.len();
        let selected_mask = self.editor.selected_mask_for_len(object_count);

        let mut indices_to_outline = Vec::new();
        let mut outline_mask = vec![false; object_count];
        let hovered_block_index = self
            .editor
            .input
            .hovered_block_index
            .filter(|index| *index < self.editor.objects.len());

        if let Some(index) = hovered_block_index
            && !selected_mask[index]
        {
            outline_mask[index] = true;
            indices_to_outline.push(index);
        }

        let has_transform_trigger_targets = hovered_block_index.is_some_and(|index| {
            !transform_trigger_target_indices_for_block(&self.editor.objects, index).is_empty()
        });

        if indices_to_outline.is_empty() && !has_transform_trigger_targets {
            self.render_meshes_mut().editor_hover_stencil.clear();
            self.render_meshes_mut().editor_hover_outline.clear();
            return;
        }

        let mut all_vertices = Vec::new();
        let mut stencil_geometry = MeshGeometry::default();
        for index in indices_to_outline {
            let obj = &self.editor.objects[index];
            all_vertices.append(&mut build_editor_object_outline_vertices(
                obj,
                EditorObjectOutlineStyle::Hover,
                3.0,
            ));
            stencil_geometry.append_geometry(build_block_geometry_for_object(obj));
        }

        append_hovered_transform_trigger_target_outlines(
            &self.editor.objects,
            hovered_block_index,
            &mut stencil_geometry,
            &mut all_vertices,
        );

        self.with_render_meshes_and_device(|device, meshes| {
            meshes.editor_hover_stencil.replace_with_geometry(
                device,
                "Editor Hover Stencil Vertex Buffer",
                &stencil_geometry,
            );

            meshes.editor_hover_outline.replace_with_vertices(
                device,
                "Editor Hover Outline Vertex Buffer",
                &all_vertices,
            );
        });
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        puffin::profile_scope!("EditorGizmoMesh");
        let mode = self.editor.input.mode;
        let visible_handles = self.editor.visible_gizmo_handles();
        let show_move_handles = mode.shows_move_gizmo() && visible_handles.position;
        let show_scale_handles = mode.shows_scale_gizmo() && visible_handles.size;
        let show_rotate_handles = mode.shows_rotate_gizmo() && visible_handles.rotation;
        if self.phase != AppPhase::Editor
            || (!show_move_handles && !show_scale_handles && !show_rotate_handles)
        {
            self.render_meshes_mut().editor_gizmo.clear();
            return;
        }

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            self.render_meshes_mut().editor_gizmo.clear();
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

        let rotation = self
            .editor
            .selected_indices_normalized()
            .first()
            .and_then(|&index| self.editor.objects.get(index))
            .map(|obj| obj.rotation)
            .or_else(|| {
                self.editor
                    .selected_transform_trigger_target()
                    .map(|(_, target)| target.rotation)
            })
            .unwrap_or([0.0, 0.0, 0.0]);

        let vertices = build_editor_gizmo_vertices(GizmoParams {
            position: bounds_position,
            size: bounds_size,
            rotation,
            axis_lengths,
            axis_width,
            resize_radius,
            resize_offsets,
            show_move_handles,
            show_scale_handles,
            show_rotate_handles,
            hovered_part,
            dragged_part,
        });
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.editor_gizmo.replace_with_vertices(
                device,
                "Editor Gizmo Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        puffin::profile_scope!("EditorSelectionOutlineMesh");
        if self.phase != AppPhase::Editor || !self.editor.input.mode.is_transform_mode() {
            self.render_meshes_mut().editor_selection_stencil.clear();
            self.render_meshes_mut().editor_selection_outline.clear();
            self.render_meshes_mut()
                .editor_selection_outline_instances
                .clear();
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            self.render_meshes_mut().editor_selection_stencil.clear();
            self.render_meshes_mut().editor_selection_outline.clear();
            self.render_meshes_mut()
                .editor_selection_outline_instances
                .clear();
            return;
        }

        if selected_indices.len() > SIMPLE_SELECTION_OUTLINE_BLOCK_THRESHOLD {
            let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
                self.render_meshes_mut().editor_selection_stencil.clear();
                self.render_meshes_mut().editor_selection_outline.clear();
                self.render_meshes_mut()
                    .editor_selection_outline_instances
                    .clear();
                return;
            };

            let bounds_object = LevelObject {
                position: bounds_position,
                size: bounds_size,
                ..LevelObject::default()
            };
            let mask_vertices =
                build_block_geometry_for_object(&bounds_object).to_triangle_vertices();
            let mut outline_vertices = build_editor_selection_outline_vertices(
                bounds_position,
                bounds_size,
                [0.0, 0.0, 0.0],
                2.0,
            );
            let mut mask_vertices = mask_vertices;
            let mut instances = vec![EditorOutlineInstance {
                mask_vertices: 0..mask_vertices.len() as u32,
                outline_vertices: 0..outline_vertices.len() as u32,
            }];
            append_transform_trigger_target_outline_instances(
                &self.editor.objects,
                selected_indices.iter().copied(),
                &mut mask_vertices,
                &mut outline_vertices,
                &mut instances,
            );
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.editor_selection_stencil.replace_with_vertices(
                    device,
                    "Editor Selection Stencil Vertex Buffer",
                    &mask_vertices,
                );
                meshes.editor_selection_outline.replace_with_vertices(
                    device,
                    "Editor Selection Outline Vertex Buffer",
                    &outline_vertices,
                );
                meshes.editor_selection_outline_instances = instances;
            });
            return;
        }

        let mut outline_vertices = Vec::new();
        let mut mask_vertices = Vec::new();
        let mut instances = Vec::new();
        for index in selected_indices {
            if let Some(obj) = self.editor.objects.get(index) {
                append_outline_instance(
                    &mut mask_vertices,
                    &mut outline_vertices,
                    &mut instances,
                    obj,
                    build_editor_object_outline_vertices(
                        obj,
                        EditorObjectOutlineStyle::Selection,
                        2.0,
                    ),
                );
            }
        }
        append_transform_trigger_target_outline_instances(
            &self.editor.objects,
            self.selected_block_indices_normalized(),
            &mut mask_vertices,
            &mut outline_vertices,
            &mut instances,
        );
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.editor_selection_stencil.replace_with_vertices(
                device,
                "Editor Selection Stencil Vertex Buffer",
                &mask_vertices,
            );
            meshes.editor_selection_outline.replace_with_vertices(
                device,
                "Editor Selection Outline Vertex Buffer",
                &outline_vertices,
            );
            meshes.editor_selection_outline_instances = instances;
        });
    }

    pub(super) fn rebuild_spawn_marker_vertices(&mut self) {
        puffin::profile_scope!("SpawnMarkerMesh");
        let vertices = build_spawn_marker_vertices(
            self.editor.spawn.position,
            matches!(self.editor.spawn.direction, Direction::Right),
        );
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.spawn_marker.replace_with_vertices(
                device,
                "Spawn Marker Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_camera_trigger_marker_vertices(&mut self) {
        puffin::profile_scope!("CameraTriggerMarkerMesh");
        self.render_meshes_mut().camera_trigger_markers.clear();
    }

    pub(super) fn rebuild_transform_trigger_marker_vertices(&mut self) {
        puffin::profile_scope!("TransformTriggerMarkerMesh");
        if self.phase != AppPhase::Editor {
            self.render_meshes_mut().transform_trigger_markers.clear();
            return;
        }

        let markers = transform_trigger_markers_for_triggers(
            &self.editor.objects,
            &self.editor.trigger_objects(),
            self.editor.selected_trigger_index(),
        );

        if markers.is_empty() {
            self.render_meshes_mut().transform_trigger_markers.clear();
            return;
        }

        let vertices = build_transform_trigger_marker_vertices(
            &markers,
            self.editor.timeline.clock.time_seconds,
        );
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.transform_trigger_markers.replace_with_vertices(
                device,
                "Transform Trigger Marker Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_block_vertices(&mut self) {
        puffin::profile_scope!("BlockMeshRebuild");
        if self.phase == AppPhase::Editor {
            self.rebuild_editor_block_vertices_split();
        } else {
            // Non-editor phases (menu preview, playing) route normal blocks through
            // instanced rendering so large levels no longer expand into one giant
            // vertex buffer. Camera/transform triggers and other special visuals stay
            // on the fallback mesh path.
            let result = if self.editor.has_camera_timeline_triggers() {
                build_chunked_block_mesh_at_time(
                    &self.gameplay.state.objects,
                    self.gameplay.state.elapsed_seconds,
                )
            } else {
                build_chunked_block_mesh(&self.gameplay.state.objects)
            };
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_chunked.replace_with_chunked_mesh(
                    device,
                    "Chunked Block Buffer",
                    &result,
                );
            });
            self.render_meshes_mut().blocks.clear();
            self.render_meshes_mut().blocks_instanced.clear();
            self.render_meshes_mut().blocks_static.clear();
            self.render_meshes_mut().blocks_static_chunked.clear();
            self.render_meshes_mut().blocks_static_instanced.clear();
            self.render_meshes_mut().blocks_selected.clear();
            self.editor.block_static_vertex_cache.clear();
            self.editor.block_static_vertex_cache_complete_len = None;
        }
    }

    pub(super) fn rebuild_practice_checkpoint_vertices(&mut self) {
        if self.phase != AppPhase::Playing
            || self.session.playtesting_editor
            || !self.session.practice_mode_enabled
            || self.session.practice_checkpoints.is_empty()
        {
            self.render_meshes_mut().practice_checkpoints.clear();
            return;
        }

        let latest_index = self.session.practice_checkpoints.len().saturating_sub(1);
        let flags = self
            .session
            .practice_checkpoints
            .iter()
            .enumerate()
            .map(|(index, checkpoint)| PracticeCheckpointFlagInstance {
                position: checkpoint.gameplay.position,
                direction: checkpoint.gameplay.direction,
                is_latest: index == latest_index,
            })
            .collect::<Vec<_>>();
        let geometry = build_practice_checkpoint_flag_geometry(&flags);
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.practice_checkpoints.replace_with_geometry(
                device,
                "Practice Checkpoint Flag Vertex Buffer",
                &geometry,
            );
        });
    }

    fn rebuild_editor_block_vertices_split(&mut self) {
        puffin::profile_scope!("BlockMeshSplitRebuild");
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

        let static_chunks = {
            puffin::profile_scope!("BlockMeshSplitStatic");
            let mut static_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if !selected_mask[index] {
                    static_objects.push(object);
                }
            }
            if self.editor.has_camera_timeline_triggers() {
                build_chunked_block_mesh_from_refs_at_time(
                    &static_objects,
                    Some(self.editor.timeline.clock.time_seconds),
                    eg_mesh::DEFAULT_RENDER_CHUNK_SIZE,
                )
            } else {
                build_chunked_block_mesh_from_refs(&static_objects)
            }
        };

        let selected_vertices = {
            puffin::profile_scope!("BlockMeshSplitSelected");
            let mut selected_objects = Vec::new();
            for (index, object) in object_source.iter().enumerate() {
                if selected_mask[index] {
                    selected_objects.push(object);
                }
            }
            if selected_objects.len() > MAX_EXPANDED_SELECTED_BLOCK_OBJECTS {
                log::warn!(
                    "Skipping selected block mesh for {} selected objects to avoid an oversized expanded mesh",
                    selected_objects.len()
                );
                MeshGeometry::default()
            } else if self.editor.has_camera_timeline_triggers() {
                build_block_geometry_from_refs_at_time(
                    &selected_objects,
                    Some(self.editor.timeline.clock.time_seconds),
                )
            } else {
                build_block_geometry_from_refs(&selected_objects)
            }
        };

        let has_selected_blocks = selected_mask.iter().any(|selected| *selected);

        self.editor.selected_mask_cache = Some(selected_mask);

        if !uses_runtime_objects && !has_selected_blocks {
            puffin::profile_scope!("BlockMeshUploadStatic");
            self.editor.block_static_vertex_cache = static_chunks.fallback_geometry();
            self.editor.block_static_vertex_cache_complete_len = Some(object_source.len());
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_static_chunked.replace_with_chunked_mesh(
                    device,
                    "Chunked Block Static Buffer",
                    &static_chunks,
                );
            });
            self.render_meshes_mut().blocks_static.clear();
            self.render_meshes_mut().blocks_static_instanced.clear();
        } else {
            puffin::profile_scope!("BlockMeshUploadStatic");
            self.editor.block_static_vertex_cache.clear();
            self.editor.block_static_vertex_cache_complete_len = None;
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_static_chunked.replace_with_chunked_mesh(
                    device,
                    "Chunked Block Static Buffer",
                    &static_chunks,
                );
            });
            self.render_meshes_mut().blocks_static.clear();
            self.render_meshes_mut().blocks_static_instanced.clear();
        }

        {
            puffin::profile_scope!("BlockMeshUploadSelected");
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_selected.replace_with_geometry(
                    device,
                    "Block Selected Vertex Buffer",
                    &selected_vertices,
                );
            });
        }
        self.render_meshes_mut().blocks.clear();
        self.render_meshes_mut().blocks_chunked.clear();
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
            self.render_meshes_mut().blocks_selected.clear();
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

            if self.editor.has_camera_timeline_triggers() {
                build_block_geometry_from_refs_at_time(
                    &selected_objects,
                    Some(self.editor.timeline.clock.time_seconds),
                )
            } else {
                build_block_geometry_from_refs(&selected_objects)
            }
        };

        {
            puffin::profile_scope!("SelectedOnlyUpload");
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_selected.replace_with_geometry(
                    device,
                    "Block Selected Vertex Buffer",
                    &selected_vertices,
                );
            });
        }
        self.render_meshes_mut().blocks.clear();
    }

    pub(super) fn rebuild_tap_indicator_vertices(&mut self) {
        puffin::profile_scope!("TapIndicatorMesh");
        let effective_mode = self.editor_effective_mode_for_playback();
        if self.phase != AppPhase::Editor || effective_mode == EditorMode::Timing {
            self.render_meshes_mut().tap_indicators.clear();
            return;
        }
        let show_timing_division_previews = effective_mode == EditorMode::Tapping;

        let hovered_tap_index = self.editor.runtime.interaction.hovered_tap_index;
        let selected_tap_index = self.editor.timeline.taps.selected_index;
        let hovered_division = self.editor.runtime.interaction.hovered_tap_division;
        let previews = if show_timing_division_previews {
            self.editor.timing_division_tap_previews().to_vec()
        } else {
            Vec::new()
        };
        let tap_positions = self.editor.timeline.taps.tap_indicator_positions.clone();
        let mut preview_indicators: Vec<([f32; 3], [f32; 4])> = previews
            .iter()
            .copied()
            .filter(|preview| {
                !tap_positions
                    .iter()
                    .any(|position| positions_match(*position, preview.indicator_position))
            })
            .map(|preview| {
                let is_hovered = hovered_division.is_some_and(|hovered| {
                    (hovered.time_seconds - preview.time_seconds).abs() <= 0.001
                        && positions_match(hovered.indicator_position, preview.indicator_position)
                });
                let alpha = if is_hovered { 0.95 } else { 0.24 };
                (preview.indicator_position, [0.05, 0.48, 0.95, alpha])
            })
            .collect();
        preview_indicators.sort_unstable_by(|a, b| {
            a.0[0]
                .total_cmp(&b.0[0])
                .then(a.0[1].total_cmp(&b.0[1]))
                .then(a.0[2].total_cmp(&b.0[2]))
                .then(a.1[3].total_cmp(&b.1[3]))
        });
        preview_indicators
            .dedup_by(|a, b| positions_match(a.0, b.0) && (a.1[3] - b.1[3]).abs() < 0.001);

        let mut indicators: Vec<([f32; 3], [f32; 4])> = tap_positions
            .iter()
            .copied()
            .enumerate()
            .map(|(index, position)| {
                let color = tap_indicator_color(index, hovered_tap_index, selected_tap_index);
                (position, color)
            })
            .collect();
        indicators.sort_unstable_by(|a, b| {
            a.0[0]
                .total_cmp(&b.0[0])
                .then(a.0[1].total_cmp(&b.0[1]))
                .then(a.0[2].total_cmp(&b.0[2]))
                .then(a.1[3].total_cmp(&b.1[3]))
        });
        indicators.dedup_by(|a, b| {
            (a.0[0] - b.0[0]).abs() < 0.001
                && (a.0[1] - b.0[1]).abs() < 0.001
                && (a.0[2] - b.0[2]).abs() < 0.001
                && (a.1[3] - b.1[3]).abs() < 0.001
        });

        let division_tap_indicators = if show_timing_division_previews {
            tap_division_marker_indicators(
                &self.editor.timeline.taps.tap_times,
                &tap_positions,
                &self.editor.timing.timing_points,
                self.editor.timeline.clock.duration_seconds,
            )
        } else {
            Vec::new()
        };

        if indicators.is_empty()
            && preview_indicators.is_empty()
            && division_tap_indicators.is_empty()
        {
            self.render_meshes_mut().tap_indicators.clear();
            return;
        }

        if indicators.is_empty() && division_tap_indicators.is_empty() {
            let vertices = build_tap_division_preview_vertices(&preview_indicators);
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.tap_indicators.replace_with_vertices(
                    device,
                    "Tap Indicator Vertex Buffer",
                    &vertices,
                );
            });
            return;
        }

        let mut vertices = build_tap_division_preview_vertices(&preview_indicators);
        vertices.extend(build_colored_tap_indicator_vertices(&indicators));
        vertices.extend(build_tap_division_tap_marker_vertices(
            &division_tap_indicators,
        ));
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.tap_indicators.replace_with_vertices(
                device,
                "Tap Indicator Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_editor_preview_player_vertices(&mut self) {
        self.render_meshes_mut().editor_preview_player.clear();
    }

    pub(super) fn rebuild_editor_preview_player_vertices_for_state(
        &mut self,
        position: [f32; 3],
        direction: Direction,
    ) {
        self.editor.timeline.preview.position = position;
        self.editor.timeline.preview.direction = direction;
        self.render_meshes_mut().editor_preview_player.clear();
        self.mark_editor_dirty(EditorDirtyFlags::hitbox_visualization_changed());
    }

    pub(super) fn rebuild_editor_hitbox_visualization_vertices(&mut self) {
        puffin::profile_scope!("EditorHitboxVisualizationMesh");
        let effective_mode = self.editor_effective_mode_for_playback();
        if self.phase != AppPhase::Editor
            || effective_mode == EditorMode::Timing
            || !self.editor.input.show_hitbox_visualization
        {
            self.render_meshes_mut().editor_hitbox_visualization.clear();
            return;
        }

        // Hitbox visualization follows the gameplay hitboxes, so when a trigger
        // opts into moving hitboxes the red outline moves with it.
        let object_source = self.editor.objects.clone();

        let player_hitbox = editor_preview_player_hitbox(self.editor.timeline.preview.position);
        let vertices =
            build_editor_hitbox_visualization_vertices(&object_source, Some(player_hitbox));
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.editor_hitbox_visualization.replace_with_vertices(
                device,
                "Editor Hitbox Visualization Vertex Buffer",
                &vertices,
            );
        });
    }

    pub(super) fn rebuild_camera_arrow_vertices(&mut self) {
        puffin::profile_scope!("CameraArrowMesh");
        if self.phase != AppPhase::Editor || self.editor_is_playing() {
            self.render_meshes_mut().camera_arrow.clear();
            return;
        }

        let (eye, target) = {
            let (e, t) = self.editor_preview_camera_view();
            (Vec3::from_array(e), Vec3::from_array(t))
        };

        let forward = {
            let dir = target - eye;
            if dir.length_squared() > f32::EPSILON {
                dir.normalize().to_array()
            } else {
                [0.0, 0.0, 1.0]
            }
        };

        let editor_camera_eye = {
            let target = self.editor.editor_camera_target();
            let offset = self.editor_camera_offset();
            (target + offset).to_array()
        };
        let vertices = build_camera_arrow_vertices(eye.to_array(), forward, editor_camera_eye);
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.camera_arrow.replace_with_vertices(
                device,
                "Camera Arrow Vertex Buffer",
                &vertices,
            );
        });
    }
}

fn editor_preview_player_hitbox(position: [f32; 3]) -> LevelObject {
    const PLAYER_WIDTH: f32 = 0.8;
    const PLAYER_HEIGHT: f32 = 0.8;
    const PLAYER_FOOTPRINT_TOLERANCE: f32 = PLAYER_WIDTH * 0.05;

    let inset_width = PLAYER_WIDTH - PLAYER_FOOTPRINT_TOLERANCE * 2.0;
    let inset_height = PLAYER_HEIGHT - PLAYER_FOOTPRINT_TOLERANCE * 2.0;
    LevelObject {
        position: [
            position[0] - inset_width * 0.5,
            position[1] + PLAYER_FOOTPRINT_TOLERANCE,
            position[2] - inset_width * 0.5,
        ],
        size: [inset_width, inset_height, inset_width],
        ..LevelObject::new("core/void")
    }
}

fn positions_match(left: [f32; 3], right: [f32; 3]) -> bool {
    (left[0] - right[0]).abs() < 0.001
        && (left[1] - right[1]).abs() < 0.001
        && (left[2] - right[2]).abs() < 0.001
}

fn tap_division_marker_indicators(
    tap_times: &[f32],
    tap_positions: &[[f32; 3]],
    timing_points: &[TimingPoint],
    duration_seconds: f32,
) -> Vec<([f32; 3], [f32; 4])> {
    tap_times
        .iter()
        .copied()
        .zip(tap_positions.iter().copied())
        .filter(|(tap_time, _)| {
            tap_time_is_timing_division(*tap_time, timing_points, duration_seconds)
        })
        .map(|(_, position)| (position, [0.0, 0.0, 0.0, 1.0]))
        .collect()
}

fn tap_indicator_color(
    index: usize,
    hovered_tap_index: Option<usize>,
    selected_tap_index: Option<usize>,
) -> [f32; 4] {
    if selected_tap_index == Some(index) {
        [0.2, 0.85, 0.95, 1.0]
    } else if hovered_tap_index == Some(index) {
        [0.1, 0.45, 0.5, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EDITOR_SELECTION_OUTLINE_COLOR, EditorObjectOutlineStyle, State,
        build_editor_object_outline_vertices, tap_indicator_color,
        transform_trigger_markers_for_triggers,
    };
    use eg_blocks::BlockId;
    use eg_mesh::blocks::build_block_geometry_for_object;
    use eg_mesh::builders::game::build_colored_tap_indicator_vertices;
    use eg_render::MeshDrawData;
    use eg_types::{
        AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EditorMode, LevelObject,
        TRANSFORM_TRIGGER_BLOCK_ID,
    };
    use eg_types::{EasingDirection, TimedTrigger, TriggerTarget};

    fn block(position: [f32; 3], size: [f32; 3]) -> LevelObject {
        LevelObject {
            id: "target".to_string(),
            position,
            size,
            ..LevelObject::default()
        }
    }

    fn mesh_draw_count(draw_data: MeshDrawData<'_>) -> u32 {
        match draw_data {
            MeshDrawData::Vertices { count, .. } | MeshDrawData::Indexed { count, .. } => count,
        }
    }

    #[tokio::test]
    async fn camera_trigger_selection_outline_uses_visual_mesh_instead_of_cube_hull() {
        let object = LevelObject {
            position: [3.0, 4.0, 5.0],
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        };

        let visual_vertices = build_block_geometry_for_object(&object).to_triangle_vertices();
        let outline_vertices =
            build_editor_object_outline_vertices(&object, EditorObjectOutlineStyle::Selection, 2.0);

        assert_ne!(visual_vertices.len(), 36);
        assert_eq!(outline_vertices.len(), visual_vertices.len());
        assert!(
            outline_vertices
                .iter()
                .all(|vertex| vertex.color == EDITOR_SELECTION_OUTLINE_COLOR)
        );
        assert!(
            outline_vertices
                .iter()
                .all(|vertex| (vertex.render_profile - 3.0).abs() <= f32::EPSILON)
        );
    }

    fn object_move_trigger() -> LevelObject {
        transform_trigger_object(0.0, 0.0, [2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }

    fn transform_trigger_object(
        time_seconds: f32,
        duration_seconds: f32,
        position: [f32; 3],
        rotation: [f32; 3],
        size: [f32; 3],
    ) -> LevelObject {
        LevelObject {
            id: format!("trigger_{time_seconds}"),
            position,
            rotation,
            size,
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: vec!["target".to_string()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    fn camera_trigger() -> LevelObject {
        LevelObject {
            id: "camera_trigger".to_string(),
            trigger: Some(TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Static,
                    transition_interval_seconds: 1.0,
                    use_full_segment_transition: false,
                    target_position: [0.0, 0.0, 0.0],
                    yaw: 0.0,
                    pitch: 0.0,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    #[tokio::test]
    async fn topmost_block_index_at_cursor_prefers_highest_overlapping_block() {
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
    }

    #[tokio::test]
    async fn sync_objects_filters_invalid_selection_and_marks_full_dirty() {
        let mut state = State::new_test().await;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        state.editor.input.selected_block_index = Some(99);
        state.editor.input.selected_block_indices = vec![0, 99];
        state.editor.input.hovered_block_index = Some(42);
        state.editor.selected_mask_cache = Some(vec![true]);

        state.editor.sync_objects();

        assert_eq!(state.editor.input.selected_block_index, Some(0));
        assert_eq!(state.editor.input.selected_block_indices, vec![0]);
        assert_eq!(state.editor.input.hovered_block_index, None);
        assert!(state.editor.runtime.dirty.sync_game_objects);
        assert!(state.editor.runtime.dirty.rebuild_block_mesh);
        assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
        assert!(state.editor.runtime.dirty.rebuild_tap_indicators);
        assert!(state.editor.runtime.dirty.rebuild_preview_player);
        assert!(state.editor.runtime.dirty.rebuild_cursor);
        assert!(state.editor.selected_mask_cache.is_none());
    }

    #[tokio::test]
    async fn sync_objects_for_drag_and_after_release_set_expected_dirty_flags() {
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
    }

    #[tokio::test]
    async fn editor_runtime_objects_for_render_previews_transforms_without_playback() {
        let mut state = State::new_test().await;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        state.editor.objects.push(object_move_trigger());
        state.editor.timeline.clock.time_seconds = 0.0;

        state.phase = AppPhase::Menu;
        assert!(state.editor_runtime_objects_for_render().is_none());

        state.phase = AppPhase::Editor;
        state.editor.timeline.playback.playing = false;
        state.editor.input.mode = EditorMode::Place;
        let transformed = state
            .editor_runtime_objects_for_render()
            .expect("expected stopped editor transform preview objects");
        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);

        state.editor.timeline.playback.playing = true;
        state.editor.input.mode = EditorMode::Timing;
        assert!(state.editor_runtime_objects_for_render().is_none());

        state.editor.input.mode = EditorMode::Place;
        let transformed = state
            .editor_runtime_objects_for_render()
            .expect("expected transformed render objects");
        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
    }

    #[tokio::test]
    async fn editor_runtime_objects_for_render_keeps_object_triggers_during_playback_runtime() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Place;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        state.editor.objects.push(object_move_trigger());
        state.editor.timeline.clock.time_seconds = 0.5;
        state.editor.timeline.playback.playing = true;
        state.editor.timeline.playback.runtime = Some(eg_game::TimelineSimulationRuntime::new(
            state.editor.spawn.position,
            state.editor.spawn.direction,
            &state.editor.objects,
            &state.editor.timeline.taps.tap_times,
        ));

        let transformed = state
            .editor_runtime_objects_for_render()
            .expect("expected transformed render objects");

        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
    }

    #[tokio::test]
    async fn process_editor_dirty_handles_idle_drag_and_full_rebuild_paths() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];

        state.editor.runtime.drag_heavy_rebuild_accumulator = 3.0;
        state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
        state.process_editor_dirty(0.01);
        assert_eq!(state.editor.runtime.drag_heavy_rebuild_accumulator, 0.0);

        state.editor.runtime.interaction.block_drag = Some(super::super::EditorBlockDrag {
            start_mouse: [0.0, 0.0],
            start_center_world: [0.0, 0.0, 0.0],
            start_drag_world: [0.0, 0.0, 0.0],
            start_cursor: [0.0, 0.0, 0.0],
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
    }

    #[tokio::test]
    async fn camera_trigger_overlay_mesh_clears_while_tap_indicators_build_by_phase_and_data() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.rebuild_camera_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .camera_trigger_markers
                .draw_data()
                .is_none()
        );

        state.phase = AppPhase::Editor;
        state.editor.objects.clear();
        state.rebuild_camera_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .camera_trigger_markers
                .draw_data()
                .is_none()
        );

        state.editor.objects = vec![camera_trigger()];
        state.rebuild_camera_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .camera_trigger_markers
                .draw_data()
                .is_none()
        );

        state.phase = AppPhase::Menu;
        state.rebuild_tap_indicator_vertices();
        assert!(state.render_meshes().tap_indicators.draw_data().is_none());

        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Tapping;
        state.editor.timeline.taps.tap_indicator_positions = vec![[1.0, 0.0, 1.0], [1.0, 0.0, 1.0]];
        state.rebuild_tap_indicator_vertices();
        assert!(state.render_meshes().tap_indicators.draw_data().is_some());

        state.editor.input.mode = EditorMode::Place;
        state.rebuild_tap_indicator_vertices();
        let compose_draw_data = state
            .render_meshes()
            .tap_indicators
            .draw_data()
            .expect("compose should draw saved tap indicators");
        let expected_saved_tap_vertices = build_colored_tap_indicator_vertices(&[(
            [1.0, 0.0, 1.0],
            tap_indicator_color(0, None, None),
        )]);
        assert_eq!(
            mesh_draw_count(compose_draw_data),
            expected_saved_tap_vertices.len() as u32
        );

        // During playback from Tapping tab (mode=Null, last_mode=Tapping), taps should remain visible
        state.editor.input.mode = EditorMode::Null;
        state.editor.runtime.interaction.last_mode = Some(EditorMode::Tapping);
        state.rebuild_tap_indicator_vertices();
        assert!(state.render_meshes().tap_indicators.draw_data().is_some());

        // During playback from Compose tab, taps should remain visible
        state.editor.runtime.interaction.last_mode = Some(EditorMode::Place);
        state.rebuild_tap_indicator_vertices();
        assert!(state.render_meshes().tap_indicators.draw_data().is_some());

        state.editor.runtime.interaction.last_mode = None;
        state.editor.input.mode = EditorMode::Timing;
        state.rebuild_tap_indicator_vertices();
        assert!(state.render_meshes().tap_indicators.draw_data().is_none());
    }

    #[tokio::test]
    async fn transform_trigger_marker_meshes_clear_or_build_by_phase_and_data() {
        let mut state = State::new_test().await;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        state.editor.objects.push(object_move_trigger());

        state.phase = AppPhase::Menu;
        state.rebuild_transform_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .transform_trigger_markers
                .draw_data()
                .is_none()
        );

        state.phase = AppPhase::Editor;
        state.rebuild_transform_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .transform_trigger_markers
                .draw_data()
                .is_some()
        );

        state
            .editor
            .objects
            .retain(|object| object.trigger.is_none());
        state.rebuild_transform_trigger_marker_vertices();
        assert!(
            state
                .render_meshes()
                .transform_trigger_markers
                .draw_data()
                .is_none()
        );
    }

    #[tokio::test]
    async fn transform_trigger_marker_sources_reflect_prior_trigger_state() {
        let objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        let triggers = vec![
            transform_trigger_object(1.0, 4.0, [8.0, 0.0, 0.0], [0.0, 90.0, 0.0], [2.0, 1.0, 1.0]),
            transform_trigger_object(
                3.0,
                1.0,
                [10.0, 0.0, 0.0],
                [0.0, 180.0, 0.0],
                [3.0, 1.0, 1.0],
            ),
        ];

        let markers = transform_trigger_markers_for_triggers(&objects, &triggers, Some(1));

        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].source_position, Some([0.0, 0.0, 0.0]));
        approx::assert_abs_diff_eq!(markers[1].source_position.unwrap()[0], 4.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(markers[1].source_size.unwrap()[0], 1.5, epsilon = 1e-6);
        assert!(markers[1].is_selected);
    }

    #[tokio::test]
    async fn transform_trigger_marker_without_sources_still_draws_target() {
        let triggers = vec![LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 4.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: Vec::new(),
                },
                transform: true,
                camera: None,
            }),
            position: [8.0, 0.0, 0.0],
            rotation: [0.0, 90.0, 0.0],
            size: [2.0, 1.0, 1.0],
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }];

        let markers = transform_trigger_markers_for_triggers(&[], &triggers, Some(0));

        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].source_position, None);
        assert_eq!(markers[0].source_size, None);
        assert_eq!(markers[0].target_position, [8.0, 0.0, 0.0]);
        assert_eq!(markers[0].target_size, [2.0, 1.0, 1.0]);
        assert!(markers[0].is_selected);
    }

    #[tokio::test]
    async fn tapping_cursor_hides_only_when_overlapping_hovered_or_selected_tap() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Tapping;
        state.editor.timeline.taps.tap_indicator_positions = vec![[1.0, 0.0, 1.0]];
        state.editor.input.cursor = [1.0, 0.0, 1.0];

        state.rebuild_editor_cursor_vertices();
        assert!(state.render_meshes().editor_cursor.draw_data().is_some());

        state.editor.runtime.interaction.hovered_tap_index = Some(0);
        state.rebuild_editor_cursor_vertices();
        assert!(state.render_meshes().editor_cursor.draw_data().is_none());

        state.editor.runtime.interaction.hovered_tap_index = None;
        state.editor.timeline.taps.selected_index = Some(0);
        state.rebuild_editor_cursor_vertices();
        assert!(state.render_meshes().editor_cursor.draw_data().is_none());

        state.editor.input.cursor = [2.0, 0.0, 2.0];
        state.rebuild_editor_cursor_vertices();
        assert!(state.render_meshes().editor_cursor.draw_data().is_some());
    }

    #[tokio::test]
    async fn rebuild_block_vertices_switches_between_editor_split_and_playing_mesh() {
        let mut state = State::new_test().await;
        state.editor.objects = vec![
            block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            block([2.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];
        state.editor.input.selected_block_index = Some(0);
        state.editor.input.selected_block_indices = vec![0];

        state.phase = AppPhase::Editor;
        state.rebuild_block_vertices();
        assert!(state.render_meshes().blocks.draw_data().is_none());
        assert!(state.render_meshes().blocks_selected.draw_data().is_some());
        assert_eq!(state.render_meshes().blocks_static_chunked.chunk_count(), 1);
        assert!(!state.render_meshes().blocks_static_chunked.is_empty());

        state.phase = AppPhase::Playing;
        state.gameplay.state.objects = state.editor.objects.clone();
        state.rebuild_block_vertices();
        assert_eq!(state.render_meshes().blocks_chunked.chunk_count(), 1);
        assert!(!state.render_meshes().blocks_chunked.is_empty());
        assert!(state.render_meshes().blocks_instanced.is_empty());
        assert!(state.render_meshes().blocks.draw_data().is_none());
        assert!(state.render_meshes().blocks_selected.draw_data().is_none());
        assert!(state.render_meshes().blocks_static.draw_data().is_none());
        assert!(state.render_meshes().blocks_static_chunked.is_empty());
        assert!(state.render_meshes().blocks_static_instanced.is_empty());
    }

    #[tokio::test]
    async fn placing_plain_block_appends_static_mesh_after_complete_rebuild() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.objects = vec![block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])];
        state.editor.input.cursor = [2.0, 0.0, 0.0];

        state.rebuild_block_vertices();
        let before_count = state.editor.block_static_vertex_cache.draw_count();
        assert_eq!(state.editor.block_static_vertex_cache_complete_len, Some(1));

        state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
        state
            .editor
            .add_block_at_cursor(eg_types::EditorPlaceMode::Stamp);

        assert!(state.editor.runtime.dirty.sync_game_objects);
        assert!(state.editor.runtime.dirty.append_block_mesh);
        assert!(!state.editor.runtime.dirty.rebuild_block_mesh);
        assert!(!state.editor.runtime.dirty.rebuild_tap_indicators);
        assert!(!state.editor.runtime.dirty.rebuild_preview_player);

        state.process_editor_dirty(0.02);

        let after_count = state.editor.block_static_vertex_cache.draw_count();
        assert!(after_count > before_count);
        assert_eq!(state.render_meshes().blocks_static_chunked.chunk_count(), 1);
        assert_eq!(state.editor.block_static_vertex_cache_complete_len, Some(2));
        assert!(!state.editor.runtime.dirty.any());
    }

    #[tokio::test]
    async fn placing_selected_block_uses_block_default_size() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.config.selected_block_id = BlockId::new("core/speedportal");
        state.editor.config.selected_block_rotation_degrees = [0.0, 90.0, 0.0];
        state.editor.input.cursor = [2.0, 0.0, 4.0];

        state
            .editor
            .add_block_at_cursor(eg_types::EditorPlaceMode::Stamp);

        assert_eq!(state.editor.objects.len(), 1);
        assert_eq!(state.editor.objects[0].position, [2.0, 0.0, 4.0]);
        assert_eq!(state.editor.objects[0].size, [2.0, 1.0, 1.0]);
        assert_eq!(state.editor.objects[0].rotation, [0.0, 90.0, 0.0]);
        assert_eq!(state.editor.objects[0].block_id, "core/speedportal");
    }

    #[tokio::test]
    async fn placing_transform_trigger_marks_target_marker_rebuild_dirty() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.config.selected_block_id = BlockId::new(TRANSFORM_TRIGGER_BLOCK_ID);
        state.editor.input.cursor = [2.0, 0.0, 4.0];
        state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();

        state
            .editor
            .add_block_at_cursor(eg_types::EditorPlaceMode::Stamp);

        assert_eq!(state.editor.objects.len(), 1);
        assert!(state.editor.objects[0].trigger.is_some());
        assert!(state.editor.runtime.dirty.rebuild_transform_trigger_markers);
    }

    #[tokio::test]
    async fn apply_editor_timeline_preview_state_updates_cursor_only_when_not_playing() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.cursor = [0.0, 0.0, 0.0];

        state.editor.timeline.playback.playing = false;
        state.apply_editor_timeline_preview_state([1.2, 2.1, 3.8], eg_types::Direction::Right);
        assert_eq!(state.editor.input.cursor, [1.0, 2.0, 4.0]);
        assert_eq!(state.editor.camera.editor_pan, [1.7, 4.3]);
        assert_eq!(state.editor.camera.editor_target_z, 2.1);

        state.editor.timeline.playback.playing = true;
        state.editor.input.cursor = [9.0, 9.0, 9.0];
        state.apply_editor_timeline_preview_state([4.2, 1.0, 2.2], eg_types::Direction::Forward);
        assert_eq!(state.editor.input.cursor, [9.0, 9.0, 9.0]);
        assert_eq!(state.editor.camera.editor_pan, [4.7, 2.7]);
        assert_eq!(state.editor.camera.editor_target_z, 1.0);
    }
}
