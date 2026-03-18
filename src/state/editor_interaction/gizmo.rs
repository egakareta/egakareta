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

                let snap_enabled = self.config.snap_rotation;
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
            candidates.extend_from_slice(&[
                (
                    GizmoDragKind::Move,
                    GizmoAxis::X,
                    Vec3::new(center.x + axis_lengths[0], center.y, center.z),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Move,
                    GizmoAxis::Y,
                    Vec3::new(center.x, center.y + axis_lengths[1], center.z),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Move,
                    GizmoAxis::Z,
                    Vec3::new(center.x, center.y, center.z + axis_lengths[2]),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Move,
                    GizmoAxis::XNeg,
                    Vec3::new(center.x - axis_lengths[0], center.y, center.z),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Move,
                    GizmoAxis::YNeg,
                    Vec3::new(center.x, center.y - axis_lengths[1], center.z),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
                (
                    GizmoDragKind::Move,
                    GizmoAxis::ZNeg,
                    Vec3::new(center.x, center.y, center.z - axis_lengths[2]),
                    GIZMO_MOVE_PICK_RADIUS_PIXELS,
                ),
            ]);
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
