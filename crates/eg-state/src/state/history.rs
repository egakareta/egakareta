/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorHistorySnapshot;

#[derive(Default, Clone)]
pub struct EditorHistoryState {
    pub undo: Vec<EditorHistorySnapshot>,
    pub redo: Vec<EditorHistorySnapshot>,
}

super::editor_context_methods! {
    fn history_snapshot(&self) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: self.objects.clone(),
            selected_block_index: self.input.selected_block_index,
            selected_block_indices: self.input.selected_block_indices.clone(),
            cursor: self.input.cursor,
            selected_block_id: self.config.selected_block_id.clone(),
            selected_block_rotation_degrees: self.config.selected_block_rotation_degrees,
            recent_block_ids: self.config.recent_block_ids.clone(),
            spawn: self.spawn.clone(),
            timeline_time_seconds: self.timeline.clock.time_seconds,
            timeline_duration_seconds: self.timeline.clock.duration_seconds,
            tap_times: self.timeline.taps.tap_times.clone(),
            tap_indicator_positions: self.timeline.taps.tap_indicator_positions.clone(),
            selected_tap_index: self.timeline.taps.selected_index,
            timing_points: self.timing.timing_points.clone(),
            selected_trigger_index: self.triggers.selected_index,
        }
    }

    pub(crate) fn record_history_state(&mut self) {
        const MAX_HISTORY: usize = 256;
        if self.history.undo.len() >= MAX_HISTORY {
            self.history.undo.remove(0);
        }
        let snapshot = self.history_snapshot();
        self.history.undo.push(snapshot);
        self.history.redo.clear();
    }

}

#[cfg(test)]
mod tests {
    use bevy_state::prelude::State;
    use eg_types::{
        AppPhase, EasingDirection, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger,
        TriggerTarget,
    };

    use crate::state::{
        EditorClipboard, EditorHistoryRes, EditorInputRes, EditorTimelineRes, EditorTriggerRes,
        collect_editor_objects_from_world, replace_editor_objects, test_support::TestApp,
        with_editor_interaction_context,
    };

