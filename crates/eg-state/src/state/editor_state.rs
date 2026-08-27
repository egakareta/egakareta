/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::add_tap_with_indicator;

super::editor_context_methods! {
    pub fn clear_interaction_drags(&mut self) {
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
    }

    pub fn effective_snap_to_grid(&self) -> bool {
        self.config.snap_to_grid && !self.input.ctrl_held
    }

    pub fn effective_snap_rotation(&self) -> bool {
        self.config.snap_rotation && !self.input.ctrl_held
    }

    pub fn set_selected_tap_index(&mut self, selected_index: Option<usize>) {
        self.timeline.taps.selected_index = selected_index.filter(|index| {
            *index < self.timeline.taps.tap_times.len()
                && *index < self.timeline.taps.tap_indicator_positions.len()
        });
    }

    pub fn set_timeline_time_seconds(&mut self, time_seconds: f32) -> bool {
        let old_time = self.timeline.clock.time_seconds;
        self.timeline.clock.time_seconds =
            time_seconds.clamp(0.0, self.timeline.clock.duration_seconds);
        (self.timeline.clock.time_seconds - old_time).abs() > 0.0001
    }

    pub fn tap_indicator_position_from_world(&self, position: [f32; 3]) -> [f32; 3] {
        [position[0] - 0.5, position[1], position[2] - 0.5]
    }

    pub fn add_tap(&mut self, indicator_position: [f32; 3]) -> f32 {
        let selected_index = add_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
            indicator_position,
        );
        self.set_selected_tap_index(selected_index);
        self.timeline.clock.time_seconds
    }

}

#[cfg(test)]
mod tests {
    use bevy_state::prelude::State;
    use eg_blocks::BlockId;
    use eg_types::{
        AppPhase, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TriggerTarget,
    };

    use super::super::{
        EditorInputRes, EditorTimelineRes, EditorTimingRes, collect_editor_objects_from_world,
        replace_editor_objects, test_support::TestApp, with_editor_interaction_context,
    };

