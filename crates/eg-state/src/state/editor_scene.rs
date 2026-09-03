/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_blocks::BlockDefinition;
use eg_editor_domain::{create_block_at_cursor, derive_timeline_elapsed_seconds};
use eg_types::EditorPlaceMode;
super::editor_context_methods! {
    pub fn sync_objects(&mut self) {
        self.normalize_block_selection();
        self.sync_trigger_selection_from_objects();
        self.pick_spatial_index.invalidate();
        self.selected_mask_cache = None;
        self.block_static_vertex_cache.clear();
        self.block_static_vertex_cache_complete_len = None;
    }

    pub fn sync_objects_for_drag(&mut self) {
        self.normalize_block_selection();
        self.pick_spatial_index.invalidate();
    }

    pub fn sync_objects_after_drag_release(&mut self) {
        self.normalize_block_selection();
        self.pick_spatial_index.invalidate();
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
        self.sync_objects();
        placed_index
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

}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
    use bevy_state::prelude::State;
    use eg_mesh::build_block_geometry_for_object;
    use eg_types::{
        AppPhase, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TriggerTarget,
    };

    use crate::state::{
        RenderEditorSelectionOverlayRes, RenderEditorTransformTriggerMarkerVerticesRes,
        replace_editor_objects, test_support::TestApp, with_editor_interaction_context,
    };

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
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        let object = LevelObject::new("core/camera_trigger");
        replace_editor_objects(app.world_mut(), vec![object.clone()]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Select;
            editor.input.selected_block_index = Some(0);
            editor.input.selected_block_indices = vec![0];
        });

        let mut schedule = Schedule::default();
        schedule
            .add_systems(crate::state::render::plugin::extract_editor_selection_overlays_system);
        schedule.run(app.world_mut());

        let visual_vertices = build_block_geometry_for_object(&object).to_triangle_vertices();
        let selection = app.world().resource::<RenderEditorSelectionOverlayRes>();

        assert_ne!(visual_vertices.len(), 36);
        assert_eq!(selection.outline_vertices.len(), visual_vertices.len());
        assert!(
            selection
                .outline_vertices
                .iter()
                .all(|vertex| vertex.color == [0.098, 0.6, 1.0, 1.0])
        );
        assert!(
            selection
                .outline_vertices
                .iter()
                .all(|vertex| (vertex.render_profile - 3.0).abs() <= f32::EPSILON)
        );
    }

    #[test]
    fn transform_trigger_marker_extraction_uses_authored_objects_and_selection() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(
            app.world_mut(),
            vec![
                LevelObject {
                    id: "target".to_string(),
                    ..LevelObject::new("core/stone")
                },
                transform_trigger_object(
                    3.0,
                    1.0,
                    [10.0, 0.0, 0.0],
                    [0.0, 180.0, 0.0],
                    [3.0, 1.0, 1.0],
                ),
                transform_trigger_object(
                    1.0,
                    4.0,
                    [8.0, 0.0, 0.0],
                    [0.0, 90.0, 0.0],
                    [2.0, 1.0, 1.0],
                ),
            ],
        );
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.set_trigger_selected(Some(0));
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                crate::state::editor_objects::observe_editor_object_changes_system,
                crate::state::render::plugin::extract_editor_transform_trigger_marker_vertices_system,
            )
                .chain(),
        );
        schedule.run(app.world_mut());

        let marker_updates = &app
            .world()
            .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
            .updates;
        assert!(!marker_updates.is_empty());
        let selected_updates = marker_updates
            .iter()
            .filter(|update| {
                update
                    .geometry
                    .vertices
                    .iter()
                    .any(|vertex| vertex.color == [1.0, 0.8, 0.18, 0.96])
            })
            .collect::<Vec<_>>();
        assert_eq!(selected_updates.len(), 1);
        assert!(selected_updates[0].marker_id.starts_with("trigger_1:"));

        schedule.run(app.world_mut());
        assert!(
            app.world()
                .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
                .updates
                .is_empty()
        );

        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.set_trigger_selected(Some(1));
        });
        schedule.run(app.world_mut());
        let selection_updates = &app
            .world()
            .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
            .updates;
        assert_eq!(selection_updates.len(), 2);

        replace_editor_objects(
            app.world_mut(),
            vec![
                LevelObject {
                    id: "target".to_string(),
                    ..LevelObject::new("core/stone")
                },
                transform_trigger_object(
                    3.0,
                    1.0,
                    [10.0, 0.0, 0.0],
                    [0.0, 180.0, 0.0],
                    [3.0, 1.0, 1.0],
                ),
            ],
        );
        schedule.run(app.world_mut());
        let marker_vertices = app
            .world()
            .resource::<RenderEditorTransformTriggerMarkerVerticesRes>();
        assert_eq!(marker_vertices.removals, vec!["trigger_1:target:0"]);
    }

    #[test]
    fn transform_trigger_marker_without_sources_still_draws_target() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(
            app.world_mut(),
            vec![LevelObject {
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
            }],
        );
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.set_trigger_selected(Some(0));
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            crate::state::render::plugin::extract_editor_transform_trigger_marker_vertices_system,
        );
        schedule.run(app.world_mut());

        assert!(
            !app.world()
                .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
                .updates
                .is_empty()
        );
    }

    #[test]
    fn transform_trigger_marker_ties_preserve_authored_order() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        let mut first =
            transform_trigger_object(1.0, 1.0, [4.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        first.id = "trigger_first".to_string();
        let mut second =
            transform_trigger_object(1.0, 1.0, [8.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        second.id = "trigger_second".to_string();
        replace_editor_objects(
            app.world_mut(),
            vec![
                LevelObject {
                    id: "target".to_string(),
                    ..LevelObject::new("core/stone")
                },
                first,
                second,
            ],
        );
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.set_trigger_selected(Some(0));
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            crate::state::render::plugin::extract_editor_transform_trigger_marker_vertices_system,
        );
        schedule.run(app.world_mut());

        let selected_update = app
            .world()
            .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
            .updates
            .iter()
            .find(|update| {
                update
                    .geometry
                    .vertices
                    .iter()
                    .any(|vertex| vertex.color == [1.0, 0.8, 0.18, 0.96])
            })
            .expect("one trigger marker should be selected");
        assert!(selected_update.marker_id.starts_with("trigger_first:"));
    }
}
