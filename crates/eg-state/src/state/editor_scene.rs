/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use super::EditorDirtyFlags;
#[cfg(test)]
use super::EditorSubsystem;
use eg_blocks::BlockDefinition;
use eg_editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds};
#[cfg(test)]
use eg_types::LevelObject;
use eg_types::{EditorPlaceMode, TRANSFORM_TRIGGER_BLOCK_ID};

#[cfg(test)]
use eg_blocks::BlockRenderProfile;
#[cfg(test)]
use eg_game::trigger_transformed_objects_at_time;
#[cfg(test)]
use eg_mesh::{
    TransformTriggerMarker, Vertex, build_block_geometry_for_object,
    build_editor_selection_outline_vertices,
};
#[cfg(test)]
use eg_types::TriggerTarget;
#[cfg(test)]
use std::borrow::Cow;

#[cfg(test)]
const EDITOR_SELECTION_OUTLINE_COLOR: [f32; 4] = [0.098, 0.6, 1.0, 1.0];

#[cfg(test)]
#[derive(Clone, Copy)]
enum EditorObjectOutlineStyle {
    Selection,
}

#[cfg(test)]
impl EditorObjectOutlineStyle {
    fn color(self) -> [f32; 4] {
        match self {
            Self::Selection => EDITOR_SELECTION_OUTLINE_COLOR,
        }
    }
}

#[cfg(test)]
fn is_camera_trigger_block(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile == BlockRenderProfile::CameraTrigger
}

#[cfg(test)]
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

#[cfg(test)]
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
    }
}

#[cfg(test)]
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

#[cfg(test)]
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

super::editor_context_methods! {
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

    #[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::{
        EDITOR_SELECTION_OUTLINE_COLOR, EditorObjectOutlineStyle, EditorSubsystem,
        build_editor_object_outline_vertices, transform_trigger_markers_for_triggers,
    };
    use eg_mesh::build_block_geometry_for_object;
    use eg_types::{
        EasingDirection, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
    };

    fn block(position: [f32; 3], size: [f32; 3]) -> LevelObject {
        LevelObject {
            id: "target".to_string(),
            position,
            size,
            ..LevelObject::default()
        }
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

    #[test]
    fn camera_trigger_selection_outline_uses_visual_mesh_instead_of_cube_hull() {
        let object = LevelObject::new("core/camera_trigger");
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

    #[test]
    fn topmost_block_index_at_cursor_prefers_highest_overlapping_block() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![
            block([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            block([0.0, 1.0, 0.0], [1.0, 2.0, 1.0]),
            block([2.0, 0.0, 2.0], [1.0, 1.0, 1.0]),
        ];

        assert_eq!(
            editor.topmost_block_index_at_cursor([0.0, 0.0, 0.0]),
            Some(1)
        );
        assert_eq!(editor.topmost_block_index_at_cursor([4.0, 0.0, 4.0]), None);
    }

    #[test]
    fn transform_trigger_marker_sources_reflect_prior_trigger_state() {
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

    #[test]
    fn transform_trigger_marker_without_sources_still_draws_target() {
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
}
