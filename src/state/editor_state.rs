use glam::{Mat4, Vec2, Vec3, Vec4};

use super::{
    EditorDirtyFlags, EditorPickResult, EditorSubsystem, GizmoAxis, GizmoDragKind, PerfStage, State,
};
use crate::editor_domain::{
    add_tap_with_indicator, clear_taps_with_indicators, remove_tap_with_indicator,
    retain_taps_up_to_duration_with_indicators,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::{AppPhase, EditorMode, LevelObject, SpawnDirection, TimingPoint};

impl EditorSubsystem {
    pub(crate) fn perf_record(&mut self, stage: PerfStage, started_at: PlatformInstant) {
        let elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
        self.perf.profiler.observe(stage, elapsed_ms);
    }

    pub(crate) fn camera_axes_xy(&self) -> (Vec2, Vec2) {
        let right = Vec2::new(
            self.camera.editor_rotation.cos(),
            self.camera.editor_rotation.sin(),
        );
        let up = Vec2::new(
            -self.camera.editor_rotation.sin(),
            self.camera.editor_rotation.cos(),
        );
        (right, up)
    }

    pub(crate) fn camera_offset(&self) -> Vec3 {
        let zoom = self.camera.editor_zoom.clamp(0.35, 4.0);
        let distance = 24.0 / zoom;
        let pitch = self
            .camera
            .editor_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(self.camera.editor_rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
    }

    pub(crate) fn view_proj(&self, viewport: Vec2) -> Mat4 {
        let aspect = viewport.x / viewport.y;
        let target = Vec3::new(self.camera.editor_pan[0], self.camera.editor_pan[1], 0.0);
        let eye = target + self.camera_offset();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        proj * view
    }

    pub(crate) fn world_to_screen_v(&self, world: Vec3, viewport: Vec2) -> Option<Vec2> {
        let view_proj = self.view_proj(viewport);
        let clip = view_proj * world.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }

        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            return None;
        }

        let screen_x = (ndc.x + 1.0) * 0.5 * viewport.x;
        let screen_y = (1.0 - ndc.y) * 0.5 * viewport.y;
        Some(Vec2::new(screen_x, screen_y))
    }

    pub(crate) fn drag_selection(&mut self, x: f64, y: f64, viewport: Vec2) -> bool {
        self.ui.pointer_screen = Some([x, y]);

        let Some(drag) = self.runtime.interaction.block_drag.clone() else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let (camera_right_xy, camera_up_xy) = self.camera_axes_xy();
        let center = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );

        let Some(origin_screen) = self.world_to_screen_v(center, viewport) else {
            return true;
        };

        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let right_world = Vec3::new(camera_right_xy.x, camera_right_xy.y, 0.0);
        let up_world = Vec3::new(camera_up_xy.x, camera_up_xy.y, 0.0);

        let Some(right_screen) = self.world_to_screen_v(center + right_world, viewport) else {
            return true;
        };
        let Some(up_screen) = self.world_to_screen_v(center + up_world, viewport) else {
            return true;
        };

        let right_screen_delta = right_screen - origin_screen;
        let up_screen_delta = up_screen - origin_screen;

        let det =
            right_screen_delta.x * up_screen_delta.y - right_screen_delta.y * up_screen_delta.x;
        if det.abs() <= f32::EPSILON {
            return true;
        }

        let inv_det = 1.0 / det;
        let world_right_factor = (effective_mouse_delta.x * up_screen_delta.y
            - effective_mouse_delta.y * up_screen_delta.x)
            * inv_det;
        let world_up_factor = (right_screen_delta.x * effective_mouse_delta.y
            - right_screen_delta.y * effective_mouse_delta.x)
            * inv_det;

        let snap_enabled = self.config.snap_to_grid;
        let snap_step = self.config.snap_step.max(0.05);

        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.objects.get_mut(block.index) {
                let mut next = [
                    block.position[0]
                        + world_right_factor * camera_right_xy.x
                        + world_up_factor * camera_up_xy.x,
                    block.position[1]
                        + world_right_factor * camera_right_xy.y
                        + world_up_factor * camera_up_xy.y,
                    block.position[2],
                ];

                if snap_enabled {
                    next[0] = (next[0] / snap_step).round() * snap_step;
                    next[1] = (next[1] / snap_step).round() * snap_step;
                    next[2] = (next[2].max(0.0) / snap_step).round() * snap_step;
                } else {
                    next[2] = next[2].max(0.0);
                }

                obj.position = next;
                if first_cursor.is_none() {
                    first_cursor = Some(next);
                }
            }
        }

        if let Some(next_position) = first_cursor {
            let bounds = self.ui.bounds as f32;
            self.ui.cursor = [
                next_position[0].clamp(-bounds, bounds),
                next_position[1].clamp(-bounds, bounds),
                next_position[2].max(0.0),
            ];
        }

        true
    }

    pub(crate) fn drag_gizmo(&mut self, x: f64, y: f64, viewport: Vec2) -> bool {
        self.ui.pointer_screen = Some([x, y]);

        let Some(drag) = self.runtime.interaction.gizmo_drag.clone() else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let center = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );
        let axis_dir = match drag.axis {
            GizmoAxis::X | GizmoAxis::XNeg => Vec3::X,
            GizmoAxis::Y | GizmoAxis::YNeg => Vec3::Y,
            GizmoAxis::Z | GizmoAxis::ZNeg => Vec3::Z,
        };

        let Some(origin_screen) = self.world_to_screen_v(center, viewport) else {
            self.runtime.interaction.gizmo_drag = Some(drag);
            return true;
        };
        let Some(axis_screen) = self.world_to_screen_v(center + axis_dir, viewport) else {
            self.runtime.interaction.gizmo_drag = Some(drag);
            return true;
        };

        let axis_screen_delta = axis_screen - origin_screen;
        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let axis_screen_dir = axis_screen_delta.normalize();
        let projected_pixels = effective_mouse_delta.dot(axis_screen_dir);
        let pixels_per_world_unit = axis_screen_delta.length();
        if pixels_per_world_unit <= f32::EPSILON {
            return true;
        }
        let world_delta = projected_pixels / pixels_per_world_unit;

        match drag.kind {
            GizmoDragKind::Move => {
                let snap_enabled = self.config.snap_to_grid;
                let snap_step = self.config.snap_step.max(0.05);
                let mut first_cursor: Option<[f32; 3]> = None;
                for block in &drag.start_blocks {
                    if let Some(obj) = self.objects.get_mut(block.index) {
                        let mut next = block.position;
                        match drag.axis {
                            GizmoAxis::X | GizmoAxis::XNeg => next[0] += world_delta,
                            GizmoAxis::Y | GizmoAxis::YNeg => next[1] += world_delta,
                            GizmoAxis::Z | GizmoAxis::ZNeg => next[2] += world_delta,
                        }
                        if snap_enabled {
                            next[0] = (next[0] / snap_step).round() * snap_step;
                            next[1] = (next[1] / snap_step).round() * snap_step;
                            next[2] = (next[2].max(0.0) / snap_step).round() * snap_step;
                        } else {
                            next[2] = next[2].max(0.0);
                        }
                        obj.position = next;
                        if first_cursor.is_none() {
                            first_cursor = Some(next);
                        }
                    }
                }
                if let Some(next_position) = first_cursor {
                    let bounds = self.ui.bounds as f32;
                    self.ui.cursor = [
                        next_position[0].clamp(-bounds, bounds),
                        next_position[1].clamp(-bounds, bounds),
                        next_position[2].max(0.0),
                    ];
                }
            }
            GizmoDragKind::Resize => {
                let snap_enabled = self.config.snap_to_grid;
                let snap_step = self.config.snap_step.max(0.05);
                let min_size = if snap_enabled { snap_step } else { 0.25 };
                for block in &drag.start_blocks {
                    if let Some(obj) = self.objects.get_mut(block.index) {
                        match drag.axis {
                            GizmoAxis::X => {
                                let mut s = block.size[0] + world_delta;
                                if snap_enabled {
                                    s = (s / snap_step).round() * snap_step;
                                }
                                obj.size[0] = s.max(min_size);
                            }
                            GizmoAxis::Y => {
                                let mut s = block.size[1] + world_delta;
                                if snap_enabled {
                                    s = (s / snap_step).round() * snap_step;
                                }
                                obj.size[1] = s.max(min_size);
                            }
                            GizmoAxis::Z => {
                                let mut s = block.size[2] + world_delta;
                                if snap_enabled {
                                    s = (s / snap_step).round() * snap_step;
                                }
                                obj.size[2] = s.max(min_size);
                            }
                            GizmoAxis::XNeg => {
                                let mut s = block.size[0] - world_delta;
                                let mut p = block.position[0] + world_delta;
                                let right_edge = block.position[0] + block.size[0];
                                if snap_enabled {
                                    p = (p / snap_step).round() * snap_step;
                                    s = (right_edge - p).max(min_size);
                                    p = right_edge - s;
                                } else {
                                    s = s.max(min_size);
                                    p = right_edge - s;
                                }
                                obj.position[0] = p;
                                obj.size[0] = s;
                            }
                            GizmoAxis::YNeg => {
                                let mut s = block.size[1] - world_delta;
                                let mut p = block.position[1] + world_delta;
                                let top_edge = block.position[1] + block.size[1];
                                if snap_enabled {
                                    p = (p / snap_step).round() * snap_step;
                                    s = (top_edge - p).max(min_size);
                                    p = top_edge - s;
                                } else {
                                    s = s.max(min_size);
                                    p = top_edge - s;
                                }
                                obj.position[1] = p;
                                obj.size[1] = s;
                            }
                            GizmoAxis::ZNeg => {
                                let mut p = block.position[2] + world_delta;
                                let upper_edge = block.position[2] + block.size[2];
                                if snap_enabled {
                                    p = (p / snap_step).round() * snap_step;
                                }
                                p = p.max(0.0);
                                let s = (upper_edge - p).max(min_size);
                                p = upper_edge - s;
                                obj.position[2] = p;
                                obj.size[2] = s;
                            }
                        }
                    }
                }
            }
        }
        true
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

    pub(crate) fn set_block_id(&mut self, block_id: String) {
        self.config.selected_block_id = crate::block_repository::normalize_block_id(&block_id);
    }

    pub(crate) fn set_mode(&mut self, mode: EditorMode) {
        self.ui.mode = mode;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;
        if mode == EditorMode::Place || mode == EditorMode::Timing {
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
            let bounds = self.ui.bounds;
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
                next_position[0].clamp(-bounds as f32, bounds as f32),
                next_position[1].clamp(-bounds as f32, bounds as f32),
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

    pub(crate) fn selected_block_id(&self) -> &str {
        self.config.selected_block_id.as_str()
    }

    pub(crate) fn timeline_time_seconds(&self) -> f32 {
        self.timeline.clock.time_seconds
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

        retain_taps_up_to_duration_with_indicators(
            &mut self.timeline.taps.tap_times,
            &mut self.timeline.taps.tap_indicator_positions,
            self.timeline.clock.duration_seconds,
        );
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
        self.timeline.cache.dirty = true;
        self.timeline.cache.rebuild_from_seconds = None;
    }

    pub(crate) fn invalidate_samples_from(&mut self, from_seconds: f32) {
        self.timeline.cache.dirty = true;
        let clamped = from_seconds.max(0.0);
        self.timeline.cache.rebuild_from_seconds = Some(
            self.timeline
                .cache
                .rebuild_from_seconds
                .map_or(clamped, |existing| existing.min(clamped)),
        );
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

    pub fn editor_selected_block_id(&self) -> &str {
        self.editor.selected_block_id()
    }

    pub fn editor_timeline_time_seconds(&self) -> f32 {
        self.editor.timeline_time_seconds()
    }

    pub fn editor_timeline_duration_seconds(&self) -> f32 {
        self.editor.timeline_duration_seconds()
    }

    pub fn editor_tap_times(&self) -> &[f32] {
        self.editor.tap_times()
    }

    pub fn editor_fps(&self) -> f32 {
        self.editor.fps()
    }

    pub fn set_editor_timeline_time_seconds(&mut self, time_seconds: f32) {
        if self.editor.set_timeline_time_seconds(time_seconds) {
            self.refresh_editor_timeline_position();
            self.resync_editor_timeline_playback_audio();
        }
    }

    pub fn set_editor_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.record_editor_history_state();
        self.editor.set_timeline_duration_seconds(duration_seconds);
        self.editor.invalidate_samples();
        self.refresh_editor_timeline_position();
        self.resync_editor_timeline_playback_audio();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        let indicator_position = self
            .editor
            .tap_indicator_position_from_world(self.editor.timeline.preview.position);
        let tap_time = self.editor.add_tap(indicator_position);
        self.editor.invalidate_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor.remove_tap();
        self.editor.invalidate_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        self.editor.clear_taps();
        self.editor.invalidate_samples();
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor.timeline_preview()
    }

    pub(crate) fn editor_timing_points(&self) -> &[TimingPoint] {
        self.editor.timing_points()
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

    pub(crate) fn load_waveform_for_current_audio(&mut self) {
        const WAVEFORM_WINDOW: usize = 256;

        let music_source = self.session.editor_music_metadata.source.clone();

        if let Some((samples, sample_rate)) =
            self.audio.state.editor.waveform_cache.get(&music_source)
        {
            self.editor.timing.waveform_samples = samples.clone();
            self.editor.timing.waveform_sample_rate = *sample_rate;
            self.audio.state.editor.waveform_loading_source = None;
            return;
        }

        if self.audio.state.editor.waveform_loading_source.as_deref() == Some(music_source.as_str())
        {
            return;
        }

        self.audio.state.editor.waveform_loading_source = Some(music_source.clone());
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;

        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::platform::audio::decode_audio_to_waveform;
            let source_for_thread = music_source.clone();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let cached_bytes = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&music_source)
                .cloned();
            let sender = self.audio.state.editor.waveform_load_channel.0.clone();

            std::thread::spawn(move || {
                let bytes = cached_bytes.or_else(|| {
                    let audio_path = format!("assets/levels/{}/{}", level_name, source_for_thread);
                    std::fs::read(&audio_path).ok()
                });

                let decoded = if let Some(bytes) = bytes {
                    decode_audio_to_waveform(bytes, WAVEFORM_WINDOW)
                } else {
                    None
                };

                let _ = sender.send((source_for_thread, decoded));
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast as _;
            use wasm_bindgen_futures::{spawn_local, JsFuture};

            let source_for_fetch = music_source.clone();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let cached_bytes = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&music_source)
                .cloned();
            let sender = self.audio.state.editor.waveform_load_channel.0.clone();

            spawn_local(async move {
                let bytes = if let Some(bytes) = cached_bytes {
                    Some(bytes)
                } else {
                    let audio_path = format!("assets/levels/{}/{}", level_name, source_for_fetch);
                    let fetched = async {
                        let window = web_sys::window()?;
                        let response_value = JsFuture::from(window.fetch_with_str(&audio_path))
                            .await
                            .ok()?;
                        let response: web_sys::Response = response_value.dyn_into().ok()?;
                        if !response.ok() {
                            return None;
                        }
                        let array_buffer =
                            JsFuture::from(response.array_buffer().ok()?).await.ok()?;
                        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                        Some(uint8_array.to_vec())
                    }
                    .await;

                    fetched
                };

                let decoded = if let Some(bytes) = bytes {
                    crate::platform::audio::decode_audio_to_waveform_async(&bytes, WAVEFORM_WINDOW)
                        .await
                } else {
                    None
                };

                let _ = sender.send((music_source, decoded));
            });
        }
    }
}

impl EditorSubsystem {
    pub(crate) fn pick_from_screen(
        &self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> Option<EditorPickResult> {
        if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
            return None;
        }

        let view_proj = self.view_proj(viewport_size);
        let inv_view_proj = view_proj.inverse();

        let ndc_x = (2.0 * x as f32 / viewport_size.x) - 1.0;
        let ndc_y = 1.0 - (2.0 * y as f32 / viewport_size.y);

        let near_clip = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let far_clip = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let mut near_world = inv_view_proj * near_clip;
        let mut far_world = inv_view_proj * far_clip;

        if near_world.w.abs() <= f32::EPSILON || far_world.w.abs() <= f32::EPSILON {
            return None;
        }

        near_world /= near_world.w;
        far_world /= far_world.w;

        let ray_origin = near_world.truncate();
        let ray_dir = (far_world.truncate() - ray_origin).normalize();

        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Z;
        let mut hit_found = false;
        let mut hit_block_index: Option<usize> = None;

        if ray_dir.z.abs() > f32::EPSILON {
            let t = -ray_origin.z / ray_dir.z;
            if t >= 0.0 {
                min_t = t;
                hit_found = true;
            }
        }

        for (index, obj) in self.objects.iter().enumerate() {
            if let Some((t, normal)) = self.ray_intersect_rotated_block(ray_origin, ray_dir, obj) {
                if t < min_t {
                    min_t = t;
                    hit_found = true;
                    hit_block_index = Some(index);
                    best_hit_normal = normal;
                }
            }
        }

        if !hit_found {
            return None;
        }

        let hit = ray_origin + ray_dir * min_t;
        let target = hit + best_hit_normal * 0.01;

        let snap_enabled = self.config.snap_to_grid;
        let snap_step = self.config.snap_step.max(0.05);

        let next_cursor = if snap_enabled {
            [
                (target.x / snap_step).floor() * snap_step,
                (target.y / snap_step).floor() * snap_step,
                (target.z / snap_step).floor() * snap_step,
            ]
        } else {
            [target.x.floor(), target.y.floor(), target.z.floor()]
        };

        let bounds = self.ui.bounds as f32;
        let next_cursor = [
            next_cursor[0].clamp(-bounds, bounds),
            next_cursor[1].clamp(-bounds, bounds),
            next_cursor[2].max(0.0),
        ];

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
        })
    }

    pub(crate) fn ray_intersect_rotated_block(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &crate::types::LevelObject,
    ) -> Option<(f32, Vec3)> {
        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let inv_angle = -obj.rotation_degrees.to_radians();

        let local_origin_xy = self.rotate_vec2(
            Vec2::new(ray_origin.x - center.x, ray_origin.y - center.y),
            inv_angle,
        );
        let local_dir_xy = self.rotate_vec2(Vec2::new(ray_dir.x, ray_dir.y), inv_angle);
        let local_origin = Vec3::new(
            local_origin_xy.x,
            local_origin_xy.y,
            ray_origin.z - center.z,
        );
        let local_dir = Vec3::new(local_dir_xy.x, local_dir_xy.y, ray_dir.z);

        let min = -half;
        let max = half;
        let mut t_min = f32::NEG_INFINITY;
        let mut t_max = f32::INFINITY;
        let mut normal_enter = Vec3::ZERO;
        let mut normal_exit = Vec3::ZERO;

        for axis in 0..3 {
            let origin_component = local_origin[axis];
            let dir_component = local_dir[axis];
            let min_component = min[axis];
            let max_component = max[axis];

            if dir_component.abs() <= f32::EPSILON {
                if origin_component < min_component || origin_component > max_component {
                    return None;
                }
                continue;
            }

            let mut t1 = (min_component - origin_component) / dir_component;
            let mut t2 = (max_component - origin_component) / dir_component;

            let axis_dir = match axis {
                0 => Vec3::X,
                1 => Vec3::Y,
                _ => Vec3::Z,
            };

            let mut n1 = -axis_dir;
            let mut n2 = axis_dir;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
                std::mem::swap(&mut n1, &mut n2);
            }

            if t1 > t_min {
                t_min = t1;
                normal_enter = n1;
            }
            if t2 < t_max {
                t_max = t2;
                normal_exit = n2;
            }

            if t_min > t_max {
                return None;
            }
        }

        if t_max < 0.0 {
            return None;
        }

        let (t_hit, normal_local) = if t_min >= 0.0 {
            (t_min, normal_enter)
        } else {
            (t_max, normal_exit)
        };

        let angle = obj.rotation_degrees.to_radians();
        let normal_xy = self.rotate_vec2(Vec2::new(normal_local.x, normal_local.y), angle);
        let normal = Vec3::new(normal_xy.x, normal_xy.y, normal_local.z);

        Some((t_hit, normal))
    }

    fn rotate_vec2(&self, v: Vec2, radians: f32) -> Vec2 {
        let (sin, cos) = radians.sin_cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    pub(crate) fn pick_gizmo_handle(
        &self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> Option<(GizmoDragKind, GizmoAxis)> {
        let selected_index = self.ui.selected_block_index?;
        let obj = self.objects.get(selected_index)?;
        let bounds_position = obj.position;
        let bounds_size = obj.size;

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.gizmo_axis_lengths_world(center, 50.0, viewport_size);
        let pointer = Vec2::new(x as f32, y as f32);

        let candidates = [
            (
                GizmoDragKind::Move,
                GizmoAxis::X,
                center + Vec3::new(axis_lengths[0], 0.0, 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::Y,
                center + Vec3::new(0.0, axis_lengths[1], 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::Z,
                center + Vec3::new(0.0, 0.0, axis_lengths[2]),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::XNeg,
                center + Vec3::new(-axis_lengths[0], 0.0, 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::YNeg,
                center + Vec3::new(0.0, -axis_lengths[1], 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::ZNeg,
                center + Vec3::new(0.0, 0.0, -axis_lengths[2]),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::X,
                Vec3::new(
                    bounds_position[0] + bounds_size[0] + 0.36,
                    center.y,
                    center.z,
                ),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::Y,
                Vec3::new(
                    center.x,
                    bounds_position[1] + bounds_size[1] + 0.36,
                    center.z,
                ),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::Z,
                Vec3::new(
                    center.x,
                    center.y,
                    bounds_position[2] + bounds_size[2] + 0.36,
                ),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::XNeg,
                Vec3::new(bounds_position[0] - 0.36, center.y, center.z),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::YNeg,
                Vec3::new(center.x, bounds_position[1] - 0.36, center.z),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::ZNeg,
                Vec3::new(center.x, center.y, bounds_position[2] - 0.36),
            ),
        ];

        let mut best: Option<(GizmoDragKind, GizmoAxis, f32)> = None;
        for (kind, axis, world) in candidates {
            if let Some(screen) = self.world_to_screen_v(world, viewport_size) {
                let dist = screen.distance(pointer);
                if dist <= 22.0 {
                    match best {
                        Some((.., best_dist)) if dist >= best_dist => {}
                        _ => best = Some((kind, axis, dist)),
                    }
                }
            }
        }

        best.map(|(kind, axis, _)| (kind, axis))
    }

    pub(crate) fn pixels_to_world_along_axis(
        &self,
        center: Vec3,
        axis: Vec3,
        pixels: f32,
        viewport_size: Vec2,
    ) -> Option<f32> {
        let origin_screen = self.world_to_screen_v(center, viewport_size)?;
        let axis_screen = self.world_to_screen_v(center + axis, viewport_size)?;
        let pixels_per_world = axis_screen.distance(origin_screen);
        if pixels_per_world.abs() <= f32::EPSILON {
            return None;
        }
        Some(pixels / pixels_per_world)
    }

    pub(crate) fn gizmo_axis_lengths_world(
        &self,
        center: Vec3,
        screen_size: f32,
        viewport_size: Vec2,
    ) -> [f32; 3] {
        let mut lengths = [1.0, 1.0, 1.0];
        for i in 0..3 {
            let axis = match i {
                0 => Vec3::X,
                1 => Vec3::Y,
                _ => Vec3::Z,
            };
            if let Some(scale) =
                self.pixels_to_world_along_axis(center, axis, screen_size, viewport_size)
            {
                lengths[i] = scale;
            }
        }
        lengths
    }

    pub(crate) fn gizmo_axis_width_world(
        &self,
        center: Vec3,
        target_pixels: f32,
        viewport_size: Vec2,
    ) -> f32 {
        let x = self.pixels_to_world_along_axis(center, Vec3::X, target_pixels, viewport_size);
        let y = self.pixels_to_world_along_axis(center, Vec3::Y, target_pixels, viewport_size);
        let z = self.pixels_to_world_along_axis(center, Vec3::Z, target_pixels, viewport_size);
        let mut sum = 0.0;
        let mut count = 0.0;
        for value in [x, y, z].into_iter().flatten() {
            sum += value;
            count += 1.0;
        }
        if count > 0.0 {
            sum / count
        } else {
            0.06
        }
    }

    pub(crate) fn update_cursor_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> crate::state::EditorInteractionChange {
        self.ui.pointer_screen = Some([x, y]);

        let Some(pick) = self.pick_from_screen(x, y, viewport_size) else {
            if self.ui.mode == crate::types::EditorMode::Select
                && self.ui.hovered_block_index.is_some()
            {
                self.ui.hovered_block_index = None;
                return crate::state::EditorInteractionChange::Hover;
            }
            return crate::state::EditorInteractionChange::None;
        };

        if self.ui.mode == crate::types::EditorMode::Select {
            if self.ui.hovered_block_index != pick.hit_block_index {
                self.ui.hovered_block_index = pick.hit_block_index;
                return crate::state::EditorInteractionChange::Hover;
            }
            return crate::state::EditorInteractionChange::None;
        }

        if pick.cursor != self.ui.cursor {
            self.ui.cursor = pick.cursor;
            return crate::state::EditorInteractionChange::Cursor;
        }

        crate::state::EditorInteractionChange::None
    }

    pub(crate) fn begin_block_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if self.ui.mode != crate::types::EditorMode::Select {
            return false;
        }

        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let Some(pick) = self.pick_from_screen(x, y, viewport_size) else {
            return false;
        };

        if pick
            .hit_block_index
            .is_some_and(|i| self.selection_contains(i))
        {
            let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
                return false;
            };
            let center = Vec3::new(
                bounds_position[0] + bounds_size[0] * 0.5,
                bounds_position[1] + bounds_size[1] * 0.5,
                bounds_position[2] + bounds_size[2] * 0.5,
            );
            let Some(center_screen) = self.world_to_screen_v(center, viewport_size) else {
                return false;
            };

            let mut start_blocks = Vec::with_capacity(selected_indices.len());
            for index in selected_indices {
                if let Some(obj) = self.objects.get(index) {
                    start_blocks.push(crate::state::EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                    });
                }
            }

            self.runtime.interaction.block_drag = Some(crate::state::EditorBlockDrag {
                start_mouse: [x, y],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks,
            });
            return true;
        }

        false
    }

    pub(crate) fn begin_gizmo_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if self.ui.mode != crate::types::EditorMode::Select {
            return false;
        }

        let indices = self.selected_indices_normalized();
        if indices.is_empty() {
            return false;
        }

        let Some((kind, axis)) = self.pick_gizmo_handle(x, y, viewport_size) else {
            return false;
        };

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            return false;
        };

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let Some(center_screen) = self.world_to_screen_v(center, viewport_size) else {
            return false;
        };

        let mut start_blocks = Vec::with_capacity(indices.len());
        for index in indices {
            if let Some(obj) = self.objects.get(index) {
                start_blocks.push(crate::state::EditorDragBlockStart {
                    index,
                    position: obj.position,
                    size: obj.size,
                });
            }
        }

        self.runtime.interaction.gizmo_drag = Some(crate::state::EditorGizmoDrag {
            axis,
            kind,
            start_mouse: [x, y],
            start_center_screen: [center_screen.x, center_screen.y],
            start_center_world: [center.x, center.y, center.z],
            start_blocks,
        });

        true
    }
}
