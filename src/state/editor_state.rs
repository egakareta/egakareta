/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{EditorDirtyFlags, EditorSubsystem, EditorTransformTriggerCapture, State};
use crate::editor_domain::{
    add_tap_with_indicator, clear_taps_with_indicators, interpolate_timeline_sample_positions,
    remove_tap_with_indicator,
};
use crate::game::TimelineSimulationRuntime;
use crate::types::{
    AppPhase, EditorMode, GameCursor, LevelObject, SpawnDirection, TimedTrigger,
    TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget, TimingPoint,
};

fn timed_trigger_transforms_objects(trigger: &TimedTrigger) -> bool {
    !matches!(trigger.target, TimedTriggerTarget::Camera)
        && matches!(trigger.action, TimedTriggerAction::TransformObjects { .. })
}

const TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS: f32 = 0.001;

fn remove_replaced_transform_trigger_targets(
    triggers: Vec<TimedTrigger>,
    time_seconds: f32,
    object_ids_to_replace: &[u32],
) -> Vec<TimedTrigger> {
    triggers
        .into_iter()
        .filter_map(|mut trigger| {
            if (trigger.time_seconds - time_seconds).abs()
                > TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
                || !matches!(trigger.action, TimedTriggerAction::TransformObjects { .. })
            {
                return Some(trigger);
            }

            let TimedTriggerTarget::Objects { object_ids } = &mut trigger.target else {
                return Some(trigger);
            };

            if !object_ids
                .iter()
                .any(|object_id| object_ids_to_replace.contains(object_id))
            {
                return Some(trigger);
            }

            object_ids.retain(|object_id| !object_ids_to_replace.contains(object_id));
            if object_ids.is_empty() {
                None
            } else {
                Some(trigger)
            }
        })
        .collect()
}

impl EditorSubsystem {
    pub(crate) fn set_pan_up_held(&mut self, held: bool) {
        self.ui.pan_up_held = held;
    }

    pub(crate) fn set_pan_down_held(&mut self, held: bool) {
        self.ui.pan_down_held = held;
    }

    pub(crate) fn set_pan_left_held(&mut self, held: bool) {
        self.ui.pan_left_held = held;
    }

    pub(crate) fn set_pan_right_held(&mut self, held: bool) {
        self.ui.pan_right_held = held;
    }

    pub(crate) fn set_shift_held(&mut self, held: bool) {
        self.ui.shift_held = held;
    }

    pub(crate) fn set_ctrl_held(&mut self, held: bool) {
        self.ui.ctrl_held = held;
    }

    pub(crate) fn set_alt_held(&mut self, held: bool) {
        self.ui.alt_held = held;
    }

    pub(crate) fn set_right_dragging(&mut self, dragging: bool) {
        self.ui.right_dragging = dragging;
    }

    pub(crate) fn set_left_mouse_down(&mut self, pressed: bool) {
        self.ui.left_mouse_down = pressed;
    }

    pub(crate) fn set_pointer_screen(&mut self, position: Option<[f64; 2]>) {
        self.ui.pointer_screen = position;
    }

