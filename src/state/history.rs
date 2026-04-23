/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{BlockMeshOperation, EditorClipboard, EditorHistorySnapshot, EditorSubsystem, State};
use crate::editor_domain::derive_tap_indicator_positions;
use crate::types::{AppPhase, LevelObject};

pub(crate) struct EditorHistoryState {
    pub(crate) undo: Vec<EditorHistorySnapshot>,
    pub(crate) redo: Vec<EditorHistorySnapshot>,
}

impl EditorSubsystem {
    fn history_snapshot(&self) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: self.objects.clone(),
            selected_block_index: self.ui.selected_block_index,
            selected_block_indices: self.ui.selected_block_indices.clone(),
            cursor: self.ui.cursor,
            selected_block_id: self.config.selected_block_id.clone(),
            spawn: self.spawn.clone(),
            timeline_time_seconds: self.timeline.clock.time_seconds,
            timeline_duration_seconds: self.timeline.clock.duration_seconds,
            tap_times: self.timeline.taps.tap_times.clone(),
            tap_indicator_positions: self.timeline.taps.tap_indicator_positions.clone(),
            timing_points: self.timing.timing_points.clone(),
            triggers: self.triggers.items.clone(),
            selected_trigger_index: self.triggers.selected_index,
            simulate_trigger_hitboxes: self.triggers.simulate_trigger_hitboxes,
        }
    }

    pub(super) fn record_history_state(&mut self) {
        const MAX_HISTORY: usize = 256;
        if self.runtime.history.undo.len() >= MAX_HISTORY {
            self.runtime.history.undo.remove(0);
        }
        let snapshot = self.history_snapshot();
        self.runtime.history.undo.push(snapshot);
        self.runtime.history.redo.clear();
    }

    fn apply_history_snapshot(&mut self, snapshot: EditorHistorySnapshot) {
        self.objects = snapshot.objects;
        self.ui.selected_block_index = snapshot
            .selected_block_index
            .filter(|index| *index < self.objects.len());
        self.ui.selected_block_indices = snapshot
            .selected_block_indices
            .into_iter()
            .filter(|index| *index < self.objects.len())
            .collect();
        self.sync_primary_selection_from_indices();
        self.ui.hovered_block_index = self.ui.selected_block_index;
        self.ui.cursor = snapshot.cursor;
        self.config.selected_block_id = snapshot.selected_block_id;
        self.spawn = snapshot.spawn;
        self.timeline.clock.time_seconds = snapshot.timeline_time_seconds.max(0.0);
        self.timeline.clock.duration_seconds = snapshot.timeline_duration_seconds.max(0.1);
        self.timeline.taps.tap_times = snapshot.tap_times;
        self.timeline.taps.tap_indicator_positions = snapshot.tap_indicator_positions;
        self.timing.timing_points = snapshot.timing_points;
        self.triggers.items = snapshot.triggers;
        self.triggers.simulate_trigger_hitboxes = snapshot.simulate_trigger_hitboxes;
        self.triggers.selected_index = snapshot
            .selected_trigger_index
            .filter(|index| *index < self.triggers.items.len());

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

        self.timeline.clock.time_seconds = self
            .timeline
            .clock
            .time_seconds
            .min(self.timeline.clock.duration_seconds);
        self.triggers
            .items
            .retain(|trigger| trigger.time_seconds.is_finite());
        self.triggers
            .items
            .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));

        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        self.invalidate_samples();
    }

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

    pub(super) fn copy_selected(&mut self) {
        let selected_indices = self.selected_indices_normalized();
        if !selected_indices.is_empty() {
            let anchor_index = self
                .ui
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

        if let Some(index) = self.topmost_block_index_at_cursor(self.ui.cursor) {
            let block = self.objects[index].clone();
            self.runtime.interaction.clipboard = Some(EditorClipboard {
                anchor: block.position,
                objects: vec![block],
            });
        }
    }

    pub(super) fn paste_from_clipboard(&mut self) -> bool {
        let Some(clipboard) = self.runtime.interaction.clipboard.clone() else {
            return false;
        };

        if clipboard.objects.is_empty() {
            return false;
        }

        self.record_history_state();

        let paste_anchor = self.ui.cursor;
        let base_len = self.objects.len();
        let mut new_indices = Vec::with_capacity(clipboard.objects.len());

        for mut block in clipboard.objects {
            block.position = [
                paste_anchor[0] + (block.position[0] - clipboard.anchor[0]),
                paste_anchor[1] + (block.position[1] - clipboard.anchor[1]),
                paste_anchor[2] + (block.position[2] - clipboard.anchor[2]),
            ];
            self.config.selected_block_id = block.block_id.clone();
            self.objects.push(block);
            new_indices.push(base_len + new_indices.len());
        }

        self.ui.selected_block_index = new_indices.first().copied();
        self.ui.selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.ui.hovered_block_index = self.ui.selected_block_index;
        self.invalidate_samples();
        true
    }

    pub(super) fn duplicate_selected(&mut self) -> bool {
        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let anchor_index = self
            .ui
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .unwrap_or(selected_indices[0]);
        let anchor = self.objects[anchor_index].position;
        let duplicates: Vec<LevelObject> = selected_indices
            .iter()
            .map(|index| self.objects[*index].clone())
            .collect();

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

        self.ui.selected_block_index = new_indices.first().copied();
        self.ui.selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.ui.hovered_block_index = self.ui.selected_block_index;
        self.invalidate_samples();
        true
    }
}

