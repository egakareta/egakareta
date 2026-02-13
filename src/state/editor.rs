use super::*;

impl State {
    pub(super) fn clear_editor_pan_keys(&mut self) {
        self.editor_pan_up_held = false;
        self.editor_pan_down_held = false;
        self.editor_pan_left_held = false;
        self.editor_pan_right_held = false;
        self.editor_shift_held = false;
        self.editor_ctrl_held = false;
    }

    pub(super) fn selected_block_indices_normalized(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .editor_selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < self.editor_objects.len())
            .collect();

        if indices.is_empty() {
            if let Some(index) = self
                .editor_selected_block_index
                .filter(|index| *index < self.editor_objects.len())
            {
                indices.push(index);
            }
        }

        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub(super) fn sync_primary_selection_from_indices(&mut self) {
        let indices = self.selected_block_indices_normalized();
        self.editor_selected_block_index = indices.first().copied();
        self.editor_selected_block_indices = indices;
    }

    pub(super) fn selection_contains(&self, index: usize) -> bool {
        self.editor_selected_block_indices.contains(&index)
            || self.editor_selected_block_index == Some(index)
    }

    pub(super) fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let indices = self.selected_block_indices_normalized();
        let first = *indices.first()?;
        let first_obj = self.editor_objects.get(first)?;
        let mut min = first_obj.position;
        let mut max = [
            first_obj.position[0] + first_obj.size[0],
            first_obj.position[1] + first_obj.size[1],
            first_obj.position[2] + first_obj.size[2],
        ];