    pub(crate) fn set_ui_input_blocking_rects(
        &mut self,
        rects: Vec<[f32; 4]>,
        pixels_per_point: f32,
    ) {
        self.ui.ui_input_blocking_rects = rects;
        self.ui.ui_input_pixels_per_point =
            if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
                pixels_per_point
            } else {
                1.0
            };
    }

    pub(crate) fn clear_ui_input_blocking_rects(&mut self) {
        self.ui.ui_input_blocking_rects.clear();
        self.ui.ui_input_pixels_per_point = 1.0;
    }

    pub(crate) fn pointer_over_ui_input(&self, x: f64, y: f64) -> bool {
        let pixels_per_point = self.ui.ui_input_pixels_per_point as f64;
        let point_x = x / pixels_per_point;
        let point_y = y / pixels_per_point;

        self.ui.ui_input_blocking_rects.iter().any(|rect| {
            point_x >= rect[0] as f64
                && point_x <= rect[2] as f64
                && point_y >= rect[1] as f64
                && point_y <= rect[3] as f64
        })
    }

    pub(crate) fn clear_interaction_drags(&mut self) {
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
    }

    pub(crate) fn left_mouse_down(&self) -> bool {
        self.ui.left_mouse_down
    }

    pub(crate) fn has_gizmo_drag(&self) -> bool {
        self.runtime.interaction.gizmo_drag.is_some()
    }

    pub(crate) fn has_block_drag(&self) -> bool {
        self.runtime.interaction.block_drag.is_some()
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.timeline.playback.playing
    }

    pub(crate) fn timeline_time_seconds(&self) -> f32 {
        self.timeline.clock.time_seconds
    }

    pub(crate) fn set_block_id(&mut self, block_id: String) {
        self.config.selected_block_id = crate::block_repository::normalize_block_id(&block_id);
        self.config
            .remember_recent_block_id(self.config.selected_block_id.clone());
    }

    pub(crate) fn set_mode(&mut self, mode: EditorMode) {
        self.ui.mode = mode;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
        self.ui.marquee_start_screen = None;
        self.ui.marquee_current_screen = None;
        if !mode.can_select() {
            self.clear_block_selection();
        }
    }

    pub(crate) fn mode(&self) -> EditorMode {
        self.ui.mode
    }

    pub(crate) fn snap_to_grid(&self) -> bool {
        self.config.snap_to_grid
    }

    pub(crate) fn effective_snap_to_grid(&self) -> bool {
        self.config.snap_to_grid && !self.ui.ctrl_held
    }

    pub(crate) fn snap_step(&self) -> f32 {
        self.config.snap_step
    }

    pub(crate) fn snap_rotation(&self) -> bool {
        self.config.snap_rotation
    }

    pub(crate) fn effective_snap_rotation(&self) -> bool {
        self.config.snap_rotation && !self.ui.ctrl_held
    }

    pub(crate) fn snap_rotation_step_degrees(&self) -> f32 {
        self.config.snap_rotation_step_degrees
    }

    pub(crate) fn selected_block(&self) -> Option<LevelObject> {
        self.selected_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.objects.get(index).cloned())
    }

    pub(crate) fn set_snap_to_grid(&mut self, snap: bool) {
        self.config.snap_to_grid = snap;
    }

    pub(crate) fn set_snap_step(&mut self, step: f32) {
        self.config.snap_step = step.max(0.05);
    }

    pub(crate) fn set_snap_rotation(&mut self, snap: bool) {
        self.config.snap_rotation = snap;
    }

    pub(crate) fn set_snap_rotation_step_degrees(&mut self, step: f32) {
        self.config.snap_rotation_step_degrees = step.max(1.0);
    }

    pub(crate) fn set_selected_block_position(&mut self, position: [f32; 3]) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let snap_step = self.config.snap_step.max(0.05);
            let next_position = if self.effective_snap_to_grid() {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1].max(0.0) / snap_step).round() * snap_step,
                    (position[2] / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1].max(0.0), position[2]]
            };
            self.objects[index].position = next_position;
            self.ui.cursor = [
                next_position[0],
                next_position[1].max(0.0),
                next_position[2],
            ];
        }
    }

    pub(crate) fn set_selected_block_size(&mut self, size: [f32; 3]) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let snap_step = self.config.snap_step.max(0.05);
            let snapped_size = if self.effective_snap_to_grid() {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.effective_snap_to_grid() {
                snap_step
            } else {
                0.25
            };
            self.objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
        }
    }

    pub(crate) fn set_selected_block_id(&mut self, block_id: String) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].block_id = crate::block_repository::normalize_block_id(&block_id);
        }
    }

    pub(crate) fn set_selected_block_rotation(&mut self, rotation_degrees: [f32; 3]) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let next_rotation = if self.effective_snap_rotation() {
                let step = self.config.snap_rotation_step_degrees.max(1.0);
                rotation_degrees.map(|component| (component / step).round() * step)
            } else {
                rotation_degrees
            };
            self.objects[index].rotation_degrees = next_rotation;
        }
    }

    pub(crate) fn rotate_selected_block_preview(&mut self) {
        self.config.selected_block_rotation_degrees[1] =
            (self.config.selected_block_rotation_degrees[1] + 90.0).rem_euclid(360.0);
    }

    pub(crate) fn set_selected_block_color_tint(&mut self, color_tint: [f32; 3]) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].color_tint = color_tint.map(|component| component.clamp(0.0, 1.0));
        }
    }

    pub(crate) fn selected_block_id(&self) -> &str {
        self.config.selected_block_id.as_str()
    }

    pub(crate) fn timeline_duration_seconds(&self) -> f32 {
        self.timeline.clock.duration_seconds
    }

    pub(crate) fn tap_times(&self) -> &[f32] {
        &self.timeline.taps.tap_times
    }

    pub(crate) fn selected_tap(&self) -> Option<(usize, f32, [f32; 3])> {
        let index = self.timeline.taps.selected_index?;
        let time_seconds = *self.timeline.taps.tap_times.get(index)?;
        let position = *self.timeline.taps.tap_indicator_positions.get(index)?;
        Some((index, time_seconds, position))
    }

    pub(crate) fn set_selected_tap_index(&mut self, selected_index: Option<usize>) {
        self.timeline.taps.selected_index = selected_index.filter(|index| {
            *index < self.timeline.taps.tap_times.len()
                && *index < self.timeline.taps.tap_indicator_positions.len()
        });
    }

    pub(crate) fn adjust_selected_tap_after_removal(&mut self, removed_index: usize) {
        self.timeline.taps.selected_index = match self.timeline.taps.selected_index {
            Some(selected_index) if selected_index == removed_index => None,
            Some(selected_index) if selected_index > removed_index => Some(selected_index - 1),
            selected_index => selected_index,
        };
    }

    pub(crate) fn fps(&self) -> f32 {
        self.perf.fps_smoothed
    }

    pub(crate) fn set_timeline_time_seconds(&mut self, time_seconds: f32) -> bool {
        let old_time = self.timeline.clock.time_seconds;
        self.timeline.clock.time_seconds =
            time_seconds.clamp(0.0, self.timeline.clock.duration_seconds);
        (self.timeline.clock.time_seconds - old_time).abs() > 0.0001
    }

    pub(crate) fn set_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.timeline.clock.duration_seconds = duration_seconds.max(0.1);
        self.timeline.clock.time_seconds = self
            .timeline
            .clock
            .time_seconds
            .min(self.timeline.clock.duration_seconds);
    }

    pub(crate) fn tap_indicator_position_from_world(&self, position: [f32; 3]) -> [f32; 3] {
        [position[0] - 0.5, position[1], position[2] - 0.5]
    }

    pub(crate) fn add_tap(&mut self, indicator_position: [f32; 3]) -> f32 {
        let selected_index = add_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
            indicator_position,
        );
        self.set_selected_tap_index(selected_index);
        self.timeline.clock.time_seconds
    }

    pub(crate) fn remove_tap(&mut self) -> f32 {
        if let Some(removed_index) = remove_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
        ) {
            self.adjust_selected_tap_after_removal(removed_index);
        }
        self.timeline.clock.time_seconds
    }

    pub(crate) fn remove_tap_at(&mut self, time_seconds: f32) -> f32 {
        if let Some(removed_index) = remove_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            time_seconds,
        ) {
            self.adjust_selected_tap_after_removal(removed_index);
        }
        time_seconds
    }

    pub(crate) fn clear_taps(&mut self) {
        clear_taps_with_indicators(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
        );
        self.timeline.taps.selected_index = None;
    }

    pub(crate) fn invalidate_samples(&mut self) {
        self.timeline.simulation_revision = self.timeline.simulation_revision.wrapping_add(1);
        self.timeline.snapshot_cache_revision = 0;
        self.timeline.snapshot_cache.clear();
        self.timeline.scrub_runtime = None;
        self.timeline.scrub_runtime_revision = 0;
    }

    pub(crate) fn invalidate_samples_from(&mut self, from_seconds: f32) {
        self.timeline.simulation_revision = self.timeline.simulation_revision.wrapping_add(1);

        // Partial invalidation: keep cached snapshots before the change point
        let step_seconds = self.timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
        if !self.timeline.snapshot_cache.is_empty() && from_seconds > step_seconds {
            let keep_count = (from_seconds / step_seconds).floor() as usize;
            let keep_count = keep_count.min(self.timeline.snapshot_cache.len());
            if keep_count > 0 {
                self.timeline.snapshot_cache.truncate(keep_count);
                // Mark as partial so rebuild can continue from the retained prefix
                self.timeline.snapshot_cache_revision = 0;
            } else {
                self.timeline.snapshot_cache.clear();
                self.timeline.snapshot_cache_revision = 0;
            }
        } else {
            self.timeline.snapshot_cache.clear();
            self.timeline.snapshot_cache_revision = 0;
        }

        // Invalidate scrub runtime if it's past the change point
        if let Some(runtime) = &self.timeline.scrub_runtime {
            if runtime.elapsed_seconds() >= from_seconds {
                self.timeline.scrub_runtime = None;
                self.timeline.scrub_runtime_revision = 0;
            }
        } else {
            self.timeline.scrub_runtime_revision = 0;
        }
    }

    pub(crate) fn timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        (
            self.timeline.preview.position,
            self.timeline.preview.direction,
        )
    }

    pub(crate) fn timing_points(&self) -> &[TimingPoint] {
        &self.timing.timing_points
    }

    pub(crate) fn playback_speed(&self) -> f32 {
        self.timing.playback_speed
    }

    pub(crate) fn set_playback_speed(&mut self, speed: f32) -> f32 {
        self.timing.playback_speed = speed.clamp(0.1, 2.0);
        self.timing.playback_speed
    }

    pub(crate) fn timing_selected_index(&self) -> Option<usize> {
        self.timing.timing_selected_index
    }

    pub(crate) fn set_timing_selected_index(&mut self, index: Option<usize>) {
        self.timing.timing_selected_index = index;
    }

    pub(crate) fn waveform_zoom(&self) -> f32 {
        self.timing.waveform_zoom
    }

    pub(crate) fn set_waveform_zoom(&mut self, zoom: f32) {
        self.timing.waveform_zoom = zoom.clamp(0.1, 10.0);
    }

    pub(crate) fn waveform_scroll(&self) -> f32 {
        self.timing.waveform_scroll
    }

    pub(crate) fn set_waveform_scroll(&mut self, scroll: f32) {
        self.timing.waveform_scroll = scroll;
    }

    pub(crate) fn waveform_samples(&self) -> &[f32] {
        &self.timing.waveform_samples
    }

    pub(crate) fn waveform_sample_rate(&self) -> u32 {
        self.timing.waveform_sample_rate
    }

    pub(crate) fn waveform_window_size(&self) -> usize {
        self.timing.waveform_window_size
    }

    pub(crate) fn waveform_loading(&self) -> bool {
        self.timing.waveform_loading
    }

    pub(crate) fn waveform_complete(&self) -> bool {
        self.timing.waveform_complete
    }

    pub(crate) fn bpm_tap_result(&self) -> Option<f32> {
        self.timing.bpm_tap_result
    }

    pub(crate) fn add_timing_point(&mut self, time_seconds: f32, bpm: f32) {
        self.timing.timing_points.push(TimingPoint {
            time_seconds,
            bpm,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        });
        self.timing
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        self.timing.mark_timing_points_changed();
    }

    pub(crate) fn remove_timing_point(&mut self, index: usize) {
        if index < self.timing.timing_points.len() {
            self.timing.timing_points.remove(index);
            self.timing.mark_timing_points_changed();
        }
    }

    pub(crate) fn update_timing_point_time(&mut self, index: usize, time: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.time_seconds = time.max(0.0);
            self.timing.mark_timing_points_changed();
        }
        self.timing
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    }

    pub(crate) fn update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.bpm = bpm.max(1.0);
            self.timing.mark_timing_points_changed();
        }
    }

    pub(crate) fn update_timing_point_time_signature(
        &mut self,
        index: usize,
        numerator: u32,
        denominator: u32,
    ) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.time_signature_numerator = numerator.max(1);
            tp.time_signature_denominator = denominator.max(1);
            self.timing.mark_timing_points_changed();
        }
    }

    pub(crate) fn bpm_tap(&mut self, now_secs: f64) {
        self.timing.bpm_tap_times.push(now_secs);
        if self.timing.bpm_tap_times.len() > 1 {
            let mut diffs = Vec::new();
            for i in 1..self.timing.bpm_tap_times.len() {
                diffs.push(self.timing.bpm_tap_times[i] - self.timing.bpm_tap_times[i - 1]);
            }
            let avg_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
            let bpm = (60.0 / avg_diff) as f32;
            self.timing.bpm_tap_result = Some(bpm);
        }
        if self.timing.bpm_tap_times.len() > 16 {
            self.timing.bpm_tap_times.remove(0);
        }
    }

    pub(crate) fn bpm_tap_reset(&mut self) {
        self.timing.bpm_tap_times.clear();
        self.timing.bpm_tap_result = None;
    }
}

