/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(test)]
use super::EditorClipboard;
use super::{EditorHistorySnapshot, EditorSubsystem};
#[cfg(test)]
use eg_editor_domain::derive_tap_indicator_positions;
#[cfg(test)]
use eg_types::{LevelObject, TriggerTarget, next_level_object_id};

#[derive(Default, Clone)]
pub struct EditorHistoryState {
    pub undo: Vec<EditorHistorySnapshot>,
    pub redo: Vec<EditorHistorySnapshot>,
}

impl EditorSubsystem {
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
        const MAX_HISTORY: usize = 256;
        if self.runtime.history.undo.len() >= MAX_HISTORY {
            self.runtime.history.undo.remove(0);
        }
        let snapshot = self.history_snapshot();
        self.runtime.history.undo.push(snapshot);
        self.runtime.history.redo.clear();
    }

    #[cfg(test)]
    fn apply_history_snapshot(&mut self, snapshot: EditorHistorySnapshot) {
        self.objects = snapshot.objects;
        self.input.selected_block_index = snapshot.selected_block_index;
        self.input.selected_block_indices = snapshot.selected_block_indices;
        self.normalize_block_selection();
        self.input.hovered_block_index = self.input.selected_block_index;
        self.input.cursor = snapshot.cursor;
        self.config.selected_block_id = snapshot.selected_block_id;
        self.config.selected_block_rotation_degrees = snapshot.selected_block_rotation_degrees;
        self.config.recent_block_ids = snapshot.recent_block_ids;
        self.spawn = snapshot.spawn;
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

        self.invalidate_samples();
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

    #[cfg(test)]
    pub(super) fn copy_selected(&mut self) {
        let selected_indices = self.selected_indices_normalized();
        if !selected_indices.is_empty() {
            let anchor_index = self
                .input
                .selected_block_index
                .filter(|index| selected_indices.contains(index))
                .unwrap_or(selected_indices[0]);
            let anchor = self.objects[anchor_index].position;
            let objects = selected_indices
                .into_iter()
                .map(|index| self.objects[index].clone())
                .collect();
            self.runtime.interaction.clipboard = Some(EditorClipboard { objects, anchor });
            return;
        }

        if let Some(index) = self.topmost_block_index_at_cursor(self.input.cursor) {
            let block = self.objects[index].clone();
            self.runtime.interaction.clipboard = Some(EditorClipboard {
                anchor: block.position,
                objects: vec![block],
            });
        }
    }

    #[cfg(test)]
    pub(super) fn paste_from_clipboard(&mut self) -> bool {
        let Some(clipboard) = self.runtime.interaction.clipboard.clone() else {
            return false;
        };

        if clipboard.objects.is_empty() {
            return false;
        }

        self.record_history_state();

        let paste_anchor = self.input.cursor;
        let base_len = self.objects.len();
        let mut new_indices = Vec::with_capacity(clipboard.objects.len());

        let mut pasted_objects = clipboard.objects;
        regenerate_object_ids_for_clipboard(&mut pasted_objects, &self.objects);

        for mut block in pasted_objects {
            block.position = [
                paste_anchor[0] + (block.position[0] - clipboard.anchor[0]),
                paste_anchor[1] + (block.position[1] - clipboard.anchor[1]),
                paste_anchor[2] + (block.position[2] - clipboard.anchor[2]),
            ];
            self.config.selected_block_id = block.block_id.clone();
            self.objects.push(block);
            new_indices.push(base_len + new_indices.len());
        }

        self.input.selected_block_index = new_indices.first().copied();
        self.input.selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.input.hovered_block_index = self.input.selected_block_index;
        self.sync_trigger_selection_from_objects();
        self.invalidate_samples();
        self.pick_spatial_index.invalidate();
        true
    }

    #[cfg(test)]
    pub(super) fn duplicate_selected(&mut self) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let anchor_index = self
            .input
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .unwrap_or(selected_indices[0]);
        let anchor = self.objects[anchor_index].position;
        let mut duplicates: Vec<LevelObject> = selected_indices
            .iter()
            .map(|index| self.objects[*index].clone())
            .collect();
        regenerate_object_ids_for_clipboard(&mut duplicates, &self.objects);

        self.runtime.interaction.clipboard = Some(EditorClipboard {
            objects: duplicates.clone(),
            anchor,
        });
        self.record_history_state();

        let base_len = self.objects.len();
        let mut new_indices = Vec::with_capacity(duplicates.len());
        for duplicated in duplicates {
            self.config.selected_block_id = duplicated.block_id.clone();
            self.objects.push(duplicated);
            new_indices.push(base_len + new_indices.len());
        }

        self.input.selected_block_index = new_indices.first().copied();
        self.input.selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.input.hovered_block_index = self.input.selected_block_index;
        self.sync_trigger_selection_from_objects();
        self.invalidate_samples();
        self.pick_spatial_index.invalidate();
        true
    }
}

