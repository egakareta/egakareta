use super::{EditorDirtyFlags, EditorSubsystem, PerfStage, State};
use crate::editor_domain::{
    add_tap_with_indicator, clear_taps_with_indicators, remove_tap_with_indicator,
};
use crate::game::TimelineSimulationRuntime;
use crate::platform::state_host::PlatformInstant;
use crate::types::{
    AppPhase, CameraKeypoint, EditorMode, LevelObject, SpawnDirection, TimingPoint,
};

impl EditorSubsystem {
    pub(crate) fn perf_record(&mut self, stage: PerfStage, started_at: PlatformInstant) {
        let elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
        self.perf.profiler.observe(stage, elapsed_ms);
    }

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

    pub(crate) fn timeline_time_seconds(&self) -> f32 {
        self.timeline.clock.time_seconds
    }

    pub(crate) fn set_block_id(&mut self, block_id: String) {
        self.config.selected_block_id = crate::block_repository::normalize_block_id(&block_id);
    }

    pub(crate) fn set_mode(&mut self, mode: EditorMode) {
        self.ui.mode = mode;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
        self.ui.marquee_start_screen = None;
        self.ui.marquee_current_screen = None;
        if !mode.is_selection_mode() {
            self.ui.selected_block_index = None;
            self.ui.selected_block_indices.clear();
            self.ui.hovered_block_index = None;
        }
    }

    pub(crate) fn mode(&self) -> EditorMode {
        self.ui.mode
    }

    pub(crate) fn snap_to_grid(&self) -> bool {
        self.config.snap_to_grid
    }

    pub(crate) fn snap_step(&self) -> f32 {
        self.config.snap_step
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

    pub(crate) fn set_selected_block_position(&mut self, position: [f32; 3]) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            let snap_step = self.config.snap_step.max(0.05);
            let next_position = if self.config.snap_to_grid {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1] / snap_step).round() * snap_step,
                    (position[2].max(0.0) / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.objects[index].position = next_position;
            self.ui.cursor = [
                next_position[0],
                next_position[1],
                next_position[2].max(0.0),
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
            let snapped_size = if self.config.snap_to_grid {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.config.snap_to_grid {
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

    pub(crate) fn set_selected_block_rotation(&mut self, rotation_degrees: f32) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].rotation_degrees = rotation_degrees;
        }
    }

    pub(crate) fn set_selected_block_roundness(&mut self, roundness: f32) {
        if let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        {
            self.objects[index].roundness = roundness.max(0.0);
        }
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
        let step = if self.config.snap_to_grid {
            self.config.snap_step.max(0.05)
        } else {
            1.0
        };
        [
            ((position[0] - 0.5) / step).round() * step,
            ((position[1] - 0.5) / step).round() * step,
            (position[2] / step).round() * step,
        ]
    }

    pub(crate) fn add_tap(&mut self, indicator_position: [f32; 3]) -> f32 {
        add_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
            indicator_position,
        );
        self.timeline.clock.time_seconds
    }

    pub(crate) fn remove_tap(&mut self) -> f32 {
        remove_tap_with_indicator(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.time_seconds,
        );
        self.timeline.clock.time_seconds
    }

    pub(crate) fn clear_taps(&mut self) {
        clear_taps_with_indicators(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
        );
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
            .sort_by(|a, b| a.time_seconds.partial_cmp(&b.time_seconds).unwrap());
    }

    pub(crate) fn remove_timing_point(&mut self, index: usize) {
        if index < self.timing.timing_points.len() {
            self.timing.timing_points.remove(index);
        }
    }

