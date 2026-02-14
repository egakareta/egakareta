use super::*;

impl State {
    pub fn set_editor_pan_up_held(&mut self, held: bool) {
        self.editor_pan_up_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_down_held(&mut self, held: bool) {
        self.editor_pan_down_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_left_held(&mut self, held: bool) {
        self.editor_pan_left_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_right_held(&mut self, held: bool) {
        self.editor_pan_right_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_shift_held(&mut self, held: bool) {
        self.editor_shift_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_ctrl_held(&mut self, held: bool) {
        self.editor_ctrl_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_alt_held(&mut self, held: bool) {
        self.editor_alt_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_block_id(&mut self, block_id: String) {
        self.editor_selected_block_id = crate::block_repository::normalize_block_id(&block_id);
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor_mode = mode;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        if mode == EditorMode::Place {
            self.editor_selected_block_index = None;
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
        }
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor_mode
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor_snap_to_grid
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor_snap_step
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor_snap_to_grid = snap;
        if self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor_snap_step = step.max(0.05);
        if self.editor_snap_to_grid && self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.selected_block_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.editor_objects.get(index).cloned())
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let bounds = self.editor.bounds;
            let snap_step = self.editor_snap_step.max(0.05);
            let next_position = if self.editor_snap_to_grid {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1] / snap_step).round() * snap_step,
                    (position[2].max(0.0) / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.editor_objects[index].position = next_position;
            self.editor.cursor = [
                next_position[0].clamp(-bounds as f32, bounds as f32),
                next_position[1].clamp(-bounds as f32, bounds as f32),
                next_position[2].max(0.0),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_size(&mut self, size: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let snap_step = self.editor_snap_step.max(0.05);
            let snapped_size = if self.editor_snap_to_grid {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.editor_snap_to_grid {
                snap_step
            } else {
                0.25
            };
            self.editor_objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_id(&mut self, block_id: String) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].block_id =
                crate::block_repository::normalize_block_id(&block_id);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_rotation(&mut self, rotation_degrees: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].rotation_degrees = rotation_degrees;
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_roundness(&mut self, roundness: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].roundness = roundness.max(0.0);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub fn editor_selected_block_id(&self) -> &str {
        &self.editor_selected_block_id
    }

    pub fn editor_timeline_time_seconds(&self) -> f32 {
        self.editor_timeline_time_seconds
    }

    pub fn editor_timeline_duration_seconds(&self) -> f32 {
        self.editor_timeline_duration_seconds
    }

    pub fn editor_tap_times(&self) -> &[f32] {
        &self.editor_tap_times
    }

    pub fn editor_fps(&self) -> f32 {
        self.editor_fps_smoothed
    }

    pub fn set_editor_timeline_time_seconds(&mut self, time_seconds: f32) {
        let clamped_time = time_seconds.clamp(0.0, self.editor_timeline_duration_seconds.max(0.0));
        if (clamped_time - self.editor_timeline_time_seconds).abs() <= f32::EPSILON {
            return;
        }

        self.editor_timeline_time_seconds = clamped_time;
        self.refresh_editor_timeline_position();
        self.resync_editor_timeline_playback_audio();
    }

    pub fn set_editor_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.record_editor_history_state();
        self.editor_timeline_duration_seconds = duration_seconds.max(0.1);
        self.editor_timeline_time_seconds = self
            .editor_timeline_time_seconds
            .min(self.editor_timeline_duration_seconds);
        retain_taps_up_to_duration_with_indicators(
            &mut self.editor_tap_times,
            &mut self.editor_tap_indicator_positions,
            self.editor_timeline_duration_seconds,
        );
        self.invalidate_editor_timeline_samples();
        self.refresh_editor_timeline_position();
        self.resync_editor_timeline_playback_audio();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor_timeline_time_seconds;
        let indicator_cell =
            self.tap_indicator_position_from_world(self.editor_timeline_preview_position);
        add_tap_with_indicator(
            &mut self.editor_tap_times,
            &mut self.editor_tap_indicator_positions,
            tap_time,
            indicator_cell,
        );
        self.invalidate_editor_timeline_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor_timeline_time_seconds;
        remove_tap_with_indicator(
            &mut self.editor_tap_times,
            &mut self.editor_tap_indicator_positions,
            tap_time,
        );
        self.invalidate_editor_timeline_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        clear_taps_with_indicators(
            &mut self.editor_tap_times,
            &mut self.editor_tap_indicator_positions,
        );
        self.invalidate_editor_timeline_samples();
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        (
            self.editor_timeline_preview_position,
            self.editor_timeline_preview_direction,
        )
    }
}
