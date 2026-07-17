/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{EditorDirtyFlags, EditorSubsystem};
use eg_blocks::BlockId;
use eg_editor_domain::{
    add_tap_with_indicator, clear_taps_with_indicators, remove_tap_with_indicator,
    snap_cell_to_step, snap_component_to_step,
};
use eg_types::{Direction, EditorMode, LevelObject, TimingPoint};
use eg_types::{EasingDirection, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget};

const TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS: f32 = 0.001;

fn remove_replaced_transform_trigger_targets(
    objects: &mut [LevelObject],
    time_seconds: f32,
    object_ids_to_replace: &[String],
) {
    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        if (trigger.time_seconds - time_seconds).abs() > TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
            || !trigger.transform
        {
            continue;
        }
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        if !object_ids
            .iter()
            .any(|object_id| object_ids_to_replace.contains(object_id))
        {
            continue;
        }
        object_ids.retain(|object_id| !object_ids_to_replace.contains(object_id));
    }
}

impl EditorSubsystem {
    pub fn set_pan_up_held(&mut self, held: bool) {
        self.input.pan_up_held = held;
    }

    pub fn set_pan_down_held(&mut self, held: bool) {
        self.input.pan_down_held = held;
    }

    pub fn set_pan_left_held(&mut self, held: bool) {
        self.input.pan_left_held = held;
    }

    pub fn set_pan_right_held(&mut self, held: bool) {
        self.input.pan_right_held = held;
    }

    pub fn set_shift_held(&mut self, held: bool) {
        self.input.shift_held = held;
    }

    pub fn set_ctrl_held(&mut self, held: bool) {
        self.input.ctrl_held = held;
    }

    pub fn set_alt_held(&mut self, held: bool) {
        self.input.alt_held = held;
    }

    pub fn set_right_dragging(&mut self, dragging: bool) {
        self.input.right_dragging = dragging;
    }

    pub fn set_left_mouse_down(&mut self, pressed: bool) {
        self.input.left_mouse_down = pressed;
    }

    pub fn set_pointer_screen(&mut self, position: Option<[f64; 2]>) {
        self.input.pointer_screen = position;
    }

