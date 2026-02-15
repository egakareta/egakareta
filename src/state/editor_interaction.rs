use super::*;
use glam::Vec4;

impl State {
    pub fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor.right_dragging {
            return false;
        }

        self.editor.pointer_screen = Some([x, y]);

        let Some(drag) = self.editor_interaction.gizmo_drag.clone() else {
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

        let Some(origin_screen) = self.world_to_screen(center) else {
            self.editor_interaction.gizmo_drag = Some(drag);
            return true;
        };
        let Some(axis_screen) = self.world_to_screen(center + axis_dir) else {
            self.editor_interaction.gizmo_drag = Some(drag);
            return true;
        };

        let axis_screen_delta = axis_screen - origin_screen;
        if axis_screen_delta.length_squared() <= f32::EPSILON {
            return true;
        }

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
                let snap_enabled = self.editor_config.snap_to_grid;
                let snap_step = self.editor_config.snap_step.max(0.05);
                let mut first_cursor: Option<[f32; 3]> = None;
                for block in &drag.start_blocks {
                    if let Some(obj) = self.editor_objects.get_mut(block.index) {
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
                    let bounds = self.editor.bounds as f32;
                    self.editor.cursor = [
                        next_position[0].clamp(-bounds, bounds),
                        next_position[1].clamp(-bounds, bounds),
                        next_position[2].max(0.0),
                    ];
                }
                self.sync_editor_objects_for_drag();
                self.rebuild_editor_cursor_vertices();
            }
            GizmoDragKind::Resize => {
                let snap_enabled = self.editor_config.snap_to_grid;
                let snap_step = self.editor_config.snap_step.max(0.05);
                let min_size = if snap_enabled { snap_step } else { 0.25 };
                for block in &drag.start_blocks {
                    if let Some(obj) = self.editor_objects.get_mut(block.index) {
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
                self.sync_editor_objects_for_drag();
            }
        }
        true
    }

    pub fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.drag_editor_gizmo_from_screen(x, y) {
            return true;
        }

        if self.phase != AppPhase::Editor
            || self.editor.right_dragging
            || self.editor.mode != EditorMode::Select
        {
            return false;
        }

        self.editor.pointer_screen = Some([x, y]);

        let Some(drag) = self.editor_interaction.block_drag.clone() else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let (camera_right_xy, camera_up_xy) = self.editor_camera_axes_xy();
        let center = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );

        let Some(origin_screen) = self.world_to_screen(center) else {
            return true;
        };

        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let right_world = Vec3::new(camera_right_xy.x, camera_right_xy.y, 0.0);
        let up_world = Vec3::new(camera_up_xy.x, camera_up_xy.y, 0.0);

        let Some(right_screen) = self.world_to_screen(center + right_world) else {
            return true;
        };
        let Some(up_screen) = self.world_to_screen(center + up_world) else {
            return true;
        };

        let right_screen_delta = right_screen - origin_screen;
        let up_screen_delta = up_screen - origin_screen;

        let det =
            right_screen_delta.x * up_screen_delta.y - right_screen_delta.y * up_screen_delta.x;
        if det.abs() <= f32::EPSILON {
            return true;
        }

        let delta_x = effective_mouse_delta.x;
        let delta_y = effective_mouse_delta.y;
        let right_units = (delta_x * up_screen_delta.y - delta_y * up_screen_delta.x) / det;
        let up_units = (delta_y * right_screen_delta.x - delta_x * right_screen_delta.y) / det;

        let move_x = right_world.x * right_units + up_world.x * up_units;
        let move_y = right_world.y * right_units + up_world.y * up_units;
        let snap_enabled = self.editor_config.snap_to_grid;
        let snap_step = self.editor_config.snap_step.max(0.05);
        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.editor_objects.get_mut(block.index) {
                let mut next = block.position;
                next[0] += move_x;
                next[1] += move_y;
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
            let bounds = self.editor.bounds as f32;
            self.editor.cursor = [
                next_position[0].clamp(-bounds, bounds),
                next_position[1].clamp(-bounds, bounds),
                next_position[2].max(0.0),
            ];
        }
        self.sync_editor_objects_for_drag();
        self.rebuild_editor_cursor_vertices();
        true
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor.right_dragging {
            return;
        }

