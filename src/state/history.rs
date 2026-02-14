use super::*;

impl State {
    fn editor_history_snapshot(&self) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: self.editor_objects.clone(),
            selected_block_index: self.editor_selected_block_index,
            selected_block_indices: self.editor_selected_block_indices.clone(),
            cursor: self.editor.cursor,
            selected_block_id: self.editor_selected_block_id.clone(),
            spawn: self.editor_spawn.clone(),
            timeline_time_seconds: self.editor_timeline_time_seconds,
            timeline_duration_seconds: self.editor_timeline_duration_seconds,
            tap_times: self.editor_tap_times.clone(),
            tap_indicator_positions: self.editor_tap_indicator_positions.clone(),
            timing_points: self.editor_timing_points.clone(),
        }
    }

    pub(super) fn record_editor_history_state(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        const MAX_HISTORY: usize = 256;
        if self.editor_history_undo.len() >= MAX_HISTORY {
            self.editor_history_undo.remove(0);
        }
        self.editor_history_undo
            .push(self.editor_history_snapshot());
        self.editor_history_redo.clear();
    }

    fn apply_editor_history_snapshot(&mut self, snapshot: EditorHistorySnapshot) {
        self.editor_objects = snapshot.objects;
        self.editor_selected_block_index = snapshot
            .selected_block_index
            .filter(|index| *index < self.editor_objects.len());
        self.editor_selected_block_indices = snapshot
            .selected_block_indices
            .into_iter()
            .filter(|index| *index < self.editor_objects.len())
            .collect();
        self.sync_primary_selection_from_indices();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.editor.cursor = snapshot.cursor;
        self.editor_selected_block_id = snapshot.selected_block_id;
        self.editor_spawn = snapshot.spawn;
        self.editor_timeline_time_seconds = snapshot.timeline_time_seconds.max(0.0);
        self.editor_timeline_duration_seconds = snapshot.timeline_duration_seconds.max(0.1);
        self.editor_tap_times = snapshot.tap_times;
        self.editor_tap_indicator_positions = snapshot.tap_indicator_positions;
        self.editor_timing_points = snapshot.timing_points;
        self.editor_tap_times
            .retain(|tap| tap.is_finite() && *tap >= 0.0);
        self.editor_tap_times.sort_by(f32::total_cmp);
        self.editor_tap_times
            .retain(|tap| *tap <= self.editor_timeline_duration_seconds);
        if self.editor_tap_indicator_positions.len() != self.editor_tap_times.len() {
            self.editor_tap_indicator_positions = derive_tap_indicator_positions(
                self.editor_spawn.position,
                self.editor_spawn.direction,
                &self.editor_tap_times,
                &self.editor_objects,
            );
        }
        self.editor_timeline_time_seconds = self
            .editor_timeline_time_seconds
            .min(self.editor_timeline_duration_seconds);

        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn editor_undo(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(snapshot) = self.editor_history_undo.pop() else {
            return;
        };

        self.editor_history_redo
            .push(self.editor_history_snapshot());
        self.apply_editor_history_snapshot(snapshot);
    }

    pub(super) fn editor_redo(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(snapshot) = self.editor_history_redo.pop() else {
            return;
        };

        self.editor_history_undo
            .push(self.editor_history_snapshot());
        self.apply_editor_history_snapshot(snapshot);
    }

    pub(super) fn editor_copy_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if !selected_indices.is_empty() {
            let anchor_index = self
                .editor_selected_block_index
                .filter(|index| selected_indices.contains(index))
                .unwrap_or(selected_indices[0]);
            let anchor = self.editor_objects[anchor_index].position;
            let objects = selected_indices
                .into_iter()
                .map(|index| self.editor_objects[index].clone())
                .collect();
            self.editor_clipboard = Some(EditorClipboard { objects, anchor });
            return;
        }

        if let Some(index) = self.topmost_block_index_at_cursor(self.editor.cursor) {
            let block = self.editor_objects[index].clone();
            self.editor_clipboard = Some(EditorClipboard {
                anchor: block.position,
                objects: vec![block],
            });
        }
    }

    pub(super) fn editor_paste_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(clipboard) = self.editor_clipboard.clone() else {
            return;
        };

        if clipboard.objects.is_empty() {
            return;
        }

        self.record_editor_history_state();

        let paste_anchor = self.editor.cursor;

        let base_len = self.editor_objects.len();
        let mut new_indices = Vec::with_capacity(clipboard.objects.len());

        for mut block in clipboard.objects {
            block.position = [
                paste_anchor[0] + (block.position[0] - clipboard.anchor[0]),
                paste_anchor[1] + (block.position[1] - clipboard.anchor[1]),
                paste_anchor[2] + (block.position[2] - clipboard.anchor[2]),
            ];
            self.editor_selected_block_id = block.block_id.clone();
            self.editor_objects.push(block);
            new_indices.push(base_len + new_indices.len());
        }

        self.editor_selected_block_index = new_indices.first().copied();
        self.editor_selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn editor_duplicate_selected_block_in_place(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            return;
        }

        let anchor_index = self
            .editor_selected_block_index
            .filter(|index| selected_indices.contains(index))
            .unwrap_or(selected_indices[0]);
        let anchor = self.editor_objects[anchor_index].position;
        let duplicates: Vec<LevelObject> = selected_indices
            .iter()
            .map(|index| self.editor_objects[*index].clone())
            .collect();

        self.editor_clipboard = Some(EditorClipboard {
            objects: duplicates.clone(),
            anchor,
        });
        self.record_editor_history_state();

        let base_len = self.editor_objects.len();
        let mut new_indices = Vec::with_capacity(duplicates.len());
        for duplicated in duplicates {
            self.editor_selected_block_id = duplicated.block_id.clone();
            self.editor_objects.push(duplicated);
            new_indices.push(base_len + new_indices.len());
        }

        self.editor_selected_block_index = new_indices.first().copied();
        self.editor_selected_block_indices = new_indices;
        self.sync_primary_selection_from_indices();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }
}
