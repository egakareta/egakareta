/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::{
    EditorBlockDrag, EditorDirtyFlags, EditorDragBlockStart, EditorSubsystem, State,
};
use crate::types::{AppPhase, EditorInteractionChange};
use glam::{EulerRot, Mat3, Vec2, Vec3};

const MARQUEE_DRAG_THRESHOLD_PX: f64 = 4.0;
const CAMERA_TRIGGER_MARQUEE_RADIUS_PX: f32 = 16.0;

fn unsnapped_drag_y_position(start_y: f32, raw_y: f32, snap_step: f32) -> f32 {
    if start_y.abs() <= f32::EPSILON && raw_y < snap_step * 0.5 {
        0.0
    } else {
        raw_y.max(0.0)
    }
}

fn drag_anchor_position(drag: &EditorBlockDrag) -> [f32; 3] {
    let mut anchor = [f32::INFINITY; 3];
    for block in &drag.start_blocks {
        anchor[0] = anchor[0].min(block.position[0]);
        anchor[1] = anchor[1].min(block.position[1]);
        anchor[2] = anchor[2].min(block.position[2]);
    }

    if anchor.iter().all(|component| component.is_finite()) {
        anchor
    } else {
        [0.0, 0.0, 0.0]
    }
}

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
        let allows_marquee = self.ui.mode.is_selection_mode();
        if phase != AppPhase::Editor || self.ui.right_dragging || !allows_marquee {
            return false;
        }
        self.ui.marquee_start_screen = Some([x, y]);
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    pub(crate) fn update_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        let allows_marquee = self.ui.mode.is_selection_mode();
        if phase != AppPhase::Editor || self.ui.right_dragging || !allows_marquee {
            return false;
        }
        if self.ui.marquee_start_screen.is_none() {
            return false;
        }
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    pub(crate) fn marquee_overlapping_blocks(&self, viewport: Vec2) -> Vec<usize> {
        let Some(start) = self.ui.marquee_start_screen else {
            return Vec::new();
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
        hits
    }

    fn rotated_block_screen_bounds(&self, index: usize, viewport: Vec2) -> Option<(Vec2, Vec2)> {
        let obj = self.objects.get(index)?;

        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let rotation = Mat3::from_euler(
            EulerRot::XYZ,
            obj.rotation_degrees[0].to_radians(),
            obj.rotation_degrees[1].to_radians(),
            obj.rotation_degrees[2].to_radians(),
        );

        let mut bounds: Option<(Vec2, Vec2)> = None;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let local = Vec3::new(half.x * sx, half.y * sy, half.z * sz);
                    let world = center + rotation * local;
                    if let Some(screen) = self.world_to_screen_v(world, viewport) {
                        grow_screen_bounds(&mut bounds, screen);
                    }
                }
            }
        }

        if let Some(center_screen) = self.world_to_screen_v(center, viewport) {
            grow_screen_bounds(&mut bounds, center_screen);
        }

        bounds
    }

    fn marquee_trigger_hit(&self, rect_min: Vec2, rect_max: Vec2, viewport: Vec2) -> Option<usize> {
        let rect_center = (rect_min + rect_max) * 0.5;
        let mut best_hit: Option<(usize, f32)> = None;

        for (trigger_index, camera_trigger) in self.camera_trigger_markers() {
            let eye = self.camera_trigger_marker_eye(&camera_trigger);
            let Some(screen) = self.world_to_screen_v(eye, viewport) else {
                continue;
            };

            let overlaps = screen.x + CAMERA_TRIGGER_MARQUEE_RADIUS_PX >= rect_min.x
                && screen.x - CAMERA_TRIGGER_MARQUEE_RADIUS_PX <= rect_max.x
                && screen.y + CAMERA_TRIGGER_MARQUEE_RADIUS_PX >= rect_min.y
                && screen.y - CAMERA_TRIGGER_MARQUEE_RADIUS_PX <= rect_max.y;

            if !overlaps {
                continue;
            }

            let distance_to_center = screen.distance(rect_center);
            match best_hit {
                Some((_, best_distance)) if distance_to_center >= best_distance => {}
                _ => best_hit = Some((trigger_index, distance_to_center)),
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

        if self.ui.mode.is_selection_mode() {
            let hits = self.marquee_overlapping_blocks(viewport);

            if additive {
                let selected_mask = self.selected_mask_for_len(self.objects.len());
                for hit in hits {
                    if hit < selected_mask.len() && !selected_mask[hit] {
                        self.add_block_to_selection(hit);
                    }
                }
            } else {
                self.replace_block_selection(hits);
            }
        } else {
            self.clear_block_selection();
        }

        let trigger_hit = self.marquee_trigger_hit(rect_min, rect_max, viewport);
        if additive {
            if let Some(index) = trigger_hit {
                self.set_trigger_selected(Some(index));
            }
        } else {
            self.set_trigger_selected(trigger_hit);
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

        self.mark_dirty(EditorDirtyFlags::selection_cursor_changed(
            self.ui.selected_block_index.is_some(),
        ));
    }

    pub(crate) fn finish_marquee_selection(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        let allows_marquee = self.ui.mode.is_selection_mode();
        if phase != AppPhase::Editor || !allows_marquee {
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

        // Cast a ray through the current cursor position.
        let Some((ray_origin, ray_dir)) = self.screen_to_ray(x, y, viewport) else {
            return true;
        };

        // Intersect with a camera-perpendicular plane through the selection center.
        // This naturally handles all 3 axes including Y (depth).
        let plane_point = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );
        let camera_forward = self.camera_forward();
        let denom = ray_dir.dot(camera_forward);
        if denom.abs() <= f32::EPSILON {
            return true;
        }
        let t = (plane_point - ray_origin).dot(camera_forward) / denom;
        if t < 0.0 {
            return true;
        }
        let current_hit = ray_origin + ray_dir * t;
        let start_drag = Vec3::new(
            drag.start_drag_world[0],
            drag.start_drag_world[1],
            drag.start_drag_world[2],
        );
        let world_delta = current_hit - start_drag;

        let snap_enabled = self.effective_snap_to_grid();
        let snap_step = self.config.snap_step.max(0.05);
        let excluded_indices: Vec<usize> =
            drag.start_blocks.iter().map(|block| block.index).collect();
        let raycast_delta = self
            .pick_block_cursor_from_screen_excluding(x, y, viewport, &excluded_indices)
            .map(|cursor| {
                let anchor = drag_anchor_position(&drag);
                [
                    cursor[0] - drag.start_cursor[0],
                    cursor[1] - anchor[1],
                    cursor[2] - drag.start_cursor[2],
                ]
            });

        // Use the raw surface hit for Y when raycast snapping is active.
        // `pick_block_cursor_from_screen_excluding` adds a 0.01 normal nudge
        // (cursor_from_ray_hit) which is fine for X/Z (start_cursor has the
        // same nudge so it cancels) but for Y the delta is computed against
        // the anchor (block position) without the nudge, so the 0.01 leaks
        // through and makes blocks float above surfaces.
        let raycast_surface_y = if raycast_delta.is_some() {
            self.pick_block_surface_from_screen_excluding(x, y, viewport, &excluded_indices)
                .map(|surface| {
                    let anchor = drag_anchor_position(&drag);
                    surface[1] - anchor[1]
                })
        } else {
            None
        };

        let effective_raycast_y = raycast_surface_y.or_else(|| raycast_delta.map(|d| d[1]));

        // Clamp Y delta so the lowest block stops at y=0 instead of each
        // block being clamped independently (which compresses stacked blocks
        // into the same position).
        let raw_delta_y = effective_raycast_y.unwrap_or(world_delta.y);
        let min_start_y = drag
            .start_blocks
            .iter()
            .map(|b| b.position[1])
            .fold(f32::INFINITY, f32::min);
        let y_shift = if min_start_y + raw_delta_y < 0.0 {
            -(min_start_y + raw_delta_y)
        } else {
            0.0
        };

        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.objects.get_mut(block.index) {
                let mut next = if let Some(delta) = raycast_delta {
                    let y = effective_raycast_y.unwrap_or(delta[1]);
                    [
                        block.position[0] + delta[0],
                        block.position[1] + y + y_shift,
                        block.position[2] + delta[2],
                    ]
                } else {
                    [
                        block.position[0] + world_delta.x,
                        block.position[1] + world_delta.y + y_shift,
                        block.position[2] + world_delta.z,
                    ]
                };

                if raycast_delta.is_none() && snap_enabled {
                    next[0] = (next[0] / snap_step).round() * snap_step;
                    next[1] = (next[1].max(0.0) / snap_step).round() * snap_step;
                    next[2] = (next[2] / snap_step).round() * snap_step;
                } else if raycast_delta.is_none() {
                    next[1] = unsnapped_drag_y_position(block.position[1], next[1], snap_step);
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

            // Compute initial drag world position by intersecting the cursor ray
            // with a camera-perpendicular plane through the selection center.
            let start_drag_world = self
                .screen_to_ray(x, y, viewport_size)
                .and_then(|(ray_origin, ray_dir)| {
                    let camera_forward = self.camera_forward();
                    let denom = ray_dir.dot(camera_forward);
                    if denom.abs() <= f32::EPSILON {
                        return None;
                    }
                    let t = (center - ray_origin).dot(camera_forward) / denom;
                    if t < 0.0 {
                        return None;
                    }
                    Some(ray_origin + ray_dir * t)
                })
                .unwrap_or(center);

            let mut start_blocks = Vec::with_capacity(selected_indices.len());
            for index in selected_indices {
                if let Some(obj) = self.objects.get(index) {
                    start_blocks.push(EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                        rotation_degrees: obj.rotation_degrees,
                    });
                }
            }

            self.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [x, y],
                start_center_world: [center.x, center.y, center.z],
                start_drag_world: [start_drag_world.x, start_drag_world.y, start_drag_world.z],
                start_blocks,
                start_cursor: pick.cursor,
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
            // When a transform trigger block is being moved on the grid via
            // click-and-drag, the source ring + connector line must follow
            // the block pose live.
            let dragging_transform_trigger_block = self
                .runtime
                .interaction
                .block_drag
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
                .block_drag
                .as_ref()
                .map(|drag| drag.start_blocks.iter().map(|b| b.index).collect())
                .unwrap_or_default();
            let dragging_transform_trigger_source =
                self.any_block_is_transform_trigger_source(&dragged_indices);
            self.mark_dirty(EditorDirtyFlags {
                rebuild_cursor: true,
                rebuild_transform_trigger_markers: dragging_transform_trigger_block
                    || dragging_transform_trigger_source,
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
        puffin::profile_scope!("SelectClick");
        if phase != AppPhase::Editor || self.ui.right_dragging {
            return EditorInteractionChange::None;
        }

        let additive = self.ui.shift_held;

        let pick = {
            puffin::profile_scope!("SelectPick");
            self.pick_from_screen(x, y, viewport)
        };
        let Some(pick) = pick else {
            {
                puffin::profile_scope!("SelectApply");
                if !additive {
                    self.clear_block_selection();
                    self.set_trigger_selected(None);
                }
                self.runtime.interaction.gizmo_drag = None;
                self.runtime.interaction.block_drag = None;
            }

            {
                puffin::profile_scope!("SelectMarkDirty");
                self.mark_dirty(EditorDirtyFlags::selection_overlay_changed());
            }
            return EditorInteractionChange::None;
        };

        let mut changed = EditorInteractionChange::None;

        {
            puffin::profile_scope!("SelectApply");
            if let Some(hit_index) = pick.hit_block_index {
                if !additive {
                    self.set_trigger_selected(None);
                }
                if additive {
                    if self.ui.selected_block_indices.contains(&hit_index) {
                        self.remove_block_from_selection(hit_index);
                    } else {
                        self.add_block_to_selection(hit_index);
                    }
                } else {
                    self.replace_block_selection(vec![hit_index]);
                }
                self.ui.hovered_block_index = Some(hit_index);
                changed = EditorInteractionChange::Hover;
            } else if !additive {
                self.clear_block_selection();
                self.set_trigger_selected(None);
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
        }

        {
            puffin::profile_scope!("SelectMarkDirty");
            self.mark_dirty(EditorDirtyFlags::selection_cursor_changed(matches!(
                changed,
                EditorInteractionChange::Cursor
            )));
        }

        changed
    }
}

impl State {
    pub(crate) fn begin_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let handled = self.editor.begin_marquee_selection(x, y, self.phase);
        if handled {
            self.editor
                .mark_dirty(EditorDirtyFlags::selection_overlay_changed());
        }
        handled
    }

    pub(crate) fn update_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let handled = self.editor.update_marquee_selection(x, y, self.phase);
        if handled {
            if self.editor.marquee_has_dragged_far_enough() {
                let viewport_size = Vec2::new(
                    self.render.gpu.config.width as f32,
                    self.render.gpu.config.height as f32,
                );
                let additive = self.editor.ui.shift_held;
                self.editor.apply_marquee_selection(viewport_size, additive);
            }
            self.editor
                .mark_dirty(EditorDirtyFlags::selection_overlay_changed());
        }
        handled
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

        self.editor
            .mark_dirty(EditorDirtyFlags::selection_overlay_changed());

        let mode = self.editor.mode();
        if self.phase == AppPhase::Editor && mode.is_selection_mode() {
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

fn grow_screen_bounds(bounds: &mut Option<(Vec2, Vec2)>, point: Vec2) {
    match bounds {
        Some((min, max)) => {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        None => *bounds = Some((point, point)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use super::super::super::{EditorBlockDrag, EditorDirtyFlags, EditorDragBlockStart};
    use crate::test_utils::assert_approx_eq as approx_eq;
    use crate::types::{AppPhase, EditorInteractionChange, EditorMode, LevelObject};
    use glam::{Vec2, Vec3};

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
        }
    }

    fn default_viewport() -> Vec2 {
        Vec2::new(1280.0, 720.0)
    }

    fn drag_delta_for_screen(
        state: &State,
        center: Vec3,
        screen: Vec2,
        viewport: Vec2,
    ) -> Option<Vec3> {
        let (ray_origin, ray_dir) =
            state
                .editor
                .screen_to_ray(screen.x as f64, screen.y as f64, viewport)?;
        let camera_forward = state.editor.camera_forward();
        let denom = ray_dir.dot(camera_forward);
        if denom.abs() <= f32::EPSILON {
            return None;
        }
        let t = (center - ray_origin).dot(camera_forward) / denom;
        if t < 0.0 {
            return None;
        }
        Some(ray_origin + ray_dir * t - center)
    }

    fn screen_for_positive_y_delta(
        state: &State,
        center: Vec3,
        origin_screen: Vec2,
        viewport: Vec2,
        min_y: f32,
        max_y: f32,
    ) -> (Vec2, f32) {
        let directions = [Vec2::Y, -Vec2::Y, Vec2::X, -Vec2::X];
        for pixels in 1..=1000 {
            for direction in directions {
                let screen = origin_screen + direction * pixels as f32;
                if let Some(delta) = drag_delta_for_screen(state, center, screen, viewport) {
                    if delta.y > min_y && delta.y < max_y {
                        return (screen, delta.y);
                    }
                }
            }
        }
        panic!("could not find drag target with positive Y delta in requested range");
    }

    #[test]
    fn marquee_threshold_activates_only_after_minimum_drag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.marquee_start_screen = Some([100.0, 100.0]);
            state.editor.ui.marquee_current_screen = Some([102.0, 102.0]);
            let (_, _, active) = state
                .editor
                .marquee_selection_rect_screen()
                .expect("marquee rect");
            assert!(!active);

            state.editor.ui.marquee_current_screen = Some([120.0, 120.0]);
            let (_, _, active) = state
                .editor
                .marquee_selection_rect_screen()
                .expect("marquee rect");
            assert!(active);
        });
    }

    #[test]
    fn begin_marquee_requires_editor_phase_and_supported_mode() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Timing;
            assert!(!state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Editor));

            state.editor.ui.mode = EditorMode::Select;
            assert!(!state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Menu));

            assert!(state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Editor));
            assert!(state.editor.ui.marquee_start_screen.is_some());
            assert!(state.editor.ui.marquee_current_screen.is_some());
        });
    }

    #[test]
    fn finish_marquee_clears_state_when_not_dragged_far_enough() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Select;
            assert!(state
                .editor
                .begin_marquee_selection(100.0, 100.0, AppPhase::Editor));
            assert!(state
                .editor
                .update_marquee_selection(101.0, 101.0, AppPhase::Editor));

            let changed = state.editor.finish_marquee_selection(
                101.0,
                101.0,
                Vec2::new(1280.0, 720.0),
                AppPhase::Editor,
            );
            assert!(!changed);
            assert!(state.editor.ui.marquee_start_screen.is_none());
            assert!(state.editor.ui.marquee_current_screen.is_none());
        });
    }

    #[test]
    fn finish_marquee_with_large_rect_selects_visible_block() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            assert!(state
                .editor
                .begin_marquee_selection(0.0, 0.0, AppPhase::Editor));
            assert!(state
                .editor
                .update_marquee_selection(2000.0, 2000.0, AppPhase::Editor));

            let changed = state.editor.finish_marquee_selection(
                2000.0,
                2000.0,
                Vec2::new(1280.0, 720.0),
                AppPhase::Editor,
            );

            assert!(changed);
            assert!(state.editor.ui.selected_block_indices.contains(&0));
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
        });
    }

    #[test]
    fn finish_marquee_additive_selection_avoids_duplicate_indices() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.ui.shift_held = true;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);

            assert!(state
                .editor
                .begin_marquee_selection(0.0, 0.0, AppPhase::Editor));
            assert!(state
                .editor
                .update_marquee_selection(2000.0, 2000.0, AppPhase::Editor));

            let changed = state.editor.finish_marquee_selection(
                2000.0,
                2000.0,
                Vec2::new(1280.0, 720.0),
                AppPhase::Editor,
            );

            assert!(changed);
            assert_eq!(state.editor.ui.selected_block_indices, vec![0, 1]);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
        });
    }

    #[test]
    fn marquee_updates_select_immediately_and_defer_mesh_rebuilds() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            state.render.meshes.editor_hover_outline.clear();

            assert!(state.begin_editor_marquee_selection(0.0, 0.0));
            state.editor.runtime.dirty = EditorDirtyFlags::default();

            for [x, y] in [[2000.0, 2000.0], [20.0, 20.0], [2000.0, 2000.0]] {
                assert!(state.update_editor_marquee_selection(x, y));
                assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
                assert!(state
                    .render
                    .meshes
                    .editor_hover_outline
                    .draw_data()
                    .is_none());
                state.editor.runtime.dirty = EditorDirtyFlags::default();
            }

            state.editor.mark_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
            state.process_editor_dirty(1.0 / 60.0);
            assert!(state
                .render
                .meshes
                .editor_hover_outline
                .draw_data()
                .is_none());
            assert!(state
                .render
                .meshes
                .editor_selection_outline
                .draw_data()
                .is_some());
        });
    }

    #[test]
    fn active_marquee_update_selects_visible_block_immediately() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            assert!(state.begin_editor_marquee_selection(0.0, 0.0));
            assert!(state.update_editor_marquee_selection(2000.0, 2000.0));

            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.hovered_block_index, Some(0));
        });
    }

    #[test]
    fn update_marquee_requires_existing_start_and_valid_phase() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Select;
            assert!(!state
                .editor
                .update_marquee_selection(10.0, 10.0, AppPhase::Editor));

            state.editor.ui.marquee_start_screen = Some([0.0, 0.0]);
            state.editor.ui.right_dragging = true;
            assert!(!state
                .editor
                .update_marquee_selection(10.0, 10.0, AppPhase::Editor));

            state.editor.ui.right_dragging = false;
            assert!(!state
                .editor
                .update_marquee_selection(10.0, 10.0, AppPhase::Menu));

            assert!(state
                .editor
                .update_marquee_selection(10.0, 10.0, AppPhase::Editor));
            assert_eq!(state.editor.ui.marquee_current_screen, Some([10.0, 10.0]));
        });
    }

    #[test]
    fn begin_block_drag_and_extended_drag_record_history_on_success() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();
            let block_center = Vec3::new(0.5, 0.5, 0.5);

            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);

            let click = state
                .editor
                .world_to_screen_v(block_center, viewport)
                .expect("block center should project to screen");

            assert!(state
                .editor
                .begin_block_drag(click.x as f64, click.y as f64, viewport));
            let drag = state
                .editor
                .runtime
                .interaction
                .block_drag
                .as_ref()
                .expect("block drag should be initialized");
            assert_eq!(drag.start_blocks.len(), 1);
            assert_eq!(drag.start_blocks[0].index, 0);

            let history_before = state.editor.runtime.history.undo.len();
            state.editor.runtime.interaction.block_drag = None;
            assert!(state.editor.begin_selected_block_drag_ext(
                click.x as f64,
                click.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert_eq!(
                state.editor.runtime.history.undo.len(),
                history_before + 1,
                "begin_selected_block_drag_ext should capture undo history"
            );
            assert!(!state.editor.begin_selected_block_drag_ext(
                click.x as f64,
                click.y as f64,
                viewport,
                AppPhase::Menu,
            ));
        });
    }

    #[test]
    fn drag_selection_handles_missing_drag_and_zero_delta() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            assert!(!state.editor.drag_selection(10.0, 20.0, viewport));

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [10.0, 20.0],
                start_center_world: [0.5, 0.5, 0.5],
                start_drag_world: [0.5, 0.5, 0.5],
                start_cursor: [0.0, 0.0, 0.0],
                start_blocks: vec![EditorDragBlockStart {
                    index: 0,
                    position: state.editor.objects[0].position,
                    size: state.editor.objects[0].size,
                    rotation_degrees: state.editor.objects[0].rotation_degrees,
                }],
            });

            let before = state.editor.objects[0].position;
            assert!(state.editor.drag_selection(10.0, 20.0, viewport));
            assert_eq!(before, state.editor.objects[0].position);
        });
    }

    #[test]
    fn drag_selection_from_screen_updates_objects_and_marks_cursor_dirty() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            state.editor.ui.mode = EditorMode::Select;
            state.editor.config.snap_to_grid = false;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center should project to screen");

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_world: [center.x, center.y, center.z],
                start_drag_world: [center.x, center.y, center.z],
                start_cursor: [center.x, center.y, center.z],
                start_blocks: vec![EditorDragBlockStart {
                    index: 0,
                    position: state.editor.objects[0].position,
                    size: state.editor.objects[0].size,
                    rotation_degrees: state.editor.objects[0].rotation_degrees,
                }],
            });

            state.editor.runtime.dirty = EditorDirtyFlags::default();
            let before = state.editor.objects[0].position;

            assert!(state.editor.drag_selection_from_screen(
                origin_screen.x as f64 + 40.0,
                origin_screen.y as f64,
                viewport,
                AppPhase::Editor,
            ));
            assert_ne!(before, state.editor.objects[0].position);
            assert!(state.editor.runtime.dirty.rebuild_cursor);

            assert!(!state.editor.drag_selection_from_screen(
                origin_screen.x as f64,
                origin_screen.y as f64,
                viewport,
                AppPhase::Menu,
            ));

            state.editor.ui.right_dragging = true;
            assert!(!state.editor.drag_selection_from_screen(
                origin_screen.x as f64,
                origin_screen.y as f64,
                viewport,
                AppPhase::Editor,
            ));
        });
    }

    #[test]
    fn drag_selection_without_snap_pins_small_ground_drift_but_allows_y_lift() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            state.editor.ui.mode = EditorMode::Select;
            state.editor.config.snap_to_grid = false;
            state.editor.config.snap_step = 1.0;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center should project to screen");
            let release_height = state.editor.config.snap_step * 0.5;

            let (small_drift_screen, small_drift_y) = screen_for_positive_y_delta(
                &state,
                center,
                origin_screen,
                viewport,
                0.0,
                release_height,
            );

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_world: [center.x, center.y, center.z],
                start_drag_world: [center.x, center.y, center.z],
                start_cursor: [center.x, center.y, center.z],
                start_blocks: vec![EditorDragBlockStart {
                    index: 0,
                    position: state.editor.objects[0].position,
                    size: state.editor.objects[0].size,
                    rotation_degrees: state.editor.objects[0].rotation_degrees,
                }],
            });

            assert!(state.editor.drag_selection(
                small_drift_screen.x as f64,
                small_drift_screen.y as f64,
                viewport,
            ));

            assert!(small_drift_y > 0.0);
            approx_eq(state.editor.objects[0].position[1], 0.0, 1e-6);
            approx_eq(state.editor.ui.cursor[1], 0.0, 1e-6);

            let (lift_screen, lift_y) = screen_for_positive_y_delta(
                &state,
                center,
                origin_screen,
                viewport,
                release_height,
                f32::INFINITY,
            );
            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_world: [center.x, center.y, center.z],
                start_drag_world: [center.x, center.y, center.z],
                start_cursor: [center.x, center.y, center.z],
                start_blocks: vec![EditorDragBlockStart {
                    index: 0,
                    position: [0.0, 0.0, 0.0],
                    size: state.editor.objects[0].size,
                    rotation_degrees: state.editor.objects[0].rotation_degrees,
                }],
            });

            assert!(state.editor.drag_selection(
                lift_screen.x as f64,
                lift_screen.y as f64,
                viewport,
            ));

            approx_eq(state.editor.objects[0].position[1], lift_y, 1e-5);
            approx_eq(state.editor.ui.cursor[1], lift_y, 1e-5);
        });
    }

    #[test]
    fn drag_selection_raycast_ignores_dragged_block_and_lands_on_block_behind_it() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            let mut support = test_block([-4.0, 0.0, -4.0]);
            support.size = [8.0, 1.0, 8.0];
            let floating = test_block([0.0, 5.0, 0.0]);

            state.editor.ui.mode = EditorMode::Select;
            state.editor.config.snap_to_grid = true;
            state.editor.config.snap_step = 1.0;
            state.editor.objects = vec![support, floating];
            state.editor.ui.selected_block_indices = vec![1];
            state.editor.ui.selected_block_index = Some(1);

            let floating_center = Vec3::new(0.5, 5.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(floating_center, viewport)
                .expect("floating block center should project to screen");
            let support_top_screen = state
                .editor
                .world_to_screen_v(Vec3::new(0.5, 1.0, 0.5), viewport)
                .expect("support top should project to screen");

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_world: [floating_center.x, floating_center.y, floating_center.z],
                start_drag_world: [floating_center.x, floating_center.y, floating_center.z],
                start_cursor: [floating_center.x, floating_center.y, floating_center.z],
                start_blocks: vec![EditorDragBlockStart {
                    index: 1,
                    position: state.editor.objects[1].position,
                    size: state.editor.objects[1].size,
                    rotation_degrees: state.editor.objects[1].rotation_degrees,
                }],
            });

            assert!(state.editor.drag_selection(
                support_top_screen.x as f64,
                support_top_screen.y as f64,
                viewport,
            ));

            approx_eq(state.editor.objects[1].position[1], 1.0, 1e-6);
            approx_eq(state.editor.ui.cursor[1], 1.0, 1e-6);
        });
    }

    #[test]
    fn drag_selection_maintains_relative_grab_offset_instead_of_corner_snapping() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            let mut support = test_block([-4.0, 0.0, -4.0]);
            support.size = [8.0, 1.0, 8.0];
            let floating = test_block([0.0, 5.0, 0.0]); // block at [0, 5, 0] center is [0.5, 5.5, 0.5]

            state.editor.ui.mode = EditorMode::Select;
            state.editor.config.snap_to_grid = false;
            state.editor.config.snap_step = 1.0;
            state.editor.objects = vec![support, floating];
            state.editor.ui.selected_block_indices = vec![1];
            state.editor.ui.selected_block_index = Some(1);

            let grab_world = Vec3::new(0.75, 5.5, 0.25); // Grab at an offset near a corner
            let grab_screen = state
                .editor
                .world_to_screen_v(grab_world, viewport)
                .expect("grab offset should project to screen");

            let target_world = Vec3::new(2.75, 1.0, 3.25);
            let target_screen = state
                .editor
                .world_to_screen_v(target_world, viewport)
                .expect("target offset should project to screen");

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [grab_screen.x as f64, grab_screen.y as f64],
                start_center_world: [0.5, 5.5, 0.5],
                start_drag_world: [0.75, 5.5, 0.25],
                start_cursor: [0.75, 5.5, 0.25],
                start_blocks: vec![EditorDragBlockStart {
                    index: 1,
                    position: state.editor.objects[1].position,
                    size: state.editor.objects[1].size,
                    rotation_degrees: state.editor.objects[1].rotation_degrees,
                }],
            });

            assert!(state.editor.drag_selection(
                target_screen.x as f64,
                target_screen.y as f64,
                viewport,
            ));

            // We moved the grab cursor from X=0.75 -> 2.75 (+2.0 offset)
            // Initial position X was 0.0, so new position X should be 2.0
            // We moved the grab cursor from Z=0.25 -> 3.25 (+3.0 offset)
            // Initial position Z was 0.0, so new position Z should be 3.0

            // Height drops to sit on top of the block below (Y=1)
            approx_eq(state.editor.objects[1].position[0], 2.0, 1e-4);
            approx_eq(state.editor.objects[1].position[1], 1.0, 1e-4);
            approx_eq(state.editor.objects[1].position[2], 3.0, 1e-4);
        });
    }

    #[test]
    fn select_block_from_screen_handles_pick_miss_and_additive_toggle() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();
            let block_center = Vec3::new(0.5, 0.5, 0.5);

            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            state.editor.runtime.dirty = EditorDirtyFlags::default();

            let click = state
                .editor
                .world_to_screen_v(block_center, viewport)
                .expect("block center should project to screen");

            let select_result = state.editor.select_block_from_screen(
                click.x as f64,
                click.y as f64,
                viewport,
                AppPhase::Editor,
            );
            assert_eq!(select_result, EditorInteractionChange::Cursor);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert!(!state.editor.runtime.dirty.rebuild_block_mesh);
            assert!(state.editor.runtime.dirty.rebuild_selection_overlays);
            assert!(state.editor.runtime.dirty.rebuild_cursor);

            state.editor.ui.shift_held = true;
            let toggle_result = state.editor.select_block_from_screen(
                click.x as f64,
                click.y as f64,
                viewport,
                AppPhase::Editor,
            );
            assert!(
                matches!(
                    toggle_result,
                    EditorInteractionChange::Hover | EditorInteractionChange::Cursor
                ),
                "expected a visible interaction change when toggling additive selection"
            );
            assert!(state.editor.ui.selected_block_indices.is_empty());
            assert_eq!(state.editor.ui.selected_block_index, None);

            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.shift_held = false;
            let miss_result = state.editor.select_block_from_screen(
                10.0,
                10.0,
                Vec2::new(0.0, viewport.y),
                AppPhase::Editor,
            );
            assert_eq!(miss_result, EditorInteractionChange::None);
            assert!(state.editor.ui.selected_block_indices.is_empty());
            assert_eq!(state.editor.ui.selected_block_index, None);

            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.shift_held = true;
            let additive_miss_result = state.editor.select_block_from_screen(
                10.0,
                10.0,
                Vec2::new(0.0, viewport.y),
                AppPhase::Editor,
            );
            assert_eq!(additive_miss_result, EditorInteractionChange::None);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
        });
    }

    #[test]
    fn select_block_without_shift_replaces_previous_selection() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])];

            let first_click = state
                .editor
                .world_to_screen_v(Vec3::new(0.5, 0.5, 0.5), viewport)
                .expect("first block center should project to screen");
            let second_click = state
                .editor
                .world_to_screen_v(Vec3::new(2.5, 0.5, 0.5), viewport)
                .expect("second block center should project to screen");

            state.editor.select_block_from_screen(
                first_click.x as f64,
                first_click.y as f64,
                viewport,
                AppPhase::Editor,
            );
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);

            state.editor.select_block_from_screen(
                second_click.x as f64,
                second_click.y as f64,
                viewport,
                AppPhase::Editor,
            );
            assert_eq!(state.editor.ui.selected_block_index, Some(1));
            assert_eq!(state.editor.ui.selected_block_indices, vec![1]);
        });
    }

    #[test]
    fn drag_selection_transform_trigger_source_sets_dirty_flag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let viewport = default_viewport();

            state.editor.ui.mode = EditorMode::Select;
            state.editor.config.snap_to_grid = false;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            // Add a transform trigger that targets object 0
            state
                .editor
                .set_triggers(vec![crate::triggers::TimedTrigger {
                    time_seconds: 1.0,
                    duration_seconds: 1.0,
                    easing: crate::triggers::TimedTriggerEasing::Linear,
                    target: crate::triggers::TimedTriggerTarget::Objects {
                        object_ids: vec![0],
                    },
                    action: crate::triggers::TimedTriggerAction::TransformObjects {
                        position: [5.0, 0.0, 0.0],
                        rotation_degrees: [0.0, 0.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                    },
                }]);

            let center = Vec3::new(0.5, 0.5, 0.5);
            let origin_screen = state
                .editor
                .world_to_screen_v(center, viewport)
                .expect("center should project to screen");

            state.editor.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
                start_center_world: [center.x, center.y, center.z],
                start_drag_world: [center.x, center.y, center.z],
                start_cursor: [center.x, center.y, center.z],
                start_blocks: vec![EditorDragBlockStart {
                    index: 0,
                    position: state.editor.objects[0].position,
                    size: state.editor.objects[0].size,
                    rotation_degrees: state.editor.objects[0].rotation_degrees,
                }],
            });

            state.editor.runtime.dirty = EditorDirtyFlags::default();

            assert!(state.editor.drag_selection_from_screen(
                origin_screen.x as f64 + 40.0,
                origin_screen.y as f64,
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