    pub fn set_ui_input_blocking_rects(&mut self, rects: Vec<[f32; 4]>, pixels_per_point: f32) {
        self.input.ui_input_blocking_rects = rects;
        self.input.ui_input_pixels_per_point =
            if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
                pixels_per_point
            } else {
                1.0
            };
    }

    pub fn clear_ui_input_blocking_rects(&mut self) {
        self.input.ui_input_blocking_rects.clear();
        self.input.ui_input_pixels_per_point = 1.0;
    }

    pub fn pointer_over_ui_input(&self, x: f64, y: f64) -> bool {
        let pixels_per_point = self.input.ui_input_pixels_per_point as f64;
        let point_x = x / pixels_per_point;
        let point_y = y / pixels_per_point;

        self.input.ui_input_blocking_rects.iter().any(|rect| {
            point_x >= rect[0] as f64
                && point_x <= rect[2] as f64
                && point_y >= rect[1] as f64
                && point_y <= rect[3] as f64
        })
    }

    pub fn clear_interaction_drags(&mut self) {
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
    }

    pub fn left_mouse_down(&self) -> bool {
        self.input.left_mouse_down
    }

    pub fn has_gizmo_drag(&self) -> bool {
        self.runtime.interaction.gizmo_drag.is_some()
    }

    pub fn has_block_drag(&self) -> bool {
        self.runtime.interaction.block_drag.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.timeline.playback.playing
    }

    pub fn timeline_time_seconds(&self) -> f32 {
        self.timeline.clock.time_seconds
    }

    pub fn set_block_id(&mut self, block_id: BlockId) {
        self.config.selected_block_id = block_id;
        self.config
            .remember_recent_block_id(self.config.selected_block_id.clone());
    }

    pub fn set_mode(&mut self, mode: EditorMode) {
        self.input.mode = mode;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
        self.input.marquee_start_screen = None;
        self.input.marquee_current_screen = None;
        if !mode.can_select() {
            self.clear_block_selection();
        }
    }

    pub fn mode(&self) -> EditorMode {
        self.input.mode
    }

    pub fn snap_to_grid(&self) -> bool {
        self.config.snap_to_grid
    }

    pub fn effective_snap_to_grid(&self) -> bool {
        self.config.snap_to_grid && !self.input.ctrl_held
    }

    pub fn snap_step(&self) -> f32 {
        self.config.snap_step
    }

    pub fn snap_rotation(&self) -> bool {
        self.config.snap_rotation
    }

    pub fn effective_snap_rotation(&self) -> bool {
        self.config.snap_rotation && !self.input.ctrl_held
    }

    pub fn snap_rotation_step_degrees(&self) -> f32 {
        self.config.snap_rotation_step_degrees
    }

    pub fn selected_block(&self) -> Option<LevelObject> {
        self.selected_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.objects.get(index).cloned())
    }

    pub fn set_snap_to_grid(&mut self, snap: bool) {
        self.config.snap_to_grid = snap;
    }

    pub fn set_snap_step(&mut self, step: f32) {
        self.config.snap_step = step.max(0.05);
    }

    pub fn set_snap_rotation(&mut self, snap: bool) {
        self.config.snap_rotation = snap;
    }

    pub fn set_snap_rotation_step_degrees(&mut self, step: f32) {
        self.config.snap_rotation_step_degrees = step.max(1.0);
    }

    pub fn set_selected_block_position(&mut self, position: [f32; 3]) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let snap_step = self.config.snap_step.max(0.05);
            let next_position = if self.effective_snap_to_grid() {
                snap_cell_to_step(position, snap_step)
            } else {
                [position[0], position[1].max(0.0), position[2]]
            };
            self.objects[index].position = next_position;
            self.input.cursor = [
                next_position[0],
                next_position[1].max(0.0),
                next_position[2],
            ];
        }
    }

    pub fn set_selected_block_size(&mut self, size: [f32; 3]) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let snap_step = self.config.snap_step.max(0.05);
            let snapped_size = if self.effective_snap_to_grid() {
                size.map(|component| snap_component_to_step(component, snap_step))
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

    pub fn set_selected_block_id(&mut self, block_id: BlockId) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].block_id = block_id;
        }
    }

    pub fn set_selected_block_rotation(&mut self, rotation: [f32; 3]) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let next_rotation = if self.effective_snap_rotation() {
                let step = self.config.snap_rotation_step_degrees.max(1.0);
                rotation.map(|component| (component / step).round() * step)
            } else {
                rotation
            };
            self.objects[index].rotation = next_rotation;
        }
    }

    pub fn rotate_selected_block_preview(&mut self) {
        self.config.selected_block_rotation_degrees[1] =
            (self.config.selected_block_rotation_degrees[1] + 90.0).rem_euclid(360.0);
    }

    pub fn set_selected_block_color_tint(&mut self, color_tint: [f32; 3]) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].color_tint = color_tint.map(|component| component.clamp(0.0, 1.0));
        }
    }

    pub fn timeline_duration_seconds(&self) -> f32 {
        self.timeline.clock.duration_seconds
    }

    pub fn tap_times(&self) -> &[f32] {
        &self.timeline.taps.tap_times
    }

    pub fn selected_tap(&self) -> Option<(usize, f32, [f32; 3])> {
        let index = self.timeline.taps.selected_index?;
        let time_seconds = *self.timeline.taps.tap_times.get(index)?;
        let position = *self.timeline.taps.tap_indicator_positions.get(index)?;
        Some((index, time_seconds, position))
    }

    pub fn set_selected_tap_index(&mut self, selected_index: Option<usize>) {
        self.timeline.taps.selected_index = selected_index.filter(|index| {
            *index < self.timeline.taps.tap_times.len()
                && *index < self.timeline.taps.tap_indicator_positions.len()
        });
    }

    pub fn adjust_selected_tap_after_removal(&mut self, removed_index: usize) {
        self.timeline.taps.selected_index = match self.timeline.taps.selected_index {
            Some(selected_index) if selected_index == removed_index => None,
            Some(selected_index) if selected_index > removed_index => Some(selected_index - 1),
            selected_index => selected_index,
        };
    }

    pub fn set_timeline_time_seconds(&mut self, time_seconds: f32) -> bool {
        let old_time = self.timeline.clock.time_seconds;
        self.timeline.clock.time_seconds =
            time_seconds.clamp(0.0, self.timeline.clock.duration_seconds);
        (self.timeline.clock.time_seconds - old_time).abs() > 0.0001
    }

    pub fn set_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.timeline.clock.duration_seconds = duration_seconds.max(0.1);
        self.timeline.clock.time_seconds = self
            .timeline
            .clock
            .time_seconds
            .min(self.timeline.clock.duration_seconds);
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

    pub fn remove_tap(&mut self) -> f32 {
        if let Some(removed_index) = remove_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
        ) {
            self.adjust_selected_tap_after_removal(removed_index);
        }
        self.timeline.clock.time_seconds
    }

    pub fn remove_tap_at(&mut self, time_seconds: f32) -> f32 {
        if let Some(removed_index) = remove_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            time_seconds,
        ) {
            self.adjust_selected_tap_after_removal(removed_index);
        }
        time_seconds
    }

    pub fn clear_taps(&mut self) {
        clear_taps_with_indicators(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
        );
        self.timeline.taps.selected_index = None;
    }

    pub fn invalidate_samples(&mut self) {
        self.timeline.simulation_revision = self.timeline.simulation_revision.wrapping_add(1);
        self.timeline.snapshot_cache_revision = 0;
        self.timeline.snapshot_cache.clear();
        self.timeline.trail_point_cache.clear();
        self.timeline.trail_segment_start_cache.clear();
        self.timeline.scrub_runtime = None;
        self.timeline.scrub_runtime_revision = 0;
    }

    pub fn invalidate_samples_from(&mut self, from_seconds: f32) {
        self.timeline.simulation_revision = self.timeline.simulation_revision.wrapping_add(1);

        // Partial invalidation: keep cached snapshots before the change point
        let step_seconds = self.timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
        if !self.timeline.snapshot_cache.is_empty() && from_seconds > step_seconds {
            let keep_count = (from_seconds / step_seconds).floor() as usize;
            let keep_count = keep_count.min(self.timeline.snapshot_cache.len());
            if keep_count > 0 {
                self.timeline.snapshot_cache.truncate(keep_count);
                self.truncate_cached_timeline_trail_to_snapshot_prefix();
                // Mark as partial so rebuild can continue from the retained prefix
                self.timeline.snapshot_cache_revision = 0;
            } else {
                self.timeline.snapshot_cache.clear();
                self.timeline.trail_point_cache.clear();
                self.timeline.trail_segment_start_cache.clear();
                self.timeline.snapshot_cache_revision = 0;
            }
        } else {
            self.timeline.snapshot_cache.clear();
            self.timeline.trail_point_cache.clear();
            self.timeline.trail_segment_start_cache.clear();
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

    fn truncate_cached_timeline_trail_to_snapshot_prefix(&mut self) {
        let Some(snapshot) = self.timeline.snapshot_cache.last() else {
            self.timeline.trail_point_cache.clear();
            self.timeline.trail_segment_start_cache.clear();
            return;
        };

        self.timeline
            .trail_point_cache
            .truncate(snapshot.trail_point_count);
        self.timeline
            .trail_segment_start_cache
            .truncate(snapshot.trail_segment_count);
    }

    pub fn timeline_preview(&self) -> ([f32; 3], Direction) {
        (
            self.timeline.preview.position,
            self.timeline.preview.direction,
        )
    }

    pub fn timing_points(&self) -> &[TimingPoint] {
        &self.timing.timing_points
    }

    pub fn playback_speed(&self) -> f32 {
        self.timing.playback_speed
    }

    pub fn set_playback_speed(&mut self, speed: f32) -> f32 {
        self.timing.playback_speed = speed.clamp(0.1, 2.0);
        self.timing.playback_speed
    }

    pub fn timing_selected_index(&self) -> Option<usize> {
        self.timing.timing_selected_index
    }

    pub fn set_timing_selected_index(&mut self, index: Option<usize>) {
        self.timing.timing_selected_index = index;
    }

    pub fn waveform_zoom(&self) -> f32 {
        self.timing.waveform_zoom
    }

    pub fn set_waveform_zoom(&mut self, zoom: f32) {
        self.timing.waveform_zoom = zoom.clamp(0.1, 10.0);
    }

    pub fn waveform_scroll(&self) -> f32 {
        self.timing.waveform_scroll
    }

    pub fn set_waveform_scroll(&mut self, scroll: f32) {
        self.timing.waveform_scroll = scroll;
    }

    pub fn waveform_samples(&self) -> &[f32] {
        &self.timing.waveform_samples
    }

    pub fn waveform_sample_rate(&self) -> u32 {
        self.timing.waveform_sample_rate
    }

    pub fn waveform_window_size(&self) -> usize {
        self.timing.waveform_window_size
    }

    pub fn waveform_loading(&self) -> bool {
        self.timing.waveform_loading
    }

    pub fn waveform_complete(&self) -> bool {
        self.timing.waveform_complete
    }

    pub fn bpm_tap_result(&self) -> Option<f32> {
        self.timing.bpm_tap_result
    }

    pub fn add_timing_point(&mut self, time_seconds: f32, bpm: f32) {
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

    pub fn remove_timing_point(&mut self, index: usize) {
        if index < self.timing.timing_points.len() {
            self.timing.timing_points.remove(index);
            self.timing.mark_timing_points_changed();
        }
    }

    pub fn update_timing_point_time(&mut self, index: usize, time: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.time_seconds = time.max(0.0);
            self.timing.mark_timing_points_changed();
        }
        self.timing
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    }

    pub fn update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.bpm = bpm.max(1.0);
            self.timing.mark_timing_points_changed();
        }
    }

    pub fn update_timing_point_time_signature(
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

    pub fn bpm_tap(&mut self, now_secs: f64) {
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

    pub fn bpm_tap_reset(&mut self) {
        self.timing.bpm_tap_times.clear();
        self.timing.bpm_tap_result = None;
    }

    pub fn add_transform_trigger(&mut self) -> bool {
        if self.timeline.playback.playing {
            return false;
        }

        let selected_objects = self
            .selected_indices_normalized()
            .into_iter()
            .filter_map(|index| {
                let object = self.objects.get(index)?;
                if object.trigger.is_some() {
                    return None;
                }
                let object_id = (!object.id.is_empty()).then(|| object.id.clone())?;
                Some((object_id, object.clone()))
            })
            .collect::<Vec<_>>();

        if selected_objects.is_empty() {
            return false;
        }

        self.record_history_state();
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
        let time_seconds = self.timeline.clock.time_seconds.max(0.0);

        let replaced_object_ids = selected_objects
            .iter()
            .map(|(object_id, _)| object_id.clone())
            .collect::<Vec<_>>();
        remove_replaced_transform_trigger_targets(
            &mut self.objects,
            time_seconds,
            &replaced_object_ids,
        );
        self.sync_trigger_selection_from_objects();

        let mut created_trigger_indices = Vec::with_capacity(selected_objects.len());
        for (object_id, object) in selected_objects {
            let object_index = self.objects.len();
            let trigger_object = LevelObject {
                position: object.position,
                rotation: object.rotation,
                size: object.size,
                block_id: BlockId::new(TRANSFORM_TRIGGER_BLOCK_ID),
                trigger: Some(TimedTrigger {
                    time_seconds,
                    duration_seconds: 1.0,
                    easing: EasingDirection::EaseInOut,
                    target: TriggerTarget::Objects {
                        object_ids: vec![object_id],
                    },
                    transform: true,
                    ..TimedTrigger::default()
                }),
                ..LevelObject::default()
            }
            .with_generated_id(&self.objects);
            self.objects.push(trigger_object);
            self.sync_trigger_selection_from_objects();
            created_trigger_indices.push(object_index);
        }

        self.replace_block_selection(created_trigger_indices);
        self.input.hovered_block_index = self.input.selected_block_index;
        self.sync_primary_selection_from_indices();
        let mut dirty = EditorDirtyFlags::trigger_object_added();
        dirty.rebuild_selection_overlays = true;
        self.mark_dirty(dirty);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::EditorSubsystem;
    use eg_blocks::BlockId;
    use eg_game::TimelineSimulationRuntime;
    use eg_mesh::{GizmoAxis, GizmoDragKind};
    use eg_types::{
        Direction, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TriggerTarget,
    };

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
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![transform_trigger_object(1.0, vec!["object_0"])];
        editor.sync_trigger_selection_from_objects();
        editor.input.selected_block_indices = vec![0];
        editor.input.selected_block_index = Some(0);

        assert!(!editor.add_transform_trigger());
        assert_eq!(editor.objects.len(), 1);
        assert_eq!(editor.trigger_objects().len(), 1);
    }

    #[test]
    fn add_transform_trigger_creates_selected_markers_for_selected_blocks() {
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.time_seconds = 2.5;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
        ];
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.selected_block_index = Some(0);
        editor.input.mode = EditorMode::Rotate;

        assert!(editor.add_transform_trigger());
        assert_eq!(editor.input.mode, EditorMode::Rotate);
        assert_object_pose(&editor.objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_object_pose(&editor.objects[1], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(editor.objects.len(), 4);
        assert_eq!(editor.objects[2].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        assert_object_pose(&editor.objects[2], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(editor.objects[2].rotation, [0.0, 0.0, 0.0]);
        assert_eq!(editor.objects[3].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        assert_object_pose(&editor.objects[3], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(editor.objects[3].rotation, [0.0, 0.0, 0.0]);
        assert_eq!(editor.input.selected_block_index, Some(2));
        assert_eq!(editor.input.selected_block_indices, vec![2, 3]);

        let mut triggers = editor.trigger_objects();
        triggers.sort_by_key(|object| match &object.trigger.as_ref().unwrap().target {
            TriggerTarget::Objects { object_ids } => object_ids[0].clone(),
            TriggerTarget::Camera => String::new(),
        });
        assert_eq!(triggers.len(), 2);

        for (trigger_object, expected_id) in triggers.iter().zip(["object_0", "object_4"]) {
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
        assert_object_pose(&triggers[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(triggers[0].rotation, [0.0, 0.0, 0.0]);
        assert_object_pose(&triggers[1], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(triggers[1].rotation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn add_transform_trigger_targets_block_after_existing_trigger_object() {
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.time_seconds = 2.5;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            transform_trigger_object(1.0, vec!["object_0"]),
            test_level_object([4.0, 0.0, 0.0]),
        ];
        editor.sync_trigger_selection_from_objects();
        editor.input.selected_block_indices = vec![2];
        editor.input.selected_block_index = Some(2);

        assert!(editor.add_transform_trigger());

        assert_eq!(editor.objects.len(), 4);
        assert_object_pose(&editor.objects[2], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(editor.input.selected_block_index, Some(3));
        assert_eq!(editor.input.selected_block_indices, vec![3]);

        let mut found_retargeted_capture = false;
        for trigger_object in &editor.trigger_objects() {
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
                approx::assert_abs_diff_eq!(trigger_object.position[0], 4.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[1], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[2], 0.0, epsilon = 1e-6);
            }
        }

        assert!(found_retargeted_capture);
    }

    #[test]
    fn add_transform_trigger_preserves_existing_moved_trigger_object_pose() {
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.time_seconds = 2.5;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
        ];
        editor
            .objects
            .push(transform_trigger(2.5, vec!["object_0"], [8.0, 0.0, 0.0]));
        editor.objects[2].position = [12.0, 1.0, 3.0];
        editor.objects[2].size = [2.0, 3.0, 4.0];
        editor.objects[2].rotation = [10.0, 20.0, 30.0];
        editor.input.selected_block_indices = vec![1];
        editor.input.selected_block_index = Some(1);

        assert!(editor.add_transform_trigger());

        assert_object_pose(&editor.objects[2], [12.0, 1.0, 3.0], [2.0, 3.0, 4.0]);
        assert_eq!(editor.objects[2].rotation, [10.0, 20.0, 30.0]);
        let triggers = editor.trigger_objects();
        let existing_trigger_object = triggers
            .iter()
            .find(|object| match &object.trigger.as_ref().unwrap().target {
                TriggerTarget::Objects { object_ids } => object_ids == &["object_0".to_string()],
                TriggerTarget::Camera => false,
            })
            .expect("existing transform trigger remains");
        assert_eq!(existing_trigger_object.position, [12.0, 1.0, 3.0]);
        assert_eq!(existing_trigger_object.rotation, [10.0, 20.0, 30.0]);
        assert_eq!(existing_trigger_object.size, [2.0, 3.0, 4.0]);
    }

    #[test]
    fn add_transform_trigger_overrides_existing_same_time_object_triggers() {
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.time_seconds = 2.5;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
            test_level_object([8.0, 0.0, 0.0]),
        ];
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.selected_block_index = Some(0);
        editor.objects.extend(vec![
            transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_4", "object_8"], [20.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_8"], [30.0, 0.0, 0.0]),
            transform_trigger(4.0, vec!["object_0"], [40.0, 0.0, 0.0]),
        ]);

        assert!(editor.add_transform_trigger());

        let mut object_zero_same_time_count = 0;
        let mut object_one_same_time_count = 0;
        let mut retained_object_two_multi_target = false;
        let mut retained_object_two_standalone = false;
        let mut retained_different_time_object_zero = false;

        for trigger_object in &editor.trigger_objects() {
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
                approx::assert_abs_diff_eq!(trigger_object.position[0], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[1], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[2], 0.0, epsilon = 1e-6);
            }

            if (trigger.time_seconds - 2.5).abs() <= 1e-6 && object_ids == &["object_4".to_string()]
            {
                object_one_same_time_count += 1;
                approx::assert_abs_diff_eq!(trigger_object.position[0], 4.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[1], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[2], 0.0, epsilon = 1e-6);
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
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.time_seconds = 2.5;
        editor.timeline.clock.duration_seconds = 8.0;
        editor.objects = vec![test_level_object([0.0, 0.0, 0.0])];
        editor.input.selected_block_indices = vec![0];
        editor.input.selected_block_index = Some(0);
        editor
            .objects
            .push(transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]));

        assert!(editor.add_transform_trigger());

        let mut found_target_only_trigger = false;
        let mut found_replacement_trigger = false;

        for trigger_object in &editor.trigger_objects() {
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
                approx::assert_abs_diff_eq!(trigger_object.position[0], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[1], 0.0, epsilon = 1e-6);
                approx::assert_abs_diff_eq!(trigger_object.position[2], 0.0, epsilon = 1e-6);
            }
        }

        assert!(found_target_only_trigger);
        assert!(found_replacement_trigger);
    }

    #[test]
    fn transform_trigger_keybind_creates_selected_trigger_block() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![test_level_object([0.0, 0.0, 0.0])];
        editor.input.selected_block_indices = vec![0];
        editor.input.selected_block_index = Some(0);

        assert!(editor.add_transform_trigger());
        assert_object_pose(&editor.objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(editor.trigger_objects().len(), 1);
        assert_eq!(editor.input.selected_block_indices, vec![1]);
        assert_eq!(editor.objects[1].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
    }

    #[test]
    fn editor_mode_switch_clears_selection_and_drag_state_when_mode_cannot_select() {
        let mut editor = EditorSubsystem::default();

        editor.input.selected_block_index = Some(0);
        editor.input.selected_block_indices = vec![0, 1];
        editor.input.hovered_block_index = Some(1);
        editor.input.marquee_start_screen = Some([10.0, 20.0]);
        editor.input.marquee_current_screen = Some([30.0, 40.0]);
        editor.runtime.interaction.gizmo_drag = Some(super::super::EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Move,
            start_mouse: [0.0, 0.0],
            start_center_screen: [0.0, 0.0],
            start_center_world: [0.0, 0.0, 0.0],
            start_blocks: Vec::new(),
        });
        editor.runtime.interaction.block_drag = Some(super::super::EditorBlockDrag {
            start_mouse: [0.0, 0.0],
            start_center_world: [0.0, 0.0, 0.0],
            start_drag_world: [0.0, 0.0, 0.0],
            start_cursor: [0.0, 0.0, 0.0],
            start_blocks: Vec::new(),
        });

        editor.set_mode(EditorMode::Null);

        assert_eq!(editor.mode(), EditorMode::Null);
        assert!(editor.runtime.interaction.gizmo_drag.is_none());
        assert!(editor.runtime.interaction.block_drag.is_none());
        assert!(editor.input.selected_block_index.is_none());
        assert!(editor.input.selected_block_indices.is_empty());
        assert!(editor.input.hovered_block_index.is_none());
        assert!(editor.input.marquee_start_screen.is_none());
        assert!(editor.input.marquee_current_screen.is_none());
    }

    #[test]
    fn selected_block_mutators_apply_snap_and_clamp_rules() {
        let mut editor = EditorSubsystem::default();

        editor.objects.push(LevelObject::default());
        editor.input.selected_block_index = Some(0);
        editor.config.snap_to_grid = true;
        editor.config.snap_step = 0.5;
        editor.config.snap_rotation = true;
        editor.config.snap_rotation_step_degrees = 15.0;
        editor.input.ctrl_held = false;

        editor.set_selected_block_position([1.24, -1.0, 2.26]);
        assert_eq!(editor.objects[0].position, [1.0, 0.0, 2.5]);
        assert_eq!(editor.input.cursor, [1.0, 0.0, 2.5]);

        editor.set_selected_block_size([0.1, 0.4, 0.62]);
        assert_eq!(editor.objects[0].size, [0.5, 0.5, 0.5]);

        editor.set_selected_block_rotation([7.0, 22.0, 44.0]);
        assert_eq!(editor.objects[0].rotation, [0.0, 15.0, 45.0]);

        editor.set_selected_block_color_tint([-1.0, 0.4, 2.0]);
        assert_eq!(editor.objects[0].color_tint, [0.0, 0.4, 1.0]);

        editor.set_selected_block_id(BlockId::new("stone"));
        assert_eq!(editor.objects[0].block_id, "core/stone");
    }

    #[test]
    fn timeline_clamp_and_cache_invalidation_helpers_work() {
        let mut editor = EditorSubsystem::default();

        editor.timeline.clock.duration_seconds = 2.0;
        assert!(editor.set_timeline_time_seconds(5.0));
        assert_eq!(editor.timeline.clock.time_seconds, 2.0);
        assert!(!editor.set_timeline_time_seconds(2.0));

        editor.timeline.clock.time_seconds = 10.0;
        editor.set_timeline_duration_seconds(0.01);
        assert_eq!(editor.timeline.clock.duration_seconds, 0.1);
        assert_eq!(editor.timeline.clock.time_seconds, 0.1);

        let mut runtime =
            TimelineSimulationRuntime::new([0.0, 0.0, 0.0], Direction::Forward, &[], &[]);
        runtime.advance_to(1.5);
        editor.timeline.snapshot_cache_step_seconds = 0.5;
        editor.timeline.snapshot_cache = vec![
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [0.0, 0.0, 0.0],
                direction: Direction::Forward,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [0.5, 0.0, 0.0],
                direction: Direction::Forward,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [1.0, 0.0, 0.0],
                direction: Direction::Forward,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
            crate::state::editor_timeline::EditorTimelineSnapshot {
                position: [1.5, 0.0, 0.0],
                direction: Direction::Right,
                is_grounded: true,
                game_over: false,
                trail_segment_count: 0,
                trail_point_count: 0,
            },
        ];
        editor.timeline.snapshot_cache_revision = 123;
        editor.timeline.scrub_runtime = Some(runtime);
        editor.timeline.scrub_runtime_revision = 55;

        editor.invalidate_samples_from(1.0);
        assert_eq!(editor.timeline.snapshot_cache.len(), 2);
        assert_eq!(editor.timeline.snapshot_cache_revision, 0);
        assert!(editor.timeline.scrub_runtime.is_none());
        assert_eq!(editor.timeline.scrub_runtime_revision, 0);

        editor.invalidate_samples();
        assert!(editor.timeline.snapshot_cache.is_empty());
        assert_eq!(editor.timeline.snapshot_cache_revision, 0);
        assert!(editor.timeline.scrub_runtime.is_none());
        assert_eq!(editor.timeline.scrub_runtime_revision, 0);
    }

    #[test]
    fn bpm_tap_estimates_tempo_and_reset_clears_state() {
        let mut editor = EditorSubsystem::default();

        editor.bpm_tap(1.0);
        assert!(editor.bpm_tap_result().is_none());

        editor.bpm_tap(1.5);
        editor.bpm_tap(2.0);

        let bpm = editor.bpm_tap_result().expect("bpm should exist");
        approx::assert_abs_diff_eq!(bpm, 120.0, epsilon = 1e-3);

        editor.bpm_tap_reset();
        assert!(editor.bpm_tap_result().is_none());
        assert!(editor.timing.bpm_tap_times.is_empty());
    }
}
