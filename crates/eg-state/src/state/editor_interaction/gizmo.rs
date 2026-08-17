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
    use bevy::state::prelude::State;
    use eg_mesh::{GizmoAxis, GizmoDragKind};
    use eg_types::{
        AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection, EditorMode,
        LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
    };
    use glam::{Vec2, Vec3};

    use super::super::super::{EditorDragBlockStart, EditorGizmoDrag};
    use crate::{
        events::pointer::{PointerMoved, PrimaryClick},
        state::{
            EditorCameraState, EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorRuntimeRes,
            EditorTriggerRes, RenderConfigRes, collect_editor_objects_from_world,
            editor_camera_view_proj, project_world_to_screen, replace_editor_objects,
            test_support::TestApp,
        },
    };

    fn gizmo_test_app(objects: Vec<LevelObject>, mode: EditorMode) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = mode;
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }
        {
            let mut render = app.world_mut().resource_mut::<RenderConfigRes>();
            render.width = 1280;
            render.height = 720;
        }
        app
    }

    fn screen_for_world(world: Vec3) -> Vec2 {
        let viewport = Vec2::new(1280.0, 720.0);
        project_world_to_screen(
            editor_camera_view_proj(&EditorCameraState::default(), viewport),
            world,
            viewport,
        )
        .expect("world position should project to screen")
    }

    fn gizmo_axis_length_for_screen(center: Vec3, target_pixels: f32) -> f32 {
        [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .map(|axis| screen_for_world(center + axis).distance(screen_for_world(center)))
            .filter(|pixels_per_world| *pixels_per_world > f32::EPSILON)
            .map(|pixels_per_world| target_pixels / pixels_per_world)
            .min_by(f32::total_cmp)
            .expect("default editor camera projects every gizmo axis")
    }

    fn start_block_for_index(index: usize, block: &LevelObject) -> EditorDragBlockStart {
        EditorDragBlockStart {
            index,
            position: block.position,
            size: block.size,
            rotation: block.rotation,
        }
    }

    fn rotate_block_with_gizmo(
        block: LevelObject,
        axis: GizmoAxis,
        snap_rotation: bool,
        snap_rotation_step_degrees: f32,
    ) -> [f32; 3] {
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Rotate);
        {
            let mut config = app.world_mut().resource_mut::<EditorConfigRes>();
            config.0.snap_rotation = snap_rotation;
            config.0.snap_rotation_step_degrees = snap_rotation_step_degrees;
        }

        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);
        let start_mouse = center_screen + Vec2::new(30.0, 0.0);
        let current_mouse = center_screen + Vec2::new(0.0, 30.0);
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis,
            kind: GizmoDragKind::Rotate,
            start_mouse: [start_mouse.x as f64, start_mouse.y as f64],
            start_center_screen: [center_screen.x, center_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: current_mouse.x as f64,
            y: current_mouse.y as f64,
        });
        app.tick();

        collect_editor_objects_from_world(app.world_mut())[0].rotation
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
        let mut app = gizmo_test_app(vec![LevelObject::default()], EditorMode::Place);
        app.world_mut()
            .write_message(PointerMoved { x: 640.0, y: 360.0 });
        app.tick();

        assert!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .hovered_gizmo
                .is_none()
        );
    }

    #[test]
    fn pick_gizmo_handle_honors_selected_block_field_visibility() {
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

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
        let mut app = gizmo_test_app(vec![selected.clone()], EditorMode::Scale);
        let scale_world = Vec3::new(
            1.0 + gizmo_axis_length_for_screen(center, 9.0),
            center.y,
            center.z,
        );
        let scale_screen = screen_for_world(scale_world);
        app.world_mut().write_message(PointerMoved {
            x: scale_screen.x as f64,
            y: scale_screen.y as f64,
        });
        app.tick();
        assert!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .hovered_gizmo
                .is_none(),
            "camera pose blocks hide size fields, so resize gizmos should be hidden"
        );

        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Move;
        app.world_mut().write_message(PointerMoved {
            x: center_screen.x as f64,
            y: center_screen.y as f64,
        });
        app.tick();
        assert!(matches!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .hovered_gizmo,
            Some((GizmoDragKind::Move, _))
        ));

        selected.trigger = Some(camera_follow);
        replace_editor_objects(app.world_mut(), vec![selected]);
        for mode in [EditorMode::Move, EditorMode::Scale, EditorMode::Rotate] {
            app.world_mut().resource_mut::<EditorInputRes>().0.mode = mode;
            app.world_mut().write_message(PointerMoved {
                x: center_screen.x as f64,
                y: center_screen.y as f64,
            });
            app.tick();
            assert!(
                app.world()
                    .resource::<EditorRuntimeRes>()
                    .0
                    .interaction
                    .hovered_gizmo
                    .is_none(),
                "camera follow blocks hide transform fields, so gizmos should be hidden"
            );
        }
    }

    #[test]
    fn drag_gizmo_move_updates_object_position_along_axis() {
        let mut app = gizmo_test_app(vec![LevelObject::default()], EditorMode::Move);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + Vec3::X);
        let axis_dir = (axis_screen - origin_screen).normalize();
        let target = origin_screen + axis_dir * 40.0;

        {
            let block = LevelObject::default();
            let mut runtime = app.world_mut().resource_mut::<EditorRuntimeRes>();
            runtime.0.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Move,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
        }
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        assert!(objects[0].position[0] > 0.0);
        approx::assert_abs_diff_eq!(objects[0].position[1], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(objects[0].position[2], 0.0, epsilon = 1e-4);
    }

    #[test]
    fn drag_gizmo_move_uses_rotated_axis() {
        let mut block = LevelObject::default();
        block.rotation = [0.0, 90.0, 0.0];
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Move);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;

        let center = Vec3::new(0.5, 0.5, 0.5);
        let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + axis);
        let axis_dir = (axis_screen - origin_screen).normalize();
        let target = origin_screen + axis_dir * 40.0;

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Move,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();

        let position = collect_editor_objects_from_world(app.world_mut())[0].position;
        approx::assert_abs_diff_eq!(position[0], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(position[1], 0.0, epsilon = 1e-4);
        assert!(position[2] < -0.1);
    }

    #[test]
    fn drag_gizmo_move_snap_preserves_rotated_axis() {
        let mut block = LevelObject::default();
        block.rotation = [0.0, 45.0, 0.0];
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Move);
        {
            let mut config = app.world_mut().resource_mut::<EditorConfigRes>();
            config.0.snap_to_grid = true;
            config.0.snap_step = 1.0;
        }

        let center = Vec3::new(0.5, 0.5, 0.5);
        let axis = glam::Quat::from_rotation_y(45.0_f32.to_radians()) * Vec3::X;
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + axis);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Move,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: axis_screen.x as f64,
            y: axis_screen.y as f64,
        });
        app.tick();

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
        let mut app = gizmo_test_app(vec![trigger.clone()], EditorMode::Move);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = None;
            input.0.selected_block_indices.clear();
        }
        app.world_mut()
            .resource_mut::<EditorTriggerRes>()
            .0
            .selected_index = Some(0);

        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + Vec3::X);
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Move,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: [center.x, center.y, center.z],
            start_blocks: vec![start_block_for_index(0, &trigger)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: axis_screen.x as f64,
            y: axis_screen.y as f64,
        });
        app.tick();

        assert!(
            collect_editor_objects_from_world(app.world_mut())[0].position[0] > 0.9,
            "expected trigger target to move on X"
        );
    }

    #[test]
    fn drag_gizmo_resize_negative_axis_preserves_opposite_edge() {
        let block = LevelObject::default();
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Scale);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + Vec3::X);
        let axis_dir = (axis_screen - origin_screen).normalize();
        let target = origin_screen + axis_dir * 20.0;

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::XNeg,
            kind: GizmoDragKind::Resize,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let object = &objects[0];
        assert!(object.size[0] >= 0.25);
        approx::assert_abs_diff_eq!(object.position[0] + object.size[0], 1.0, epsilon = 1e-3);
    }

    #[test]
    fn drag_gizmo_resize_rotated_positive_axis_preserves_opposite_face() {
        let mut block = LevelObject::default();
        block.position = [2.0, 1.0, 3.0];
        block.size = [2.0, 1.0, 1.0];
        block.rotation = [0.0, 90.0, 0.0];
        let anchored_face_before = local_face_center(&block, Vec3::X, -1.0);
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Scale);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;

        let center = Vec3::new(3.0, 1.5, 3.5);
        let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + axis);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Resize,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: axis_screen.x as f64,
            y: axis_screen.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let object = &objects[0];
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
        let mut block = LevelObject::default();
        block.position = [2.0, 1.0, 3.0];
        block.size = [2.0, 1.0, 1.0];
        block.rotation = [0.0, 90.0, 0.0];
        let anchored_face_before = local_face_center(&block, Vec3::X, 1.0);
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Scale);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;

        let center = Vec3::new(3.0, 1.5, 3.5);
        let axis = glam::Quat::from_rotation_y(90.0_f32.to_radians()) * Vec3::X;
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + axis);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::XNeg,
            kind: GizmoDragKind::Resize,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: axis_screen.x as f64,
            y: axis_screen.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        let object = &objects[0];
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
        let block = LevelObject::default();
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Rotate);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_rotation = false;
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let start = origin_screen + Vec2::new(30.0, 0.0);
        let target = origin_screen + Vec2::new(0.0, 30.0);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
            axis: GizmoAxis::Y,
            kind: GizmoDragKind::Rotate,
            start_mouse: [start.x as f64, start.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();

        assert!(collect_editor_objects_from_world(app.world_mut())[0].rotation[1].abs() > 1.0);
    }

    #[test]
    fn pick_and_begin_gizmo_drag_cover_move_scale_rotate_paths() {
        let mut app = gizmo_test_app(vec![LevelObject::default()], EditorMode::Move);
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        app.world_mut().write_message(PrimaryClick {
            x: center_screen.x as f64,
            y: center_screen.y as f64,
        });
        app.tick();
        assert!(matches!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .gizmo_drag,
            Some(EditorGizmoDrag {
                kind: GizmoDragKind::Move,
                ..
            })
        ));

        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.left_mouse_down = false;
            input.0.mode = EditorMode::Scale;
        }
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = None;
        let scale_world = Vec3::new(
            1.0 + gizmo_axis_length_for_screen(center, 9.0),
            center.y,
            center.z,
        );
        let scale_screen = screen_for_world(scale_world);
        app.world_mut().write_message(PointerMoved {
            x: scale_screen.x as f64,
            y: scale_screen.y as f64,
        });
        app.tick();
        assert!(matches!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .hovered_gizmo,
            Some((GizmoDragKind::Resize, _))
        ));

        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Rotate;
        let ring_radius = gizmo_axis_length_for_screen(center, 100.0) * 0.78;
        let rotate_world = center + Vec3::new(ring_radius, 0.0, 0.0);
        let rotate_screen = screen_for_world(rotate_world);
        app.world_mut().write_message(PointerMoved {
            x: rotate_screen.x as f64,
            y: rotate_screen.y as f64,
        });
        app.tick();
        assert!(matches!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .hovered_gizmo,
            Some((GizmoDragKind::Rotate, _))
        ));
    }

    #[test]
    fn drag_gizmo_from_screen_respects_phase_for_move_and_resize() {
        let block = LevelObject::default();
        let mut app = gizmo_test_app(vec![block.clone()], EditorMode::Move);
        app.world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = false;
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let axis_screen = screen_for_world(center + Vec3::X);
        let axis_dir = (axis_screen - origin_screen).normalize();
        let target = origin_screen + axis_dir * 24.0;

        let move_drag = EditorGizmoDrag {
            axis: GizmoAxis::X,
            kind: GizmoDragKind::Move,
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_screen: [origin_screen.x, origin_screen.y],
            start_center_world: center.to_array(),
            start_blocks: vec![start_block_for_index(0, &block)],
        };

        app.world_mut().insert_resource(State::new(AppPhase::Menu));
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(move_drag.clone());
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position,
            block.position
        );

        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(move_drag);
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();
        assert!(
            collect_editor_objects_from_world(app.world_mut())[0].position[0] > block.position[0]
        );

        {
            let mut runtime = app.world_mut().resource_mut::<EditorRuntimeRes>();
            runtime.0.interaction.gizmo_drag = Some(EditorGizmoDrag {
                axis: GizmoAxis::X,
                kind: GizmoDragKind::Resize,
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_screen: [origin_screen.x, origin_screen.y],
                start_center_world: center.to_array(),
                start_blocks: vec![start_block_for_index(0, &block)],
            });
        }
        app.world_mut().write_message(PointerMoved {
            x: target.x as f64,
            y: target.y as f64,
        });
        app.tick();
        assert!(collect_editor_objects_from_world(app.world_mut())[0].size[0] > block.size[0]);
    }

    #[test]
    fn begin_gizmo_drag_ext_follows_phase_guards_and_records_history() {
        let block = LevelObject::default();
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        let mut menu_app = gizmo_test_app(vec![block.clone()], EditorMode::Move);
        menu_app
            .world_mut()
            .insert_resource(State::new(AppPhase::Menu));
        menu_app.world_mut().write_message(PrimaryClick {
            x: center_screen.x as f64,
            y: center_screen.y as f64,
        });
        menu_app.tick();
        assert!(
            menu_app
                .world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .gizmo_drag
                .is_none()
        );

        let mut app = gizmo_test_app(vec![block], EditorMode::Move);
        let history_before = app.world().resource::<EditorHistoryRes>().0.undo.len();
        app.world_mut().write_message(PrimaryClick {
            x: center_screen.x as f64,
            y: center_screen.y as f64,
        });
        app.tick();
        assert!(
            app.world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .gizmo_drag
                .is_some()
        );
        assert_eq!(
            app.world().resource::<EditorHistoryRes>().0.undo.len(),
            history_before + 1
        );

        assert!(screen_for_world(center + Vec3::X).distance(center_screen) > 0.0);
    }
    #[test]
    fn drag_gizmo_y_prevents_compression_when_moving_below_ground() {
        // Two blocks at different heights.
        let mut low = LevelObject::default();
        low.position = [0.0, 0.0, 0.0];
        let mut high = LevelObject::default();
        high.position = [0.0, 3.0, 0.0];
        let mut app = gizmo_test_app(vec![low.clone(), high.clone()], EditorMode::Select);
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .selected_block_indices = vec![0, 1];

        let center = Vec3::new(0.5, 2.0, 0.5);
        let center_screen = screen_for_world(center);

        // Simulate a large negative Y gizmo drag.
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .gizmo_drag = Some(EditorGizmoDrag {
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
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;

        // Drag far below ground (simulate large negative world delta via screen).
        let far_screen = Vec2::new(center_screen.x, center_screen.y - 2000.0);
        app.world_mut().write_message(PointerMoved {
            x: far_screen.x as f64,
            y: far_screen.y as f64,
        });
        app.tick();

        // The low block should stop at y=0, the high block should maintain its
        // relative 3-unit offset (staying at y=3 or wherever the delta places it
        // without going negative for the lowest block).
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

    /// Helper: compute the signed angular delta (in degrees) that the rotation
    /// drag code would produce for the given screen-space mouse positions,
    /// camera, and world-space axis direction.
    fn compute_rotation_delta_degrees(
        camera: &EditorCameraState,
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
        let target = camera.editor_camera_target();
        let eye = target + camera.editor_camera_offset();
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
        let rot = rotate_block_with_gizmo(LevelObject::default(), GizmoAxis::Y, false, 15.0);
        // With no pre-rotation, the Y ring axis is world Y.
        // Rotation should mostly affect the Y Euler component.
        assert!(
            rot[1].abs() > 1.0,
            "Y rotation should be non-trivial, got {rot:?}"
        );
        // X and Z should remain near zero since we rotated around Y
        // and the block had no prior rotation.
        approx::assert_abs_diff_eq!(rot[0], 0.0, epsilon = 1.0);
        approx::assert_abs_diff_eq!(rot[2], 0.0, epsilon = 1.0);
    }

    #[test]
    fn drag_gizmo_rotate_around_x_with_y90_uses_world_axis() {
        let mut block = LevelObject::default();
        block.rotation = [0.0, 90.0, 0.0];
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        let start_mouse = center_screen + Vec2::new(30.0, 0.0);
        let current_mouse = center_screen + Vec2::new(0.0, 30.0);
        let actual = rotate_block_with_gizmo(block, GizmoAxis::X, false, 15.0);

        let start_degrees = [0.0_f32, 90.0, 0.0];
        let start_quat = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            start_degrees[0].to_radians(),
            start_degrees[1].to_radians(),
            start_degrees[2].to_radians(),
        );
        let world_axis = start_quat * Vec3::X;

        let delta = compute_rotation_delta_degrees(
            &EditorCameraState::default(),
            start_mouse,
            current_mouse,
            center_screen,
            world_axis,
        );
        assert!(delta.abs() > 1.0, "delta should be non-trivial");

        let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);

        approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = 0.5);

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
    }

    #[test]
    fn drag_gizmo_rotate_around_y_with_xz_rotation_uses_world_axis() {
        // Use X+Z rotation so the world Y axis actually diverges from local Y.
        // (Pure X rotation doesn't change the Y axis direction.)
        let mut block = LevelObject::default();
        block.rotation = [45.0, 0.0, 45.0];
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        let start_mouse = center_screen + Vec2::new(30.0, 0.0);
        let current_mouse = center_screen + Vec2::new(0.0, 30.0);
        let actual = rotate_block_with_gizmo(block, GizmoAxis::Y, false, 15.0);

        let start_degrees = [45.0_f32, 0.0, 45.0];
        let start_quat = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            start_degrees[0].to_radians(),
            start_degrees[1].to_radians(),
            start_degrees[2].to_radians(),
        );
        let world_axis = start_quat * Vec3::Y;

        let delta = compute_rotation_delta_degrees(
            &EditorCameraState::default(),
            start_mouse,
            current_mouse,
            center_screen,
            world_axis,
        );
        assert!(delta.abs() > 1.0);

        let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);

        approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = 0.5);

        // Verify it differs from naive Euler addition
        let naive = [45.0, delta, 45.0];
        let differs = (actual[0] - naive[0]).abs() > 1.0
            || (actual[1] - naive[1]).abs() > 1.0
            || (actual[2] - naive[2]).abs() > 1.0;
        assert!(
            differs,
            "quaternion rotation should differ from naive Euler: actual={actual:?}, naive={naive:?}"
        );
    }

    #[test]
    fn drag_gizmo_rotate_around_z_with_combined_rotation_uses_world_axis() {
        let mut block = LevelObject::default();
        block.rotation = [30.0, 20.0, 0.0];
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        let start_mouse = center_screen + Vec2::new(30.0, 0.0);
        let current_mouse = center_screen + Vec2::new(0.0, 30.0);
        let actual = rotate_block_with_gizmo(block, GizmoAxis::Z, false, 15.0);

        let start_degrees = [30.0_f32, 20.0, 0.0];
        let start_quat = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            start_degrees[0].to_radians(),
            start_degrees[1].to_radians(),
            start_degrees[2].to_radians(),
        );
        let world_axis = start_quat * Vec3::Z;

        let delta = compute_rotation_delta_degrees(
            &EditorCameraState::default(),
            start_mouse,
            current_mouse,
            center_screen,
            world_axis,
        );
        assert!(delta.abs() > 1.0);

        let expected = expected_rotation_after_delta(start_degrees, world_axis, delta);

        approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = 0.5);
    }

    #[test]
    fn drag_gizmo_rotate_with_snap_rounds_delta_before_composition() {
        let mut block = LevelObject::default();
        block.rotation = [0.0, 45.0, 0.0];
        let center = Vec3::new(0.5, 0.5, 0.5);
        let center_screen = screen_for_world(center);

        let start_mouse = center_screen + Vec2::new(30.0, 0.0);
        let current_mouse = center_screen + Vec2::new(0.0, 30.0);
        let actual = rotate_block_with_gizmo(block, GizmoAxis::X, true, 15.0);

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
            &EditorCameraState::default(),
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

        approx::assert_abs_diff_eq!(actual[0], expected[0], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[1], expected[1], epsilon = 0.5);
        approx::assert_abs_diff_eq!(actual[2], expected[2], epsilon = 0.5);
    }
}
