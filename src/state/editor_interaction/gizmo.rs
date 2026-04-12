/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::{
    EditorDirtyFlags, EditorDragBlockStart, EditorGizmoDrag, EditorSubsystem, State,
};
use crate::types::AppPhase;
use crate::types::{GizmoAxis, GizmoDragKind};
use glam::{Vec2, Vec3};

const GIZMO_MOVE_PICK_RADIUS_PIXELS: f32 = 40.0;
const GIZMO_RESIZE_PICK_RADIUS_PIXELS: f32 = 32.0;
const GIZMO_ROTATE_PICK_RADIUS_PIXELS: f32 = 18.0;

impl EditorSubsystem {
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
        let mut axis_dir = match drag.axis {
            GizmoAxis::X | GizmoAxis::XNeg => Vec3::X,
            GizmoAxis::Y | GizmoAxis::YNeg => Vec3::Y,
            GizmoAxis::Z | GizmoAxis::ZNeg => Vec3::Z,
        };

        if drag.kind == GizmoDragKind::Rotate {
            let rotation = drag
                .start_blocks
                .first()
                .map(|b| {
                    glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        b.rotation_degrees[0].to_radians(),
                        b.rotation_degrees[1].to_radians(),
                        b.rotation_degrees[2].to_radians(),
                    )
                })
                .unwrap_or_default();
            axis_dir = rotation * axis_dir;
        }

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
                let snap_enabled = self.effective_snap_to_grid();
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
                            next[1] = (next[1].max(0.0) / snap_step).round() * snap_step;
                            next[2] = (next[2] / snap_step).round() * snap_step;
                        } else {
                            next[1] = next[1].max(0.0);
                        }
                        obj.position = next;
                        if first_cursor.is_none() {
                            first_cursor = Some(next);
                        }
                    }
                }
                if let Some(next_position) = first_cursor {
                    self.ui.cursor = [
                        next_position[0],
                        next_position[1].max(0.0),
                        next_position[2],
                    ];
                }
            }
            GizmoDragKind::Resize => {
                let snap_enabled = self.effective_snap_to_grid();
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
                                let mut p = block.position[1] + world_delta;
                                let top_edge = block.position[1] + block.size[1];
                                if snap_enabled {
                                    p = (p / snap_step).round() * snap_step;
                                }
                                p = p.max(0.0);
                                let s = (top_edge - p).max(min_size);
                                p = top_edge - s;
                                obj.position[1] = p;
                                obj.size[1] = s;
                            }
                            GizmoAxis::ZNeg => {
                                let mut p = block.position[2] + world_delta;
                                let upper_edge = block.position[2] + block.size[2];
                                if snap_enabled {
                                    p = (p / snap_step).round() * snap_step;
                                }
                                let s = (upper_edge - p).max(min_size);
                                p = upper_edge - s;
                                obj.position[2] = p;
                                obj.size[2] = s;
                            }
                        }
                    }
                }
            }
            GizmoDragKind::Rotate => {
                let axis_index = match drag.axis {
                    GizmoAxis::X | GizmoAxis::XNeg => 0,
                    GizmoAxis::Y | GizmoAxis::YNeg => 1,
                    GizmoAxis::Z | GizmoAxis::ZNeg => 2,
                };

                let mut raw_delta_degrees = 0.0;
                let start_vec = Vec2::new(
                    drag.start_mouse[0] as f32 - drag.start_center_screen[0],
                    drag.start_mouse[1] as f32 - drag.start_center_screen[1],
                );
                let current_vec = Vec2::new(
                    x as f32 - drag.start_center_screen[0],
                    y as f32 - drag.start_center_screen[1],
                );

                if start_vec.length_squared() > f32::EPSILON
                    && current_vec.length_squared() > f32::EPSILON
                {
                    let start_angle = start_vec.y.atan2(start_vec.x);
                    let current_angle = current_vec.y.atan2(current_vec.x);
                    let mut diff = current_angle - start_angle;

                    if diff > std::f32::consts::PI {
                        diff -= std::f32::consts::TAU;
                    } else if diff < -std::f32::consts::PI {
                        diff += std::f32::consts::TAU;
                    }

                    let target = Vec3::new(
                        self.camera.editor_pan[0],
                        self.camera.editor_target_z,
                        self.camera.editor_pan[1],
                    );
                    let eye = target + self.camera_offset();
                    let view_dir = (target - eye).normalize_or_zero();

                    let is_facing_camera = axis_dir.dot(view_dir) < 0.0;
                    let sign = if is_facing_camera { -1.0 } else { 1.0 };

                    raw_delta_degrees = diff.to_degrees() * sign;
                }

                let snap_enabled = self.effective_snap_rotation();
                let snap_step = self.config.snap_rotation_step_degrees.max(1.0);

                for block in &drag.start_blocks {
                    if let Some(obj) = self.objects.get_mut(block.index) {
                        let mut next = block.rotation_degrees;
                        next[axis_index] = block.rotation_degrees[axis_index] + raw_delta_degrees;
                        if snap_enabled {
                            next[axis_index] = (next[axis_index] / snap_step).round() * snap_step;
                        }
                        obj.rotation_degrees = next;
                    }
                }
            }
        }
        self.runtime.interaction.gizmo_drag = Some(drag);
        true
    }

    pub(crate) fn pick_gizmo_handle(
        &self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> Option<(GizmoDragKind, GizmoAxis)> {
        let mode = self.ui.mode;
        let allow_move = mode.shows_move_gizmo();
        let allow_scale = mode.shows_scale_gizmo();
        let allow_rotate = mode.shows_rotate_gizmo();
        if !allow_move && !allow_scale && !allow_rotate {
            return None;
        }

        let (bounds_position, bounds_size) = self.selected_group_bounds()?;

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.gizmo_axis_lengths_world(center, 100.0, viewport_size);
        let resize_offsets = self.gizmo_axis_lengths_world(center, 9.0, viewport_size);
        let pointer = Vec2::new(x as f32, y as f32);

        let mut candidates: Vec<(GizmoDragKind, GizmoAxis, Vec3, f32)> = Vec::new();
        if allow_move {
            for &(axis, dir, neg) in &[
                (GizmoAxis::X, Vec3::X, false),
                (GizmoAxis::Y, Vec3::Y, false),
                (GizmoAxis::Z, Vec3::Z, false),
                (GizmoAxis::XNeg, Vec3::X, true),
                (GizmoAxis::YNeg, Vec3::Y, true),
                (GizmoAxis::ZNeg, Vec3::Z, true),
            ] {
                let axis_len = match axis {
                    GizmoAxis::X | GizmoAxis::XNeg => axis_lengths[0],
                    GizmoAxis::Y | GizmoAxis::YNeg => axis_lengths[1],
                    GizmoAxis::Z | GizmoAxis::ZNeg => axis_lengths[2],
                };
                let sign = if neg { -1.0 } else { 1.0 };
                let tip = center + dir * axis_len * sign;
                let base = center + dir * 0.01 * sign;
                let mid = (tip + base) * 0.5;
                candidates.push((
                    GizmoDragKind::Move,
                    axis,
                    tip,
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ));
                candidates.push((
                    GizmoDragKind::Move,
                    axis,
                    mid,
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ));
                candidates.push((
                    GizmoDragKind::Move,
                    axis,
                    base,
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ));
            }
        }

        if allow_scale {
            candidates.extend_from_slice(&[
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::X,
                    Vec3::new(
                        bounds_position[0] + bounds_size[0] + resize_offsets[0],
                        center.y,
                        center.z,
                    ),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::Y,
                    Vec3::new(
                        center.x,
                        bounds_position[1] + bounds_size[1] + resize_offsets[1],
                        center.z,
                    ),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::Z,
                    Vec3::new(
                        center.x,
                        center.y,
                        bounds_position[2] + bounds_size[2] + resize_offsets[2],
                    ),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::XNeg,
                    Vec3::new(bounds_position[0] - resize_offsets[0], center.y, center.z),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::YNeg,
                    Vec3::new(center.x, bounds_position[1] - resize_offsets[1], center.z),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Resize,
                    GizmoAxis::ZNeg,
                    Vec3::new(center.x, center.y, bounds_position[2] - resize_offsets[2]),
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ),
            ]);
        }

        if allow_rotate {
            let rotation_degrees = self
                .selected_indices_normalized()
                .first()
                .and_then(|&index| self.objects.get(index))
                .map(|obj| obj.rotation_degrees)
                .unwrap_or([0.0, 0.0, 0.0]);
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                rotation_degrees[0].to_radians(),
                rotation_degrees[1].to_radians(),
                rotation_degrees[2].to_radians(),
            );

            let ring_radius = axis_lengths[0] * 0.78;
            for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
                for sample in 0..2 {
                    let theta = (sample as f32 / 2.0) * std::f32::consts::TAU;
                    let (sin_t, cos_t) = theta.sin_cos();
                    let mut local = match axis {
                        GizmoAxis::X => Vec3::new(0.0, cos_t, sin_t),
                        GizmoAxis::Y => Vec3::new(sin_t, 0.0, cos_t),
                        GizmoAxis::Z => Vec3::new(cos_t, sin_t, 0.0),
                        _ => Vec3::ZERO,
                    };
                    local = rotation * local;
                    let sample_world = center + local * ring_radius;
                    candidates.push((
                        GizmoDragKind::Rotate,
                        axis,
                        sample_world,
                        GIZMO_ROTATE_PICK_RADIUS_PIXELS * 2.5,
                    ));
                }
            }
        }

        let target = Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_target_z,
            self.camera.editor_pan[1],
        );
        let eye = target + self.camera_offset();

        let mut best: Option<(GizmoDragKind, GizmoAxis, f32, f32)> = None;
        for (kind, axis, world, pick_radius) in candidates {
            if let Some(screen) = self.world_to_screen_v(world, viewport_size) {
                let dist = screen.distance(pointer);
                if dist <= pick_radius {
                    let depth = world.distance_squared(eye);
                    match best {
                        Some((.., best_dist, best_depth)) => {
                            let is_better = if (dist - best_dist).abs() > 10.0 {
                                dist < best_dist
                            } else {
                                depth < best_depth
                            };
                            if is_better {
                                best = Some((kind, axis, dist, depth));
                            }
                        }
                        None => best = Some((kind, axis, dist, depth)),
                    }
                }
            }
        }

        best.map(|(kind, axis, _, _)| (kind, axis))
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
        let scale = self.gizmo_axis_width_world(center, screen_size, viewport_size);
        [scale, scale, scale]
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

        let mut min_scale: Option<f32> = None;
        for scale in [x, y, z].into_iter().flatten() {
            if min_scale.is_none() || scale < min_scale.unwrap() {
                min_scale = Some(scale);
            }
        }

        min_scale.unwrap_or(0.06)
    }

    pub(crate) fn begin_gizmo_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if !self.ui.mode.shows_gizmo() {
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
                start_blocks.push(EditorDragBlockStart {
                    index,
                    position: obj.position,
                    size: obj.size,
                    rotation_degrees: obj.rotation_degrees,
                });
            }
        }

        self.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
            axis,
            kind,
            start_mouse: [x, y],
            start_center_screen: [center_screen.x, center_screen.y],
            start_center_world: [center.x, center.y, center.z],
            start_blocks,
        });

        true
    }

    pub(crate) fn drag_gizmo_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.drag_gizmo(x, y, viewport) {
            self.sync_objects_for_drag();
            if self.runtime.interaction.gizmo_drag.as_ref().map(|d| d.kind)
                == Some(GizmoDragKind::Move)
            {
                self.mark_dirty(EditorDirtyFlags {
                    rebuild_cursor: true,
                    ..EditorDirtyFlags::default()
                });
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn begin_gizmo_drag_ext(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.begin_gizmo_drag(x, y, viewport) {
            self.record_history_state();
            true
        } else {
            false
        }
    }
}

impl State {
    pub(crate) fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .drag_gizmo_from_screen(x, y, viewport, self.phase)
    }

    pub(crate) fn begin_editor_gizmo_drag(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .begin_gizmo_drag_ext(x, y, viewport_size, self.phase)
    }

    pub(crate) fn editor_gizmo_axis_lengths_world(
        &self,
        center: Vec3,
        target_pixels: f32,
    ) -> [f32; 3] {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .gizmo_axis_lengths_world(center, target_pixels, viewport_size)
    }

    pub(crate) fn editor_gizmo_axis_width_world(&self, center: Vec3, target_pixels: f32) -> f32 {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .gizmo_axis_width_world(center, target_pixels, viewport_size)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{EditorDirtyFlags, EditorDragBlockStart, EditorGizmoDrag, State};
    use crate::test_utils::assert_approx_eq as approx_eq;
    use crate::types::{AppPhase, EditorMode, GizmoAxis, GizmoDragKind, LevelObject};
    use glam::{Vec2, Vec3};

    fn test_block() -> LevelObject {
        LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    fn start_block_for_index(index: usize, block: &LevelObject) -> EditorDragBlockStart {
        EditorDragBlockStart {
            index,
            position: block.position,
            size: block.size,
            rotation_degrees: block.rotation_degrees,
        }
    }

    #[test]
    fn pick_gizmo_handle_returns_none_when_mode_has_no_gizmo() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Place;
            state.editor.objects = vec![test_block()];
            state.editor.ui.selected_block_index = Some(0);

            let viewport = Vec2::new(1280.0, 720.0);
            let hit = state.editor.pick_gizmo_handle(640.0, 360.0, viewport);
            assert!(hit.is_none());
        });
    }

    #[test]
    fn drag_gizmo_move_updates_object_position_along_axis() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_to_grid = false;
            state.editor.objects = vec![test_block()];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = state
                .editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 40.0;

            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            assert!(state
                .editor
                .drag_gizmo(target.x as f64, target.y as f64, viewport));
            assert!(state.editor.objects[0].position[0] > 0.0);
            approx_eq(state.editor.objects[0].position[1], 0.0, 1e-4);
            approx_eq(state.editor.objects[0].position[2], 0.0, 1e-4);
        });
    }

    #[test]
    fn drag_gizmo_resize_negative_axis_preserves_opposite_edge() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_to_grid = false;
            state.editor.objects = vec![test_block()];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = state
                .editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 20.0;

            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::XNeg,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            assert!(state
                .editor
                .drag_gizmo(target.x as f64, target.y as f64, viewport));

            let object = &state.editor.objects[0];
            assert!(object.size[0] >= 0.25);
            approx_eq(object.position[0] + object.size[0], 1.0, 1e-3);
        });
    }

    #[test]
    fn drag_gizmo_rotate_changes_selected_block_rotation() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_rotation = false;
            state.editor.objects = vec![test_block()];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start = origin_screen + Vec2::new(30.0, 0.0);
            let target = origin_screen + Vec2::new(0.0, 30.0);

            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start.x as f64, start.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            assert!(state
                .editor
                .drag_gizmo(target.x as f64, target.y as f64, viewport));
            assert!(state.editor.objects[0].rotation_degrees[1].abs() > 1.0);
        });
    }

    #[test]
    fn helper_conversions_handle_valid_and_invalid_viewports() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let center = Vec3::new(0.5, 0.5, 0.5);

            let valid = state.editor.pixels_to_world_along_axis(
                center,
                Vec3::X,
                64.0,
                Vec2::new(1280.0, 720.0),
            );
            assert!(valid.is_some());
            assert!(valid.unwrap_or_default() > 0.0);

            let invalid = state.editor.pixels_to_world_along_axis(
                center,
                Vec3::X,
                64.0,
                Vec2::new(0.0, 720.0),
            );
            let _ = invalid;

            let fallback_width =
                state
                    .editor
                    .gizmo_axis_width_world(center, 100.0, Vec2::new(0.0, 0.0));
            let _ = fallback_width;
        });
    }

    #[test]
    fn pick_and_begin_gizmo_drag_cover_move_scale_rotate_paths() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.objects = vec![test_block()];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);

            state.editor.ui.mode = EditorMode::Move;
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let move_hit = state.editor.pick_gizmo_handle(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport,
            );
            assert!(matches!(move_hit, Some((GizmoDragKind::Move, _))));
            assert!(state.editor.begin_gizmo_drag(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport
            ));

            state.editor.runtime.interaction.gizmo_drag = None;
            state.editor.ui.mode = EditorMode::Scale;
            let scale_lengths = state.editor.gizmo_axis_lengths_world(center, 9.0, viewport);
            let scale_world = Vec3::new(1.0 + scale_lengths[0], center.y, center.z);
            let scale_screen = state
                .editor
                .world_to_screen_v(scale_world, viewport)
                .expect("scale handle projects");
            let scale_hit = state.editor.pick_gizmo_handle(
                scale_screen.x as f64,
                scale_screen.y as f64,
                viewport,
            );
            assert!(matches!(scale_hit, Some((GizmoDragKind::Resize, _))));

            state.editor.ui.mode = EditorMode::Rotate;
            let ring_radius = state
                .editor
                .gizmo_axis_lengths_world(center, 100.0, viewport)[0]
                * 0.78;
            let rotate_world = center + Vec3::new(ring_radius, 0.0, 0.0);
            let rotate_screen = state
                .editor
                .world_to_screen_v(rotate_world, viewport)
                .expect("rotate ring projects");
            let rotate_hit = state.editor.pick_gizmo_handle(
                rotate_screen.x as f64,
                rotate_screen.y as f64,
                viewport,
            );
            assert!(matches!(rotate_hit, Some((GizmoDragKind::Rotate, _))));
        });
    }

    #[test]
    fn drag_gizmo_from_screen_respects_phase_and_move_dirty_flag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_to_grid = false;
            state.editor.objects = vec![test_block()];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = state
                .editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 24.0;

            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            state.editor.runtime.dirty = EditorDirtyFlags::default();
            assert!(!state.editor.drag_gizmo_from_screen(
                target.x as f64,
                target.y as f64,
                viewport,
                AppPhase::Menu,
            ));

            assert!(state.editor.drag_gizmo_from_screen(
                target.x as f64,
                target.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert!(state.editor.runtime.dirty.rebuild_cursor);

            state.editor.runtime.dirty = EditorDirtyFlags::default();
            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
            assert!(state.editor.drag_gizmo_from_screen(
                target.x as f64,
                target.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert!(!state.editor.runtime.dirty.rebuild_cursor);
        });
    }

    #[test]
    fn begin_gizmo_drag_ext_and_state_wrappers_follow_phase_guards() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.objects = vec![test_block()];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.mode = EditorMode::Move;

            let viewport = Vec2::new(1280.0, 720.0);
            let center_screen = state
                .editor
                .world_to_screen_v(Vec3::new(0.5, 0.5, 0.5), viewport)
                .expect("center projects");

            assert!(!state.editor.begin_gizmo_drag_ext(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport,
                AppPhase::Menu,
            ));

            let history_before = state.editor.runtime.history.undo.len();
            assert!(state.editor.begin_gizmo_drag_ext(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert_eq!(state.editor.runtime.history.undo.len(), history_before + 1);

            state.phase = AppPhase::Menu;
            assert!(!state.begin_editor_gizmo_drag(center_screen.x as f64, center_screen.y as f64));

            state.phase = AppPhase::Editor;
            let _ = state.begin_editor_gizmo_drag(center_screen.x as f64, center_screen.y as f64);
            assert!(state.drag_editor_gizmo_from_screen(
                center_screen.x as f64 + 8.0,
                center_screen.y as f64
            ));

            let lengths = state.editor_gizmo_axis_lengths_world(Vec3::new(0.5, 0.5, 0.5), 100.0);
            assert!(lengths[0] > 0.0 && lengths[1] > 0.0 && lengths[2] > 0.0);
            assert!(state.editor_gizmo_axis_width_world(Vec3::new(0.5, 0.5, 0.5), 100.0) > 0.0);
        });
    }
}
