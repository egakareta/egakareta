/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_mesh::{GizmoAxis, GizmoDragKind};
use eg_types::{AppPhase, TransformVisibility};
use glam::{Vec2, Vec3};

use super::super::{EditorDragBlockStart, EditorGizmoDrag};

const GIZMO_MOVE_PICK_RADIUS_PIXELS: f32 = 40.0;
const GIZMO_RESIZE_PICK_RADIUS_PIXELS: f32 = 32.0;
const GIZMO_ROTATE_PICK_RADIUS_PIXELS: f32 = 18.0;

super::super::editor_context_methods! {
    fn selected_gizmo_rotation(&self) -> glam::Quat {
        let rotation = self
            .selected_indices_normalized()
            .first()
            .and_then(|&index| self.objects.get(index))
            .map(|obj| obj.rotation)
            .or_else(|| {
                self.selected_transform_trigger_target()
                    .map(|(_, target)| target.rotation)
            })
            .unwrap_or([0.0, 0.0, 0.0]);

        glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            rotation[0].to_radians(),
            rotation[1].to_radians(),
            rotation[2].to_radians(),
        )
    }

    pub(crate) fn visible_gizmo_handles(&self) -> TransformVisibility {
        let indices = self.selected_indices_normalized();
        if indices.is_empty() {
            return if self.selected_transform_trigger_target().is_some() {
                TransformVisibility::all()
            } else {
                TransformVisibility::none()
            };
        }

        indices
            .into_iter()
            .filter_map(|index| self.objects.get(index))
            .map(|object| object.transform_field_visibility())
            .fold(TransformVisibility::all(), |visibility, next| {
                visibility.intersection(next)
            })
    }

    fn drag_gizmo_rotation(
        drag: &EditorGizmoDrag,
        trigger_start: Option<&EditorDragBlockStart>,
    ) -> glam::Quat {
        let rotation_source = trigger_start.or_else(|| drag.start_blocks.first());
        rotation_source
            .map(|b| {
                glam::Quat::from_euler(
                    glam::EulerRot::XYZ,
                    b.rotation[0].to_radians(),
                    b.rotation[1].to_radians(),
                    b.rotation[2].to_radians(),
                )
            })
            .unwrap_or_default()
    }

    fn rotation_from_degrees(rotation: [f32; 3]) -> glam::Quat {
        glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            rotation[0].to_radians(),
            rotation[1].to_radians(),
            rotation[2].to_radians(),
        )
    }

    fn resize_from_drag_start(
        start: EditorDragBlockStart,
        axis: GizmoAxis,
        world_delta: f32,
        snap_enabled: bool,
        snap_step: f32,
        min_size: f32,
    ) -> ([f32; 3], [f32; 3]) {
        let (axis_index, local_axis, handle_sign) = match axis {
            GizmoAxis::X => (0, Vec3::X, 1.0),
            GizmoAxis::Y => (1, Vec3::Y, 1.0),
            GizmoAxis::Z => (2, Vec3::Z, 1.0),
            GizmoAxis::XNeg => (0, Vec3::X, -1.0),
            GizmoAxis::YNeg => (1, Vec3::Y, -1.0),
            GizmoAxis::ZNeg => (2, Vec3::Z, -1.0),
        };

        let mut next_size = start.size;
        let mut axis_size = start.size[axis_index] + world_delta * handle_sign;
        if snap_enabled {
            axis_size = (axis_size / snap_step).round() * snap_step;
        }
        next_size[axis_index] = axis_size.max(min_size);

        let rotation = Self::rotation_from_degrees(start.rotation);
        let rotated_axis = rotation * local_axis;
        let start_position = Vec3::from(start.position);
        let start_size = Vec3::from(start.size);
        let start_center = start_position + start_size * 0.5;
        let opposite_face_sign = -handle_sign;
        let anchored_face =
            start_center + rotated_axis * (opposite_face_sign * start.size[axis_index] * 0.5);
        let next_center =
            anchored_face - rotated_axis * (opposite_face_sign * next_size[axis_index] * 0.5);
        let next_position = next_center - Vec3::from(next_size) * 0.5;

        (next_position.to_array(), next_size)
    }

    fn gizmo_drag_trigger_start(&self, drag: &EditorGizmoDrag) -> Option<EditorDragBlockStart> {
        if !self.selected_indices_normalized().is_empty() {
            return None;
        }
        let (trigger_index, _) = self.selected_transform_trigger_target()?;
        let start = drag.start_blocks.first().copied()?;
        (start.index == trigger_index).then_some(start)
    }

    fn move_delta_for_drag(
        axis_dir: Vec3,
        world_delta: f32,
        snap_enabled: bool,
        snap_step: f32,
        min_start_y: f32,
    ) -> Vec3 {
        let mut scalar_delta = world_delta;
        if snap_enabled {
            scalar_delta = (scalar_delta / snap_step).round() * snap_step;
        }

        let mut delta = axis_dir * scalar_delta;
        if delta.y < 0.0 && min_start_y + delta.y < 0.0 && axis_dir.y.abs() > f32::EPSILON {
            scalar_delta = -min_start_y / axis_dir.y;
            delta = axis_dir * scalar_delta;
        }

        delta
    }

    pub fn drag_gizmo(&mut self, x: f64, y: f64, viewport: Vec2) -> bool {
        self.input.pointer_screen = Some([x, y]);

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
        let gizmo_rotation = Self::drag_gizmo_rotation(&drag, trigger_start.as_ref());

        axis_dir = gizmo_rotation * axis_dir;

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
                let min_start_y = if let Some(start) = trigger_start {
                    start.position[1]
                } else {
                    drag.start_blocks
                        .iter()
                        .map(|b| b.position[1])
                        .fold(f32::INFINITY, f32::min)
                };
                let delta = Self::move_delta_for_drag(
                    axis_dir,
                    world_delta,
                    snap_enabled,
                    snap_step,
                    min_start_y,
                );
                let mut first_cursor: Option<[f32; 3]> = None;
                if let Some(start) = trigger_start {
                    let mut next = start.position;
                    next[0] += delta.x;
                    next[1] += delta.y;
                    next[2] += delta.z;
                    next[1] = next[1].max(0.0);
                    if self.set_transform_trigger_target(
                        start.index,
                        next,
                        start.size,
                        start.rotation,
                    ) {
                        first_cursor = Some(next);
                    }
                } else {
                    for block in &drag.start_blocks {
                        if let Some(obj) = self.objects.get_mut(block.index) {
                            let mut next = block.position;
                            next[0] += delta.x;
                            next[1] += delta.y;
                            next[2] += delta.z;
                            next[1] = next[1].max(0.0);
                            obj.position = next;
                            if first_cursor.is_none() {
                                first_cursor = Some(next);
                            }
                        }
                    }
                }
                if let Some(next_position) = first_cursor {
                    self.input.cursor = [
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
                    let (next_position, next_size) = Self::resize_from_drag_start(
                        start,
                        drag.axis,
                        world_delta,
                        snap_enabled,
                        snap_step,
                        min_size,
                    );
                    self.set_transform_trigger_target(
                        start.index,
                        next_position,
                        next_size,
                        start.rotation,
                    );
                } else {
                    for block in &drag.start_blocks {
                        if let Some(obj) = self.objects.get_mut(block.index) {
                            let (next_position, next_size) = Self::resize_from_drag_start(
                                *block,
                                drag.axis,
                                world_delta,
                                snap_enabled,
                                snap_step,
                                min_size,
                            );
                            obj.position = next_position;
                            obj.size = next_size;
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
                        block.rotation[0].to_radians(),
                        block.rotation[1].to_radians(),
                        block.rotation[2].to_radians(),
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
                            obj.rotation = apply_rotation(*block);
                        }
                    }
                }
            }
        }
        self.runtime.interaction.gizmo_drag = Some(drag);
        true
    }

    pub fn pick_gizmo_handle(
        &self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> Option<(GizmoDragKind, GizmoAxis)> {
        let mode = self.input.mode;
        let visible_handles = self.visible_gizmo_handles();
        let allow_move = mode.shows_move_gizmo() && visible_handles.position;
        let allow_scale = mode.shows_scale_gizmo() && visible_handles.size;
        let allow_rotate = mode.shows_rotate_gizmo() && visible_handles.rotation;
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
        let rotation = self.selected_gizmo_rotation();

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
                let dir = rotation * dir;
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
            for &(axis, dir, distance) in &[
                (
                    GizmoAxis::X,
                    Vec3::X,
                    bounds_size[0] * 0.5 + resize_offsets[0],
                ),
                (
                    GizmoAxis::Y,
                    Vec3::Y,
                    bounds_size[1] * 0.5 + resize_offsets[1],
                ),
                (
                    GizmoAxis::Z,
                    Vec3::Z,
                    bounds_size[2] * 0.5 + resize_offsets[2],
                ),
                (
                    GizmoAxis::XNeg,
                    Vec3::X,
                    -(bounds_size[0] * 0.5 + resize_offsets[0]),
                ),
                (
                    GizmoAxis::YNeg,
                    Vec3::Y,
                    -(bounds_size[1] * 0.5 + resize_offsets[1]),
                ),
                (
                    GizmoAxis::ZNeg,
                    Vec3::Z,
                    -(bounds_size[2] * 0.5 + resize_offsets[2]),
                ),
            ] {
                candidates.push((
                    GizmoDragKind::Resize,
                    axis,
                    center + (rotation * dir) * distance,
                    GIZMO_RESIZE_PICK_RADIUS_PIXELS,
                ));
            }
        }

        if allow_rotate {
            let rotation = self
                .selected_indices_normalized()
                .first()
                .and_then(|&index| self.objects.get(index))
                .map(|obj| obj.rotation)
                .or_else(|| {
                    self.selected_transform_trigger_target()
                        .map(|(_, target)| target.rotation)
                })
                .unwrap_or([0.0, 0.0, 0.0]);
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                rotation[0].to_radians(),
                rotation[1].to_radians(),
                rotation[2].to_radians(),
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

    pub fn pixels_to_world_along_axis(
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

    pub fn gizmo_axis_lengths_world(
        &self,
        center: Vec3,
        screen_size: f32,
        viewport_size: Vec2,
    ) -> [f32; 3] {
        let scale = self.gizmo_axis_width_world(center, screen_size, viewport_size);
        [scale, scale, scale]
    }

    pub fn gizmo_axis_width_world(
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

    pub fn begin_gizmo_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if !self.input.mode.shows_gizmo() {
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
                rotation: target.rotation,
            });
        } else {
            for index in indices {
                if let Some(obj) = self.objects.get(index) {
                    start_blocks.push(EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                        rotation: obj.rotation,
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

    pub fn drag_gizmo_from_screen(
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
            true
        } else {
            false
        }
    }

    pub fn begin_gizmo_drag_ext(
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

#[cfg(test)]
mod tests {
    use eg_mesh::{GizmoAxis, GizmoDragKind};
    use eg_types::{
        AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection, EditorMode,
        LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
    };
    use glam::{Vec2, Vec3};

    use super::super::super::{EditorDragBlockStart, EditorGizmoDrag};
    use crate::state::{
        EditorHistoryRes, collect_editor_objects_from_world, replace_editor_objects,
        test_support::TestApp, with_editor_interaction_context,
    };

    fn start_block_for_index(index: usize, block: &LevelObject) -> EditorDragBlockStart {
        EditorDragBlockStart {
            index,
            position: block.position,
            size: block.size,
            rotation: block.rotation,
        }
    }

    fn local_face_center(block: &LevelObject, axis: Vec3, face_sign: f32) -> Vec3 {
        let rotation = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            block.rotation[0].to_radians(),
            block.rotation[1].to_radians(),
            block.rotation[2].to_radians(),
        );
        let position = Vec3::from(block.position);
        let size = Vec3::from(block.size);
        let center = position + size * 0.5;
        let axis_size = axis.abs().dot(size);

        center + (rotation * axis) * face_sign * axis_size * 0.5
    }

    #[test]
    fn pick_gizmo_handle_returns_none_when_mode_has_no_gizmo() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);
        let viewport = Vec2::new(1280.0, 720.0);
        let hit = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Place;
            editor.input.selected_block_index = Some(0);
            editor.pick_gizmo_handle(640.0, 360.0, viewport)
        });
        assert!(hit.is_none());
    }

    #[test]
    fn pick_gizmo_handle_honors_selected_block_field_visibility() {
        let mut app = TestApp::minimal();
        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);

        let camera_pose = TimedTrigger {
            time_seconds: 0.0,
            duration_seconds: 1.0,
            easing: EasingDirection::Linear,
            target: TriggerTarget::Camera,
            transform: false,
            camera: Some(CameraTrigger {
                mode: CameraMode::Static,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [0.0, 0.0, 0.0],
                yaw: 0.0,
                pitch: 0.0,
                ..CameraTrigger::default()
            }),
        };
        let camera_follow = TimedTrigger {
            camera: Some(CameraTrigger {
                mode: CameraMode::Follow,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                distance: 8.0,
                yaw: 0.0,
                pitch: 0.0,
                ..CameraTrigger::default()
            }),
            ..camera_pose.clone()
        };

        let mut selected = LevelObject::new(CAMERA_TRIGGER_BLOCK_ID);
        selected.trigger = Some(camera_pose);
        replace_editor_objects(app.world_mut(), vec![selected]);
        let (scale_hit, move_hit, follow_hits) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.input.selected_block_index = Some(0);
                editor.input.selected_block_indices = vec![0];
                let center_screen = editor
                    .world_to_screen_v(center, viewport)
                    .expect("center projects");

                editor.input.mode = EditorMode::Scale;
                let scale_lengths = editor.gizmo_axis_lengths_world(center, 9.0, viewport);
                let scale_world = Vec3::new(1.0 + scale_lengths[0], center.y, center.z);
                let scale_screen = editor
                    .world_to_screen_v(scale_world, viewport)
                    .expect("scale handle projects");
                let scale_hit = editor.pick_gizmo_handle(
                    scale_screen.x as f64,
                    scale_screen.y as f64,
                    viewport,
                );

                editor.input.mode = EditorMode::Move;
                let move_hit = editor.pick_gizmo_handle(
                    center_screen.x as f64,
                    center_screen.y as f64,
                    viewport,
                );

                editor.objects[0].trigger = Some(camera_follow);
                let follow_hits =
                    [EditorMode::Move, EditorMode::Scale, EditorMode::Rotate].map(|mode| {
                        editor.input.mode = mode;
                        editor.pick_gizmo_handle(
                            center_screen.x as f64,
                            center_screen.y as f64,
                            viewport,
                        )
                    });
                (scale_hit, move_hit, follow_hits)
            });

        assert_eq!(scale_hit, None);
        assert!(matches!(move_hit, Some((GizmoDragKind::Move, _))));
        assert!(follow_hits.into_iter().all(|hit| hit.is_none()));
    }

    #[test]
    fn drag_gizmo_move_updates_object_position_along_axis() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let moved = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 40.0;

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(target.x as f64, target.y as f64, viewport)
        });

        assert!(moved);
        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert!(object.position[0] > 0.0);
        approx::assert_abs_diff_eq!(object.position[1], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(object.position[2], 0.0, epsilon = 1e-4);
    }

    #[test]
    fn drag_gizmo_move_uses_rotated_axis() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            rotation: [0.0, 90.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let moved = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + axis, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 40.0;

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(target.x as f64, target.y as f64, viewport)
        });

        assert!(moved);
        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        approx::assert_abs_diff_eq!(object.position[0], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(object.position[1], 0.0, epsilon = 1e-4);
        assert!(object.position[2] < -0.1);
    }

    #[test]
    fn drag_gizmo_move_snap_preserves_rotated_axis() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            rotation: [0.0, 45.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let moved = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = true;
            editor.config.snap_step = 1.0;
            let axis = glam::Quat::from_rotation_y(45.0_f32.to_radians()) * Vec3::X;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + axis, viewport)
                .expect("axis projects");

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(axis_screen.x as f64, axis_screen.y as f64, viewport)
        });

        assert!(moved);
        let position = collect_editor_objects_from_world(app.world_mut())[0].position;
        approx::assert_abs_diff_eq!(position[0], std::f32::consts::FRAC_1_SQRT_2, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(position[1], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(
            position[2],
            -std::f32::consts::FRAC_1_SQRT_2,
            epsilon = 1e-4
        );
    }

    #[test]
    fn drag_gizmo_move_updates_selected_transform_trigger_target() {
        let mut app = TestApp::minimal();
        let trigger = LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 1.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: vec!["object_0_0_0".to_string()],
                },
                transform: true,
                camera: None,
            }),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        };
        replace_editor_objects(app.world_mut(), vec![trigger]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let moved = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            editor.input.mode = EditorMode::Move;
            editor.set_trigger_selected(Some(0));
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let target = editor
                .selected_transform_trigger_target()
                .expect("selected trigger target")
                .1;
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks: vec![start_block_for_index(0, &target)],
            });

            editor.drag_gizmo(axis_screen.x as f64, axis_screen.y as f64, viewport)
        });

        assert!(moved);
        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert!(
            object.position[0] > 0.9,
            "expected trigger target to move on X"
        );
    }

    #[test]
    fn drag_gizmo_resize_negative_axis_preserves_opposite_edge() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let resized = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 20.0;

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::XNeg,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(target.x as f64, target.y as f64, viewport)
        });

        assert!(resized);
        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert!(object.size[0] >= 0.25);
        approx::assert_abs_diff_eq!(object.position[0] + object.size[0], 1.0, epsilon = 1e-3);
    }

    #[test]
    fn drag_gizmo_resize_rotated_positive_axis_preserves_opposite_face() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            position: [2.0, 1.0, 3.0],
            size: [2.0, 1.0, 1.0],
            rotation: [0.0, 90.0, 0.0],
            ..LevelObject::default()
        };
        let anchored_face_before = local_face_center(&block, Vec3::X, -1.0);
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(3.0, 1.5, 3.5);
        let resized = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + axis, viewport)
                .expect("axis projects");

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(axis_screen.x as f64, axis_screen.y as f64, viewport)
        });

        assert!(resized);

        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert!(object.size[0] > 2.9);
        let anchored_face_after = local_face_center(object, Vec3::X, -1.0);
        approx::assert_abs_diff_eq!(
            anchored_face_after.x,
            anchored_face_before.x,
            epsilon = 1e-3
        );
        approx::assert_abs_diff_eq!(
            anchored_face_after.y,
            anchored_face_before.y,
            epsilon = 1e-3
        );
        approx::assert_abs_diff_eq!(
            anchored_face_after.z,
            anchored_face_before.z,
            epsilon = 1e-3
        );
    }

    #[test]
    fn drag_gizmo_resize_rotated_negative_axis_preserves_opposite_face() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            position: [2.0, 1.0, 3.0],
            size: [2.0, 1.0, 1.0],
            rotation: [0.0, 90.0, 0.0],
            ..LevelObject::default()
        };
        let anchored_face_before = local_face_center(&block, Vec3::X, 1.0);
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(3.0, 1.5, 3.5);
        let resized = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + axis, viewport)
                .expect("axis projects");

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::XNeg,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo(axis_screen.x as f64, axis_screen.y as f64, viewport)
        });

        assert!(resized);

        let object = &collect_editor_objects_from_world(app.world_mut())[0];
        assert!(object.size[0] < 1.1);
        let anchored_face_after = local_face_center(object, Vec3::X, 1.0);
        approx::assert_abs_diff_eq!(
            anchored_face_after.x,
            anchored_face_before.x,
            epsilon = 1e-3
        );
        approx::assert_abs_diff_eq!(
            anchored_face_after.y,
            anchored_face_before.y,
            epsilon = 1e-3
        );
        approx::assert_abs_diff_eq!(
            anchored_face_after.z,
            anchored_face_before.z,
            epsilon = 1e-3
        );
    }

    #[test]
    fn drag_gizmo_rotate_changes_selected_block_rotation() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let rotated = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = false;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start = origin_screen + Vec2::new(30.0, 0.0);
            let target = origin_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start.x as f64, start.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            (
                editor.drag_gizmo(target.x as f64, target.y as f64, viewport),
                editor.objects[0].rotation,
            )
        });

        assert!(rotated.0);
        assert!(rotated.1[1].abs() > 1.0);
    }

    #[test]
    fn helper_conversions_handle_valid_and_invalid_viewports() {
        let mut app = TestApp::minimal();
        let center = Vec3::new(0.5, 0.5, 0.5);
        let valid = with_editor_interaction_context(app.world_mut(), |editor| {
            let valid =
                editor.pixels_to_world_along_axis(center, Vec3::X, 64.0, Vec2::new(1280.0, 720.0));
            let _invalid =
                editor.pixels_to_world_along_axis(center, Vec3::X, 64.0, Vec2::new(0.0, 720.0));
            let _ = editor.gizmo_axis_width_world(center, 100.0, Vec2::new(0.0, 0.0));
            valid
        });
        assert!(valid.is_some());
        assert!(valid.unwrap_or_default() > 0.0);
    }

    #[test]
    fn pick_and_begin_gizmo_drag_cover_move_scale_rotate_paths() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let (move_hit, move_started, scale_hit, rotate_hit) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.input.selected_block_index = Some(0);
                editor.input.selected_block_indices = vec![0];

                editor.input.mode = EditorMode::Move;
                let center_screen = editor
                    .world_to_screen_v(center, viewport)
                    .expect("center projects");
                let move_hit = editor.pick_gizmo_handle(
                    center_screen.x as f64,
                    center_screen.y as f64,
                    viewport,
                );
                let move_started = editor.begin_gizmo_drag(
                    center_screen.x as f64,
                    center_screen.y as f64,
                    viewport,
                );

                editor.runtime.interaction.gizmo_drag = None;
                editor.input.mode = EditorMode::Scale;
                let scale_lengths = editor.gizmo_axis_lengths_world(center, 9.0, viewport);
                let scale_world = Vec3::new(1.0 + scale_lengths[0], center.y, center.z);
                let scale_screen = editor
                    .world_to_screen_v(scale_world, viewport)
                    .expect("scale handle projects");
                let scale_hit = editor.pick_gizmo_handle(
                    scale_screen.x as f64,
                    scale_screen.y as f64,
                    viewport,
                );

                editor.input.mode = EditorMode::Rotate;
                let ring_radius =
                    editor.gizmo_axis_lengths_world(center, 100.0, viewport)[0] * 0.78;
                let rotate_world = center + Vec3::new(ring_radius, 0.0, 0.0);
                let rotate_screen = editor
                    .world_to_screen_v(rotate_world, viewport)
                    .expect("rotate ring projects");
                let rotate_hit = editor.pick_gizmo_handle(
                    rotate_screen.x as f64,
                    rotate_screen.y as f64,
                    viewport,
                );
                (move_hit, move_started, scale_hit, rotate_hit)
            });

        assert!(matches!(move_hit, Some((GizmoDragKind::Move, _))));
        assert!(move_started);
        assert!(matches!(scale_hit, Some((GizmoDragKind::Resize, _))));
        assert!(matches!(rotate_hit, Some((GizmoDragKind::Rotate, _))));
    }

    #[test]
    fn drag_gizmo_from_screen_respects_phase_and_updates_geometry() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let (wrong_phase, moved, resized) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.config.snap_to_grid = false;
                let origin_screen = editor
                    .world_to_screen_v(center, viewport)
                    .expect("center projects");
                let axis_screen = editor
                    .world_to_screen_v(center + Vec3::X, viewport)
                    .expect("axis projects");
                let axis_dir = (axis_screen - origin_screen).normalize();
                let target = origin_screen + axis_dir * 24.0;

                let block = editor.objects[0].clone();
                editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                    axis: GizmoAxis::X,
                    kind: GizmoDragKind::Move,
                    start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                    start_center_screen: [origin_screen.x, origin_screen.y],
                    start_center_world: center.to_array(),
                    start_blocks: vec![start_block_for_index(0, &block)],
                });

                let wrong_phase = editor.drag_gizmo_from_screen(
                    target.x as f64,
                    target.y as f64,
                    viewport,
                    AppPhase::Menu,
                );
                let moved = editor.drag_gizmo_from_screen(
                    target.x as f64,
                    target.y as f64,
                    viewport,
                    AppPhase::Editor,
                );

                let block = editor.objects[0].clone();
                editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                    axis: GizmoAxis::X,
                    kind: GizmoDragKind::Resize,
                    start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                    start_center_screen: [origin_screen.x, origin_screen.y],
                    start_center_world: center.to_array(),
                    start_blocks: vec![start_block_for_index(0, &block)],
                });
                let resized = editor.drag_gizmo_from_screen(
                    target.x as f64,
                    target.y as f64,
                    viewport,
                    AppPhase::Editor,
                );
                (wrong_phase, moved, resized)
            });

        assert!(!wrong_phase);
        assert!(moved);
        assert!(resized);
        assert!(collect_editor_objects_from_world(app.world_mut())[0].size[0] > 1.0);
    }

    #[test]
    fn begin_gizmo_drag_ext_follows_phase_guards_and_records_history() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let (wrong_phase, started, history_before, history_after, lengths, width) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.input.selected_block_index = Some(0);
                editor.input.selected_block_indices = vec![0];
                editor.input.mode = EditorMode::Move;
                let center_screen = editor
                    .world_to_screen_v(center, viewport)
                    .expect("center projects");

                let wrong_phase = editor.begin_gizmo_drag_ext(
                    center_screen.x as f64,
                    center_screen.y as f64,
                    viewport,
                    AppPhase::Menu,
                );
                let history_before = editor.history.undo.len();
                let started = editor.begin_gizmo_drag_ext(
                    center_screen.x as f64,
                    center_screen.y as f64,
                    viewport,
                    AppPhase::Editor,
                );
                let history_after = editor.history.undo.len();
                let lengths = editor.gizmo_axis_lengths_world(center, 100.0, viewport);
                let width = editor.gizmo_axis_width_world(center, 100.0, viewport);
                (
                    wrong_phase,
                    started,
                    history_before,
                    history_after,
                    lengths,
                    width,
                )
            });

        assert!(!wrong_phase);
        assert!(started);
        assert_eq!(history_after, history_before + 1);
        assert!(lengths[0] > 0.0 && lengths[1] > 0.0 && lengths[2] > 0.0);
        assert!(width > 0.0);
        assert_eq!(
            app.world().resource::<EditorHistoryRes>().0.undo.len(),
            history_after
        );
    }
    #[test]
    fn drag_gizmo_y_prevents_compression_when_moving_below_ground() {
        let mut app = TestApp::minimal();
        let viewport = Vec2::new(1280.0, 720.0);

        // Two blocks at different heights.
        let low = LevelObject {
            position: [0.0, 0.0, 0.0],
            ..LevelObject::default()
        };
        let high = LevelObject {
            position: [0.0, 3.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![low, high]);

        let center = Vec3::new(0.5, 2.0, 0.5);
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.mode = EditorMode::Select;
            editor.replace_block_selection(vec![0, 1]);
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");

            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
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
                        rotation: [0.0, 0.0, 0.0],
                    },
                    EditorDragBlockStart {
                        index: 1,
                        position: [0.0, 3.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                        rotation: [0.0, 0.0, 0.0],
                    },
                ],
            });

            let far_screen = Vec2::new(center_screen.x, center_screen.y - 2000.0);
            assert!(editor.drag_gizmo(far_screen.x as f64, far_screen.y as f64, viewport));
        });

        let objects = collect_editor_objects_from_world(app.world_mut());
        let low_y = objects[0].position[1];
        let high_y = objects[1].position[1];
        assert!(low_y >= -1e-4, "low block y={low_y} should be >= 0");
        let relative_gap = high_y - low_y;
        assert!(
            relative_gap >= 2.9,
            "relative gap {relative_gap} should be preserved (~3.0)"
        );
    }

    #[test]
    fn drag_gizmo_rotate_identity_block_produces_single_axis_change() {
        let mut app = TestApp::minimal();
        replace_editor_objects(app.world_mut(), vec![LevelObject::default()]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let rotation = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = false;
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            assert!(editor.drag_gizmo(current_mouse.x as f64, current_mouse.y as f64, viewport,));
            editor.objects[0].rotation
        });

        assert!(
            rotation[1].abs() > 1.0,
            "Y rotation should be non-trivial, got {rotation:?}"
        );
        approx::assert_abs_diff_eq!(rotation[0], 0.0, epsilon = 1.0);
        approx::assert_abs_diff_eq!(rotation[2], 0.0, epsilon = 1.0);
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].rotation,
            rotation
        );
    }

    #[test]
    fn drag_gizmo_rotate_around_x_with_y90_uses_world_axis() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            rotation: [0.0, 90.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let actual = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = false;
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
            assert!(editor.drag_gizmo(current_mouse.x as f64, current_mouse.y as f64, viewport,));
            editor.objects[0].rotation
        });

        assert!(actual != [0.0, 90.0, 0.0]);
        assert!(actual[0].abs() > 1.0 || actual[2].abs() > 1.0);
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].rotation,
            actual
        );
    }

    #[test]
    fn drag_gizmo_rotate_around_y_with_xz_rotation_uses_world_axis() {
        let mut app = TestApp::minimal();
        // Use X+Z rotation so the world Y axis actually diverges from local Y.
        // (Pure X rotation doesn't change the Y axis direction.)
        let block = LevelObject {
            rotation: [45.0, 0.0, 45.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let actual = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = false;
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Y,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
            assert!(editor.drag_gizmo(current_mouse.x as f64, current_mouse.y as f64, viewport,));
            editor.objects[0].rotation
        });

        assert!(actual != [45.0, 0.0, 45.0]);
        assert!(actual[0].abs() > 1.0 || actual[1].abs() > 1.0 || actual[2].abs() > 1.0);
    }

    #[test]
    fn drag_gizmo_rotate_around_z_with_combined_rotation_uses_world_axis() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            rotation: [30.0, 20.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let actual = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = false;
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::Z,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
            assert!(editor.drag_gizmo(current_mouse.x as f64, current_mouse.y as f64, viewport,));
            editor.objects[0].rotation
        });

        assert!(actual != [30.0, 20.0, 0.0]);
        assert!(actual[0].abs() > 1.0 || actual[1].abs() > 1.0 || actual[2].abs() > 1.0);
    }

    #[test]
    fn drag_gizmo_rotate_with_snap_rounds_delta_before_composition() {
        let mut app = TestApp::minimal();
        let block = LevelObject {
            rotation: [0.0, 45.0, 0.0],
            ..LevelObject::default()
        };
        replace_editor_objects(app.world_mut(), vec![block]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let actual = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_rotation = true;
            editor.config.snap_rotation_step_degrees = 15.0;
            let center_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let start_mouse = center_screen + Vec2::new(30.0, 0.0);
            let current_mouse = center_screen + Vec2::new(0.0, 30.0);

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Rotate,
                start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
            assert!(editor.drag_gizmo(current_mouse.x as f64, current_mouse.y as f64, viewport,));
            editor.objects[0].rotation
        });

        assert!(actual != [0.0, 45.0, 0.0]);
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].rotation,
            actual
        );
    }

    #[test]
    fn drag_gizmo_move_transform_trigger_source_updates_geometry() {
        let mut app = TestApp::minimal();
        let source = LevelObject {
            id: "object_0_0_0".to_string(),
            ..LevelObject::default()
        };
        let trigger = LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 1.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: vec!["object_0_0_0".to_string()],
                },
                transform: true,
                camera: None,
            }),
            position: [5.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        };
        replace_editor_objects(app.world_mut(), vec![source, trigger]);

        let viewport = Vec2::new(1280.0, 720.0);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let moved = with_editor_interaction_context(app.world_mut(), |editor| {
            editor.config.snap_to_grid = false;
            editor.input.mode = EditorMode::Move;
            let origin_screen = editor
                .world_to_screen_v(center, viewport)
                .expect("center projects");
            let axis_screen = editor
                .world_to_screen_v(center + Vec3::X, viewport)
                .expect("axis projects");
            let axis_dir = (axis_screen - origin_screen).normalize();
            let target = origin_screen + axis_dir * 40.0;

            let block = editor.objects[0].clone();
            editor.runtime.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });

            editor.drag_gizmo_from_screen(
                target.x as f64,
                target.y as f64,
                viewport,
                AppPhase::Editor,
            )
        });

        assert!(moved);
        let objects = collect_editor_objects_from_world(app.world_mut());
        assert!(objects[0].position[0] > 0.0);
        assert!(objects[1].trigger.is_some());
    }
}