    pub(crate) fn update_timing_point_time(&mut self, index: usize, time: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.time_seconds = time.max(0.0);
        }
        self.timing
            .timing_points
            .sort_by(|a, b| a.time_seconds.partial_cmp(&b.time_seconds).unwrap());
    }

    pub(crate) fn update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        if let Some(tp) = self.timing.timing_points.get_mut(index) {
            tp.bpm = bpm.max(1.0);
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
        if self.phase == AppPhase::Editor {
            self.editor.set_shift_held(held);
        }
    }

    pub(crate) fn set_editor_ctrl_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_ctrl_held(held);
        }
    }

    pub(crate) fn set_editor_alt_held(&mut self, held: bool) {
        if self.phase == AppPhase::Editor {
            self.editor.set_alt_held(held);
        }
    }

    pub(crate) fn set_editor_block_id(&mut self, block_id: String) {
        self.editor.set_block_id(block_id);
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor.set_mode(mode);
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor.mode()
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor.snap_to_grid()
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor.snap_step()
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor.set_snap_to_grid(snap);
        if self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor.set_snap_step(step);
        if self.editor.config.snap_to_grid && self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
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

    pub(crate) fn set_editor_selected_block_rotation(&mut self, rotation_degrees: f32) {
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

    pub(crate) fn set_editor_selected_block_roundness(&mut self, roundness: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        self.editor.set_selected_block_roundness(roundness);

        if self.editor.ui.selected_block_index.is_some() {
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
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
        let changed = self.editor.set_timeline_time_seconds(time_seconds);
        if self.phase == AppPhase::Editor {
            self.apply_editor_timeline_preview_from_cache();
        }
        if changed {
            self.resync_editor_timeline_playback_audio();
        }
    }

    fn apply_editor_timeline_preview_from_cache(&mut self) {
        if self.phase != AppPhase::Editor || self.editor.timeline.playback.playing {
            return;
        }

        let solve_started_at = PlatformInstant::now();
        self.rebuild_editor_timeline_snapshot_cache_if_needed();

        let duration_seconds = self.editor.timeline.clock.duration_seconds;
        let step_seconds = self
            .editor
            .timeline
            .snapshot_cache_step_seconds
            .max(1.0 / 480.0);
        let cache_len = self.editor.timeline.snapshot_cache.len();
        if cache_len == 0 {
            self.perf_record(PerfStage::PreviewSolveTimeline, solve_started_at);
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

        let position = [
            lower.position[0] + (upper.position[0] - lower.position[0]) * alpha,
            lower.position[1] + (upper.position[1] - lower.position[1]) * alpha,
            lower.position[2] + (upper.position[2] - lower.position[2]) * alpha,
        ];
        let direction = if alpha < 0.5 {
            lower.direction
        } else {
            upper.direction
        };

        self.apply_editor_timeline_preview_state(position, direction);
        self.perf_record(PerfStage::PreviewSolveTimeline, solve_started_at);
    }

    fn rebuild_editor_timeline_snapshot_cache_if_needed(&mut self) {
        if self.editor.timeline.snapshot_cache_revision == self.editor.timeline.simulation_revision
            && !self.editor.timeline.snapshot_cache.is_empty()
        {
            return;
        }

        let rebuild_started_at = PlatformInstant::now();
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

        let mut runtime = TimelineSimulationRuntime::new(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.objects,
            &self.editor.timeline.taps.tap_times,
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

        self.perf_record(PerfStage::TimelineSampleRebuild, rebuild_started_at);
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
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    /// Removes the tap event at the current timeline position, if one exists.
    ///
    /// This operation is recorded in history and invalidates simulation samples from the removed tap's time.
    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor.remove_tap();
        self.editor.invalidate_samples_from(tap_time);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    /// Clears all tap events from the editor's timeline.
    ///
    /// This operation is recorded in history and invalidates all simulation samples.
    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        self.editor.clear_taps();
        self.editor.invalidate_samples();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_add_camera_keypoint(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        self.editor.add_camera_keypoint();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_remove_camera_keypoint(&mut self, index: usize) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        self.editor.remove_camera_keypoint(index);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn set_editor_camera_keypoint_selected(&mut self, selected: Option<usize>) {
        if self.phase == AppPhase::Editor {
            self.editor.set_camera_keypoint_selected(selected);
            self.mark_editor_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
        }
    }

    pub(crate) fn editor_update_camera_keypoint(&mut self, index: usize, keypoint: CameraKeypoint) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        self.editor.update_camera_keypoint(index, keypoint);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_capture_selected_camera_keypoint(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();
        self.editor.capture_selected_camera_keypoint();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_apply_selected_camera_keypoint(&mut self) {
        if self.phase == AppPhase::Editor {
            self.editor
                .apply_selected_camera_keypoint_to_editor_camera();
        }
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor.timeline_preview()
    }

    pub(crate) fn editor_timing_points(&self) -> &[TimingPoint] {
        self.editor.timing_points()
    }

    pub(crate) fn editor_camera_keypoints(&self) -> &[CameraKeypoint] {
        self.editor.camera_keypoints()
    }

    pub(crate) fn editor_selected_camera_keypoint_index(&self) -> Option<usize> {
        self.editor.selected_camera_keypoint_index()
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
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
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