impl State {
    pub(super) fn record_editor_history_state(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }
        self.editor.record_history_state();
    }

    /// Reverts the last recorded action in the editor.
    ///
    /// If successful, it synchronizes the editor's visual objects and markers.
    pub fn editor_undo(&mut self) {
        if self.phase == AppPhase::Editor && self.editor.undo() {
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_spawn_marker_vertices();
        }
    }

    /// Re-applies a previously undone action in the editor.
    ///
    /// If successful, it synchronizes the editor's visual objects and markers.
    pub fn editor_redo(&mut self) {
        if self.phase == AppPhase::Editor && self.editor.redo() {
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_spawn_marker_vertices();
        }
    }

    pub(super) fn editor_copy_block(&mut self) {
        if self.phase == AppPhase::Editor {
            self.editor.copy_selected();
        }
    }

    pub(super) fn editor_paste_block(&mut self) {
        if self.phase == AppPhase::Editor && self.editor.paste_from_clipboard() {
            let changed_indices = self.editor.selected_indices_normalized();
            self.sync_editor_objects_with_mesh_operation(
                BlockMeshOperation::AppendObject,
                &changed_indices,
            );
            self.rebuild_editor_cursor_vertices();
        }
    }

    pub(super) fn editor_duplicate_selected_block_in_place(&mut self) {
        if self.phase == AppPhase::Editor && self.editor.duplicate_selected() {
            let changed_indices = self.editor.selected_indices_normalized();
            self.sync_editor_objects_with_mesh_operation(
                BlockMeshOperation::AppendObject,
                &changed_indices,
            );
            self.rebuild_editor_cursor_vertices();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AppPhase, LevelObject, TimedTrigger, TimedTriggerAction, TimedTriggerTarget,
    };

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn test_history_max_limit() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            // Record more than 256 times
            for i in 0..300 {
                state.editor.ui.cursor[0] = i as f32;
                state.record_editor_history_state();
            }

            assert_eq!(state.editor.runtime.history.undo.len(), 256);
            // The first one should be i=44 (300 - 256)
            assert_eq!(state.editor.runtime.history.undo[0].cursor[0], 44.0);
        });
    }

    #[test]
    fn test_undo_redo_clears_redo_on_record() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.record_editor_history_state();
            state.editor_undo();
            assert_eq!(state.editor.runtime.history.redo.len(), 1);

            state.record_editor_history_state();
            assert_eq!(state.editor.runtime.history.redo.len(), 0);
        });
    }

    #[test]
    fn test_apply_snapshot_constraints() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            let mut snapshot = state.editor.history_snapshot();
            snapshot.timeline_time_seconds = -10.0;
            snapshot.timeline_duration_seconds = 0.0; // Should be clamped to 0.1
            snapshot.tap_times = vec![5.0, -1.0, f32::NAN, 2.0];
            snapshot.triggers = vec![
                TimedTrigger {
                    time_seconds: 5.0,
                    target: TimedTriggerTarget::Camera,
                    action: TimedTriggerAction::MoveTo {
                        position: [0.0, 0.0, 0.0],
                    },
                    easing: Default::default(),
                    duration_seconds: 0.0,
                },
                TimedTrigger {
                    time_seconds: f32::NAN,
                    target: TimedTriggerTarget::Camera,
                    action: TimedTriggerAction::MoveTo {
                        position: [0.0, 0.0, 0.0],
                    },
                    easing: Default::default(),
                    duration_seconds: 0.0,
                },
                TimedTrigger {
                    time_seconds: 1.0,
                    target: TimedTriggerTarget::Camera,
                    action: TimedTriggerAction::MoveTo {
                        position: [0.0, 0.0, 0.0],
                    },
                    easing: Default::default(),
                    duration_seconds: 0.0,
                },
            ];

            state.editor.apply_history_snapshot(snapshot);

            assert_eq!(state.editor.timeline.clock.time_seconds, 0.0);
            assert_eq!(state.editor.timeline.clock.duration_seconds, 0.1);

            // Tap times: -1.0 and NAN removed, 5.0 clamped by duration 0.1, 2.0 clamped by duration 0.1
            // Wait, duration is 0.1, so all tap times > 0.1 are removed by retain(|tap| *tap <= duration)
            assert_eq!(state.editor.timeline.taps.tap_times.len(), 0);

            // Triggers: NAN removed, others sorted
            assert_eq!(state.editor.triggers.items.len(), 2);
            assert_eq!(state.editor.triggers.items[0].time_seconds, 1.0);
            assert_eq!(state.editor.triggers.items[1].time_seconds, 5.0);
        });
    }

    #[test]
    fn test_copy_paste_multiple_blocks() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor.objects =
                vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])];
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.selected_block_index = Some(0); // Anchor

            state.editor_copy_block();

            state.editor.ui.cursor = [20.0, 5.0, 20.0];
            state.editor_paste_block();

            assert_eq!(state.editor.objects.len(), 4);
            // Pasted blocks should be relative to anchor [10, 0, 10]
            // Block 0 at [10, 0, 10] -> [20 + (10-10), 5 + (0-0), 20 + (10-10)] = [20, 5, 20]
            // Block 1 at [12, 1, 12] -> [20 + (12-10), 5 + (1-0), 20 + (12-10)] = [22, 6, 22]
            assert_eq!(state.editor.objects[2].position, [20.0, 5.0, 20.0]);
            assert_eq!(state.editor.objects[3].position, [22.0, 6.0, 22.0]);

            // Selection should be the two new blocks
            assert_eq!(state.editor.ui.selected_block_indices, vec![2, 3]);
        });
    }

    #[test]
    fn test_copy_topmost_at_cursor() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor.objects = vec![test_block([10.0, 0.0, 10.0])];
            state.editor.ui.selected_block_indices.clear();
            state.editor.ui.selected_block_index = None;
            state.editor.ui.cursor = [10.0, 0.0, 10.0];

            state.editor_copy_block();
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            state.editor.ui.cursor = [30.0, 0.0, 30.0];
            state.editor_paste_block();
            assert_eq!(state.editor.objects.len(), 2);
            assert_eq!(state.editor.objects[1].position, [30.0, 0.0, 30.0]);
        });
    }

    #[test]
    fn test_duplicate_selected_multiple() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor.objects =
                vec![test_block([10.0, 0.0, 10.0]), test_block([12.0, 1.0, 12.0])];
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.selected_block_index = Some(1); // Anchor

            state.editor_duplicate_selected_block_in_place();

            assert_eq!(state.editor.objects.len(), 4);
            assert_eq!(state.editor.objects[2].position, [10.0, 0.0, 10.0]);
            assert_eq!(state.editor.objects[3].position, [12.0, 1.0, 12.0]);

            // Selection should be the two new blocks
            assert_eq!(state.editor.ui.selected_block_indices, vec![2, 3]);
        });
    }

    #[test]
    fn test_empty_clipboard_paste() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.runtime.interaction.clipboard = None;
            assert!(!state.editor.paste_from_clipboard());

            state.editor.runtime.interaction.clipboard = Some(EditorClipboard {
                objects: vec![],
                anchor: [0.0, 0.0, 0.0],
            });
            assert!(!state.editor.paste_from_clipboard());
        });
    }

    #[test]
    fn test_undo_redo_empty_stacks() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.runtime.history.undo.clear();
            state.editor.runtime.history.redo.clear();

            assert!(!state.editor.undo());
            assert!(!state.editor.redo());
        });
    }

    #[test]
    fn test_duplicate_empty_selection() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.selected_block_indices.clear();
            assert!(!state.editor.duplicate_selected());
        });
    }

    #[test]
    fn test_apply_snapshot_selection_filtering() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            let mut snapshot = state.editor.history_snapshot();
            snapshot.selected_block_index = Some(10); // Out of bounds
            snapshot.selected_block_indices = vec![0, 10]; // 10 is out of bounds
            snapshot.selected_trigger_index = Some(5); // Out of bounds

            state.editor.apply_history_snapshot(snapshot);

            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert!(state.editor.triggers.selected_index.is_none());
        });
    }

    #[test]
    fn test_record_history_not_in_editor() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Menu;
            let initial_undo = state.editor.runtime.history.undo.len();
            state.record_editor_history_state();
            assert_eq!(state.editor.runtime.history.undo.len(), initial_undo);
        });
    }
}
