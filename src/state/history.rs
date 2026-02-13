use super::*;

impl State {
    fn editor_history_snapshot(&self) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: self.editor_objects.clone(),
            selected_block_index: self.editor_selected_block_index,
            selected_block_indices: self.editor_selected_block_indices.clone(),
            cursor: self.editor.cursor,
            selected_kind: self.editor_selected_kind,
            spawn: self.editor_spawn.clone(),
            timeline_step: self.editor_timeline_step,
            timeline_length: self.editor_timeline_length,
            tap_steps: self.editor_tap_steps.clone(),
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
        self.editor_selected_kind = snapshot.selected_kind;
        self.editor_spawn = snapshot.spawn;
        self.editor_timeline_step = snapshot.timeline_step;
        self.editor_timeline_length = snapshot.timeline_length.max(1);
        self.editor_tap_steps = snapshot.tap_steps;
        self.editor_tap_steps
            .retain(|step| *step < self.editor_timeline_length);
        self.editor_timeline_step = self
            .editor_timeline_step
            .min(self.editor_timeline_length.saturating_sub(1));

        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
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

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_clipboard_block = Some(self.editor_objects[index].clone());
            return;
        }

        if let Some(index) = self.topmost_block_index_at_cursor(self.editor.cursor) {
            self.editor_clipboard_block = Some(self.editor_objects[index].clone());
        }
    }

    pub(super) fn editor_paste_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(mut block) = self.editor_clipboard_block.clone() else {
            return;
        };

        self.record_editor_history_state();

        block.position = [
            self.editor.cursor[0] as f32,
            self.editor.cursor[1] as f32,
            self.editor.cursor[2] as f32,
        ];

        self.editor_selected_kind = block.kind;
        self.editor_objects.push(block);
        self.editor_selected_block_index = Some(self.editor_objects.len() - 1);
        self.editor_selected_block_indices = self.editor_selected_block_index.into_iter().collect();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }
}