        for index in indices.into_iter().skip(1) {
            if let Some(obj) = self.editor_objects.get(index) {
                min[0] = min[0].min(obj.position[0]);
                min[1] = min[1].min(obj.position[1]);
                min[2] = min[2].min(obj.position[2]);
                max[0] = max[0].max(obj.position[0] + obj.size[0]);
                max[1] = max[1].max(obj.position[1] + obj.size[1]);
                max[2] = max[2].max(obj.position[2] + obj.size[2]);
            }
        }

        Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
    }

    pub(super) fn reset_playing_camera_defaults(&mut self) {
        self.playing_camera_rotation = -45.0f32.to_radians();
        self.playing_camera_pitch = 45.0f32.to_radians();
    }

    pub(super) fn enter_playing_phase(
        &mut self,
        level_name: Option<String>,
        playtesting_editor: bool,
    ) {
        self.phase = AppPhase::Playing;
        self.playtesting_editor = playtesting_editor;
        self.playing_level_name = level_name;
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
    }

    pub(super) fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.editor_level_name = Some(level_name);
        self.playtesting_editor = false;
        self.editor_right_dragging = false;
        self.editor_mode = EditorMode::Place;
        self.editor_selected_block_index = None;
        self.editor_selected_block_indices.clear();
        self.editor_hovered_block_index = None;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        self.editor_history_undo.clear();
        self.editor_history_redo.clear();
        self.clear_editor_pan_keys();
        self.editor_camera_rotation = -45.0f32.to_radians();
        self.editor_camera_pitch = 45.0f32.to_radians();
        self.editor_zoom = 1.0;
        self.game = GameState::new();
        self.trail_vertex_count = 0;
    }

    pub(super) fn enter_menu_phase(&mut self) {
        self.playtesting_editor = false;
        self.editor_level_name = None;
        self.editor_selected_block_index = None;
        self.editor_selected_block_indices.clear();
        self.editor_hovered_block_index = None;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        self.playing_level_name = None;
        self.editor_right_dragging = false;
        self.clear_editor_pan_keys();
        self.phase = AppPhase::Menu;
    }

    pub fn set_editor_pan_up_held(&mut self, held: bool) {
        self.editor_pan_up_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_down_held(&mut self, held: bool) {
        self.editor_pan_down_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_left_held(&mut self, held: bool) {
        self.editor_pan_left_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_right_held(&mut self, held: bool) {
        self.editor_pan_right_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_shift_held(&mut self, held: bool) {
        self.editor_shift_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_ctrl_held(&mut self, held: bool) {
        self.editor_ctrl_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_block_kind(&mut self, kind: BlockKind) {
        self.editor_selected_kind = kind;
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor_mode = mode;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        if mode == EditorMode::Place {
            self.editor_selected_block_index = None;
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
        }
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor_mode
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor_snap_to_grid
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor_snap_step
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor_snap_to_grid = snap;
        if self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor_snap_step = step.max(0.05);
        if self.editor_snap_to_grid && self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.selected_block_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.editor_objects.get(index).cloned())
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let bounds = self.editor.bounds;
            let snap_step = self.editor_snap_step.max(0.05);
            let next_position = if self.editor_snap_to_grid {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1] / snap_step).round() * snap_step,
                    (position[2].max(0.0) / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.editor_objects[index].position = next_position;
            self.editor.cursor = [
                (next_position[0].floor() as i32).clamp(-bounds, bounds),
                (next_position[1].floor() as i32).clamp(-bounds, bounds),
                (next_position[2].floor() as i32).max(0),
            ];
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

        if self.editor_gizmo_drag.is_none() && self.editor_block_drag.is_none() {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let snap_step = self.editor_snap_step.max(0.05);
            let snapped_size = if self.editor_snap_to_grid {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.editor_snap_to_grid {
                snap_step
            } else {
                0.25
            };
            self.editor_objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_kind(&mut self, kind: BlockKind) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].kind = kind;
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

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].rotation_degrees = rotation_degrees;
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub fn editor_selected_block_kind(&self) -> BlockKind {
        self.editor_selected_kind
    }

    pub fn editor_timeline_step(&self) -> u32 {
        self.editor_timeline_step
    }

    pub fn editor_timeline_length(&self) -> u32 {
        self.editor_timeline_length
    }

    pub fn editor_tap_steps(&self) -> &[u32] {
        &self.editor_tap_steps
    }

    pub fn set_editor_timeline_step(&mut self, step: u32) {
        self.record_editor_history_state();
        let max_step = self.editor_timeline_length.saturating_sub(1);
        self.editor_timeline_step = step.min(max_step);
        self.refresh_editor_timeline_position();
    }

    pub fn set_editor_timeline_length(&mut self, length: u32) {
        self.record_editor_history_state();
        let length = length.max(1);
        let max_step = length.saturating_sub(1);
        self.editor_timeline_length = length;
        self.editor_timeline_step = self.editor_timeline_step.min(max_step);
        self.editor_tap_steps.retain(|step| *step < length);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        add_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        remove_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        clear_tap_steps(&mut self.editor_tap_steps);
        self.refresh_editor_timeline_position();
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor_timeline_position(self.editor_timeline_step)
    }

    pub(super) fn editor_camera_axes_xy(&self) -> (Vec2, Vec2) {
        let right = Vec2::new(
            self.editor_camera_rotation.cos(),
            self.editor_camera_rotation.sin(),
        );
        let up = Vec2::new(
            -self.editor_camera_rotation.sin(),
            self.editor_camera_rotation.cos(),
        );
        (right, up)
    }

    pub(super) fn editor_camera_offset(&self) -> Vec3 {
        let zoom = self.editor_zoom.clamp(0.35, 4.0);
        let distance = 24.0 / zoom;
        let pitch = self
            .editor_camera_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(self.editor_camera_rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
    }

    pub(super) fn playing_camera_offset(&self) -> Vec3 {
        let distance = 28.28;
        let rotation = if self.game.game_over || !self.game.started {
            self.playing_camera_rotation
        } else {
            -45.0f32.to_radians()
        };
        let pitch = if self.game.game_over || !self.game.started {
            self.playing_camera_pitch
        } else {
            45.0f32.to_radians()
        };

        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
    }

    pub fn adjust_editor_zoom(&mut self, delta: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        const ZOOM_SENSITIVITY: f32 = 0.12;
        let factor = (1.0 + delta * ZOOM_SENSITIVITY).max(0.1);
        self.editor_zoom = (self.editor_zoom * factor).clamp(0.35, 4.0);
    }

    pub fn pan_editor_camera_by_input(&mut self, screen_x: f32, screen_y: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let (camera_right_xy, camera_up_xy) = self.editor_camera_axes_xy();
        let world_delta = camera_right_xy * screen_x + camera_up_xy * screen_y;

        let max_pan = self.editor.bounds as f32;
        self.editor_camera_pan[0] =
            (self.editor_camera_pan[0] + world_delta.x).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] =
            (self.editor_camera_pan[1] + world_delta.y).clamp(-max_pan, max_pan);
    }

    pub(super) fn update_editor_pan_from_keys(&mut self, frame_dt: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let mut input = Vec2::ZERO;
        if self.editor_pan_left_held {
            input.x -= 1.0;
        }
        if self.editor_pan_right_held {
            input.x += 1.0;
        }
        if self.editor_pan_up_held {
            input.y += 1.0;
        }
        if self.editor_pan_down_held {
            input.y -= 1.0;
        }

        if input.length_squared() <= f32::EPSILON {
            return;
        }

        let input = input.normalize();
        let pitch = self
            .editor_camera_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_factor = pitch.cos();
        let vertical_factor = pitch.sin();

        let mut speed_multiplier = 1.0;
        if self.editor_shift_held {
            speed_multiplier = 0.3;
        }

        const PAN_SPEED_UNITS_PER_SEC: f32 = 40.0;
        self.pan_editor_camera_by_input(
            input.x * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
            input.y * horizontal_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );

        self.adjust_editor_zoom(
            input.y * vertical_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return;
        }

        self.editor_pointer_screen = Some([x, y]);

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            if self.editor_mode == EditorMode::Select && self.editor_hovered_block_index.is_some() {
                self.editor_hovered_block_index = None;
                self.rebuild_editor_hover_outline_vertices();
            }
            return;
        };

        if self.editor_mode == EditorMode::Select {
            if self.editor_hovered_block_index != pick.hit_block_index {
                self.editor_hovered_block_index = pick.hit_block_index;
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
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
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

        self.editor_gizmo_drag = Some(EditorGizmoDrag {
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
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
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

            self.editor_block_drag = Some(EditorBlockDrag {
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

    pub(super) fn editor_view_proj(&self) -> Option<Mat4> {
        if self.config.width == 0 || self.config.height == 0 {
            return None;
        }

        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let eye = target + self.editor_camera_offset();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        Some(proj * view)
    }

    pub(super) fn world_to_screen(&self, world: Vec3) -> Option<Vec2> {
        let view_proj = self.editor_view_proj()?;
        let clip = view_proj * world.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }

        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            return None;
        }

        let screen_x = (ndc.x + 1.0) * 0.5 * self.config.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.config.height as f32;
        Some(Vec2::new(screen_x, screen_y))
    }

    pub(super) fn pick_editor_gizmo_handle(
        &self,
        x: f64,
        y: f64,
    ) -> Option<(GizmoDragKind, GizmoAxis)> {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
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

    pub(super) fn pixels_to_world_along_axis(
        &self,
        center: Vec3,
        axis: Vec3,
        pixels: f32,
    ) -> Option<f32> {
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

    pub(super) fn rotate_vec2(v: Vec2, radians: f32) -> Vec2 {
        let sin = radians.sin();
        let cos = radians.cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    pub(super) fn ray_intersect_rotated_block(
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

    pub(super) fn editor_pick_from_screen(&self, x: f64, y: f64) -> Option<EditorPickResult> {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return None;
        }

        if self.config.width == 0 || self.config.height == 0 {
            return None;
        }

        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let offset = self.editor_camera_offset();
        let eye = target + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let inv_view_proj = (proj * view).inverse();

        let ndc_x = (2.0 * x as f32 / self.config.width as f32) - 1.0;
        let ndc_y = 1.0 - (2.0 * y as f32 / self.config.height as f32);

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
        let bounds = self.editor.bounds;
        let next_cursor = [
            (target.x.floor() as i32).clamp(-bounds, bounds),
            (target.y.floor() as i32).clamp(-bounds, bounds),
            (target.z.floor() as i32).max(0),
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

        let additive = self.editor_shift_held;

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            if !additive {
                self.editor_selected_block_indices.clear();
                self.editor_selected_block_index = None;
                self.editor_hovered_block_index = None;
            }
            self.editor_gizmo_drag = None;
            self.editor_block_drag = None;
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_hover_outline_vertices();
            self.rebuild_editor_selection_outline_vertices();
            return;
        };

        if let Some(hit_index) = pick.hit_block_index {
            if additive {
                if let Some(existing) = self
                    .editor_selected_block_indices
                    .iter()
                    .position(|idx| *idx == hit_index)
                {
                    self.editor_selected_block_indices.remove(existing);
                } else {
                    self.editor_selected_block_indices.push(hit_index);
                }
            } else {
                self.editor_selected_block_indices.clear();
                self.editor_selected_block_indices.push(hit_index);
            }
            self.editor_hovered_block_index = Some(hit_index);
        } else if !additive {
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
        }

        self.sync_primary_selection_from_indices();
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;

        if let Some(index) = self.editor_selected_block_index {
            if let Some(obj) = self.editor_objects.get(index) {
                self.editor.cursor = [
                    obj.position[0].floor() as i32,
                    obj.position[1].floor() as i32,
                    obj.position[2].floor() as i32,
                ];
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
        if !self.editor_right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;

        if self.phase == AppPhase::Editor {
            self.editor_camera_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor_camera_pitch = (self.editor_camera_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        } else if self.phase == AppPhase::Playing && (self.game.game_over || !self.game.started) {
            self.playing_camera_rotation -= dx as f32 * ROTATE_SPEED;
            self.playing_camera_pitch = (self.playing_camera_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        }
    }

    pub fn move_editor_up(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, 1);
        }
    }

    pub fn move_editor_down(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, -1);
        }
    }

    pub fn editor_remove_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        let selected_indices = self.selected_block_indices_normalized();
        if !selected_indices.is_empty() {
            for index in selected_indices.into_iter().rev() {
                if index < self.editor_objects.len() {
                    self.editor_objects.remove(index);
                }
            }
            self.editor_selected_block_index = None;
            self.editor_selected_block_indices.clear();
            self.editor_hovered_block_index = None;
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            return;
        }

        remove_topmost_block_at_cursor(&mut self.editor_objects, self.editor.cursor);

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub fn editor_playtest(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.stop_audio();

        let transition = build_editor_playtest_transition(
            &self.editor_objects,
            self.editor_level_name.as_deref(),
            self.editor_spawn.clone(),
            &self.editor_tap_steps,
            self.editor_timeline_step,
        );

        self.enter_playing_phase(transition.playing_level_name, true);
        self.game = GameState::new();
        self.game.objects = transition.objects;
        self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        self.playing_camera_rotation = transition.camera_rotation;
        self.playing_camera_pitch = transition.camera_pitch;
        self.editor_right_dragging = false;
        self.rebuild_block_vertices();
    }

    pub fn editor_set_spawn_here(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        let cursor = self.editor.cursor;
        self.editor_spawn.position = [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32];

        self.sync_editor_objects();
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.editor_spawn.direction = toggle_spawn_direction(self.editor_spawn.direction);
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn back_to_menu(&mut self) {
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.playtesting_editor, &self.editor_objects)
        {
            self.playtesting_editor = false;
            self.phase = AppPhase::Editor;
            self.game = GameState::new();
            self.game.objects = objects;
            self.rebuild_block_vertices();
            return;
        }

        self.enter_menu_phase();

        self.game = GameState::new();
        self.game.objects = create_menu_scene();
        self.rebuild_block_vertices();
        self.trail_vertex_count = 0;
    }

    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();

        self.game = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn restart_level(&mut self) {
        self.stop_audio();
        self.game = GameState::new();

        if self.playtesting_editor {
            let transition = build_editor_playtest_transition(
                &self.editor_objects,
                self.editor_level_name.as_deref(),
                self.editor_spawn.clone(),
                &self.editor_tap_steps,
                self.editor_timeline_step,
            );
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        } else if let Some(level_name) = self.playing_level_name.clone() {
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let transition = build_playing_transition_from_metadata(metadata);
                self.game.objects = transition.objects;
                self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
            }
        }

        self.game.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    pub(super) fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let init = editor_session_init_from_metadata(self.load_level_metadata(&level_name));
        self.editor_objects = init.objects;
        self.editor_spawn = init.spawn;
        self.editor_music_metadata = init.music;
        self.editor_tap_steps = init.tap_steps;
        self.editor_timeline_step = init.timeline_step;
        self.editor.cursor = init.cursor;
        self.editor_camera_pan = init.camera_pan;

        self.sync_editor_objects();
        // Refresh cursor/camera to match the current timeline step.
        self.set_editor_timeline_step(self.editor_timeline_step);
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        load_builtin_level_metadata(level_name)
    }

    pub(super) fn stop_audio(&mut self) {
        self.audio.stop();
    }

    pub(super) fn start_audio(&mut self, level_name: &str, metadata: &LevelMetadata) {
        if let Some(bytes) = self.local_audio_cache.get(&metadata.music.source) {
            self.audio.start_with_bytes(&metadata.music.source, bytes);
        } else {
            self.audio.start(level_name, &metadata.music.source);
        }
    }

    pub fn trigger_audio_import(&self) {
        let sender = self.audio_import_channel.0.clone();
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                    let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                    let _ = sender.send((filename, bytes));
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                pollster::block_on(async {
                    if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                        let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                        let _ = sender.send((filename, bytes));
                    }
                });
            });
        }
    }

    pub fn update_audio_imports(&mut self) {
        while let Ok((filename, bytes)) = self.audio_import_channel.1.try_recv() {
            self.editor_music_metadata.source = filename.clone();
            self.local_audio_cache.insert(filename, bytes);
        }
    }

    pub fn export_level_ldz(&self) -> Result<Vec<u8>, String> {
        let metadata = self.current_editor_metadata();
        let audio_bytes = self
            .local_audio_cache
            .get(&metadata.music.source)
            .cloned()
            .or_else(|| {
                read_editor_music_bytes(self.editor_level_name.as_deref(), &metadata.music.source)
            });
        let audio_file = audio_bytes
            .as_ref()
            .map(|bytes| (metadata.music.source.as_str(), bytes.as_slice()));

        build_ldz_archive(&metadata, audio_file)
    }

    pub fn import_level_ldz(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = read_metadata_from_ldz(data)?;
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    pub fn export_level(&self) -> String {
        serialize_level_metadata_pretty(&self.current_editor_metadata()).unwrap_or_default()
    }

    pub fn import_level(&mut self, json: &str) -> Result<(), String> {
        let metadata = parse_level_metadata_json(json)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(
            self.editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            self.editor_music_metadata.clone(),
            self.editor_spawn.clone(),
            self.editor_tap_steps.clone(),
            self.editor_timeline_step,
            self.editor_objects.clone(),
        )
    }

    pub(super) fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
        self.editor_objects = metadata.objects;
        self.editor_selected_block_index = None;
        self.editor_selected_block_indices.clear();
        self.editor_hovered_block_index = None;
        self.editor_spawn = metadata.spawn;
        self.editor_tap_steps = metadata.taps;
        self.editor_tap_steps.sort_unstable();
        self.editor_timeline_step = metadata.timeline_step;
        self.editor_level_name = Some(metadata.name);
        self.editor_music_metadata = metadata.music;

        if let Some(first) = self.editor_objects.first() {
            self.editor.cursor = [
                first.position[0].round() as i32,
                first.position[1].round() as i32,
                first.position[2].round() as i32,
            ];
        } else {
            self.editor.cursor = [0, 0, 0];
        }

        self.editor_camera_pan = [
            self.editor.cursor[0] as f32 + 0.5,
            self.editor.cursor[1] as f32 + 0.5,
        ];

        self.editor_history_undo.clear();
        self.editor_history_redo.clear();

        self.sync_editor_objects();
        self.set_editor_timeline_step(self.editor_timeline_step);
        self.rebuild_spawn_marker_vertices();
    }

    pub fn load_builtin_level_into_editor(&mut self, name: &str) {
        if let Some(metadata) = self.load_level_metadata(name) {
            let _ = self.import_level(&serde_json::to_string(&metadata).unwrap());
            self.editor_level_name = Some(name.to_string());
        }
    }

    pub fn editor_level_name(&self) -> Option<String> {
        self.editor_level_name.clone()
    }

    pub fn set_editor_level_name(&mut self, name: String) {
        self.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.editor_music_metadata = metadata;
    }

    pub fn editor_show_import(&self) -> bool {
        self.editor_show_import
    }

    pub fn set_editor_show_import(&mut self, show: bool) {
        self.editor_show_import = show;
    }

    pub fn editor_import_text(&self) -> &str {
        &self.editor_import_text
    }

    pub fn set_editor_import_text(&mut self, text: String) {
        self.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.editor_show_metadata = show;
    }

    pub fn available_levels(&self) -> &[String] {
        &self.menu.levels
    }

    pub fn trigger_level_export(&self) {
        match self.export_level_ldz() {
            Ok(data) => {
                let filename = format!(
                    "{}.ldz",
                    self.editor_level_name()
                        .unwrap_or_else(|| "level".to_string())
                );

                if let Err(error) = save_level_export(&filename, &data) {
                    log_platform_error(&format!("Export failed: {}", error));
                }
            }
            Err(e) => {
                log_platform_error(&format!("Export failed: {}", e));
            }
        }
    }

    pub fn complete_import(&mut self) {
        let text = self.editor_import_text.clone();
        // Try LDZ first (base64)
        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            if let Err(e) = self.import_level_ldz(&data) {
                log_platform_error(&format!("LDZ Import failed: {}", e));
            } else {
                self.editor_show_import = false;
                self.editor_import_text.clear();
                return;
            }
        }

        // Fallback to raw JSON
        let text = self.editor_import_text.clone();
        if let Err(e) = self.import_level(&text) {
            log_platform_error(&format!("JSON Import failed: {}", e));
        } else {
            self.editor_show_import = false;
            self.editor_import_text.clear();
        }
    }

    pub(super) fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        move_cursor_xy(&mut self.editor.cursor, dx, dy, self.editor.bounds);
        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn editor_history_snapshot(&self) -> EditorHistorySnapshot {
        EditorHistorySnapshot {
            objects: self.editor_objects.clone(),
            selected_block_index: self.editor_selected_block_index,
            selected_block_indices: self.editor_selected_block_indices.clone(),
            cursor: self.editor.cursor,
            selected_kind: self.editor_selected_kind,
            spawn: self.editor_spawn.clone(),
            timeline_step: self.editor_timeline_step,
            timeline_length: self.editor_timeline_length,
            tap_steps: self.editor_tap_steps.clone(),
        }
    }

    pub(super) fn record_editor_history_state(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        const MAX_HISTORY: usize = 256;
        if self.editor_history_undo.len() >= MAX_HISTORY {
            self.editor_history_undo.remove(0);
        }
        self.editor_history_undo
            .push(self.editor_history_snapshot());
        self.editor_history_redo.clear();
    }

    pub(super) fn apply_editor_history_snapshot(&mut self, snapshot: EditorHistorySnapshot) {
        self.editor_objects = snapshot.objects;
        self.editor_selected_block_index = snapshot
            .selected_block_index
            .filter(|index| *index < self.editor_objects.len());
        self.editor_selected_block_indices = snapshot
            .selected_block_indices
            .into_iter()
            .filter(|index| *index < self.editor_objects.len())
            .collect();
        self.sync_primary_selection_from_indices();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.editor.cursor = snapshot.cursor;
        self.editor_selected_kind = snapshot.selected_kind;
        self.editor_spawn = snapshot.spawn;
        self.editor_timeline_step = snapshot.timeline_step;
        self.editor_timeline_length = snapshot.timeline_length.max(1);
        self.editor_tap_steps = snapshot.tap_steps;
        self.editor_tap_steps
            .retain(|step| *step < self.editor_timeline_length);
        self.editor_timeline_step = self
            .editor_timeline_step
            .min(self.editor_timeline_length.saturating_sub(1));

        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(super) fn editor_undo(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(snapshot) = self.editor_history_undo.pop() else {
            return;
        };

        self.editor_history_redo
            .push(self.editor_history_snapshot());
        self.apply_editor_history_snapshot(snapshot);
    }

    pub(super) fn editor_redo(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(snapshot) = self.editor_history_redo.pop() else {
            return;
        };

        self.editor_history_undo
            .push(self.editor_history_snapshot());
        self.apply_editor_history_snapshot(snapshot);
    }

    pub(super) fn editor_copy_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_clipboard_block = Some(self.editor_objects[index].clone());
            return;
        }

        if let Some(index) = self.topmost_block_index_at_cursor(self.editor.cursor) {
            self.editor_clipboard_block = Some(self.editor_objects[index].clone());
        }
    }

    pub(super) fn editor_paste_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(mut block) = self.editor_clipboard_block.clone() else {
            return;
        };

        self.record_editor_history_state();

        block.position = [
            self.editor.cursor[0] as f32,
            self.editor.cursor[1] as f32,
            self.editor.cursor[2] as f32,
        ];

        self.editor_selected_kind = block.kind;
        self.editor_objects.push(block);
        self.editor_selected_block_index = Some(self.editor_objects.len() - 1);
        self.editor_selected_block_indices = self.editor_selected_block_index.into_iter().collect();
        self.editor_hovered_block_index = self.editor_selected_block_index;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(super) fn place_editor_block(&mut self) {
        self.record_editor_history_state();
        self.editor_objects.push(create_block_at_cursor(
            self.editor.cursor,
            self.editor_selected_kind,
        ));
        self.editor_selected_block_index = None;
        self.editor_selected_block_indices.clear();
        self.editor_hovered_block_index = None;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn sync_editor_objects(&mut self) {
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.editor_selected_block_index {
            if index >= self.editor_objects.len() {
                self.editor_selected_block_index = None;
            }
        }
        self.editor_selected_block_indices
            .retain(|index| *index < self.editor_objects.len());
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.editor_hovered_block_index {
            if index >= self.editor_objects.len() {
                self.editor_hovered_block_index = None;
            }
        }
        self.game.objects = self.editor_objects.clone();
        self.rebuild_block_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(super) fn topmost_block_index_at_cursor(&self, cursor: [i32; 3]) -> Option<usize> {
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.editor_objects.iter().enumerate() {
            let occupies_x = cursor[0] as f32 + 0.5 >= obj.position[0]
                && cursor[0] as f32 + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_y = cursor[1] as f32 + 0.5 >= obj.position[1]
                && cursor[1] as f32 + 0.5 <= obj.position[1] + obj.size[1];
            if occupies_x && occupies_y {
                let top = obj.position[2] + obj.size[2];
                if top > top_height {
                    top_height = top;
                    top_index = Some(index);
                }
            }
        }

        top_index
    }

    pub(super) fn apply_spawn_to_game(&mut self, position: [f32; 3], direction: SpawnDirection) {
        let centered_position = [
            position[0].floor() + 0.5,
            position[1].floor() + 0.5,
            position[2],
        ];
        self.game.position = centered_position;
        self.game.direction = direction.into();
        self.game.vertical_velocity = 0.0;
        self.game.is_grounded = true;
        self.game.trail_segments = vec![vec![centered_position]];
    }

    pub(super) fn editor_timeline_position(&self, step: u32) -> ([f32; 3], SpawnDirection) {
        derive_timeline_position(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_tap_steps,
            step,
            &self.editor_objects,
        )
    }

    pub(super) fn refresh_editor_timeline_position(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let (position, ..) = self.editor_timeline_position(self.editor_timeline_step);
        let bounds = self.editor.bounds;
        self.editor.cursor = [
            position[0].round() as i32,
            position[1].round() as i32,
            position[2].round() as i32,
        ];
        self.editor.cursor[0] = self.editor.cursor[0].clamp(-bounds, bounds);
        self.editor.cursor[1] = self.editor.cursor[1].clamp(-bounds, bounds);
        self.editor.cursor[2] = self.editor.cursor[2].max(0);

        let max_pan = bounds as f32;
        self.editor_camera_pan[0] = (position[0] + 0.5).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] = (position[1] + 0.5).clamp(-max_pan, max_pan);

        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn rebuild_editor_cursor_vertices(&mut self) {
        let vertices = build_editor_cursor_vertices(self.editor.cursor);
        self.editor_cursor_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_cursor_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Cursor Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_cursor_vertex_buffer = None;
        }
    }

    pub(super) fn rebuild_editor_hover_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_hover_outline_vertex_count = 0;
            self.editor_hover_outline_vertex_buffer = None;
            return;
        }

        let Some(index) = self
            .editor_hovered_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            self.editor_hover_outline_vertex_count = 0;
            self.editor_hover_outline_vertex_buffer = None;
            return;
        };

        if self.selection_contains(index) {
            self.editor_hover_outline_vertex_count = 0;
            self.editor_hover_outline_vertex_buffer = None;
            return;
        }

        let obj = &self.editor_objects[index];
        let vertices = build_editor_hover_outline_vertices(obj.position, obj.size);
        self.editor_hover_outline_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_hover_outline_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Hover Outline Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_hover_outline_vertex_buffer = None;
        }
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_gizmo_vertex_count = 0;
            self.editor_gizmo_vertex_buffer = None;
            return;
        }

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            self.editor_gizmo_vertex_count = 0;
            self.editor_gizmo_vertex_buffer = None;
            return;
        };

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.editor_gizmo_axis_lengths_world(center, 50.0);
        let axis_width = self.editor_gizmo_axis_width_world(center, 3.0);
        let vertices =
            build_editor_gizmo_vertices(bounds_position, bounds_size, axis_lengths, axis_width);
        self.editor_gizmo_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_gizmo_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Gizmo Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_gizmo_vertex_buffer = None;
        }
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_selection_outline_vertex_count = 0;
            self.editor_selection_outline_vertex_buffer = None;
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            self.editor_selection_outline_vertex_count = 0;
            self.editor_selection_outline_vertex_buffer = None;
            return;
        }

        let mut vertices = Vec::new();
        for index in selected_indices {
            if let Some(obj) = self.editor_objects.get(index) {
                vertices.extend(build_editor_selection_outline_vertices(
                    obj.position,
                    obj.size,
                ));
            }
        }
        self.editor_selection_outline_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_selection_outline_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Selection Outline Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_selection_outline_vertex_buffer = None;
        }
    }

    pub(super) fn rebuild_spawn_marker_vertices(&mut self) {
        let vertices = build_spawn_marker_vertices(
            self.editor_spawn.position,
            matches!(self.editor_spawn.direction, SpawnDirection::Right),
        );
        self.spawn_marker_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.spawn_marker_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Spawn Marker Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.spawn_marker_vertex_buffer = None;
        }
    }

    pub(super) fn rebuild_block_vertices(&mut self) {
        let vertices = build_block_vertices(&self.game.objects);

        self.block_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.block_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Block Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.block_vertex_buffer = None;
        }
    }
}
