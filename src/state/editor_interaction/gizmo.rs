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
    fn gizmo_drag_trigger_start(&self, drag: &EditorGizmoDrag) -> Option<EditorDragBlockStart> {
        if !self.selected_indices_normalized().is_empty() {
            return None;
        }
        let (trigger_index, _) = self.selected_transform_trigger_target()?;
        let start = drag.start_blocks.first().copied()?;
        (start.index == trigger_index).then_some(start)
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
        let mut axis_dir = match drag.axis {
            GizmoAxis::X | GizmoAxis::XNeg => Vec3::X,
            GizmoAxis::Y | GizmoAxis::YNeg => Vec3::Y,
            GizmoAxis::Z | GizmoAxis::ZNeg => Vec3::Z,
        };

        let trigger_start = self.gizmo_drag_trigger_start(&drag);

        if drag.kind == GizmoDragKind::Rotate {
            let rotation_source = trigger_start.as_ref().or_else(|| drag.start_blocks.first());
            let rotation = rotation_source
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
                // Clamp Y-axis delta so the lowest block stops at y=0 instead
                // of each block being clamped independently (which compresses
                // stacked blocks into the same position).
                let clamped_delta = if matches!(drag.axis, GizmoAxis::Y | GizmoAxis::YNeg) {
                    let min_start_y = drag
                        .start_blocks
                        .iter()
                        .map(|b| b.position[1])
                        .fold(f32::INFINITY, f32::min);
                    let min_next_y = min_start_y + world_delta;
                    if min_next_y < 0.0 {
                        world_delta - min_next_y
                    } else {
                        world_delta
                    }
                } else {
                    world_delta
                };
                let mut first_cursor: Option<[f32; 3]> = None;
                if let Some(start) = trigger_start {
                    let mut next = start.position;
                    match drag.axis {
                        GizmoAxis::X | GizmoAxis::XNeg => next[0] += clamped_delta,
                        GizmoAxis::Y | GizmoAxis::YNeg => next[1] += clamped_delta,
                        GizmoAxis::Z | GizmoAxis::ZNeg => next[2] += clamped_delta,
                    }
                    if snap_enabled {
                        next[0] = (next[0] / snap_step).round() * snap_step;
                        next[1] = (next[1].max(0.0) / snap_step).round() * snap_step;
                        next[2] = (next[2] / snap_step).round() * snap_step;
                    } else {
                        next[1] = next[1].max(0.0);
                    }
                    if self.set_transform_trigger_target(
                        start.index,
                        next,
                        start.size,
                        start.rotation_degrees,
                    ) {
                        first_cursor = Some(next);
                    }
                } else {
                    for block in &drag.start_blocks {
                        if let Some(obj) = self.objects.get_mut(block.index) {
                            let mut next = block.position;
                            match drag.axis {
                                GizmoAxis::X | GizmoAxis::XNeg => next[0] += clamped_delta,
                                GizmoAxis::Y | GizmoAxis::YNeg => next[1] += clamped_delta,
                                GizmoAxis::Z | GizmoAxis::ZNeg => next[2] += clamped_delta,
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
                if let Some(start) = trigger_start {
                    let mut next_position = start.position;
                    let mut next_size = start.size;
                    match drag.axis {
                        GizmoAxis::X => {
                            let mut s = start.size[0] + world_delta;
                            if snap_enabled {
                                s = (s / snap_step).round() * snap_step;
                            }
                            next_size[0] = s.max(min_size);
                        }
                        GizmoAxis::Y => {
                            let mut s = start.size[1] + world_delta;
                            if snap_enabled {
                                s = (s / snap_step).round() * snap_step;
                            }
                            next_size[1] = s.max(min_size);
                        }
                        GizmoAxis::Z => {
                            let mut s = start.size[2] + world_delta;
                            if snap_enabled {
                                s = (s / snap_step).round() * snap_step;
                            }
                            next_size[2] = s.max(min_size);
                        }
                        GizmoAxis::XNeg => {
                            let mut s = start.size[0] - world_delta;
                            let mut p = start.position[0] + world_delta;
                            let right_edge = start.position[0] + start.size[0];
                            if snap_enabled {
                                p = (p / snap_step).round() * snap_step;
                                s = (right_edge - p).max(min_size);
                                p = right_edge - s;
                            } else {
                                s = s.max(min_size);
                                p = right_edge - s;
                            }
                            next_position[0] = p;
                            next_size[0] = s;
                        }
                        GizmoAxis::YNeg => {
                            let mut p = start.position[1] + world_delta;
                            let top_edge = start.position[1] + start.size[1];
                            if snap_enabled {
                                p = (p / snap_step).round() * snap_step;
                            }
                            p = p.max(0.0);
                            let s = (top_edge - p).max(min_size);
                            p = top_edge - s;
                            next_position[1] = p;
                            next_size[1] = s;
                        }
                        GizmoAxis::ZNeg => {
                            let mut p = start.position[2] + world_delta;
                            let upper_edge = start.position[2] + start.size[2];
                            if snap_enabled {
                                p = (p / snap_step).round() * snap_step;
                            }
                            let s = (upper_edge - p).max(min_size);
                            p = upper_edge - s;
                            next_position[2] = p;
                            next_size[2] = s;
                        }
                    }
                    self.set_transform_trigger_target(
                        start.index,
                        next_position,
                        next_size,
                        start.rotation_degrees,
                    );
                } else {
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
            }
            GizmoDragKind::Rotate => {
                let local_axis = match drag.axis {
                    GizmoAxis::X | GizmoAxis::XNeg => Vec3::X,
                    GizmoAxis::Y | GizmoAxis::YNeg => Vec3::Y,
                    GizmoAxis::Z | GizmoAxis::ZNeg => Vec3::Z,
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
                let mut delta_degrees = raw_delta_degrees;
                if snap_enabled {
                    delta_degrees = (delta_degrees / snap_step).round() * snap_step;
                }

                let apply_rotation = |block: EditorDragBlockStart| {
                    let block_rotation = glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        block.rotation_degrees[0].to_radians(),
                        block.rotation_degrees[1].to_radians(),
                        block.rotation_degrees[2].to_radians(),
                    );
                    let world_axis = block_rotation * local_axis;
                    let delta_quat =
                        glam::Quat::from_axis_angle(world_axis, delta_degrees.to_radians());
                    let new_rotation = delta_quat * block_rotation;
                    let (rx, ry, rz) = new_rotation.to_euler(glam::EulerRot::XYZ);
                    [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]
                };

                if let Some(start) = trigger_start {
                    let next_rotation = apply_rotation(start);
                    self.set_transform_trigger_target(
                        start.index,
                        start.position,
                        start.size,
                        next_rotation,
                    );
                } else {
                    for block in &drag.start_blocks {
                        if let Some(obj) = self.objects.get_mut(block.index) {
                            obj.rotation_degrees = apply_rotation(*block);
                        }
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
                .or_else(|| {
                    self.selected_transform_trigger_target()
                        .map(|(_, target)| target.rotation_degrees)
                })
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
        let trigger_target = if indices.is_empty() {
            self.selected_transform_trigger_target()
        } else {
            None
        };
        if indices.is_empty() && trigger_target.is_none() {
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

        let mut start_blocks = Vec::with_capacity(indices.len().max(1));
        if let Some((trigger_index, target)) = trigger_target {
            start_blocks.push(EditorDragBlockStart {
                index: trigger_index,
                position: target.position,
                size: target.size,
                rotation_degrees: target.rotation_degrees,
            });
        } else {
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
            let is_move = self.runtime.interaction.gizmo_drag.as_ref().map(|d| d.kind)
                == Some(GizmoDragKind::Move);
            let trigger_drag = self
                .runtime
                .interaction
                .gizmo_drag
                .as_ref()
                .is_some_and(|drag| self.gizmo_drag_trigger_start(drag).is_some());
            // The dragged block(s) may be a transform trigger block on the
            // grid (not the virtual trigger target). When the user moves,
            // resizes, or rotates the trigger block itself, the source ring
            // and connector line should follow the block pose live.
            let dragging_transform_trigger_block = self
                .runtime
                .interaction
                .gizmo_drag
                .as_ref()
                .is_some_and(|drag| {
                    drag.start_blocks.iter().any(|b| {
                        self.objects
                            .get(b.index)
                            .is_some_and(|o| o.is_transform_trigger())
                    })
                });
            // Check if any dragged blocks are sources of transform triggers
            let dragged_indices: Vec<usize> = self
                .runtime
                .interaction
                .gizmo_drag
                .as_ref()
                .map(|drag| drag.start_blocks.iter().map(|b| b.index).collect())
                .unwrap_or_default();
            let dragging_transform_trigger_source =
                self.any_block_is_transform_trigger_source(&dragged_indices);
            if is_move || trigger_drag {
                self.mark_dirty(EditorDirtyFlags {
                    rebuild_cursor: is_move,
                    rebuild_block_mesh: trigger_drag,
                    rebuild_hitbox_visualization: trigger_drag,
                    rebuild_transform_trigger_markers: trigger_drag
                        || dragging_transform_trigger_block
                        || dragging_transform_trigger_source,
                    rebuild_preview_player: trigger_drag,
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
    use crate::triggers::{
        TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
    };
    use crate::types::{AppPhase, EditorMode, GizmoAxis, GizmoDragKind, LevelObject};
    use glam::{Vec2, Vec3};

    fn test_block() -> LevelObject {
        LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
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
    fn drag_gizmo_move_updates_selected_transform_trigger_target() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_to_grid = false;
            state.editor.ui.mode = EditorMode::Move;
            state.editor.set_triggers(vec![TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 1.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![0],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [0.0, 0.0, 0.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            }]);
            state.editor.set_trigger_selected(Some(0));

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
            let target = state
                .editor
                .selected_transform_trigger_target()
                .expect("selected trigger target")
                .1;
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks: vec![start_block_for_index(0, &target)],
            });

            state
                .editor
                .drag_gizmo(axis_screen.x as f64, axis_screen.y as f64, viewport);

            let TimedTriggerAction::TransformObjects { position, .. } =
                &state.editor.triggers()[0].action
            else {
                panic!("expected transform trigger");
            };
            assert!(position[0] > 0.9, "expected trigger target to move on X");
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

    #[test]
    fn drag_gizmo_y_prevents_compression_when_moving_below_ground() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = Vec2::new(1280.0, 720.0);

            // Two blocks at different heights.
            let mut low = test_block();
            low.position = [0.0, 0.0, 0.0];
            let mut high = test_block();
            high.position = [0.0, 3.0, 0.0];
            state.editor.objects = vec![low, high];
            state.editor.ui.mode = EditorMode::Select;
            state.editor.replace_block_selection(vec![0, 1]);

            let center = Vec3::new(0.5, 2.0, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            // Simulate a large negative Y gizmo drag.
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Move,
                start_mouse: [center_screen.x as f64, center_screen.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: [0.5, 2.0, 0.5],
                start_blocks: vec![
                    EditorDragBlockStart {
                        index: 0,
                        position: [0.0, 0.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                        rotation_degrees: [0.0, 0.0, 0.0],
                    },
                    EditorDragBlockStart {
                        index: 1,
                        position: [0.0, 3.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                        rotation_degrees: [0.0, 0.0, 0.0],
                    },
                ],
            });

            // Drag far below ground (simulate large negative world_delta via screen).
            // We call drag_gizmo directly with a fake viewport and a screen position
            // that yields a large negative Y world delta.
            // Instead, directly test via the internal method by placing mouse far away.
            let far_screen = Vec2::new(center_screen.x, center_screen.y - 2000.0);
            state
                .editor
                .drag_gizmo(far_screen.x as f64, far_screen.y as f64, viewport);

            // The low block should stop at y=0, the high block should maintain its
            // relative 3-unit offset (staying at y=3 or wherever the delta places it
            // without going negative for the lowest block).
            let low_y = state.editor.objects[0].position[1];
            let high_y = state.editor.objects[1].position[1];
            assert!(low_y >= -1e-4, "low block y={low_y} should be >= 0");
            let relative_gap = high_y - low_y;
            assert!(
                relative_gap >= 2.9,
                "relative gap {relative_gap} should be preserved (~3.0)"
            );
        });
    }

    /// Helper: compute the signed angular delta (in degrees) that the rotation
    /// drag code would produce for the given screen-space mouse positions,
    /// camera, and world-space axis direction.
    fn compute_rotation_delta_degrees(
        state: &State,
        start_mouse: Vec2,
        current_mouse: Vec2,
        center_screen: Vec2,
        axis_dir: Vec3,
    ) -> f32 {
        let sv = start_mouse - center_screen;
        let cv = current_mouse - center_screen;
        if sv.length_squared() <= f32::EPSILON || cv.length_squared() <= f32::EPSILON {
            return 0.0;
        }
        let start_angle = sv.y.atan2(sv.x);
        let current_angle = cv.y.atan2(cv.x);
        let mut diff = current_angle - start_angle;
        if diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        } else if diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }
        let target = Vec3::new(
            state.editor.camera.editor_pan[0],
            state.editor.camera.editor_target_z,
            state.editor.camera.editor_pan[1],
        );
        let eye = target + state.editor.camera_offset();
        let view_dir = (target - eye).normalize_or_zero();
        let sign = if axis_dir.dot(view_dir) < 0.0 {
            -1.0
        } else {
            1.0
        };
        diff.to_degrees() * sign
    }

    /// Helper: given a start rotation and a delta around a world-space axis,
    /// compute the expected Euler XYZ result via quaternion composition.
    fn expected_rotation_after_delta(
        start_degrees: [f32; 3],
        world_axis: Vec3,
        delta_degrees: f32,
    ) -> [f32; 3] {
        let start_quat = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            start_degrees[0].to_radians(),
            start_degrees[1].to_radians(),
            start_degrees[2].to_radians(),
        );
        let delta_quat = glam::Quat::from_axis_angle(world_axis, delta_degrees.to_radians());
        let result_quat = delta_quat * start_quat;
        let (rx, ry, rz) = result_quat.to_euler(glam::EulerRot::XYZ);
        [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]
    }

    #[test]
    fn drag_gizmo_rotate_identity_block_produces_single_axis_change() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_rotation = false;
            state.editor.objects = vec![test_block()];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            // 90° clockwise screen-space rotation
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            assert!(state.editor.drag_gizmo(
                current_mouse.x as f64,
                current_mouse.y as f64,
                viewport,
            ));

            let rot = state.editor.objects[0].rotation_degrees;
            // With no pre-rotation, the Y ring axis is world Y.
            // Rotation should mostly affect the Y Euler component.
            assert!(
                rot[1].abs() > 1.0,
                "Y rotation should be non-trivial, got {rot:?}"
            );
            // X and Z should remain near zero since we rotated around Y
            // and the block had no prior rotation.
            approx_eq(rot[0], 0.0, 1.0);
            approx_eq(rot[2], 0.0, 1.0);
        });
    }

    #[test]
    fn drag_gizmo_rotate_around_x_with_y90_uses_world_axis() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_rotation = false;

            let mut block = test_block();
            block.rotation_degrees = [0.0, 90.0, 0.0];
            state.editor.objects = vec![block];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block_snap = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block_snap)],
            });

            assert!(state.editor.drag_gizmo(
                current_mouse.x as f64,
                current_mouse.y as f64,
                viewport,
            ));

            let start_degrees = [0.0_f32, 90.0, 0.0];
            let start_quat = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                start_degrees[0].to_radians(),
                start_degrees[1].to_radians(),
                start_degrees[2].to_radians(),
            );
            let world_axis = start_quat * Vec3::X;

            let delta = compute_rotation_delta_degrees(
                &state,
                start_mouse,
                current_mouse,
                center_screen,
                world_axis,
            );
            assert!(delta.abs() > 1.0, "delta should be non-trivial");

            let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);
            let actual = state.editor.objects[0].rotation_degrees;

            approx_eq(actual[0], expected[0], 0.5);
            approx_eq(actual[1], expected[1], 0.5);
            approx_eq(actual[2], expected[2], 0.5);

            // Verify it differs from naive Euler addition (the old buggy behavior).
            // Old code would set X = 0 + delta, Y = 90, Z = 0.
            let naive = [delta, 90.0, 0.0];
            let differs = (actual[0] - naive[0]).abs() > 1.0
                || (actual[1] - naive[1]).abs() > 1.0
                || (actual[2] - naive[2]).abs() > 1.0;
            assert!(
                differs,
                "quaternion rotation should differ from naive Euler addition: actual={actual:?}, naive={naive:?}"
            );
        });
    }

    #[test]
    fn drag_gizmo_rotate_around_y_with_xz_rotation_uses_world_axis() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_rotation = false;

            // Use X+Z rotation so the world Y axis actually diverges from local Y.
            // (Pure X rotation doesn't change the Y axis direction.)
            let mut block = test_block();
            block.rotation_degrees = [45.0, 0.0, 45.0];
            state.editor.objects = vec![block];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block_snap = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block_snap)],
            });

            assert!(state.editor.drag_gizmo(
                current_mouse.x as f64,
                current_mouse.y as f64,
                viewport,
            ));

            let start_degrees = [45.0_f32, 0.0, 45.0];
            let start_quat = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                start_degrees[0].to_radians(),
                start_degrees[1].to_radians(),
                start_degrees[2].to_radians(),
            );
            let world_axis = start_quat * Vec3::Y;

            let delta = compute_rotation_delta_degrees(
                &state,
                start_mouse,
                current_mouse,
                center_screen,
                world_axis,
            );
            assert!(delta.abs() > 1.0);

            let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);
            let actual = state.editor.objects[0].rotation_degrees;

            approx_eq(actual[0], expected[0], 0.5);
            approx_eq(actual[1], expected[1], 0.5);
            approx_eq(actual[2], expected[2], 0.5);

            // Verify it differs from naive Euler addition
            let naive = [45.0, delta, 45.0];
            let differs = (actual[0] - naive[0]).abs() > 1.0
                || (actual[1] - naive[1]).abs() > 1.0
                || (actual[2] - naive[2]).abs() > 1.0;
            assert!(
                differs,
                "quaternion rotation should differ from naive Euler: actual={actual:?}, naive={naive:?}"
            );
        });
    }

    #[test]
    fn drag_gizmo_rotate_around_z_with_combined_rotation_uses_world_axis() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_rotation = false;

            let mut block = test_block();
            block.rotation_degrees = [30.0, 20.0, 0.0];
            state.editor.objects = vec![block];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block_snap = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Z,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block_snap)],
            });

            assert!(state.editor.drag_gizmo(
                current_mouse.x as f64,
                current_mouse.y as f64,
                viewport,
            ));

            let start_degrees = [30.0_f32, 20.0, 0.0];
            let start_quat = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                start_degrees[0].to_radians(),
                start_degrees[1].to_radians(),
                start_degrees[2].to_radians(),
            );
            let world_axis = start_quat * Vec3::Z;

            let delta = compute_rotation_delta_degrees(
                &state,
                start_mouse,
                current_mouse,
                center_screen,
                world_axis,
            );
            assert!(delta.abs() > 1.0);

            let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);
            let actual = state.editor.objects[0].rotation_degrees;

            approx_eq(actual[0], expected[0], 0.5);
            approx_eq(actual[1], expected[1], 0.5);
            approx_eq(actual[2], expected[2], 0.5);
        });
    }

    #[test]
    fn drag_gizmo_rotate_with_snap_rounds_delta_before_composition() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_rotation = true;
            state.editor.config.snap_rotation_step_degrees = 15.0;

            let mut block = test_block();
            block.rotation_degrees = [0.0, 45.0, 0.0];
            state.editor.objects = vec![block];

            let viewport = Vec2::new(1280.0, 720.0);
            let center = Vec3::new(0.5, 0.5, 0.5);
            let center_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block_snap = state.editor.objects[0].clone();
            state.editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block_snap)],
            });

            assert!(state.editor.drag_gizmo(
                current_mouse.x as f64,
                current_mouse.y as f64,
                viewport,
            ));

            // Compute the snapped delta and verify quaternion composition
            let start_degrees = [0.0_f32, 45.0, 0.0];
            let start_quat = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                start_degrees[0].to_radians(),
                start_degrees[1].to_radians(),
                start_degrees[2].to_radians(),
            );
            let world_axis = start_quat * Vec3::X;

            let raw_delta = compute_rotation_delta_degrees(
                &state,
                start_mouse,
                current_mouse,
                center_screen,
                world_axis,
            );
            // The code snaps the delta to 15° increments
            let snapped_delta = (raw_delta / 15.0).round() * 15.0;
            assert!(
                snapped_delta.abs() >= 15.0,
                "snapped delta should be at least one snap step"
            );

            let expected = expected_rotation_after_delta(start_degrees, world_axis, snapped_delta);
            let actual = state.editor.objects[0].rotation_degrees;

            approx_eq(actual[0], expected[0], 0.5);
            approx_eq(actual[1], expected[1], 0.5);
            approx_eq(actual[2], expected[2], 0.5);
        });
    }

    #[test]
    fn drag_gizmo_move_transform_trigger_source_sets_dirty_flag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.config.snap_to_grid = false;
            state.editor.ui.mode = EditorMode::Move;
            // Create a regular block that will be the source of a transform trigger
            state.editor.objects = vec![test_block()];
            // Add a transform trigger that targets object 0
            state.editor.set_triggers(vec![TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 1.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![0],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [5.0, 0.0, 0.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            }]);

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

            state.editor.runtime.dirty = EditorDirtyFlags::default();
            assert!(state.editor.drag_gizmo_from_screen(
                target.x as f64,
                target.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert!(
                state.editor.runtime.dirty.rebuild_transform_trigger_markers,
                "Moving a transform trigger source block should mark transform trigger markers dirty"
            );
        });
    }
}