impl State {
    pub(super) fn editor_effective_mode_for_playback(&self) -> EditorMode {
        if self.editor.timeline.playback.playing && self.editor.ui.mode == EditorMode::Null {
            self.editor
                .runtime
                .interaction
                .last_mode
                .unwrap_or(self.editor.ui.mode)
        } else {
            self.editor.ui.mode
        }
    }

    pub(super) fn editor_is_effectively_timing_mode(&self) -> bool {
        self.editor_effective_mode_for_playback() == EditorMode::Timing
    }

    pub(crate) fn set_editor_pan_up_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_pan_up_held(held);
        }
    }

    pub(crate) fn set_editor_pan_down_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_pan_down_held(held);
        }
    }

    pub(crate) fn set_editor_pan_left_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_pan_left_held(held);
        }
    }

    pub(crate) fn set_editor_pan_right_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_pan_right_held(held);
        }
    }

    pub(crate) fn set_editor_shift_held(&mut self, held: bool) {
        self.editor.set_shift_held(held);
    }

    pub(crate) fn set_editor_ctrl_held(&mut self, held: bool) {
        self.editor.set_ctrl_held(held);
    }

    pub(crate) fn set_editor_alt_held(&mut self, held: bool) {
        self.editor.set_alt_held(held);
    }

    pub(crate) fn set_editor_block_id(&mut self, block_id: String) {
        self.editor.set_block_id(block_id);
        self.session.app_settings.editor_selected_block_id =
            self.editor.config.selected_block_id.clone();
        self.persist_app_settings();
        if self.phase == AppPhase::Editor {
            self.rebuild_editor_cursor_vertices();
        }
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor.runtime.interaction.hovered_tap_index = None;
        self.editor.runtime.interaction.hovered_tap_division = None;
        self.editor.set_mode(mode);
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
        self.editor.runtime.dirty.rebuild_tap_indicators = true;
    }

    pub(super) fn refresh_editor_tapping_preview_on_mode_entry(&mut self) {
        if self.phase != AppPhase::Editor
            || self.editor_effective_mode_for_playback() != EditorMode::Tapping
        {
            return;
        }

        self.editor.timeline.tap_division_preview_cache_revision = 0;
        self.editor
            .timeline
            .tap_division_preview_cache_timing_revision = 0;
        self.editor.timeline.tap_division_preview_cache.clear();
        self.editor.sync_tap_indicators_to_spawn();
        self.rebuild_editor_timeline_snapshot_cache_if_needed();

        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            rebuild_cursor: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor.mode()
    }

    pub(crate) fn set_editor_ui_input_blocking_rects(
        &mut self,
        rects: Vec<[f32; 4]>,
        pixels_per_point: f32,
    ) {
        self.editor
            .set_ui_input_blocking_rects(rects, pixels_per_point);
    }

    pub(crate) fn clear_editor_ui_input_blocking_rects(&mut self) {
        self.editor.clear_ui_input_blocking_rects();
    }

    pub(crate) fn editor_pointer_over_ui_input(&self, x: f64, y: f64) -> bool {
        self.phase == AppPhase::Editor && self.editor.pointer_over_ui_input(x, y)
    }

    pub(crate) fn game_cursor(&self, _pointer_over_egui: bool) -> GameCursor {
        if self.phase == AppPhase::Playing && !self.is_game_paused() {
            return GameCursor::Hidden;
        }

        GameCursor::Default
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor.snap_to_grid()
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor.snap_step()
    }

    pub(crate) fn editor_snap_rotation(&self) -> bool {
        self.editor.snap_rotation()
    }

    pub(crate) fn editor_snap_rotation_step_degrees(&self) -> f32 {
        self.editor.snap_rotation_step_degrees()
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor.set_snap_to_grid(snap);
        self.session.app_settings.editor_snap_to_grid = self.editor.config.snap_to_grid;
        self.persist_app_settings();
        if self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor.set_snap_step(step);
        self.session.app_settings.editor_snap_step = self.editor.config.snap_step;
        self.persist_app_settings();
        if self.editor.config.snap_to_grid && self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_rotation(&mut self, snap: bool) {
        self.editor.set_snap_rotation(snap);
        self.session.app_settings.editor_rotation_snap = self.editor.config.snap_rotation;
        self.persist_app_settings();
        if self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_rotation(obj.rotation_degrees);
            }
        }
    }

    pub(crate) fn set_editor_snap_rotation_step(&mut self, step: f32) {
        self.editor.set_snap_rotation_step_degrees(step);
        self.session.app_settings.editor_rotation_snap_step =
            self.editor.config.snap_rotation_step_degrees;
        self.persist_app_settings();
        if self.editor.config.snap_rotation && self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_rotation(obj.rotation_degrees);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.editor.selected_block()
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor.runtime.interaction.gizmo_drag.is_none()
            && self.editor.runtime.interaction.block_drag.is_none()
        {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        self.editor.set_selected_block_position(position);

        if self.editor.ui.selected_block_index.is_some() {
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

        if self.editor.runtime.interaction.gizmo_drag.is_none()
            && self.editor.runtime.interaction.block_drag.is_none()
        {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        self.editor.set_selected_block_size(size);

        if self.editor.ui.selected_block_index.is_some() {
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

        self.editor.set_selected_block_id(block_id);

        if self.editor.ui.selected_block_index.is_some() {
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_rotation(&mut self, rotation_degrees: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        self.editor.set_selected_block_rotation(rotation_degrees);

        if self.editor.ui.selected_block_index.is_some() {
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn rotate_editor_selected_block_preview(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor.rotate_selected_block_preview();
        self.rebuild_editor_cursor_vertices();
    }

    pub(crate) fn set_editor_selected_block_color_tint(&mut self, color_tint: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        self.editor.set_selected_block_color_tint(color_tint);

        if self.editor.ui.selected_block_index.is_some() {
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    /// Returns the unique identifier of the currently selected block type in the editor.
    pub fn editor_selected_block_id(&self) -> &str {
        self.editor.selected_block_id()
    }

    /// Returns whether the editor timeline is currently playing.
    pub fn editor_is_playing(&self) -> bool {
        self.editor.is_playing()
    }

    /// Returns the current playback time of the editor timeline in seconds.
    pub fn editor_timeline_time_seconds(&self) -> f32 {
        self.editor.timeline_time_seconds()
    }

    /// Returns the total duration of the editor timeline in seconds.
    pub fn editor_timeline_duration_seconds(&self) -> f32 {
        self.editor.timeline_duration_seconds()
    }

    /// Returns a slice of all tap event times recorded in the editor's timeline.
    pub fn editor_tap_times(&self) -> &[f32] {
        self.editor.tap_times()
    }

    /// Returns the smoothed frames-per-second (FPS) measurement for the editor.
    pub fn editor_fps(&self) -> f32 {
        self.editor.fps()
    }

    pub(crate) fn editor_marquee_selection_rect_screen(
        &self,
    ) -> Option<([f64; 2], [f64; 2], bool)> {
        self.editor.marquee_selection_rect_screen()
    }

    /// Sets the current playback time of the editor timeline.
    ///
    /// This updates the visual state of the level to reflect the new time
    /// and synchronizes audio playback if necessary.
    pub fn set_editor_timeline_time_seconds(&mut self, time_seconds: f32) {
        puffin::profile_scope!("TimelineSeek");
        let is_effectively_timing = self.editor_is_effectively_timing_mode();
        let changed = self.editor.set_timeline_time_seconds(time_seconds);
        if self.phase == AppPhase::Editor && !is_effectively_timing {
            puffin::profile_scope!("SeekPreview");
            self.apply_editor_timeline_preview_from_cache();
        }
        if changed
            && self.phase == AppPhase::Editor
            && !is_effectively_timing
            && self.editor.has_object_transform_triggers()
        {
            puffin::profile_scope!("SeekDirtyBlockMesh");
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_block_mesh: true,
                rebuild_hitbox_visualization: true,
                rebuild_transform_trigger_markers: true,
                ..EditorDirtyFlags::default()
            });
        }
        if changed && self.phase == AppPhase::Editor && self.editor_mode() == EditorMode::Tapping {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_tap_indicators: true,
                ..EditorDirtyFlags::default()
            });
        }
        if changed {
            if self.phase == AppPhase::Editor && self.editor.timeline.playback.playing {
                const PLAYBACK_SEEK_RESYNC_DEBOUNCE_SECONDS: f32 = 0.12;
                self.editor.timeline.playback.pending_seek_time_seconds =
                    Some(self.editor.timeline.clock.time_seconds);
                self.editor.timeline.playback.seek_resync_cooldown_seconds =
                    PLAYBACK_SEEK_RESYNC_DEBOUNCE_SECONDS;
            } else {
                puffin::profile_scope!("SeekAudioResync");
                self.resync_editor_timeline_playback_audio();
            }
        }
    }

    pub(crate) fn set_editor_timeline_time_seconds_preserving_editor_camera(
        &mut self,
        time_seconds: f32,
    ) {
        let editor_pan = self.editor.camera.editor_pan;
        let editor_target_z = self.editor.camera.editor_target_z;
        self.set_editor_timeline_time_seconds(time_seconds);
        self.editor.camera.editor_pan = editor_pan;
        self.editor.camera.editor_target_z = editor_target_z;
    }

    fn apply_editor_timeline_preview_from_cache(&mut self) {
        if self.phase != AppPhase::Editor || self.editor.timeline.playback.playing {
            return;
        }

        puffin::profile_scope!("PreviewSolveTimeline");
        self.rebuild_editor_timeline_snapshot_cache_if_needed();

        let duration_seconds = self.editor.timeline.clock.duration_seconds;
        let step_seconds = self
            .editor
            .timeline
            .snapshot_cache_step_seconds
            .max(1.0 / 480.0);
        let cache_len = self.editor.timeline.snapshot_cache.len();
        if cache_len == 0 {
            return;
        }

        let target_time = self
            .editor
            .timeline
            .clock
            .time_seconds
            .clamp(0.0, duration_seconds);
        let max_index = cache_len.saturating_sub(1);
        let sample_position = (target_time / step_seconds).clamp(0.0, max_index as f32);
        let lower_index = sample_position.floor() as usize;
        let upper_index = (lower_index + 1).min(max_index);
        let alpha = (sample_position - lower_index as f32).clamp(0.0, 1.0);

        let lower = &self.editor.timeline.snapshot_cache[lower_index];
        let upper = &self.editor.timeline.snapshot_cache[upper_index];

        let position = interpolate_timeline_sample_positions(
            lower.position,
            lower.direction,
            upper.position,
            upper.direction,
            alpha,
        );
        let direction = if alpha < 0.5 {
            lower.direction
        } else {
            upper.direction
        };

        self.apply_editor_timeline_preview_state(position, direction);
    }

    fn rebuild_editor_timeline_snapshot_cache_if_needed(&mut self) {
        if self.editor.timeline.snapshot_cache_revision == self.editor.timeline.simulation_revision
            && !self.editor.timeline.snapshot_cache.is_empty()
        {
            return;
        }

        puffin::profile_scope!("TimelineSamples");
        let duration_seconds = self.editor.timeline.clock.duration_seconds.max(0.0);
        let step_seconds = self
            .editor
            .timeline
            .snapshot_cache_step_seconds
            .max(1.0 / 480.0);

        let total_sample_count =
            ((duration_seconds / step_seconds).ceil() as usize).saturating_add(1);

        // Partial rebuild: reuse retained prefix from invalidate_samples_from
        let existing_count = self.editor.timeline.snapshot_cache.len();
        let resume_index =
            if existing_count > 0 && self.editor.timeline.snapshot_cache_revision == 0 {
                existing_count
            } else {
                self.editor.timeline.snapshot_cache.clear();
                0
            };

        let mut runtime = TimelineSimulationRuntime::new_with_triggers(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.objects,
            &self.editor.timeline.taps.tap_times,
            self.editor.triggers(),
            self.editor.simulate_trigger_hitboxes(),
        );

        // Fast-forward runtime to the resume point
        if resume_index > 0 {
            let resume_time = ((resume_index - 1) as f32 * step_seconds).min(duration_seconds);
            runtime.advance_to(resume_time);
        }

        self.editor
            .timeline
            .snapshot_cache
            .reserve(total_sample_count.saturating_sub(existing_count));
        for index in resume_index..total_sample_count.max(1) {
            let sample_time = (index as f32 * step_seconds).min(duration_seconds);
            runtime.advance_to(sample_time);
            let snapshot = runtime.snapshot();
            self.editor.timeline.snapshot_cache.push(
                crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: snapshot.position,
                    direction: snapshot.direction,
                },
            );
        }

        self.editor.timeline.snapshot_cache_revision = self.editor.timeline.simulation_revision;
        self.editor.timeline.scrub_runtime = None;
        self.editor.timeline.scrub_runtime_revision = 0;
    }

    /// Sets the total duration of the editor timeline.
    ///
    /// This operation is recorded in history and invalidates existing simulation samples.
    pub fn set_editor_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.record_editor_history_state();
        self.editor.set_timeline_duration_seconds(duration_seconds);
        self.editor.invalidate_samples();
        self.resync_editor_timeline_playback_audio();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    /// Adds a tap event at the current timeline position.
    ///
    /// The tap position is derived from the world-space preview position.
    /// This operation is recorded in history.
    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        let indicator_position = self
            .editor
            .tap_indicator_position_from_world(self.editor.timeline.preview.position);
        let tap_time = self.editor.add_tap(indicator_position);
        self.editor.invalidate_samples_from(tap_time);
        self.refresh_editor_after_tap_change(None);
    }

    /// Removes the tap event at the current timeline position, if one exists.
    ///
    /// This operation is recorded in history and invalidates simulation samples from the removed tap's time.
    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor.remove_tap();
        self.editor.invalidate_samples_from(tap_time);
        self.refresh_editor_after_tap_change(None);
    }

    /// Removes the tap event nearest to the given timestamp, if one exists.
    ///
    /// This operation is recorded in history and invalidates simulation samples from the removed tap's time.
    pub fn editor_remove_tap_at(&mut self, time_seconds: f32) {
        self.record_editor_history_state();
        let tap_time = self.editor.remove_tap_at(time_seconds);
        self.editor.invalidate_samples_from(tap_time);
        self.refresh_editor_after_tap_change(None);
    }

    /// Clears all tap events from the editor's timeline.
    ///
    /// This operation is recorded in history and invalidates all simulation samples.
    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        self.editor.clear_taps();
        self.editor.invalidate_samples();
        self.refresh_editor_after_tap_change(None);
    }

    pub(crate) fn editor_add_camera_trigger(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        let trigger = self
            .editor
            .capture_current_camera_trigger(self.editor.timeline.clock.time_seconds);
        self.editor.add_trigger(trigger);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_hitbox_visualization: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_add_trigger(&mut self, trigger: TimedTrigger) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let transforms_objects = timed_trigger_transforms_objects(&trigger);
        self.record_editor_history_state();
        self.editor.add_trigger(trigger);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_block_mesh: transforms_objects,
            rebuild_hitbox_visualization: transforms_objects,
            rebuild_transform_trigger_markers: transforms_objects,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_transform_trigger_capture_active(&self) -> bool {
        self.editor.runtime.transform_trigger_capture.is_some()
    }

    pub(crate) fn begin_editor_transform_trigger_capture(&mut self) -> bool {
        if self.phase != AppPhase::Editor
            || self.editor.runtime.transform_trigger_capture.is_some()
            || self.editor.timeline.playback.playing
        {
            return false;
        }

        let original_objects = self
            .selected_block_indices_normalized()
            .into_iter()
            .filter_map(|index| {
                self.editor
                    .objects
                    .get(index)
                    .cloned()
                    .map(|object| (index, object))
            })
            .collect::<Vec<_>>();

        if original_objects.is_empty() {
            return false;
        }

        let previous_mode = self.editor_mode();
        self.editor.runtime.transform_trigger_capture = Some(EditorTransformTriggerCapture {
            time_seconds: self.editor.timeline.clock.time_seconds.max(0.0),
            original_objects,
            previous_mode,
        });
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        self.session.editor_show_place_window = false;

        if !previous_mode.is_selection_mode() {
            self.set_editor_mode(EditorMode::Move);
        }

        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_cursor: true,
            ..EditorDirtyFlags::default()
        });

        true
    }

    pub(crate) fn commit_editor_transform_trigger_capture(&mut self) -> bool {
        if self.phase != AppPhase::Editor {
            return false;
        }

        let Some(capture) = self.editor.runtime.transform_trigger_capture.take() else {
            return false;
        };

        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;

        let mut triggers = Vec::with_capacity(capture.original_objects.len());
        let mut replaced_object_ids = Vec::with_capacity(capture.original_objects.len());
        for (index, original_object) in &capture.original_objects {
            let Some(final_object) = self.editor.objects.get(*index).cloned() else {
                continue;
            };
            let Ok(object_id) = u32::try_from(*index) else {
                continue;
            };
            replaced_object_ids.push(object_id);

            triggers.push(TimedTrigger {
                time_seconds: capture.time_seconds,
                duration_seconds: 1.0,
                easing: TimedTriggerEasing::EaseInOut,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![object_id],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: final_object.position,
                    rotation_degrees: final_object.rotation_degrees,
                    size: final_object.size,
                },
            });

            if let Some(object) = self.editor.objects.get_mut(*index) {
                *object = original_object.clone();
            }
        }

        if !triggers.is_empty() {
            self.editor.record_history_state_force();
            let retained_triggers = remove_replaced_transform_trigger_targets(
                std::mem::take(&mut self.editor.triggers.items),
                capture.time_seconds,
                &replaced_object_ids,
            );
            self.editor.set_triggers(retained_triggers);
            for trigger in triggers {
                self.editor.add_trigger(trigger);
            }
        }

        self.set_editor_mode(capture.previous_mode);
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_selection_outline_vertices();
        self.mark_editor_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            rebuild_hitbox_visualization: true,
            rebuild_transform_trigger_markers: true,
            ..EditorDirtyFlags::default()
        });

        true
    }

    pub(crate) fn cancel_editor_transform_trigger_capture(&mut self) -> bool {
        if self.phase != AppPhase::Editor {
            return false;
        }

        let Some(capture) = self.editor.runtime.transform_trigger_capture.take() else {
            return false;
        };

        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        for (index, original_object) in &capture.original_objects {
            if let Some(object) = self.editor.objects.get_mut(*index) {
                *object = original_object.clone();
            }
        }

        self.set_editor_mode(capture.previous_mode);
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_selection_outline_vertices();
        self.mark_editor_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
            rebuild_cursor: true,
            rebuild_hitbox_visualization: true,
            rebuild_transform_trigger_markers: true,
            ..EditorDirtyFlags::default()
        });

        true
    }

    pub(crate) fn editor_capture_selected_camera_trigger(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        let Some(selected_index) = self.editor.selected_trigger_index() else {
            return;
        };
        let Some(trigger) = self.editor.triggers().get(selected_index).cloned() else {
            return;
        };
        if !EditorSubsystem::is_camera_track_trigger(&trigger) {
            return;
        }

        let captured = self
            .editor
            .capture_current_camera_trigger(trigger.time_seconds);
        let mut updated = trigger;
        if let (
            crate::types::TimedTriggerAction::CameraPose {
                target_position,
                rotation,
                pitch,
                ..
            },
            crate::types::TimedTriggerAction::CameraPose {
                target_position: captured_target_position,
                rotation: captured_rotation,
                pitch: captured_pitch,
                ..
            },
        ) = (&mut updated.action, captured.action)
        {
            *target_position = captured_target_position;
            *rotation = captured_rotation;
            *pitch = captured_pitch;
            self.editor.update_trigger(selected_index, updated);
        }

        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_apply_selected_camera_trigger(&mut self) {
        if self.phase == AppPhase::Editor
            && self.editor.apply_selected_camera_trigger_to_editor_camera()
        {
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..EditorDirtyFlags::default()
            });
        }
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor.timeline_preview()
    }

    pub(crate) fn editor_timing_points(&self) -> &[TimingPoint] {
        self.editor.timing_points()
    }

    pub(crate) fn editor_triggers(&self) -> &[TimedTrigger] {
        self.editor.triggers()
    }

    pub(crate) fn editor_simulate_trigger_hitboxes(&self) -> bool {
        self.editor.simulate_trigger_hitboxes()
    }

    #[cfg(test)]
    pub(crate) fn editor_hitbox_visualization_enabled(&self) -> bool {
        self.editor.ui.show_hitbox_visualization
    }

    pub(crate) fn toggle_editor_hitbox_visualization(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor.ui.show_hitbox_visualization = !self.editor.ui.show_hitbox_visualization;
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_hitbox_visualization: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_selected_trigger_index(&self) -> Option<usize> {
        self.editor.selected_trigger_index()
    }

    pub(crate) fn editor_remove_trigger(&mut self, index: usize) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let transforms_objects = self
            .editor
            .triggers()
            .get(index)
            .is_some_and(timed_trigger_transforms_objects);
        self.record_editor_history_state();
        self.editor.remove_trigger(index);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_block_mesh: transforms_objects,
            rebuild_hitbox_visualization: true,
            rebuild_transform_trigger_markers: transforms_objects,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn set_editor_trigger_selected(&mut self, selected: Option<usize>) {
        if self.phase == AppPhase::Editor {
            self.editor.set_trigger_selected(selected);
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_transform_trigger_markers: true,
                ..EditorDirtyFlags::default()
            });
        }
    }

    pub(crate) fn editor_update_trigger(&mut self, index: usize, trigger: TimedTrigger) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let transforms_objects = timed_trigger_transforms_objects(&trigger)
            || self
                .editor
                .triggers()
                .get(index)
                .is_some_and(timed_trigger_transforms_objects);
        self.record_editor_history_state();
        self.editor.update_trigger(index, trigger);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_block_mesh: transforms_objects,
            rebuild_hitbox_visualization: true,
            rebuild_transform_trigger_markers: transforms_objects,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn set_editor_simulate_trigger_hitboxes(&mut self, enabled: bool) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor.simulate_trigger_hitboxes() == enabled {
            return;
        }

        self.record_editor_history_state();
        self.editor.set_simulate_trigger_hitboxes(enabled);
        self.editor.invalidate_samples();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_preview_player: true,
            rebuild_block_mesh: true,
            rebuild_hitbox_visualization: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_playback_speed(&self) -> f32 {
        self.editor.playback_speed()
    }

    pub(crate) fn set_editor_playback_speed(&mut self, speed: f32) {
        let actual_speed = self.editor.set_playback_speed(speed);
        self.audio.state.runtime.set_speed(actual_speed);
    }

    pub(crate) fn editor_timing_selected_index(&self) -> Option<usize> {
        self.editor.timing_selected_index()
    }

    pub(crate) fn set_editor_timing_selected_index(&mut self, index: Option<usize>) {
        self.editor.set_timing_selected_index(index);
    }

    pub(crate) fn editor_waveform_zoom(&self) -> f32 {
        self.editor.waveform_zoom()
    }

    pub(crate) fn set_editor_waveform_zoom(&mut self, zoom: f32) {
        self.editor.set_waveform_zoom(zoom);
    }

    pub(crate) fn editor_waveform_scroll(&self) -> f32 {
        self.editor.waveform_scroll()
    }

    pub(crate) fn set_editor_waveform_scroll(&mut self, scroll: f32) {
        self.editor.set_waveform_scroll(scroll);
    }

    pub(crate) fn editor_waveform_samples(&self) -> &[f32] {
        self.editor.waveform_samples()
    }

    pub(crate) fn editor_waveform_sample_rate(&self) -> u32 {
        self.editor.waveform_sample_rate()
    }

    pub(crate) fn editor_waveform_window_size(&self) -> usize {
        self.editor.waveform_window_size()
    }

    pub(crate) fn editor_waveform_loading(&self) -> bool {
        self.editor.waveform_loading()
    }

    pub(crate) fn editor_waveform_complete(&self) -> bool {
        self.editor.waveform_complete()
    }

    pub(crate) fn editor_bpm_tap_result(&self) -> Option<f32> {
        self.editor.bpm_tap_result()
    }

    pub(crate) fn editor_add_timing_point(&mut self, time_seconds: f32, bpm: f32) {
        self.record_editor_history_state();
        self.editor.add_timing_point(time_seconds, bpm);
    }

    pub(crate) fn editor_remove_timing_point(&mut self, index: usize) {
        self.record_editor_history_state();
        self.editor.remove_timing_point(index);
    }

    pub(crate) fn editor_update_timing_point_time(&mut self, index: usize, time: f32) {
        self.record_editor_history_state();
        self.editor.update_timing_point_time(index, time);
    }

    pub(crate) fn editor_update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        self.record_editor_history_state();
        self.editor.update_timing_point_bpm(index, bpm);
    }

    pub(crate) fn editor_update_timing_point_time_signature(
        &mut self,
        index: usize,
        numerator: u32,
        denominator: u32,
    ) {
        self.record_editor_history_state();
        self.editor
            .update_timing_point_time_signature(index, numerator, denominator);
    }

    pub(crate) fn editor_bpm_tap(&mut self) {
        let now_secs = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                web_time::SystemTime::now()
                    .duration_since(web_time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0)
            }
            #[cfg(target_arch = "wasm32")]
            {
                js_sys::Date::now() / 1000.0
            }
        };
        self.editor.bpm_tap(now_secs);
    }

    pub(crate) fn editor_bpm_tap_reset(&mut self) {
        self.editor.bpm_tap_reset();
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::game::TimelineSimulationRuntime;
    use crate::test_utils::assert_approx_eq as approx_eq;
    use crate::types::{
        AppPhase, EditorMode, GameCursor, GizmoAxis, GizmoDragKind, LevelObject, SpawnDirection,
        TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
    };

    fn test_level_object(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    fn assert_object_pose(object: &LevelObject, position: [f32; 3], size: [f32; 3]) {
        for component in 0..3 {
            approx_eq(object.position[component], position[component], 1e-6);
            approx_eq(object.size[component], size[component], 1e-6);
        }
    }

    fn transform_trigger(
        time_seconds: f32,
        object_ids: Vec<u32>,
        position: [f32; 3],
    ) -> TimedTrigger {
        TimedTrigger {
            time_seconds,
            duration_seconds: 1.0,
            easing: TimedTriggerEasing::EaseInOut,
            target: TimedTriggerTarget::Objects { object_ids },
            action: TimedTriggerAction::TransformObjects {
                position,
                rotation_degrees: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
            },
        }
    }

    #[test]
    fn transform_trigger_capture_cancel_restores_original_blocks() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.objects = vec![test_level_object([1.0, 2.0, 3.0])];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.mode = EditorMode::Place;

            assert!(state.begin_editor_transform_trigger_capture());
            assert!(state.editor_transform_trigger_capture_active());
            assert_eq!(state.editor.ui.mode, EditorMode::Move);

            state.editor.objects[0].position = [9.0, 8.0, 7.0];
            state.editor.objects[0].size = [2.0, 3.0, 4.0];

            assert!(state.cancel_editor_transform_trigger_capture());
            assert!(!state.editor_transform_trigger_capture_active());
            assert_eq!(state.editor.ui.mode, EditorMode::Place);
            assert!(state.editor.triggers.items.is_empty());
            assert_object_pose(&state.editor.objects[0], [1.0, 2.0, 3.0], [1.0, 1.0, 1.0]);
        });
    }

    #[test]
    fn transform_trigger_capture_commit_restores_blocks_and_adds_per_object_triggers() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.timeline.clock.time_seconds = 2.5;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.objects = vec![
                test_level_object([0.0, 0.0, 0.0]),
                test_level_object([4.0, 0.0, 0.0]),
            ];
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.mode = EditorMode::Rotate;

            assert!(state.begin_editor_transform_trigger_capture());
            state.editor.objects[0].position = [1.0, 2.0, 3.0];
            state.editor.objects[0].rotation_degrees = [0.0, 45.0, 0.0];
            state.editor.objects[0].size = [2.0, 1.0, 1.0];
            state.editor.objects[1].position = [5.0, 6.0, 7.0];
            state.editor.objects[1].rotation_degrees = [15.0, 0.0, 90.0];
            state.editor.objects[1].size = [1.0, 3.0, 2.0];

            assert!(state.commit_editor_transform_trigger_capture());
            assert!(!state.editor_transform_trigger_capture_active());
            assert_eq!(state.editor.ui.mode, EditorMode::Rotate);
            assert_object_pose(&state.editor.objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            assert_object_pose(&state.editor.objects[1], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

            let mut triggers = state.editor.triggers.items.clone();
            triggers.sort_by_key(|trigger| match &trigger.target {
                TimedTriggerTarget::Objects { object_ids } => object_ids[0],
                TimedTriggerTarget::Camera => u32::MAX,
            });
            assert_eq!(triggers.len(), 2);

            for (trigger, expected_id) in triggers.iter().zip([0, 1]) {
                approx_eq(trigger.time_seconds, 2.5, 1e-6);
                approx_eq(trigger.duration_seconds, 1.0, 1e-6);
                assert_eq!(trigger.easing, TimedTriggerEasing::EaseInOut);
                assert_eq!(
                    trigger.target,
                    TimedTriggerTarget::Objects {
                        object_ids: vec![expected_id],
                    }
                );
            }

            match &triggers[0].action {
                TimedTriggerAction::TransformObjects {
                    position,
                    rotation_degrees,
                    size,
                } => {
                    for component in 0..3 {
                        approx_eq(position[component], [1.0, 2.0, 3.0][component], 1e-6);
                        approx_eq(
                            rotation_degrees[component],
                            [0.0, 45.0, 0.0][component],
                            1e-6,
                        );
                        approx_eq(size[component], [2.0, 1.0, 1.0][component], 1e-6);
                    }
                }
                _ => panic!("expected transform objects action"),
            }
            match &triggers[1].action {
                TimedTriggerAction::TransformObjects {
                    position,
                    rotation_degrees,
                    size,
                } => {
                    for component in 0..3 {
                        approx_eq(position[component], [5.0, 6.0, 7.0][component], 1e-6);
                        approx_eq(
                            rotation_degrees[component],
                            [15.0, 0.0, 90.0][component],
                            1e-6,
                        );
                        approx_eq(size[component], [1.0, 3.0, 2.0][component], 1e-6);
                    }
                }
                _ => panic!("expected transform objects action"),
            }
        });
    }

    #[test]
    fn transform_trigger_capture_commit_overrides_existing_same_time_object_triggers() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.timeline.clock.time_seconds = 2.5;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.objects = vec![
                test_level_object([0.0, 0.0, 0.0]),
                test_level_object([4.0, 0.0, 0.0]),
                test_level_object([8.0, 0.0, 0.0]),
            ];
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.triggers.items = vec![
                transform_trigger(2.5, vec![0], [10.0, 0.0, 0.0]),
                transform_trigger(2.5, vec![1, 2], [20.0, 0.0, 0.0]),
                transform_trigger(2.5, vec![2], [30.0, 0.0, 0.0]),
                transform_trigger(4.0, vec![0], [40.0, 0.0, 0.0]),
            ];

            assert!(state.begin_editor_transform_trigger_capture());
            state.editor.objects[0].position = [1.0, 2.0, 3.0];
            state.editor.objects[1].position = [5.0, 6.0, 7.0];

            assert!(state.commit_editor_transform_trigger_capture());

            let mut object_zero_same_time_count = 0;
            let mut object_one_same_time_count = 0;
            let mut retained_object_two_multi_target = false;
            let mut retained_object_two_standalone = false;
            let mut retained_different_time_object_zero = false;

            for trigger in &state.editor.triggers.items {
                let TimedTriggerTarget::Objects { object_ids } = &trigger.target else {
                    continue;
                };

                if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &[0] {
                    object_zero_same_time_count += 1;
                    if let TimedTriggerAction::TransformObjects { position, .. } = &trigger.action {
                        approx_eq(position[0], 1.0, 1e-6);
                        approx_eq(position[1], 2.0, 1e-6);
                        approx_eq(position[2], 3.0, 1e-6);
                    }
                }

                if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &[1] {
                    object_one_same_time_count += 1;
                    if let TimedTriggerAction::TransformObjects { position, .. } = &trigger.action {
                        approx_eq(position[0], 5.0, 1e-6);
                        approx_eq(position[1], 6.0, 1e-6);
                        approx_eq(position[2], 7.0, 1e-6);
                    }
                }

                if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &[2] {
                    if let TimedTriggerAction::TransformObjects { position, .. } = &trigger.action {
                        if (position[0] - 20.0).abs() <= 1e-6 {
                            retained_object_two_multi_target = true;
                        } else if (position[0] - 30.0).abs() <= 1e-6 {
                            retained_object_two_standalone = true;
                        }
                    }
                }

                if (trigger.time_seconds - 4.0).abs() <= 1e-6 && object_ids == &[0] {
                    retained_different_time_object_zero = true;
                }
            }

            assert_eq!(object_zero_same_time_count, 1);
            assert_eq!(object_one_same_time_count, 1);
            assert!(retained_object_two_multi_target);
            assert!(retained_object_two_standalone);
            assert!(retained_different_time_object_zero);
        });
    }

    #[test]
    fn transform_trigger_keybind_starts_capture_and_escape_cancels_it() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.objects = vec![test_level_object([0.0, 0.0, 0.0])];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);

            state.process_keyboard_input("t", true, true);
            assert!(state.editor_transform_trigger_capture_active());

            state.editor.objects[0].position = [3.0, 4.0, 5.0];
            state.process_keyboard_input("Escape", true, true);

            assert!(!state.editor_transform_trigger_capture_active());
            assert_object_pose(&state.editor.objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
            assert!(state.editor.triggers.items.is_empty());
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
        });
    }

    #[test]
    fn game_cursor_is_hidden_while_playing_or_playtesting() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Playing;
            state.session.playtesting_editor = false;
            state.session.game_paused = false;
            assert_eq!(state.game_cursor(false), GameCursor::Hidden);
            assert_eq!(state.game_cursor(true), GameCursor::Hidden);

            state.session.playtesting_editor = true;
            assert_eq!(state.game_cursor(false), GameCursor::Hidden);
            assert_eq!(state.game_cursor(true), GameCursor::Hidden);
        });
    }

    #[test]
    fn game_cursor_uses_default_cursor_while_real_gameplay_is_paused() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Playing;
            state.session.playtesting_editor = false;
            state.session.game_paused = true;

            assert_eq!(state.game_cursor(false), GameCursor::Default);
            assert_eq!(state.game_cursor(true), GameCursor::Default);
        });
    }

    #[test]
    fn game_cursor_uses_default_cursor_outside_playing_including_over_egui() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Menu;
            assert_eq!(state.game_cursor(false), GameCursor::Default);
            assert_eq!(state.game_cursor(true), GameCursor::Default);

            state.phase = AppPhase::Editor;
            assert_eq!(state.game_cursor(false), GameCursor::Default);
            assert_eq!(state.game_cursor(true), GameCursor::Default);

            state.phase = AppPhase::GameOver;
            assert_eq!(state.game_cursor(false), GameCursor::Default);
            assert_eq!(state.game_cursor(true), GameCursor::Default);
        });
    }

    #[test]
    fn editor_mode_switch_clears_selection_and_drag_state_when_mode_cannot_select() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0, 1];
            state.editor.ui.hovered_block_index = Some(1);
            state.editor.ui.marquee_start_screen = Some([10.0, 20.0]);
            state.editor.ui.marquee_current_screen = Some([30.0, 40.0]);
            state.editor.runtime.interaction.gizmo_drag = Some(super::super::EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [0.0, 0.0],
                start_center_screen: [0.0, 0.0],
                start_center_world: [0.0, 0.0, 0.0],
                start_blocks: Vec::new(),
            });
            state.editor.runtime.interaction.block_drag = Some(super::super::EditorBlockDrag {
                start_mouse: [0.0, 0.0],
                start_center_world: [0.0, 0.0, 0.0],
                start_drag_world: [0.0, 0.0, 0.0],
                start_cursor: [0.0, 0.0, 0.0],
                start_blocks: Vec::new(),
            });

            state.editor.set_mode(EditorMode::Null);

            assert_eq!(state.editor.mode(), EditorMode::Null);
            assert!(state.editor.runtime.interaction.gizmo_drag.is_none());
            assert!(state.editor.runtime.interaction.block_drag.is_none());
            assert!(state.editor.ui.selected_block_index.is_none());
            assert!(state.editor.ui.selected_block_indices.is_empty());
            assert!(state.editor.ui.hovered_block_index.is_none());
            assert!(state.editor.ui.marquee_start_screen.is_none());
            assert!(state.editor.ui.marquee_current_screen.is_none());
        });
    }

    #[test]
    fn selected_block_mutators_apply_snap_and_clamp_rules() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.objects.push(LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            });
            state.editor.ui.selected_block_index = Some(0);
            state.editor.config.snap_to_grid = true;
            state.editor.config.snap_step = 0.5;
            state.editor.config.snap_rotation = true;
            state.editor.config.snap_rotation_step_degrees = 15.0;
            state.editor.ui.ctrl_held = false;

            state.editor.set_selected_block_position([1.24, -1.0, 2.26]);
            assert_eq!(state.editor.objects[0].position, [1.0, 0.0, 2.5]);
            assert_eq!(state.editor.ui.cursor, [1.0, 0.0, 2.5]);

            state.editor.set_selected_block_size([0.1, 0.4, 0.62]);
            assert_eq!(state.editor.objects[0].size, [0.5, 0.5, 0.5]);

            state.editor.set_selected_block_rotation([7.0, 22.0, 44.0]);
            assert_eq!(state.editor.objects[0].rotation_degrees, [0.0, 15.0, 45.0]);

            state.editor.set_selected_block_color_tint([-1.0, 0.4, 2.0]);
            assert_eq!(state.editor.objects[0].color_tint, [0.0, 0.4, 1.0]);

            state.editor.set_selected_block_id("stone".to_string());
            assert_eq!(state.editor.objects[0].block_id, "core/stone");
        });
    }

    #[test]
    fn timeline_clamp_and_cache_invalidation_helpers_work() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.timeline.clock.duration_seconds = 2.0;
            assert!(state.editor.set_timeline_time_seconds(5.0));
            assert_eq!(state.editor.timeline.clock.time_seconds, 2.0);
            assert!(!state.editor.set_timeline_time_seconds(2.0));

            state.editor.timeline.clock.time_seconds = 10.0;
            state.editor.set_timeline_duration_seconds(0.01);
            assert_eq!(state.editor.timeline.clock.duration_seconds, 0.1);
            assert_eq!(state.editor.timeline.clock.time_seconds, 0.1);

            let mut runtime =
                TimelineSimulationRuntime::new([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[]);
            runtime.advance_to(1.5);
            state.editor.timeline.snapshot_cache_step_seconds = 0.5;
            state.editor.timeline.snapshot_cache = vec![
                crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: [0.0, 0.0, 0.0],
                    direction: SpawnDirection::Forward,
                },
                crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: [0.5, 0.0, 0.0],
                    direction: SpawnDirection::Forward,
                },
                crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: [1.0, 0.0, 0.0],
                    direction: SpawnDirection::Forward,
                },
                crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: [1.5, 0.0, 0.0],
                    direction: SpawnDirection::Right,
                },
            ];
            state.editor.timeline.snapshot_cache_revision = 123;
            state.editor.timeline.scrub_runtime = Some(runtime);
            state.editor.timeline.scrub_runtime_revision = 55;

            state.editor.invalidate_samples_from(1.0);
            assert_eq!(state.editor.timeline.snapshot_cache.len(), 2);
            assert_eq!(state.editor.timeline.snapshot_cache_revision, 0);
            assert!(state.editor.timeline.scrub_runtime.is_none());
            assert_eq!(state.editor.timeline.scrub_runtime_revision, 0);

            state.editor.invalidate_samples();
            assert!(state.editor.timeline.snapshot_cache.is_empty());
            assert_eq!(state.editor.timeline.snapshot_cache_revision, 0);
            assert!(state.editor.timeline.scrub_runtime.is_none());
            assert_eq!(state.editor.timeline.scrub_runtime_revision, 0);
        });
    }

    #[test]
    fn bpm_tap_estimates_tempo_and_reset_clears_state() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.bpm_tap(1.0);
            assert!(state.editor.bpm_tap_result().is_none());

            state.editor.bpm_tap(1.5);
            state.editor.bpm_tap(2.0);

            let bpm = state.editor.bpm_tap_result().expect("bpm should exist");
            approx_eq(bpm, 120.0, 1e-3);

            state.editor.bpm_tap_reset();
            assert!(state.editor.bpm_tap_result().is_none());
            assert!(state.editor.timing.bpm_tap_times.is_empty());
        });
    }

    #[test]
    fn state_wrappers_update_timeline_taps_and_simulate_hitbox_flag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = crate::types::AppPhase::Editor;
            state.editor.timeline.preview.position = [3.5, 0.0, 4.5];

            state.set_editor_timeline_duration_seconds(6.0);
            assert_eq!(state.editor_timeline_duration_seconds(), 6.0);

            state.set_editor_timeline_time_seconds(3.0);
            assert_eq!(state.editor_timeline_time_seconds(), 3.0);

            state.editor_add_tap();
            assert_eq!(state.editor_tap_times().len(), 1);

            state.editor_remove_tap_at(3.0);
            assert_eq!(state.editor_tap_times().len(), 0);

            state.editor_add_tap();
            assert_eq!(state.editor_tap_times().len(), 1);

            state.editor_remove_tap();
            assert_eq!(state.editor_tap_times().len(), 0);

            state.editor_add_tap();
            state.editor.timeline.preview.position = [4.5, 0.0, 4.5];
            state.set_editor_timeline_time_seconds(4.0);
            state.editor_add_tap();
            assert_eq!(state.editor_tap_times().len(), 2);

            state.editor_clear_taps();
            assert!(state.editor_tap_times().is_empty());

            assert!(!state.editor_simulate_trigger_hitboxes());
            state.set_editor_simulate_trigger_hitboxes(true);
            assert!(state.editor_simulate_trigger_hitboxes());

            state.set_editor_playback_speed(4.0);
            assert_eq!(state.editor_playback_speed(), 2.0);

            state.set_editor_waveform_zoom(25.0);
            state.set_editor_waveform_scroll(2.5);
            assert_eq!(state.editor_waveform_zoom(), 10.0);
            assert_eq!(state.editor_waveform_scroll(), 2.5);
        });
    }

    #[test]
    fn tap_mutations_mark_indicators_and_preview_dirty() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.timeline.clock.duration_seconds = 1.0;
            state.set_editor_timeline_time_seconds(0.125);
            assert_eq!(
                state.editor.timeline.preview.direction,
                SpawnDirection::Forward
            );
            state.editor.camera.editor_pan = [8.0, -6.0];
            state.editor.camera.editor_target_z = 3.0;
            let original_pan = state.editor.camera.editor_pan;
            let original_target_z = state.editor.camera.editor_target_z;

            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.editor_add_tap();
            assert!(state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(state.editor.runtime.dirty.rebuild_preview_player);
            assert_eq!(
                state.editor.timeline.preview.direction,
                SpawnDirection::Right
            );
            assert_eq!(state.editor.camera.editor_pan, original_pan);
            assert_eq!(state.editor.camera.editor_target_z, original_target_z);

            state.editor.runtime.dirty = crate::state::EditorDirtyFlags::default();
            state.editor_remove_tap();
            assert!(state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(state.editor.runtime.dirty.rebuild_preview_player);
            assert_eq!(
                state.editor.timeline.preview.direction,
                SpawnDirection::Forward
            );
            assert_eq!(state.editor.camera.editor_pan, original_pan);
            assert_eq!(state.editor.camera.editor_target_z, original_target_z);
        });
    }

    #[test]
    fn adding_tap_recomputes_all_indicator_positions() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.objects.clear();
            state.editor.spawn.position = [0.0, 0.0, 0.0];
            state.editor.spawn.direction = SpawnDirection::Forward;
            state.editor.timeline.clock.duration_seconds = 2.0;

            let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
            state.set_editor_timeline_time_seconds(4.0 * step_time);
            state.editor_add_tap();

            state.set_editor_timeline_time_seconds(2.0 * step_time);
            state.editor_add_tap();

            let expected = crate::editor_domain::derive_tap_indicator_positions(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.timeline.taps.tap_times,
                &state.editor.objects,
            );

            assert_eq!(state.editor.timeline.taps.tap_indicator_positions, expected);
        });
    }

    #[test]
    fn pan_input_wrappers_are_phase_gated_but_modifiers_are_global() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Menu;

            state.set_editor_pan_up_held(true);
            state.set_editor_pan_down_held(true);
            state.set_editor_pan_left_held(true);
            state.set_editor_pan_right_held(true);
            state.set_editor_shift_held(true);
            state.set_editor_ctrl_held(true);
            state.set_editor_alt_held(true);

            assert!(!state.editor.ui.pan_up_held);
            assert!(!state.editor.ui.pan_down_held);
            assert!(!state.editor.ui.pan_left_held);
            assert!(!state.editor.ui.pan_right_held);
            assert!(state.editor.ui.shift_held);
            assert!(state.editor.ui.ctrl_held);
            assert!(state.editor.ui.alt_held);

            state.set_editor_shift_held(false);
            state.set_editor_ctrl_held(false);
            state.set_editor_alt_held(false);

            state.phase = AppPhase::Editor;
            state.set_editor_pan_up_held(true);
            state.set_editor_pan_down_held(true);
            state.set_editor_pan_left_held(true);
            state.set_editor_pan_right_held(true);
            state.set_editor_shift_held(true);
            state.set_editor_ctrl_held(true);
            state.set_editor_alt_held(true);

            assert!(state.editor.ui.pan_up_held);
            assert!(state.editor.ui.pan_down_held);
            assert!(state.editor.ui.pan_left_held);
            assert!(state.editor.ui.pan_right_held);
            assert!(state.editor.ui.shift_held);
            assert!(state.editor.ui.ctrl_held);
            assert!(state.editor.ui.alt_held);
        });
    }

    #[test]
    fn editor_effective_mode_for_playback_uses_last_mode_when_ui_is_null() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.timeline.playback.playing = true;
            state.editor.ui.mode = EditorMode::Null;
            state.editor.runtime.interaction.last_mode = Some(EditorMode::Timing);
            assert_eq!(
                state.editor_effective_mode_for_playback(),
                EditorMode::Timing
            );
            assert!(state.editor_is_effectively_timing_mode());

            state.editor.runtime.interaction.last_mode = None;
            assert_eq!(state.editor_effective_mode_for_playback(), EditorMode::Null);
            assert!(!state.editor_is_effectively_timing_mode());

            state.editor.timeline.playback.playing = false;
            state.editor.ui.mode = EditorMode::Place;
            assert_eq!(
                state.editor_effective_mode_for_playback(),
                EditorMode::Place
            );
        });
    }

    #[test]
    fn editor_wrapper_setters_and_getters_cover_timing_and_waveform_state() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor_add_timing_point(2.0, 130.0);
            state.editor_add_timing_point(1.0, 120.0);
            assert_eq!(state.editor_timing_points().len(), 2);
            assert!(
                state.editor_timing_points()[0].time_seconds
                    <= state.editor_timing_points()[1].time_seconds
            );

            state.set_editor_timing_selected_index(Some(1));
            assert_eq!(state.editor_timing_selected_index(), Some(1));
            state.editor_update_timing_point_time(1, -5.0);
            assert!(state.editor_timing_points()[0].time_seconds >= 0.0);
            state.editor_update_timing_point_bpm(0, 0.2);
            assert!(state.editor_timing_points()[0].bpm >= 1.0);
            state.editor_update_timing_point_time_signature(0, 0, 0);
            assert!(state.editor_timing_points()[0].time_signature_numerator >= 1);
            assert!(state.editor_timing_points()[0].time_signature_denominator >= 1);
            state.editor_remove_timing_point(5);
            state.editor_remove_timing_point(0);
            assert!(state.editor_timing_points().len() <= 1);

            state.set_editor_waveform_zoom(0.01);
            state.set_editor_waveform_scroll(3.25);
            assert_eq!(state.editor_waveform_zoom(), 0.1);
            assert_eq!(state.editor_waveform_scroll(), 3.25);

            state.editor.timing.waveform_samples = vec![0.1, 0.2, 0.3];
            state.editor.timing.waveform_sample_rate = 44_100;
            assert_eq!(state.editor_waveform_samples(), &[0.1, 0.2, 0.3]);
            assert_eq!(state.editor_waveform_sample_rate(), 44_100);
        });
    }

    #[test]
    fn set_editor_block_id_and_snap_settings_update_app_settings() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;

            state.editor.objects.push(LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            });
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.set_editor_block_id("core/lava".to_string());
            assert_eq!(state.editor_selected_block_id(), "core/lava");
            assert_eq!(state.app_settings().editor_selected_block_id, "core/lava");

            state.set_editor_snap_to_grid(true);
            state.set_editor_snap_step(0.2);
            state.set_editor_snap_rotation(true);
            state.set_editor_snap_rotation_step(7.5);

            assert!(state.editor_snap_to_grid());
            assert_eq!(state.editor_snap_step(), 0.2);
            assert!(state.editor_snap_rotation());
            assert_eq!(state.editor_snap_rotation_step_degrees(), 7.5);
            assert!(state.app_settings().editor_snap_to_grid);
            assert_eq!(state.app_settings().editor_snap_step, 0.2);
            assert!(state.app_settings().editor_rotation_snap);
            assert_eq!(state.app_settings().editor_rotation_snap_step, 7.5);
        });
    }
}