#[cfg(test)]
fn regenerate_object_ids_for_clipboard(
    objects: &mut [LevelObject],
    existing_objects: &[LevelObject],
) {
    let mut id_map = std::collections::HashMap::<String, String>::new();
    let mut allocated_objects = existing_objects.to_vec();

    for object in objects.iter_mut() {
        let old_id = object.id.clone();
        object.id = next_level_object_id(&allocated_objects);
        if !old_id.is_empty() {
            id_map.insert(old_id, object.id.clone());
        }
        allocated_objects.push(object.clone());
    }

    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        for object_id in object_ids {
            if let Some(new_id) = id_map.get(object_id) {
                *object_id = new_id.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eg_types::LevelObject;

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

    #[test]
    fn test_history_max_limit() {
        let mut editor = EditorSubsystem::default();

        // Record more than 256 times
        for i in 0..300 {
            editor.input.cursor[0] = i as f32;
            editor.record_history_state();
        }

        assert_eq!(editor.runtime.history.undo.len(), 256);
        // The first one should be i=44 (300 - 256)
        assert_eq!(editor.runtime.history.undo[0].cursor[0], 44.0);
    }

    #[test]
    fn test_undo_redo_clears_redo_on_record() {
        let mut editor = EditorSubsystem::default();

        editor.record_history_state();
        assert!(editor.undo());
        assert_eq!(editor.runtime.history.redo.len(), 1);

        editor.record_history_state();
        assert_eq!(editor.runtime.history.redo.len(), 0);
    }

    #[test]
    fn test_apply_snapshot_constraints() {
        let mut editor = EditorSubsystem::default();

        let mut snapshot = editor.history_snapshot();
        snapshot.timeline_time_seconds = -10.0;
        snapshot.timeline_duration_seconds = 0.0; // Should be clamped to 0.1
        snapshot.tap_times = vec![5.0, -1.0, f32::NAN, 2.0];

        editor.apply_history_snapshot(snapshot);

        assert_eq!(editor.timeline.clock.time_seconds, 0.0);
        assert_eq!(editor.timeline.clock.duration_seconds, 0.1);

        // Tap times: -1.0 and NAN removed, 5.0 clamped by duration 0.1, 2.0 clamped by duration 0.1
        // Wait, duration is 0.1, so all tap times > 0.1 are removed by retain(|tap| *tap <= duration)
        assert_eq!(editor.timeline.taps.tap_times.len(), 0);
    }

    #[test]
    fn test_copy_paste_multiple_blocks() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])];
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.selected_block_index = Some(0); // Anchor

        editor.copy_selected();

        editor.input.cursor = [20.0, 5.0, 20.0];
        assert!(editor.paste_from_clipboard());

        assert_eq!(editor.objects.len(), 4);
        // Pasted blocks should be relative to anchor [10, 0, 10]
        // Block 0 at [10, 0, 10] -> [20 + (10-10), 5 + (0-0), 20 + (10-10)] = [20, 5, 20]
        // Block 1 at [12, 1, 12] -> [20 + (12-10), 5 + (1-0), 20 + (12-10)] = [22, 6, 22]
        assert_eq!(editor.objects[2].position, [20.0, 5.0, 20.0]);
        assert_eq!(editor.objects[3].position, [22.0, 6.0, 22.0]);

        // Selection should be the two new blocks
        assert_eq!(editor.input.selected_block_indices, vec![2, 3]);
    }

    #[test]
    fn test_copy_topmost_at_cursor() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![test_block([10.0, 0.0, 10.0])];
        editor.input.selected_block_indices.clear();
        editor.input.selected_block_index = None;
        editor.input.cursor = [10.0, 0.0, 10.0];

        editor.copy_selected();
        assert!(editor.runtime.interaction.clipboard.is_some());

        editor.input.cursor = [30.0, 0.0, 30.0];
        assert!(editor.paste_from_clipboard());
        assert_eq!(editor.objects.len(), 2);
        assert_eq!(editor.objects[1].position, [30.0, 0.0, 30.0]);
    }

    #[test]
    fn test_duplicate_selected_multiple() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])];
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.selected_block_index = Some(1); // Anchor

        assert!(editor.duplicate_selected());

        assert_eq!(editor.objects.len(), 4);
        assert_eq!(editor.objects[2].position, [10.0, 0.0, 10.0]);
        assert_eq!(editor.objects[3].position, [12.0, 1.0, 12.0]);

        // Selection should be the two new blocks
        assert_eq!(editor.input.selected_block_indices, vec![2, 3]);
    }

    #[test]
    fn test_empty_clipboard_paste() {
        let mut editor = EditorSubsystem::default();
        editor.runtime.interaction.clipboard = None;
        assert!(!editor.paste_from_clipboard());

        editor.runtime.interaction.clipboard = Some(EditorClipboard {
            objects: vec![],
            anchor: [0.0, 0.0, 0.0],
        });
        assert!(!editor.paste_from_clipboard());
    }

    #[test]
    fn test_undo_redo_empty_stacks() {
        let mut editor = EditorSubsystem::default();
        editor.runtime.history.undo.clear();
        editor.runtime.history.redo.clear();

        assert!(!editor.undo());
        assert!(!editor.redo());
    }

    #[test]
    fn test_duplicate_empty_selection() {
        let mut editor = EditorSubsystem::default();
        editor.input.selected_block_indices.clear();
        assert!(!editor.duplicate_selected());
    }

    #[test]
    fn test_apply_snapshot_selection_filtering() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![test_block([0.0, 0.0, 0.0])];
        let mut snapshot = editor.history_snapshot();
        snapshot.selected_block_index = Some(10); // Out of bounds
        snapshot.selected_block_indices = vec![0, 10]; // 10 is out of bounds
        snapshot.selected_trigger_index = Some(5); // Out of bounds

        editor.apply_history_snapshot(snapshot);

        assert_eq!(editor.input.selected_block_index, Some(0));
        assert_eq!(editor.input.selected_block_indices, vec![0]);
        assert!(editor.triggers.selected_index.is_none());
    }
}
