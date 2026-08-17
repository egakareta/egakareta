/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::add_tap_with_indicator;

/// Whether a configured snap mode is active for the current input state.
///
/// Holding Ctrl temporarily disables both position and rotation snapping.
pub(crate) fn effective_snap_enabled(configured: bool, ctrl_held: bool) -> bool {
    configured && !ctrl_held
}

super::editor_context_methods! {
    #[cfg(test)]
    pub fn set_pan_up_held(&mut self, held: bool) {
        self.input.pan_up_held = held;
    }

    #[cfg(test)]
    pub fn set_pan_down_held(&mut self, held: bool) {
        self.input.pan_down_held = held;
    }

    #[cfg(test)]
    pub fn set_pan_left_held(&mut self, held: bool) {
        self.input.pan_left_held = held;
    }

    #[cfg(test)]
    pub fn set_pan_right_held(&mut self, held: bool) {
        self.input.pan_right_held = held;
    }

    #[cfg(test)]
    pub fn set_shift_held(&mut self, held: bool) {
        self.input.shift_held = held;
    }

    #[cfg(test)]
    pub fn set_ctrl_held(&mut self, held: bool) {
        self.input.ctrl_held = held;
    }

    #[cfg(test)]
    pub fn set_alt_held(&mut self, held: bool) {
        self.input.alt_held = held;
    }

    #[cfg(test)]
    pub fn set_right_dragging(&mut self, dragging: bool) {
        self.input.right_dragging = dragging;
    }

    #[cfg(test)]
    pub fn set_left_mouse_down(&mut self, pressed: bool) {
        self.input.left_mouse_down = pressed;
    }

    #[cfg(test)]
    pub fn set_pointer_screen(&mut self, position: Option<[f64; 2]>) {
        self.input.pointer_screen = position;
    }

    #[cfg(test)]
    pub fn set_ui_input_blocking_rects(&mut self, rects: Vec<[f32; 4]>, pixels_per_point: f32) {
        self.input.ui_input_blocking_rects = rects;
        self.input.ui_input_pixels_per_point =
            if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
                pixels_per_point
            } else {
                1.0
            };
    }

    #[cfg(test)]
    pub fn clear_ui_input_blocking_rects(&mut self) {
        self.input.ui_input_blocking_rects.clear();
        self.input.ui_input_pixels_per_point = 1.0;
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn left_mouse_down(&self) -> bool {
        self.input.left_mouse_down
    }

    #[cfg(test)]
    pub fn has_gizmo_drag(&self) -> bool {
        self.runtime.interaction.gizmo_drag.is_some()
    }

    #[cfg(test)]
    pub fn has_block_drag(&self) -> bool {
        self.runtime.interaction.block_drag.is_some()
    }

    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.timeline.playback.playing
    }

    #[cfg(test)]
    pub fn timeline_time_seconds(&self) -> f32 {
        self.timeline.clock.time_seconds
    }

    #[cfg(test)]
    pub fn set_block_id(&mut self, block_id: BlockId) {
        self.config.selected_block_id = block_id;
        self.config
            .remember_recent_block_id(self.config.selected_block_id.clone());
    }

    #[cfg(test)]
    pub fn mode(&self) -> EditorMode {
        self.input.mode
    }

    #[cfg(test)]
    pub fn snap_to_grid(&self) -> bool {
        self.config.snap_to_grid
    }

    pub fn effective_snap_to_grid(&self) -> bool {
        effective_snap_enabled(self.config.snap_to_grid, self.input.ctrl_held)
    }

    #[cfg(test)]
    pub fn snap_step(&self) -> f32 {
        self.config.snap_step
    }

    #[cfg(test)]
    pub fn snap_rotation(&self) -> bool {
        self.config.snap_rotation
    }

    pub fn effective_snap_rotation(&self) -> bool {
        effective_snap_enabled(self.config.snap_rotation, self.input.ctrl_held)
    }

    #[cfg(test)]
    pub fn snap_rotation_step_degrees(&self) -> f32 {
        self.config.snap_rotation_step_degrees
    }

    #[cfg(test)]
    pub fn selected_block(&self) -> Option<LevelObject> {
        self.selected_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.objects.get(index).cloned())
    }

    #[cfg(test)]
    pub fn set_snap_to_grid(&mut self, snap: bool) {
        self.config.snap_to_grid = snap;
    }

    #[cfg(test)]
    pub fn set_snap_step(&mut self, step: f32) {
        self.config.snap_step = step.max(0.05);
    }

    #[cfg(test)]
    pub fn set_snap_rotation(&mut self, snap: bool) {
        self.config.snap_rotation = snap;
    }

    #[cfg(test)]
    pub fn set_snap_rotation_step_degrees(&mut self, step: f32) {
        self.config.snap_rotation_step_degrees = step.max(1.0);
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn set_selected_block_id(&mut self, block_id: BlockId) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].block_id = block_id;
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn rotate_selected_block_preview(&mut self) {
        self.config.selected_block_rotation_degrees[1] =
            (self.config.selected_block_rotation_degrees[1] + 90.0).rem_euclid(360.0);
    }

    #[cfg(test)]
    pub fn set_selected_block_color_tint(&mut self, color_tint: [f32; 3]) {
        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].color_tint = color_tint.map(|component| component.clamp(0.0, 1.0));
        }
    }

    #[cfg(test)]
    pub fn timeline_duration_seconds(&self) -> f32 {
        self.timeline.clock.duration_seconds
    }

    #[cfg(test)]
    pub fn tap_times(&self) -> &[f32] {
        &self.timeline.taps.tap_times
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn clear_taps(&mut self) {
        clear_taps_with_indicators(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
        );
        self.timeline.taps.selected_index = None;
    }

    #[cfg(test)]
    pub fn timeline_preview(&self) -> ([f32; 3], Direction) {
        (
            self.timeline.preview.position,
            self.timeline.preview.direction,
        )
    }

    #[cfg(test)]
    pub fn timing_points(&self) -> &[TimingPoint] {
        &self.timing.timing_points
    }

    #[cfg(test)]
    pub fn playback_speed(&self) -> f32 {
        self.timing.playback_speed
    }

    #[cfg(test)]
    pub fn set_playback_speed(&mut self, speed: f32) -> f32 {
        self.timing.playback_speed = speed.clamp(0.1, 2.0);
        self.timing.playback_speed
    }

    #[cfg(test)]
    pub fn timing_selected_index(&self) -> Option<usize> {
        self.timing.timing_selected_index
    }

    #[cfg(test)]
    pub fn set_timing_selected_index(&mut self, index: Option<usize>) {
        self.timing.timing_selected_index = index;
    }

    #[cfg(test)]
    pub fn waveform_zoom(&self) -> f32 {
        self.timing.waveform_zoom
    }

    #[cfg(test)]
    pub fn set_waveform_zoom(&mut self, zoom: f32) {
        self.timing.waveform_zoom = zoom.clamp(0.1, 10.0);
    }

    #[cfg(test)]
    pub fn waveform_scroll(&self) -> f32 {
        self.timing.waveform_scroll
    }

    #[cfg(test)]
    pub fn set_waveform_scroll(&mut self, scroll: f32) {
        self.timing.waveform_scroll = scroll;
    }

    #[cfg(test)]
    pub fn waveform_samples(&self) -> &[f32] {
        &self.timing.waveform_samples
    }

    #[cfg(test)]
    pub fn waveform_sample_rate(&self) -> u32 {
        self.timing.waveform_sample_rate
    }

    #[cfg(test)]
    pub fn waveform_window_size(&self) -> usize {
        self.timing.waveform_window_size
    }

    #[cfg(test)]
    pub fn waveform_loading(&self) -> bool {
        self.timing.waveform_loading
    }

    #[cfg(test)]
    pub fn waveform_complete(&self) -> bool {
        self.timing.waveform_complete
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn remove_timing_point(&mut self, index: usize) {
        if index < self.timing.timing_points.len() {
            self.timing.timing_points.remove(index);
            self.timing.mark_timing_points_changed();
        }
    }

    #[cfg(test)]
    pub fn update_timing_point_time(&mut self, index: usize, time: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.time_seconds = time.max(0.0);
            self.timing.mark_timing_points_changed();
        }
        self.timing
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    }

    #[cfg(test)]
    pub fn update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.bpm = bpm.max(1.0);
            self.timing.mark_timing_points_changed();
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
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
        true
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_blocks::BlockId;
    use eg_types::{
        Direction, EasingDirection, EditorMode, LevelObject, TRANSFORM_TRIGGER_BLOCK_ID,
        TimedTrigger, TriggerTarget,
    };

    use crate::{
        events::{block::UpdateSelectedBlock, selection::AddTransformTrigger},
        state::{
            EditorConfigRes, EditorInputRes, EditorTimelineRes, EditorTimelineState,
            EditorTimingState, collect_editor_objects_from_world, replace_editor_objects,
            test_support::TestApp,
        },
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

    fn transform_trigger_test_app(
        objects: Vec<LevelObject>,
        selected_indices: Vec<usize>,
        time_seconds: f32,
    ) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(eg_types::AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = selected_indices.first().copied();
            input.0.selected_block_indices = selected_indices;
            input.0.mode = EditorMode::Rotate;
        }
        {
            let mut timeline = app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.clock.time_seconds = time_seconds;
            timeline.0.clock.duration_seconds = 8.0;
        }
        app.world_mut().write_message(AddTransformTrigger);
        app.tick();
        app
    }

    #[test]
    fn add_transform_trigger_ignores_trigger_block_selection() {
        let mut app = transform_trigger_test_app(
            vec![transform_trigger_object(1.0, vec!["object_0"])],
            vec![0],
            1.0,
        );

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects.len(), 1);
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.trigger.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn add_transform_trigger_creates_selected_markers_for_selected_blocks() {
        let mut app = transform_trigger_test_app(
            vec![
                test_level_object([0.0, 0.0, 0.0]),
                test_level_object([4.0, 0.0, 0.0]),
            ],
            vec![0, 1],
            2.5,
        );
        let objects = collect_editor_objects_from_world(app.world_mut());

        assert_eq!(
            app.world().resource::<EditorInputRes>().0.mode,
            EditorMode::Rotate
        );
        assert_object_pose(&objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_object_pose(&objects[1], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(objects.len(), 4);
        assert_eq!(objects[2].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        assert_object_pose(&objects[2], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(objects[2].rotation, [0.0, 0.0, 0.0]);
        assert_eq!(objects[3].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
        assert_object_pose(&objects[3], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(objects[3].rotation, [0.0, 0.0, 0.0]);
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_index,
            Some(2)
        );
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![2, 3]
        );

        let mut triggers = objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .cloned()
            .collect::<Vec<_>>();
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
        let mut app = transform_trigger_test_app(
            vec![
                test_level_object([0.0, 0.0, 0.0]),
                transform_trigger_object(1.0, vec!["object_0"]),
                test_level_object([4.0, 0.0, 0.0]),
            ],
            vec![2],
            2.5,
        );
        let objects = collect_editor_objects_from_world(app.world_mut());

        assert_eq!(objects.len(), 4);
        assert_object_pose(&objects[2], [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_index,
            Some(3)
        );
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![3]
        );

        let mut found_retargeted_capture = false;
        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
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
        let mut objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
        ];
        objects.push(transform_trigger(2.5, vec!["object_0"], [8.0, 0.0, 0.0]));
        objects[2].position = [12.0, 1.0, 3.0];
        objects[2].size = [2.0, 3.0, 4.0];
        objects[2].rotation = [10.0, 20.0, 30.0];
        let mut app = transform_trigger_test_app(objects, vec![1], 2.5);
        let objects = collect_editor_objects_from_world(app.world_mut());

        assert_object_pose(&objects[2], [12.0, 1.0, 3.0], [2.0, 3.0, 4.0]);
        assert_eq!(objects[2].rotation, [10.0, 20.0, 30.0]);
        let triggers = objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .collect::<Vec<_>>();
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
        let mut objects = vec![
            test_level_object([0.0, 0.0, 0.0]),
            test_level_object([4.0, 0.0, 0.0]),
            test_level_object([8.0, 0.0, 0.0]),
        ];
        objects.extend(vec![
            transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_4", "object_8"], [20.0, 0.0, 0.0]),
            transform_trigger(2.5, vec!["object_8"], [30.0, 0.0, 0.0]),
            transform_trigger(4.0, vec!["object_0"], [40.0, 0.0, 0.0]),
        ]);
        let mut app = transform_trigger_test_app(objects, vec![0, 1], 2.5);
        let objects = collect_editor_objects_from_world(app.world_mut());

        let mut object_zero_same_time_count = 0;
        let mut object_one_same_time_count = 0;
        let mut retained_object_two_multi_target = false;
        let mut retained_object_two_standalone = false;
        let mut retained_different_time_object_zero = false;

        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
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
        let mut objects = vec![test_level_object([0.0, 0.0, 0.0])];
        objects.push(transform_trigger(2.5, vec!["object_0"], [10.0, 0.0, 0.0]));
        let mut app = transform_trigger_test_app(objects, vec![0], 2.5);
        let objects = collect_editor_objects_from_world(app.world_mut());

        let mut found_target_only_trigger = false;
        let mut found_replacement_trigger = false;

        for trigger_object in objects.iter().filter(|object| object.trigger.is_some()) {
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
        let mut app =
            transform_trigger_test_app(vec![test_level_object([0.0, 0.0, 0.0])], vec![0], 0.0);
        let objects = collect_editor_objects_from_world(app.world_mut());

        assert_object_pose(&objects[0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.trigger.is_some())
                .count(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![1]
        );
        assert_eq!(objects[1].block_id, TRANSFORM_TRIGGER_BLOCK_ID);
    }

    #[test]
    fn selected_block_mutators_apply_snap_and_clamp_rules() {
        let mut app = transform_trigger_test_app(vec![LevelObject::default()], vec![0], 0.0);
        {
            let mut config = app.world_mut().resource_mut::<EditorConfigRes>();
            config.0.snap_to_grid = true;
            config.0.snap_step = 0.5;
            config.0.snap_rotation = true;
            config.0.snap_rotation_step_degrees = 15.0;
        }

        app.world_mut()
            .write_message(UpdateSelectedBlock(LevelObject {
                position: [1.24, -1.0, 2.26],
                size: [0.1, 0.4, 0.62],
                rotation: [7.0, 22.0, 44.0],
                color_tint: [-1.0, 0.4, 2.0],
                block_id: BlockId::new("stone"),
                ..LevelObject::default()
            }));
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert_eq!(objects[0].position, [1.0, 0.0, 2.5]);
        assert_eq!(
            app.world().resource::<EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.5]
        );
        assert_eq!(objects[0].size, [0.5, 0.5, 0.5]);
        assert_eq!(objects[0].rotation, [0.0, 15.0, 45.0]);
        assert_eq!(objects[0].color_tint, [0.0, 0.4, 1.0]);
        assert_eq!(objects[0].block_id, "core/stone");
    }

    #[test]
    fn timeline_cache_invalidation_helpers_work() {
        let mut timeline = EditorTimelineState::default();

        let mut runtime =
            eg_game::TimelineSimulationRuntime::new([0.0, 0.0, 0.0], Direction::Forward, &[], &[]);
        runtime.advance_to(1.5);
        timeline.snapshot_cache_step_seconds = 0.5;
        timeline.snapshot_cache = vec![
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
        timeline.snapshot_cache_revision = 123;
        timeline.scrub_runtime = Some(runtime);
        timeline.scrub_runtime_revision = 55;

        timeline.invalidate_samples_from(1.0);
        assert_eq!(timeline.snapshot_cache.len(), 2);
        assert_eq!(timeline.snapshot_cache_revision, 0);
        assert!(timeline.scrub_runtime.is_none());
        assert_eq!(timeline.scrub_runtime_revision, 0);

        timeline.invalidate_samples();
        assert!(timeline.snapshot_cache.is_empty());
        assert_eq!(timeline.snapshot_cache_revision, 0);
        assert!(timeline.scrub_runtime.is_none());
        assert_eq!(timeline.scrub_runtime_revision, 0);
    }

    #[test]
    fn bpm_tap_estimates_tempo_and_reset_clears_state() {
        let mut timing = EditorTimingState::default();

        timing.record_bpm_tap_at(1.0);
        assert!(timing.bpm_tap_result.is_none());

        timing.record_bpm_tap_at(1.5);
        timing.record_bpm_tap_at(2.0);

        let bpm = timing.bpm_tap_result.expect("bpm should exist");
        approx::assert_abs_diff_eq!(bpm, 120.0, epsilon = 1e-3);

        timing.reset_bpm_tap();
        assert!(timing.bpm_tap_result.is_none());
        assert!(timing.bpm_tap_times.is_empty());
    }
}