    fn editor_app(objects: Vec<LevelObject>) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        app
    }

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!(
                "object_{:.0}_{:.0}_{:.0}",
                position[0], position[1], position[2]
            ),
            position,
            ..LevelObject::default()
        }
    }

    fn test_transform_trigger(object_ids: Vec<&str>) -> LevelObject {
        LevelObject {
            id: "trigger".to_string(),
            position: [9.0, 0.0, 0.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects {
                    object_ids: object_ids.into_iter().map(str::to_string).collect(),
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    #[test]
    fn test_history_max_limit() {
        let mut app = editor_app(vec![test_block([0.0, 0.0, 0.0])]);

        for i in 0..300 {
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.config.snap_step = 1.0;
                editor.objects[0].position = [i as f32 + 0.25, 0.0, 0.0];
                editor.input.selected_block_index = Some(0);
                editor.input.selected_block_indices = vec![0];
            });
            app.world_mut()
                .write_message(crate::events::selection::SnapSelectionToGrid);
            app.tick_state();
        }

        let history = &app.world().resource::<EditorHistoryRes>().0;
        assert_eq!(history.undo.len(), 256);
        assert_eq!(history.undo[0].objects[0].position[0], 44.25);
    }

    #[test]
    fn test_undo_redo_clears_redo_on_record() {
        let mut app = editor_app(vec![test_block([0.0, 0.0, 0.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_index = Some(0);
            editor.input.selected_block_indices = vec![0];
        });

        app.world_mut()
            .write_message(crate::events::block::RemoveBlock);
        app.tick_state();
        app.world_mut().write_message(crate::events::history::Undo);
        app.tick_state();
        assert_eq!(app.world().resource::<EditorHistoryRes>().0.redo.len(), 1);

        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_step = 1.0;
            editor.objects[0].position = [0.25, 0.0, 0.0];
            editor.input.selected_block_index = Some(0);
            editor.input.selected_block_indices = vec![0];
        });
        app.world_mut()
            .write_message(crate::events::selection::SnapSelectionToGrid);
        app.tick_state();

        assert!(app.world().resource::<EditorHistoryRes>().0.redo.is_empty());
    }

    #[test]
    fn test_apply_snapshot_constraints() {
        let mut app = editor_app(Vec::new());
        let mut snapshot =
            with_editor_interaction_context(app.world_mut(), |editor| editor.history_snapshot());
        snapshot.timeline_time_seconds = -10.0;
        snapshot.timeline_duration_seconds = 0.0;
        snapshot.tap_times = vec![5.0, -1.0, f32::NAN, 2.0];
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .undo
            .push(snapshot);

        app.world_mut().write_message(crate::events::history::Undo);
        app.tick_state();

        let timeline = &app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.clock.time_seconds, 0.0);
        assert_eq!(timeline.clock.duration_seconds, 0.1);
        assert!(timeline.taps.tap_times.is_empty());
    }

    #[test]
    fn test_copy_paste_multiple_blocks() {
        let mut app = editor_app(vec![
            test_block([10.0, 0.0, 10.0]),
            test_block([12.0, 1.0, 12.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices = vec![0, 1];
            editor.input.selected_block_index = Some(0);
        });

        app.world_mut()
            .write_message(crate::events::block::CopyBlock);
        app.tick_state();
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.cursor = [20.0, 5.0, 20.0];
        });
        app.world_mut()
            .write_message(crate::events::block::PasteBlock);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        assert_eq!(objects[2].position, [20.0, 5.0, 20.0]);
        assert_eq!(objects[3].position, [22.0, 6.0, 22.0]);
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![2, 3]
        );
    }

    #[test]
    fn test_copy_topmost_at_cursor() {
        let mut app = editor_app(vec![test_block([10.0, 0.0, 10.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices.clear();
            editor.input.selected_block_index = None;
            editor.input.cursor = [10.0, 0.0, 10.0];
        });

        app.world_mut()
            .write_message(crate::events::block::CopyBlock);
        app.tick_state();
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.cursor = [30.0, 0.0, 30.0];
        });
        app.world_mut()
            .write_message(crate::events::block::PasteBlock);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[1].position, [30.0, 0.0, 30.0]);
    }

    #[test]
    fn test_duplicate_selected_multiple() {
        let mut app = editor_app(vec![
            test_block([10.0, 0.0, 10.0]),
            test_block([12.0, 1.0, 12.0]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices = vec![0, 1];
            editor.input.selected_block_index = Some(1);
        });

        app.world_mut()
            .write_message(crate::events::block::DuplicateBlock);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        assert_eq!(objects[2].position, [10.0, 0.0, 10.0]);
        assert_eq!(objects[3].position, [12.0, 1.0, 12.0]);
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![2, 3]
        );
    }

    #[test]
    fn test_empty_clipboard_paste() {
        let mut app = editor_app(vec![test_block([0.0, 0.0, 0.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.runtime.interaction.clipboard = None;
        });
        app.world_mut()
            .write_message(crate::events::block::PasteBlock);
        app.tick_state();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);

        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.runtime.interaction.clipboard = Some(EditorClipboard {
                objects: vec![],
                anchor: [0.0, 0.0, 0.0],
            });
        });
        app.world_mut()
            .write_message(crate::events::block::PasteBlock);
        app.tick_state();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
    }

    #[test]
    fn test_undo_redo_empty_stacks() {
        let mut app = editor_app(Vec::new());
        app.world_mut().write_message(crate::events::history::Undo);
        app.world_mut().write_message(crate::events::history::Redo);
        app.tick_state();

        let history = &app.world().resource::<EditorHistoryRes>().0;
        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_duplicate_empty_selection() {
        let mut app = editor_app(vec![test_block([0.0, 0.0, 0.0])]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_indices.clear();
            editor.input.selected_block_index = None;
        });
        app.world_mut()
            .write_message(crate::events::block::DuplicateBlock);
        app.tick_state();

        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
    }

    #[test]
    fn test_apply_snapshot_selection_filtering() {
        let mut app = editor_app(vec![test_block([0.0, 0.0, 0.0])]);
        let mut snapshot =
            with_editor_interaction_context(app.world_mut(), |editor| editor.history_snapshot());
        snapshot.selected_block_index = Some(10);
        snapshot.selected_block_indices = vec![0, 10];
        snapshot.selected_trigger_index = Some(5);
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .undo
            .push(snapshot);

        app.world_mut().write_message(crate::events::history::Undo);
        app.tick_state();

        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_index, Some(0));
        assert_eq!(input.selected_block_indices, vec![0]);
        assert!(
            app.world()
                .resource::<EditorTriggerRes>()
                .0
                .selected_index
                .is_none()
        );
    }

    #[test]
    fn remove_block_dereferences_deleted_transform_trigger_target() {
        let mut app = editor_app(vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
            test_transform_trigger(vec!["object_2_0_0"]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_index = Some(1);
            editor.input.selected_block_indices = vec![1];
        });

        app.world_mut()
            .write_message(crate::events::block::RemoveBlock);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 3);
        let trigger = objects
            .iter()
            .find(|object| object.trigger.is_some())
            .and_then(|object| object.trigger.as_ref())
            .expect("trigger remains");
        assert_eq!(
            trigger.target,
            TriggerTarget::Objects {
                object_ids: Vec::new(),
            }
        );
    }

    #[test]
    fn remove_block_preserves_transform_trigger_targets_after_deleted_block() {
        let mut app = editor_app(vec![
            test_block([0.0, 0.0, 0.0]),
            test_block([2.0, 0.0, 0.0]),
            test_block([4.0, 0.0, 0.0]),
            test_transform_trigger(vec!["object_4_0_0"]),
        ]);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_index = Some(0);
            editor.input.selected_block_indices = vec![0];
        });

        app.world_mut()
            .write_message(crate::events::block::RemoveBlock);
        app.tick_state();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects[1].position, [4.0, 0.0, 0.0]);
        let trigger = objects
            .iter()
            .find(|object| object.trigger.is_some())
            .and_then(|object| object.trigger.as_ref())
            .expect("trigger remains");
        assert_eq!(
            trigger.target,
            TriggerTarget::Objects {
                object_ids: vec!["object_4_0_0".to_string()],
            }
        );
    }
}