        self.editor.pointer_screen = Some([x, y]);

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            if self.editor.mode == EditorMode::Select && self.editor.hovered_block_index.is_some() {
                self.editor.hovered_block_index = None;
                self.rebuild_editor_hover_outline_vertices();
            }
            return;
        };

        if self.editor.mode == EditorMode::Select {
            if self.editor.hovered_block_index != pick.hit_block_index {
                self.editor.hovered_block_index = pick.hit_block_index;
                self.rebuild_editor_hover_outline_vertices();
            }
            return;
        }

        if pick.cursor != self.editor.cursor {
            self.editor.cursor = pick.cursor;
            self.rebuild_editor_cursor_vertices();
        }
    }

    pub(super) fn begin_editor_gizmo_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor.mode != EditorMode::Select {
            return false;
        }

        let indices = self.selected_block_indices_normalized();
        if indices.is_empty() {
            return false;
        }

        let Some((kind, axis)) = self.pick_editor_gizmo_handle(x, y) else {
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
        let Some(center_screen) = self.world_to_screen(center) else {
            return false;
        };

        let mut start_blocks = Vec::with_capacity(indices.len());
        for index in indices {
            if let Some(obj) = self.editor_objects.get(index) {
                start_blocks.push(EditorDragBlockStart {
                    index,
                    position: obj.position,
                    size: obj.size,
                });
            }
        }

        self.editor_interaction.gizmo_drag = Some(EditorGizmoDrag {
            axis,
            kind,
            start_mouse: [x, y],
            start_center_screen: [center_screen.x, center_screen.y],
            start_center_world: [center.x, center.y, center.z],
            start_blocks,
        });
        self.record_editor_history_state();
        true
    }

    pub(super) fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor.mode != EditorMode::Select {
            return false;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
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
            let Some(center_screen) = self.world_to_screen(center) else {
                return false;
            };

            let mut start_blocks = Vec::with_capacity(selected_indices.len());
            for index in selected_indices {
                if let Some(obj) = self.editor_objects.get(index) {
                    start_blocks.push(EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                    });
                }
            }

            self.editor_interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [x, y],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks,
            });
            self.record_editor_history_state();
            return true;
        }

        false
    }

    fn editor_view_proj(&self) -> Option<Mat4> {
        if self.gpu.config.width == 0 || self.gpu.config.height == 0 {
            return None;
        }

        let aspect = self.gpu.config.width as f32 / self.gpu.config.height as f32;
        let target = Vec3::new(
            self.editor_camera.editor_pan[0],
            self.editor_camera.editor_pan[1],
            0.0,
        );
        let eye = target + self.editor_camera_offset();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        Some(proj * view)
    }

    fn world_to_screen(&self, world: Vec3) -> Option<Vec2> {
        let view_proj = self.editor_view_proj()?;
        let clip = view_proj * world.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }

        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            return None;
        }

        let screen_x = (ndc.x + 1.0) * 0.5 * self.gpu.config.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.gpu.config.height as f32;
        Some(Vec2::new(screen_x, screen_y))
    }

    fn pick_editor_gizmo_handle(&self, x: f64, y: f64) -> Option<(GizmoDragKind, GizmoAxis)> {
        if self.phase != AppPhase::Editor || self.editor.mode != EditorMode::Select {
            return None;
        }

        let (bounds_position, bounds_size) = self.selected_group_bounds()?;

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.editor_gizmo_axis_lengths_world(center, 50.0);
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
            if let Some(screen) = self.world_to_screen(world) {
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

    fn pixels_to_world_along_axis(&self, center: Vec3, axis: Vec3, pixels: f32) -> Option<f32> {
        let origin_screen = self.world_to_screen(center)?;
        let axis_screen = self.world_to_screen(center + axis)?;
        let pixels_per_world = axis_screen.distance(origin_screen);
        if pixels_per_world <= f32::EPSILON {
            return None;
        }
        Some(pixels / pixels_per_world)
    }

    pub(super) fn editor_gizmo_axis_lengths_world(
        &self,
        center: Vec3,
        target_pixels: f32,
    ) -> [f32; 3] {
        let x = self
            .pixels_to_world_along_axis(center, Vec3::X, target_pixels)
            .unwrap_or(1.0);
        let y = self
            .pixels_to_world_along_axis(center, Vec3::Y, target_pixels)
            .unwrap_or(1.0);
        let z = self
            .pixels_to_world_along_axis(center, Vec3::Z, target_pixels)
            .unwrap_or(1.0);
        [x, y, z]
    }

    pub(super) fn editor_gizmo_axis_width_world(&self, center: Vec3, target_pixels: f32) -> f32 {
        let x = self.pixels_to_world_along_axis(center, Vec3::X, target_pixels);
        let y = self.pixels_to_world_along_axis(center, Vec3::Y, target_pixels);
        let z = self.pixels_to_world_along_axis(center, Vec3::Z, target_pixels);
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

    fn rotate_vec2(v: Vec2, radians: f32) -> Vec2 {
        let sin = radians.sin();
        let cos = radians.cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    fn ray_intersect_rotated_block(
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &LevelObject,
    ) -> Option<(f32, Vec3)> {
        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let inv_angle = -obj.rotation_degrees.to_radians();

        let local_origin_xy = Self::rotate_vec2(
            Vec2::new(ray_origin.x - center.x, ray_origin.y - center.y),
            inv_angle,
        );
        let local_dir_xy = Self::rotate_vec2(Vec2::new(ray_dir.x, ray_dir.y), inv_angle);
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

        let normal_world_xy = Self::rotate_vec2(
            Vec2::new(normal_local.x, normal_local.y),
            obj.rotation_degrees.to_radians(),
        );
        let normal_world = Vec3::new(normal_world_xy.x, normal_world_xy.y, normal_local.z);
        Some((t_hit, normal_world.normalize_or_zero()))
    }

    fn editor_pick_from_screen(&self, x: f64, y: f64) -> Option<EditorPickResult> {
        if self.phase != AppPhase::Editor || self.editor.right_dragging {
            return None;
        }

        if self.gpu.config.width == 0 || self.gpu.config.height == 0 {
            return None;
        }

        let aspect = self.gpu.config.width as f32 / self.gpu.config.height as f32;
        let target = Vec3::new(
            self.editor_camera.editor_pan[0],
            self.editor_camera.editor_pan[1],
            0.0,
        );
        let offset = self.editor_camera_offset();
        let eye = target + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let inv_view_proj = (proj * view).inverse();

        let ndc_x = (2.0 * x as f32 / self.gpu.config.width as f32) - 1.0;
        let ndc_y = 1.0 - (2.0 * y as f32 / self.gpu.config.height as f32);

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

        for (index, obj) in self.editor_objects.iter().enumerate() {
            if let Some((t, normal)) = Self::ray_intersect_rotated_block(ray_origin, ray_dir, obj) {
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

        let snap_enabled = self.editor_config.snap_to_grid;
        let snap_step = self.editor_config.snap_step.max(0.05);

        let next_cursor = if snap_enabled {
            [
                (target.x / snap_step).floor() * snap_step,
                (target.y / snap_step).floor() * snap_step,
                (target.z / snap_step).floor() * snap_step,
            ]
        } else {
            [target.x.floor(), target.y.floor(), target.z.floor()]
        };

        let bounds = self.editor.bounds as f32;
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

    pub(super) fn select_editor_block_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let additive = self.editor.shift_held;

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            if !additive {
                self.editor.selected_block_indices.clear();
                self.editor.selected_block_index = None;
                self.editor.hovered_block_index = None;
            }
            self.editor_interaction.gizmo_drag = None;
            self.editor_interaction.block_drag = None;
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_hover_outline_vertices();
            self.rebuild_editor_selection_outline_vertices();
            return;
        };

        if let Some(hit_index) = pick.hit_block_index {
            if additive {
                if let Some(existing) = self
                    .editor
                    .selected_block_indices
                    .iter()
                    .position(|idx| *idx == hit_index)
                {
                    self.editor.selected_block_indices.remove(existing);
                    if self.editor.selected_block_indices.is_empty() {
                        self.editor.selected_block_index = None;
                        self.editor.hovered_block_index = None;
                    }
                } else {
                    self.editor.selected_block_indices.push(hit_index);
                }
            } else {
                self.editor.selected_block_indices.clear();
                self.editor.selected_block_indices.push(hit_index);
            }
            self.editor.hovered_block_index = Some(hit_index);
        } else if !additive {
            self.editor.selected_block_indices.clear();
            self.editor.selected_block_index = None;
            self.editor.hovered_block_index = None;
        }

        self.sync_primary_selection_from_indices();
        self.editor_interaction.gizmo_drag = None;
        self.editor_interaction.block_drag = None;

        if let Some(index) = self.editor.selected_block_index {
            if let Some(obj) = self.editor_objects.get(index) {
                self.editor.cursor = [obj.position[0], obj.position[1], obj.position[2]];
                self.rebuild_editor_cursor_vertices();
            }
        } else if pick.cursor != self.editor.cursor {
            self.editor.cursor = pick.cursor;
            self.rebuild_editor_cursor_vertices();
        }

        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        if !self.editor.right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;

        if self.phase == AppPhase::Editor {
            self.editor_camera.editor_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor_camera.editor_pitch = (self.editor_camera.editor_pitch
                + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        } else if self.phase == AppPhase::Playing && (self.game.game_over || !self.game.started) {
            self.editor_camera.playing_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor_camera.playing_pitch = (self.editor_camera.playing_pitch
                + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        }
    }
}
