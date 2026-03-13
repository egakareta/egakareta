use super::super::{
    EditorBlockDrag, EditorDirtyFlags, EditorDragBlockStart, EditorInteractionChange,
    EditorSubsystem, PerfStage, State,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::AppPhase;
use glam::{Vec2, Vec3};

const MARQUEE_DRAG_THRESHOLD_PX: f64 = 4.0;
const CAMERA_KEYPOINT_MARQUEE_RADIUS_PX: f32 = 16.0;

impl EditorSubsystem {
    fn marquee_has_dragged_far_enough(&self) -> bool {
        let Some(start) = self.ui.marquee_start_screen else {
            return false;
        };
        let current = self.ui.marquee_current_screen.unwrap_or(start);
        let dx = current[0] - start[0];
        let dy = current[1] - start[1];
        (dx * dx + dy * dy) >= MARQUEE_DRAG_THRESHOLD_PX * MARQUEE_DRAG_THRESHOLD_PX
    }

    pub(crate) fn marquee_selection_rect_screen(&self) -> Option<([f64; 2], [f64; 2], bool)> {
        let start = self.ui.marquee_start_screen?;
        let current = self.ui.marquee_current_screen.unwrap_or(start);
        Some((start, current, self.marquee_has_dragged_far_enough()))
    }

    pub(crate) fn begin_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        if phase != AppPhase::Editor || self.ui.right_dragging || !self.ui.mode.is_selection_mode()
        {
            return false;
        }
        self.ui.marquee_start_screen = Some([x, y]);
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    pub(crate) fn update_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        if phase != AppPhase::Editor || self.ui.right_dragging || !self.ui.mode.is_selection_mode()
        {
            return false;
        }
        if self.ui.marquee_start_screen.is_none() {
            return false;
        }
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    fn rotated_block_screen_bounds(&self, index: usize, viewport: Vec2) -> Option<(Vec2, Vec2)> {
        let obj = self.objects.get(index)?;

        let center_xy = Vec2::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
        );
        let half = Vec2::new(obj.size[0] * 0.5, obj.size[1] * 0.5);
        let angle = obj.rotation_degrees.to_radians();
        let (sin, cos) = angle.sin_cos();
        let rotate_xy = |v: Vec2| Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);

        let z0 = obj.position[2];
        let z1 = obj.position[2] + obj.size[2];

        let mut points = Vec::with_capacity(10);
        for local_x in [-half.x, half.x] {
            for local_y in [-half.y, half.y] {
                let rotated = rotate_xy(Vec2::new(local_x, local_y));
                for z in [z0, z1] {
                    let world = Vec3::new(center_xy.x + rotated.x, center_xy.y + rotated.y, z);
                    if let Some(screen) = self.world_to_screen_v(world, viewport) {
                        points.push(screen);
                    }
                }
            }
        }

        if let Some(center_screen) = self.world_to_screen_v(
            Vec3::new(center_xy.x, center_xy.y, z0 + obj.size[2] * 0.5),
            viewport,
        ) {
            points.push(center_screen);
        }

        if points.is_empty() {
            return None;
        }

        let mut min = points[0];
        let mut max = points[0];
        for p in points.into_iter().skip(1) {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }

        Some((min, max))
    }

    fn marquee_keypoint_hit(
        &self,
        rect_min: Vec2,
        rect_max: Vec2,
        viewport: Vec2,
    ) -> Option<usize> {
        let rect_center = (rect_min + rect_max) * 0.5;
        let mut best_hit: Option<(usize, f32)> = None;

        for (index, keypoint) in self.camera_keypoints().iter().enumerate() {
            let eye = self.camera_keypoint_marker_eye(keypoint);
            let Some(screen) = self.world_to_screen_v(eye, viewport) else {
                continue;
            };

            let overlaps = screen.x + CAMERA_KEYPOINT_MARQUEE_RADIUS_PX >= rect_min.x
                && screen.x - CAMERA_KEYPOINT_MARQUEE_RADIUS_PX <= rect_max.x
                && screen.y + CAMERA_KEYPOINT_MARQUEE_RADIUS_PX >= rect_min.y
                && screen.y - CAMERA_KEYPOINT_MARQUEE_RADIUS_PX <= rect_max.y;

            if !overlaps {
                continue;
            }

            let distance_to_center = screen.distance(rect_center);
            match best_hit {
                Some((_, best_distance)) if distance_to_center >= best_distance => {}
                _ => best_hit = Some((index, distance_to_center)),
            }
        }

        best_hit.map(|(index, _)| index)
    }

    fn apply_marquee_selection(&mut self, viewport: Vec2, additive: bool) {
        let Some(start) = self.ui.marquee_start_screen else {
            return;
        };
        let current = self.ui.marquee_current_screen.unwrap_or(start);

        let rect_min = Vec2::new(
            start[0].min(current[0]) as f32,
            start[1].min(current[1]) as f32,
        );
        let rect_max = Vec2::new(
            start[0].max(current[0]) as f32,
            start[1].max(current[1]) as f32,
        );

        let mut hits = Vec::new();
        for index in 0..self.objects.len() {
            let Some((obj_min, obj_max)) = self.rotated_block_screen_bounds(index, viewport) else {
                continue;
            };
            let overlaps = obj_max.x >= rect_min.x
                && obj_min.x <= rect_max.x
                && obj_max.y >= rect_min.y
                && obj_min.y <= rect_max.y;
            if overlaps {
                hits.push(index);
            }
        }

        if additive {
            for hit in hits {
                if !self.ui.selected_block_indices.contains(&hit) {
                    self.ui.selected_block_indices.push(hit);
                }
            }
        } else {
            self.ui.selected_block_indices = hits;
        }

        let keypoint_hit = self.marquee_keypoint_hit(rect_min, rect_max, viewport);
        if additive {
            if let Some(index) = keypoint_hit {
                self.set_camera_keypoint_selected(Some(index));
            }
        } else {
            self.set_camera_keypoint_selected(keypoint_hit);
        }

        self.sync_primary_selection_from_indices();
        self.selected_mask_cache = None;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        self.ui.hovered_block_index = self.ui.selected_block_index;

        if let Some(index) = self.ui.selected_block_index {
            if let Some(obj) = self.objects.get(index) {
                self.ui.cursor = [obj.position[0], obj.position[1], obj.position[2]];
            }
        }

        self.mark_dirty(EditorDirtyFlags {
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: self.ui.selected_block_index.is_some(),
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn finish_marquee_selection(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor || !self.ui.mode.is_selection_mode() {
            self.ui.marquee_start_screen = None;
            self.ui.marquee_current_screen = None;
            return false;
        }

        let had_marquee = self.ui.marquee_start_screen.is_some();
        if !had_marquee {
            return false;
        }

        self.ui.marquee_current_screen = Some([x, y]);
        let additive = self.ui.shift_held;

        if self.marquee_has_dragged_far_enough() {
            self.apply_marquee_selection(viewport, additive);
            self.ui.marquee_start_screen = None;
            self.ui.marquee_current_screen = None;
            return true;
        }

        self.ui.marquee_start_screen = None;
        self.ui.marquee_current_screen = None;
        false
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
            self.ui.cursor = [
                next_position[0],
                next_position[1],
                next_position[2].max(0.0),
            ];
        }

        true
    }

    pub(crate) fn begin_block_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if !self.ui.mode.is_selection_mode() {
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
                    start_blocks.push(EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                    });
                }
            }

            self.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [x, y],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks,
            });
            return true;
        }

        false
    }

    pub(crate) fn drag_selection_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor || self.ui.right_dragging || !self.ui.mode.is_selection_mode()
        {
            return false;
        }

        if self.drag_selection(x, y, viewport) {
            self.sync_objects_for_drag();
            self.mark_dirty(EditorDirtyFlags {
                rebuild_cursor: true,
                ..EditorDirtyFlags::default()
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn begin_selected_block_drag_ext(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.begin_block_drag(x, y, viewport) {
            self.record_history_state();
            true
        } else {
            false
        }
    }

    pub(crate) fn select_block_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> EditorInteractionChange {
        let selection_total_started_at = PlatformInstant::now();
        if phase != AppPhase::Editor || self.ui.right_dragging {
            self.perf_record(PerfStage::SelectionClick, selection_total_started_at);
            return EditorInteractionChange::None;
        }

        let additive = self.ui.shift_held;

        let pick_started_at = PlatformInstant::now();
        let Some(pick) = self.pick_from_screen(x, y, viewport) else {
            self.perf_record(PerfStage::SelectionPick, pick_started_at);
            let apply_started_at = PlatformInstant::now();
            if !additive {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
                self.ui.hovered_block_index = None;
                self.set_camera_keypoint_selected(None);
            }
            self.runtime.interaction.gizmo_drag = None;
            self.runtime.interaction.block_drag = None;
            self.perf_record(PerfStage::SelectionApply, apply_started_at);

            let mark_dirty_started_at = PlatformInstant::now();
            self.mark_dirty(EditorDirtyFlags {
                rebuild_block_mesh: true,
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
            self.perf_record(PerfStage::SelectionMarkDirty, mark_dirty_started_at);
            self.perf_record(PerfStage::SelectionClick, selection_total_started_at);
            return EditorInteractionChange::None;
        };
        self.perf_record(PerfStage::SelectionPick, pick_started_at);

        let mut changed = EditorInteractionChange::None;
        let apply_started_at = PlatformInstant::now();

        if let Some(hit_keypoint_index) = pick.hit_camera_keypoint_index {
            if additive && self.selected_camera_keypoint_index() == Some(hit_keypoint_index) {
                self.set_camera_keypoint_selected(None);
            } else {
                self.set_camera_keypoint_selected(Some(hit_keypoint_index));
            }

            if !additive {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
            }
            self.ui.hovered_block_index = None;
            changed = EditorInteractionChange::Hover;
        } else if let Some(hit_index) = pick.hit_block_index {
            if !additive {
                self.set_camera_keypoint_selected(None);
            }
            if additive {
                if let Some(existing) = self
                    .ui
                    .selected_block_indices
                    .iter()
                    .position(|idx| *idx == hit_index)
                {
                    self.ui.selected_block_indices.remove(existing);
                    if self.ui.selected_block_indices.is_empty() {
                        self.ui.selected_block_index = None;
                        self.ui.hovered_block_index = None;
                    }
                } else {
                    self.ui.selected_block_indices.push(hit_index);
                }
            } else {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_indices.push(hit_index);
            }
            self.ui.hovered_block_index = Some(hit_index);
            changed = EditorInteractionChange::Hover;
        } else if !additive {
            self.ui.selected_block_indices.clear();
            self.ui.selected_block_index = None;
            self.ui.hovered_block_index = None;
            self.set_camera_keypoint_selected(None);
            changed = EditorInteractionChange::Hover;
        }

        self.sync_primary_selection_from_indices();
        self.selected_mask_cache = None;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        if let Some(index) = self.ui.selected_block_index {
            if let Some(obj) = self.objects.get(index) {
                self.ui.cursor = [obj.position[0], obj.position[1], obj.position[2]];
                changed = EditorInteractionChange::Cursor;
            }
        } else if pick.cursor != self.ui.cursor {
            self.ui.cursor = pick.cursor;
            changed = EditorInteractionChange::Cursor;
        }

        self.perf_record(PerfStage::SelectionApply, apply_started_at);

        let mark_dirty_started_at = PlatformInstant::now();
        self.mark_dirty(EditorDirtyFlags {
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: matches!(changed, EditorInteractionChange::Cursor),
            ..EditorDirtyFlags::default()
        });
        self.perf_record(PerfStage::SelectionMarkDirty, mark_dirty_started_at);
        self.perf_record(PerfStage::SelectionClick, selection_total_started_at);

        changed
    }
}

impl State {
    pub(crate) fn begin_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        self.editor.begin_marquee_selection(x, y, self.phase)
    }

    pub(crate) fn update_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        self.editor.update_marquee_selection(x, y, self.phase)
    }

    pub(crate) fn finish_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        let had_marquee = self.editor.ui.marquee_start_screen.is_some();
        if !had_marquee {
            return false;
        }

        let marquee_consumed =
            self.editor
                .finish_marquee_selection(x, y, viewport_size, self.phase);
        if marquee_consumed {
            return true;
        }

        if self.phase == AppPhase::Editor && self.editor.mode().is_selection_mode() {
            self.editor
                .select_block_from_screen(x, y, viewport_size, self.phase);
            return true;
        }

        false
    }

    pub(crate) fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .drag_selection_from_screen(x, y, viewport, self.phase)
    }

    pub(crate) fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .begin_selected_block_drag_ext(x, y, viewport_size, self.phase)
    }
}
