use super::{EditorClipboard, EditorHistorySnapshot, EditorSubsystem, State};
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
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
        }
    }

    pub(super) fn editor_duplicate_selected_block_in_place(&mut self) {
        if self.phase == AppPhase::Editor && self.editor.duplicate_selected() {
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
        }
    }
}
