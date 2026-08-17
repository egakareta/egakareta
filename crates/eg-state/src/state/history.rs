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

pub(crate) const MAX_EDITOR_HISTORY: usize = 256;

/// Record a complete pre-mutation editor snapshot, retaining bounded undo
/// history and invalidating any redo branch.
pub(crate) fn record_history_snapshot(
    history: &mut EditorHistoryState,
    snapshot: EditorHistorySnapshot,
) {
    if history.undo.len() >= MAX_EDITOR_HISTORY {
        history.undo.remove(0);
    }
    history.undo.push(snapshot);
    history.redo.clear();
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
        self.record_history_state_force();
    }

    pub(super) fn record_history_state_force(&mut self) {
        let snapshot = self.history_snapshot();
        record_history_snapshot(&mut self.runtime.history, snapshot);
    }

    #[cfg(test)]
    fn apply_history_snapshot(&mut self, snapshot: EditorHistorySnapshot) {
        *self.objects = snapshot.objects;
        self.input.selected_block_index = snapshot.selected_block_index;
        self.input.selected_block_indices = snapshot.selected_block_indices;
        self.normalize_block_selection();
        self.input.hovered_block_index = self.input.selected_block_index;
        self.input.cursor = snapshot.cursor;
        self.config.selected_block_id = snapshot.selected_block_id;
        self.config.selected_block_rotation_degrees = snapshot.selected_block_rotation_degrees;
        self.config.recent_block_ids = snapshot.recent_block_ids;
        *self.spawn = snapshot.spawn;
        self.timeline.clock.time_seconds = snapshot.timeline_time_seconds.max(0.0);
        self.timeline.clock.duration_seconds = snapshot.timeline_duration_seconds.max(0.1);
        self.timeline.taps.tap_times = snapshot.tap_times;
        self.timeline.taps.tap_indicator_positions = snapshot.tap_indicator_positions;
        self.timeline.taps.selected_index = snapshot
            .selected_tap_index
            .filter(|index| *index < self.timeline.taps.tap_times.len());
        self.timing.timing_points = snapshot.timing_points;
        self.timing.mark_timing_points_changed();
        self.sync_trigger_selection_from_objects();
        let trigger_count = self.trigger_objects().len();
        self.triggers.selected_index = snapshot
            .selected_trigger_index
            .filter(|index| *index < trigger_count);

        self.timeline
            .taps
            .tap_times
            .retain(|tap| tap.is_finite() && *tap >= 0.0);
        self.timeline.taps.tap_times.sort_by(f32::total_cmp);
        self.timeline
            .taps
            .tap_times
            .retain(|tap| *tap <= self.timeline.clock.duration_seconds);

        if self.timeline.taps.tap_indicator_positions.len() != self.timeline.taps.tap_times.len() {
            self.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
                self.spawn.position,
                self.spawn.direction,
                &self.timeline.taps.tap_times,
                &self.objects,
            );
        }
        self.timeline.taps.selected_index = self
            .timeline
            .taps
            .selected_index
            .filter(|index| *index < self.timeline.taps.tap_times.len());

        self.timeline.clock.time_seconds = self
            .timeline
            .clock
            .time_seconds
            .min(self.timeline.clock.duration_seconds);
        self.sync_trigger_selection_from_objects();

        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        self.timeline.invalidate_samples();
        self.pick_spatial_index.invalidate();
    }

    #[cfg(test)]
    pub(super) fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.runtime.history.undo.pop() {
            let current = self.history_snapshot();
            self.runtime.history.redo.push(current);
            self.apply_history_snapshot(snapshot);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(super) fn redo(&mut self) -> bool {
        if let Some(snapshot) = self.runtime.history.redo.pop() {
            let current = self.history_snapshot();
            self.runtime.history.undo.push(current);
            self.apply_history_snapshot(snapshot);
            true
        } else {
            false
        }
    }

}

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_blocks::BlockId;
    use eg_types::{AppPhase, LevelObject, SpawnMetadata};

    use crate::{
        events::{
            block::{CopyBlock, DuplicateBlock, PasteBlock},
            history::{Redo, Undo},
        },
        state::{
            EditorClipboard, EditorHistoryRes, EditorHistorySnapshot, EditorHistoryState,
            EditorInputRes, EditorRuntimeRes, EditorTimelineRes, EditorTriggerRes,
            collect_editor_objects_from_world, record_history_snapshot, replace_editor_objects,
            test_support::TestApp,
        },
    };

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

    fn editor_test_app(objects: Vec<LevelObject>, selected_indices: Vec<usize>) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        let mut input = app.world_mut().resource_mut::<EditorInputRes>();
        input.0.selected_block_index = selected_indices.first().copied();
        input.0.selected_block_indices = selected_indices;
        app
    }

    fn test_history_snapshot(cursor: [f32; 3]) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: Vec::new(),
            selected_block_index: None,
            selected_block_indices: Vec::new(),
            cursor,
            selected_block_id: BlockId::new("core/stone"),
            selected_block_rotation_degrees: [0.0; 3],
            recent_block_ids: Vec::new(),
            spawn: SpawnMetadata::default(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 1.0,
            tap_times: Vec::new(),
            tap_indicator_positions: Vec::new(),
            selected_tap_index: None,
            timing_points: Vec::new(),
            selected_trigger_index: None,
        }
    }

    #[test]
    fn test_history_max_limit() {
        let mut history = EditorHistoryState::default();

        // Record more than 256 times
        for i in 0..300 {
            record_history_snapshot(&mut history, test_history_snapshot([i as f32, 0.0, 0.0]));
        }

        assert_eq!(history.undo.len(), 256);
        // The first one should be i=44 (300 - 256)
        assert_eq!(history.undo[0].cursor[0], 44.0);
    }

    #[test]
    fn test_undo_redo_clears_redo_on_record() {
        let mut history = EditorHistoryState {
            redo: vec![test_history_snapshot([0.0; 3])],
            ..EditorHistoryState::default()
        };

        record_history_snapshot(&mut history, test_history_snapshot([1.0, 0.0, 0.0]));
        assert_eq!(history.undo.len(), 1);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_apply_snapshot_constraints() {
        let mut app = editor_test_app(Vec::new(), Vec::new());
        let mut snapshot = test_history_snapshot([0.0; 3]);
        snapshot.timeline_time_seconds = -10.0;
        snapshot.timeline_duration_seconds = 0.0; // Should be clamped to 0.1
        snapshot.tap_times = vec![5.0, -1.0, f32::NAN, 2.0];
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .undo
            .push(snapshot);

        app.world_mut().write_message(Undo);
        app.tick();

        let timeline = &app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.clock.time_seconds, 0.0);
        assert_eq!(timeline.clock.duration_seconds, 0.1);

        // Tap times: -1.0 and NAN removed, 5.0 clamped by duration 0.1, 2.0 clamped by duration 0.1
        // Wait, duration is 0.1, so all tap times > 0.1 are removed by retain(|tap| *tap <= duration)
        assert!(timeline.taps.tap_times.is_empty());
    }

    #[test]
    fn test_copy_paste_multiple_blocks() {
        let mut app = editor_test_app(
            vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])],
            vec![0, 1],
        );

        app.world_mut().write_message(CopyBlock);
        app.tick();
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [20.0, 5.0, 20.0];
        app.world_mut().write_message(PasteBlock);
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        // Pasted blocks should be relative to anchor [10, 0, 10]
        // Block 0 at [10, 0, 10] -> [20 + (10-10), 5 + (0-0), 20 + (10-10)] = [20, 5, 20]
        // Block 1 at [12, 1, 12] -> [20 + (12-10), 5 + (1-0), 20 + (12-10)] = [22, 6, 22]
        assert_eq!(objects[2].position, [20.0, 5.0, 20.0]);
        assert_eq!(objects[3].position, [22.0, 6.0, 22.0]);

        // Selection should be the two new blocks
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
        let mut app = editor_test_app(vec![test_block([10.0, 0.0, 10.0])], Vec::new());
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [10.0, 0.0, 10.0];

        app.world_mut().write_message(CopyBlock);
        app.tick();
        assert!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .clipboard
                .is_some()
        );

        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [30.0, 0.0, 30.0];
        app.world_mut().write_message(PasteBlock);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[1].position, [30.0, 0.0, 30.0]);
    }

    #[test]
    fn test_duplicate_selected_multiple() {
        let mut app = editor_test_app(
            vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])],
            vec![1, 0],
        );

        app.world_mut().write_message(DuplicateBlock);
        app.tick();
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 4);
        assert_eq!(objects[2].position, [10.0, 0.0, 10.0]);
        assert_eq!(objects[3].position, [12.0, 1.0, 12.0]);

        // Selection should be the two new blocks
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
        let mut app = editor_test_app(vec![test_block([0.0, 0.0, 0.0])], Vec::new());
        app.world_mut().write_message(PasteBlock);
        app.tick();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .clipboard = Some(EditorClipboard {
            objects: vec![],
            anchor: [0.0, 0.0, 0.0],
        });
        app.world_mut().write_message(PasteBlock);
        app.tick();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
    }

    #[test]
    fn test_undo_redo_empty_stacks() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .undo
            .clear();
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .redo
            .clear();

        app.world_mut().write_message(Undo);
        app.tick();
        app.world_mut().write_message(Redo);
        app.tick();
        let history = &app.world().resource::<EditorHistoryRes>().0;
        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_duplicate_empty_selection() {
        let mut app = editor_test_app(vec![test_block([0.0, 0.0, 0.0])], Vec::new());
        app.world_mut().write_message(DuplicateBlock);
        app.tick();
        assert_eq!(collect_editor_objects_from_world(app.world_mut()).len(), 1);
    }

    #[test]
    fn test_apply_snapshot_selection_filtering() {
        let object = test_block([0.0, 0.0, 0.0]);
        let mut app = editor_test_app(vec![object.clone()], Vec::new());
        let mut snapshot = test_history_snapshot([0.0; 3]);
        snapshot.objects = vec![object];
        snapshot.selected_block_index = Some(10); // Out of bounds
        snapshot.selected_block_indices = vec![0, 10]; // 10 is out of bounds
        snapshot.selected_trigger_index = Some(5); // Out of bounds
        app.world_mut()
            .resource_mut::<EditorHistoryRes>()
            .0
            .undo
            .push(snapshot);

        app.world_mut().write_message(Undo);
        app.tick();

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
}