    fn editor_app(objects: Vec<LevelObject>) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        app
    }

    fn test_level_object(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!("object_{:.0}", position[0]),
            position,
            ..LevelObject::default()
        }
    }

    fn assert_object_pose(object: &LevelObject, position: [f32; 3], size: [f32; 3]) {
        for component in 0..3 {
            approx::assert_abs_diff_eq!(
                object.position[component],
                position[component],
                epsilon = 1e-6
            );
            approx::assert_abs_diff_eq!(object.size[component], size[component], epsilon = 1e-6);
        }
    }

    fn transform_trigger(
        time_seconds: f32,
        object_ids: Vec<&str>,
        position: [f32; 3],
    ) -> LevelObject {
        let object_ids: Vec<String> = object_ids.into_iter().map(str::to_string).collect();
        LevelObject {
            id: format!("trigger_{time_seconds:.1}_{}", object_ids.join("_")),
            position,
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects { object_ids },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    fn transform_trigger_object(time_seconds: f32, target_ids: Vec<&str>) -> LevelObject {
        transform_trigger(time_seconds, target_ids, [9.0, 0.0, 0.0])
    }

    #[test]
    fn add_transform_trigger_ignores_trigger_block_selection() {
        let mut app = editor_app(vec![transform_trigger_object(1.0, vec!["object_0"])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices = vec![0];
            editor.input.selected_block_index = Some(0);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 1);
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.trigger.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn add_transform_trigger_creates_selected_markers_for_selected_blocks() {
        let mut app = editor_app(vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.time_seconds = 2.5;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.input.selected_block_indices = vec![0, 1];
            editor.input.selected_block_index = Some(0);
            editor.input.mode = EditorMode::Rotate;
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        assert_object_pose(&objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_object_pose(&objects[1], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        for (trigger_object, expected_id) in objects[2..].iter().zip(["object_0", "object_4"]) {
            assert_eq!(trigger_object.block_id, TRANSFORM_TRIGGER_BLOCK_ID);
            assert_object_pose(
                trigger_object,
                [if expected_id == "object_0" { 0.0 } else { 4.0 }, 0.0, 0.0],
                [1.0, 1.0, 1.0],
            );
            assert_eq!(trigger_object.rotation, [0.0, 0.0, 0.0]);
            let trigger = trigger_object
                .trigger
                .as_ref()
                .expect("trigger object payload");
            approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
            approx::assert_abs_diff_eq!(trigger.duration_seconds, 1.0, epsilon = 1e-6);
            assert_eq!(trigger.easing, EasingDirection::EaseInOut);
            assert_eq!(
                trigger.target,
                TriggerTarget::Objects {
                    object_ids: vec![expected_id.to_string()],
                }
            );
        }

        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.mode, EditorMode::Rotate);
        assert_eq!(input.selected_block_index, Some(2));
        assert_eq!(input.selected_block_indices, vec![2, 3]);
    }

    #[test]
    fn add_transform_trigger_targets_block_after_existing_trigger_object() {
        let mut app = editor_app(vec![
            test_level_object([0.0, 0.0, 0.0]),
            transform_trigger_object(1.0, vec!["object_0"]),
            test_level_object([4.0, 0.0, 0.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.time_seconds = 2.5;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.input.selected_block_indices = vec![2];
            editor.input.selected_block_index = Some(2);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        assert_object_pose(&objects[2], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_index, Some(3));
        assert_eq!(input.selected_block_indices, vec![3]);

        let mut found_retargeted_capture = false;
        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
            let trigger = trigger_object
                .trigger
                .as_ref()
                .expect("trigger object payload");
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                continue;
            };
            if (trigger.time_seconds - 2.5).abs() <= 1e-6 {
                assert_eq!(object_ids, &["object_4".to_string()]);
                found_retargeted_capture = true;
                assert_object_pose(trigger_object, [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            }
        }

        assert!(found_retargeted_capture);
    }

    #[test]
    fn add_transform_trigger_preserves_existing_moved_trigger_object_pose() {
        let mut existing_trigger = transform_trigger(2.5, vec!["object_0"], [8.0, 0.0, 0.0]);
        existing_trigger.position = [12.0, 1.0, 3.0];
        existing_trigger.size = [2.0, 3.0, 4.0];
        existing_trigger.rotation = [10.0, 20.0, 30.0];
        let mut app = editor_app(vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
            existing_trigger,
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.time_seconds = 2.5;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.input.selected_block_indices = vec![1];
            editor.input.selected_block_index = Some(1);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let existing_trigger_object = objects
            .iter()
            .find(|object| {
                object.trigger.as_ref().is_some_and(|trigger| {
                    trigger.target
                        == TriggerTarget::Objects {
                            object_ids: vec!["object_0".to_string()],
                        }
                })
            })
            .expect("existing transform trigger remains");
        assert_object_pose(existing_trigger_object, [12.0, 1.0, 3.0], [2.0, 3.0, 4.0]);
        assert_eq!(existing_trigger_object.rotation, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn add_transform_trigger_overrides_existing_same_time_object_triggers() {
        let mut app = editor_app(vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
            test_level_object([8.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_4", "object_8"], [20.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_8"], [30.0, 0.0, 0.0]),
            transform_trigger(4.0, vec!["object_0"], [40.0, 0.0, 0.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.time_seconds = 2.5;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.input.selected_block_indices = vec![0, 1];
            editor.input.selected_block_index = Some(0);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let mut object_zero_same_time_count = 0;
        let mut object_one_same_time_count = 0;
        let mut retained_object_two_multi_target = false;
        let mut retained_object_two_standalone = false;
        let mut retained_different_time_object_zero = false;

        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
            let trigger = trigger_object
                .trigger
                .as_ref()
                .expect("trigger object payload");
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                continue;
            };

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &["object_0".to_string()]
            {
                object_zero_same_time_count += 1;
                assert_object_pose(trigger_object, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            }

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &["object_4".to_string()]
            {
                object_one_same_time_count += 1;
                assert_object_pose(trigger_object, [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            }

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &["object_8".to_string()]
            {
                if (trigger_object.position[0] - 20.0).abs() <= 1e-6 {
                    retained_object_two_multi_target = true;
                } else if (trigger_object.position[0] - 30.0).abs() <= 1e-6 {
                    retained_object_two_standalone = true;
                }
            }

            if (trigger.time_seconds - 4.0).abs() <= 1e-6 && object_ids == &["object_0".to_string()]
            {
                retained_different_time_object_zero = true;
            }
        }

        assert_eq!(object_zero_same_time_count, 1);
        assert_eq!(object_one_same_time_count, 1);
        assert!(retained_object_two_multi_target);
        assert!(retained_object_two_standalone);
        assert!(retained_different_time_object_zero);
    }

    #[test]
    fn add_transform_trigger_preserves_replaced_trigger_with_no_sources() {
        let mut app = editor_app(vec![
            test_level_object([0.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.timeline.clock.time_seconds = 2.5;
            editor.timeline.clock.duration_seconds = 8.0;
            editor.input.selected_block_indices = vec![0];
            editor.input.selected_block_index = Some(0);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let mut found_target_only_trigger = false;
        let mut found_replacement_trigger = false;
        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
            let trigger = trigger_object
                .trigger
                .as_ref()
                .expect("trigger object payload");
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                continue;
            };

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids.is_empty() {
                found_target_only_trigger = true;
                approx::assert_abs_diff_eq!(trigger_object.position[0], 10.0, epsilon = 1e-6);
            }

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &["object_0".to_string()]
            {
                found_replacement_trigger = true;
                assert_object_pose(trigger_object, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            }
        }

        assert!(found_target_only_trigger);
        assert!(found_replacement_trigger);
    }

    #[test]
    fn add_transform_trigger_creates_selected_trigger_block() {
        let mut app = editor_app(vec![test_level_object([0.0, 0.0, 0.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices = vec![0];
            editor.input.selected_block_index = Some(0);
        });

        app.world_mut()
            .write_message(crate::events::selection::AddTransformTrigger);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_object_pose(&objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.trigger.is_some())
                .count(),
            1
        );
        assert_eq!(objects[1].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![1]
        );
    }

    #[test]
    fn selected_block_update_message_applies_snap_and_clamp_rules() {
        let mut app = editor_app(vec![LevelObject::default()]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_index = Some(0);
            editor.config.snap_to_grid = true;
            editor.config.snap_step = 0.5;
            editor.config.snap_rotation = true;
            editor.config.snap_rotation_step_degrees = 15.0;
            editor.input.ctrl_held = false;
        });

        app.world_mut()
            .write_message(crate::events::block::UpdateSelectedBlock(LevelObject {
                position: [1.24, -1.0, 2.26],
                size: [0.1, 0.4, 0.62],
                rotation: [7.0, 22.0, 44.0],
                color_tint: [-1.0, 0.4, 2.0],
                block_id: BlockId::new("stone"),
                ..LevelObject::default()
            }));
        app.tick_state();

        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert_eq!(object.position, [1.0, 0.0, 2.5]);
        assert_eq!(
            app.world().resource::<EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.5]
        );
        assert_eq!(object.size, [0.5, 0.5, 0.5]);
        assert_eq!(object.rotation, [0.0, 15.0, 45.0]);
        assert_eq!(object.color_tint, [0.0, 0.4, 1.0]);
        assert_eq!(object.block_id, "core/stone");
    }

    #[test]
    fn timeline_messages_clamp_time_and_duration() {
        let mut app = editor_app(Vec::new());

        app.world_mut()
            .write_message(crate::events::timeline::SetTimelineDuration(2.0));
        app.tick_state();
        app.world_mut()
            .write_message(crate::events::timeline::SetTimelineTime(5.0));
        app.tick_state();
        assert_eq!(
            app.world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            2.0
        );

        app.world_mut()
            .write_message(crate::events::timeline::SetTimelineDuration(0.01));
        app.tick_state();
        let clock = &app.world().resource::<EditorTimelineRes>().0.clock;
        assert_eq!(clock.duration_seconds, 0.1);
        assert_eq!(clock.time_seconds, 0.1);
    }

    #[test]
    fn bpm_tap_messages_update_and_reset_timing_state() {
        let mut app = TestApp::minimal();

        app.world_mut().write_message(crate::events::bpm::BpmTap);
        app.tick_state();
        let timing = &app.world().resource::<EditorTimingRes>().0;
        assert_eq!(timing.bpm_tap_times.len(), 1);
        assert!(timing.bpm_tap_result.is_none());

        app.world_mut().write_message(crate::events::bpm::BpmTap);
        app.tick_state();
        assert_eq!(
            app.world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .len(),
            2
        );
        assert!(
            app.world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_some()
        );

        app.world_mut()
            .write_message(crate::events::bpm::BpmTapReset);
        app.tick_state();
        let timing = &app.world().resource::<EditorTimingRes>().0;
        assert!(timing.bpm_tap_times.is_empty());
        assert!(timing.bpm_tap_result.is_none());
    }
}
