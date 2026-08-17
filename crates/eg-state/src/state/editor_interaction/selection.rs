/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::{AppPhase, EditorInteractionChange};
#[cfg(test)]
use eg_types::{EditorInputState, LevelObject};
use glam::{EulerRot, Mat3, Vec2, Vec3};

use super::super::{EditorBlockDrag, EditorDragBlockStart};
#[cfg(test)]
use crate::state::project_world_to_screen;

const MARQUEE_DRAG_THRESHOLD_PX: f64 = 4.0;
const CAMERA_TRIGGER_MARQUEE_RADIUS_PX: f32 = 16.0;
const BLOCK_FACE_CORNER_INDICES: [[usize; 4]; 6] = [
    [0, 2, 3, 1],
    [4, 5, 7, 6],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 4, 6, 2],
    [1, 3, 7, 5],
];

#[derive(Clone, Copy)]
struct ProjectedBlockScreenGeometry {
    bounds: (Vec2, Vec2),
    corners: [Option<Vec2>; 8],
}

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

#[cfg(test)]
pub(crate) fn marquee_has_dragged_far_enough(input: &EditorInputState) -> bool {
    let Some(start) = input.marquee_start_screen else {
        return false;
    };
    let current = input.marquee_current_screen.unwrap_or(start);
    let dx = current[0] - start[0];
    let dy = current[1] - start[1];
    (dx * dx + dy * dy) >= MARQUEE_DRAG_THRESHOLD_PX * MARQUEE_DRAG_THRESHOLD_PX
}

#[cfg(test)]
pub(crate) fn begin_marquee_selection_for_input(
    input: &mut EditorInputState,
    x: f64,
    y: f64,
    phase: AppPhase,
) -> bool {
    if phase != AppPhase::Editor || input.right_dragging || !input.mode.is_transform_mode() {
        return false;
    }
    input.marquee_start_screen = Some([x, y]);
    input.marquee_current_screen = Some([x, y]);
    true
}

#[cfg(test)]
pub(crate) fn update_marquee_selection_for_input(
    input: &mut EditorInputState,
    x: f64,
    y: f64,
    phase: AppPhase,
) -> bool {
    if phase != AppPhase::Editor
        || input.right_dragging
        || !input.mode.is_transform_mode()
        || input.marquee_start_screen.is_none()
    {
        return false;
    }
    input.marquee_current_screen = Some([x, y]);
    true
}

super::super::editor_context_methods! {
    pub(crate) fn marquee_has_dragged_far_enough(&self) -> bool {
        let Some(start) = self.input.marquee_start_screen else {
            return false;
        };
        let current = self.input.marquee_current_screen.unwrap_or(start);
        let dx = current[0] - start[0];
        let dy = current[1] - start[1];
        (dx * dx + dy * dy) >= MARQUEE_DRAG_THRESHOLD_PX * MARQUEE_DRAG_THRESHOLD_PX
    }

    #[cfg(test)]
    pub fn marquee_selection_rect_screen(&self) -> Option<([f64; 2], [f64; 2], bool)> {
        let start = self.input.marquee_start_screen?;
        let current = self.input.marquee_current_screen.unwrap_or(start);
        Some((start, current, self.marquee_has_dragged_far_enough()))
    }

    pub fn begin_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        let allows_marquee = self.input.mode.is_transform_mode();
        if phase != AppPhase::Editor || self.input.right_dragging || !allows_marquee {
            return false;
        }
        self.input.marquee_start_screen = Some([x, y]);
        self.input.marquee_current_screen = Some([x, y]);
        true
    }

    pub fn update_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        let allows_marquee = self.input.mode.is_transform_mode();
        if phase != AppPhase::Editor || self.input.right_dragging || !allows_marquee {
            return false;
        }
        if self.input.marquee_start_screen.is_none() {
            return false;
        }
        self.input.marquee_current_screen = Some([x, y]);
        true
    }

    pub fn marquee_overlapping_blocks(&self, viewport: Vec2) -> Vec<usize> {
        let Some(start) = self.input.marquee_start_screen else {
            return Vec::new();
        };
        let current = self.input.marquee_current_screen.unwrap_or(start);

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
            let Some(geometry) = self.projected_block_screen_geometry(index, viewport) else {
                continue;
            };
            if screen_bounds_overlap_rect(geometry.bounds, rect_min, rect_max)
                && projected_block_overlaps_rect(&geometry, rect_min, rect_max)
            {
                hits.push(index);
            }
        }
        hits
    }

    fn projected_block_screen_geometry(
        &self,
        index: usize,
        viewport: Vec2,
    ) -> Option<ProjectedBlockScreenGeometry> {
        let obj = self.objects.get(index)?;

        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let rotation = Mat3::from_euler(
            EulerRot::XYZ,
            obj.rotation[0].to_radians(),
            obj.rotation[1].to_radians(),
            obj.rotation[2].to_radians(),
        );

        let mut bounds: Option<(Vec2, Vec2)> = None;
        let mut corners = [None; 8];
        let mut corner_index = 0;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let local = Vec3::new(half.x * sx, half.y * sy, half.z * sz);
                    let world = center + rotation * local;
                    corners[corner_index] = self.world_to_screen_v(world, viewport);
                    if let Some(screen) = corners[corner_index] {
                        grow_screen_bounds(&mut bounds, screen);
                    }
                    corner_index += 1;
                }
            }
        }

        if let Some(center_screen) = self.world_to_screen_v(center, viewport) {
            grow_screen_bounds(&mut bounds, center_screen);
        }

        bounds.map(|bounds| ProjectedBlockScreenGeometry { bounds, corners })
    }

    fn marquee_trigger_hit(&self, rect_min: Vec2, rect_max: Vec2, viewport: Vec2) -> Option<usize> {
        let rect_center = (rect_min + rect_max) * 0.5;
        let mut best_hit: Option<(usize, f32)> = None;

        for (trigger_index, camera_trigger) in self.camera_trigger_markers() {
            let eye = Vec3::from_array(camera_trigger.eye());
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

    pub(crate) fn apply_marquee_selection(&mut self, viewport: Vec2, additive: bool) {
        let Some(start) = self.input.marquee_start_screen else {
            return;
        };
        let current = self.input.marquee_current_screen.unwrap_or(start);

        let rect_min = Vec2::new(
            start[0].min(current[0]) as f32,
            start[1].min(current[1]) as f32,
        );
        let rect_max = Vec2::new(
            start[0].max(current[0]) as f32,
            start[1].max(current[1]) as f32,
        );

        if self.input.mode.is_transform_mode() {
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

        self.input.hovered_block_index = self.input.selected_block_index;

        if let Some(index) = self.input.selected_block_index
            && let Some(obj) = self.objects.get(index)
        {
            self.input.cursor = [obj.position[0], obj.position[1], obj.position[2]];
        }

    }

    pub fn finish_marquee_selection(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        let allows_marquee = self.input.mode.is_transform_mode();
        if phase != AppPhase::Editor || !allows_marquee {
            self.input.marquee_start_screen = None;
            self.input.marquee_current_screen = None;
            return false;
        }

        let had_marquee = self.input.marquee_start_screen.is_some();
        if !had_marquee {
            return false;
        }

        self.input.marquee_current_screen = Some([x, y]);
        let additive = self.input.shift_held;

        if self.marquee_has_dragged_far_enough() {
            self.apply_marquee_selection(viewport, additive);
            self.input.marquee_start_screen = None;
            self.input.marquee_current_screen = None;
            return true;
        }

        self.input.marquee_start_screen = None;
        self.input.marquee_current_screen = None;
        if !additive {
            self.clear_block_selection();
            self.set_trigger_selected(None);
            self.sync_primary_selection_from_indices();
            self.input.hovered_block_index = None;
            self.runtime.interaction.gizmo_drag = None;
            self.runtime.interaction.block_drag = None;
            return true;
        }
        false
    }

    pub fn drag_selection(&mut self, x: f64, y: f64, viewport: Vec2) -> bool {
        self.input.pointer_screen = Some([x, y]);

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

        self.pick_spatial_index.invalidate();

        if let Some(next_position) = first_cursor {
            self.input.cursor = [
                next_position[0],
                next_position[1].max(0.0),
                next_position[2],
            ];
        }

        true
    }

    pub fn begin_block_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if !self.input.mode.is_transform_mode() {
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
                        rotation: obj.rotation,
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

    pub fn drag_selection_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor
            || self.input.right_dragging
            || !self.input.mode.is_transform_mode()
        {
            return false;
        }

        if self.drag_selection(x, y, viewport) {
            self.sync_objects_for_drag();
            true
        } else {
            false
        }
    }

    pub fn begin_selected_block_drag_ext(
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

    pub fn select_block_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> EditorInteractionChange {
        puffin::profile_scope!("SelectClick");
        if phase != AppPhase::Editor || self.input.right_dragging {
            return EditorInteractionChange::None;
        }

        let additive = self.input.shift_held;

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
                    if self.input.selected_block_indices.contains(&hit_index) {
                        self.remove_block_from_selection(hit_index);
                    } else {
                        self.add_block_to_selection(hit_index);
                    }
                } else {
                    self.replace_block_selection(vec![hit_index]);
                }
                self.input.hovered_block_index = Some(hit_index);
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

            if let Some(index) = self.input.selected_block_index {
                if let Some(obj) = self.objects.get(index) {
                    self.input.cursor = [obj.position[0], obj.position[1], obj.position[2]];
                    changed = EditorInteractionChange::Cursor;
                }
            } else if pick.cursor != self.input.cursor {
                self.input.cursor = pick.cursor;
                changed = EditorInteractionChange::Cursor;
            }
        }

        changed
    }
}

#[cfg(test)]
fn projected_block_screen_geometry_for_view(
    obj: &LevelObject,
    view_proj: glam::Mat4,
    viewport: Vec2,
) -> Option<ProjectedBlockScreenGeometry> {
    let center = Vec3::new(
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    );
    let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
    let rotation = Mat3::from_euler(
        EulerRot::XYZ,
        obj.rotation[0].to_radians(),
        obj.rotation[1].to_radians(),
        obj.rotation[2].to_radians(),
    );

    let mut bounds: Option<(Vec2, Vec2)> = None;
    let mut corners = [None; 8];
    let mut corner_index = 0;
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let local = Vec3::new(half.x * sx, half.y * sy, half.z * sz);
                let world = center + rotation * local;
                corners[corner_index] = project_world_to_screen(view_proj, world, viewport);
                if let Some(screen) = corners[corner_index] {
                    grow_screen_bounds(&mut bounds, screen);
                }
                corner_index += 1;
            }
        }
    }

    if let Some(center_screen) = project_world_to_screen(view_proj, center, viewport) {
        grow_screen_bounds(&mut bounds, center_screen);
    }

    bounds.map(|bounds| ProjectedBlockScreenGeometry { bounds, corners })
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

fn screen_bounds_overlap_rect(bounds: (Vec2, Vec2), rect_min: Vec2, rect_max: Vec2) -> bool {
    let (obj_min, obj_max) = bounds;
    obj_max.x >= rect_min.x
        && obj_min.x <= rect_max.x
        && obj_max.y >= rect_min.y
        && obj_min.y <= rect_max.y
}

fn projected_block_overlaps_rect(
    geometry: &ProjectedBlockScreenGeometry,
    rect_min: Vec2,
    rect_max: Vec2,
) -> bool {
    let mut checked_projected_face = false;

    for face_indices in BLOCK_FACE_CORNER_INDICES {
        let Some(face) = projected_face_corners(geometry, face_indices) else {
            continue;
        };
        checked_projected_face = true;
        if polygon_overlaps_rect(&face, rect_min, rect_max) {
            return true;
        }
    }

    !checked_projected_face && screen_bounds_overlap_rect(geometry.bounds, rect_min, rect_max)
}

fn projected_face_corners(
    geometry: &ProjectedBlockScreenGeometry,
    indices: [usize; 4],
) -> Option<[Vec2; 4]> {
    Some([
        geometry.corners[indices[0]]?,
        geometry.corners[indices[1]]?,
        geometry.corners[indices[2]]?,
        geometry.corners[indices[3]]?,
    ])
}

fn polygon_overlaps_rect(polygon: &[Vec2; 4], rect_min: Vec2, rect_max: Vec2) -> bool {
    if polygon
        .iter()
        .any(|point| point_in_rect(*point, rect_min, rect_max))
    {
        return true;
    }

    let rect_corners = [
        rect_min,
        Vec2::new(rect_max.x, rect_min.y),
        rect_max,
        Vec2::new(rect_min.x, rect_max.y),
    ];
    if rect_corners
        .iter()
        .any(|point| point_in_polygon(*point, polygon))
    {
        return true;
    }

    let rect_edges = [
        (rect_corners[0], rect_corners[1]),
        (rect_corners[1], rect_corners[2]),
        (rect_corners[2], rect_corners[3]),
        (rect_corners[3], rect_corners[0]),
    ];

    for edge_start_index in 0..polygon.len() {
        let edge_start = polygon[edge_start_index];
        let edge_end = polygon[(edge_start_index + 1) % polygon.len()];
        if rect_edges.iter().any(|(rect_start, rect_end)| {
            segments_intersect(edge_start, edge_end, *rect_start, *rect_end)
        }) {
            return true;
        }
    }

    false
}

fn point_in_rect(point: Vec2, rect_min: Vec2, rect_max: Vec2) -> bool {
    point.x >= rect_min.x && point.x <= rect_max.x && point.y >= rect_min.y && point.y <= rect_max.y
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2; 4]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;

    for current in 0..polygon.len() {
        let a = polygon[current];
        let b = polygon[previous];
        if point_on_segment(point, a, b) {
            return true;
        }
        if (a.y > point.y) != (b.y > point.y) {
            let intersection_x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < intersection_x {
                inside = !inside;
            }
        }
        previous = current;
    }

    inside
}

fn segments_intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let ab_c = cross_2d(b - a, c - a);
    let ab_d = cross_2d(b - a, d - a);
    let cd_a = cross_2d(d - c, a - c);
    let cd_b = cross_2d(d - c, b - c);

    if ab_c.abs() <= f32::EPSILON && point_on_segment(c, a, b) {
        return true;
    }
    if ab_d.abs() <= f32::EPSILON && point_on_segment(d, a, b) {
        return true;
    }
    if cd_a.abs() <= f32::EPSILON && point_on_segment(a, c, d) {
        return true;
    }
    if cd_b.abs() <= f32::EPSILON && point_on_segment(b, c, d) {
        return true;
    }

    (ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0)
}

fn point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> bool {
    cross_2d(b - a, point - a).abs() <= f32::EPSILON
        && point.x >= a.x.min(b.x)
        && point.x <= a.x.max(b.x)
        && point.y >= a.y.min(b.y)
        && point.y <= a.y.max(b.y)
}

fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_types::{AppPhase, EditorInputState, EditorMode, LevelObject};
    use glam::{Vec2, Vec3, Vec4};

    use super::{
        super::super::{EditorBlockDrag, EditorDragBlockStart},
        begin_marquee_selection_for_input, marquee_has_dragged_far_enough,
        projected_block_overlaps_rect, projected_block_screen_geometry_for_view,
        screen_bounds_overlap_rect, update_marquee_selection_for_input,
    };
    use crate::{
        events::pointer::{MouseButton, PointerMoved, PrimaryClick},
        state::{
            EditorCameraState, EditorHistoryRes, EditorInputRes, EditorRuntimeRes, RenderConfigRes,
            collect_editor_objects_from_world, editor_camera_view_proj, project_world_to_screen,
            replace_editor_objects, test_support::TestApp,
        },
    };

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!(
                "object_{:.0}_{:.0}_{:.0}",
                position[0], position[1], position[2]
            ),
            position,
            ..LevelObject::default()
        }
    }

    fn default_viewport() -> Vec2 {
        Vec2::new(1280.0, 720.0)
    }

    fn screen_for_world(world: Vec3) -> Vec2 {
        let viewport = default_viewport();
        project_world_to_screen(
            editor_camera_view_proj(&EditorCameraState::default(), viewport),
            world,
            viewport,
        )
        .expect("world position should project to screen")
    }

    fn marquee_test_app(objects: Vec<LevelObject>, mode: EditorMode) -> TestApp {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        replace_editor_objects(app.world_mut(), objects);
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = mode;
        }
        {
            let mut render = app.world_mut().resource_mut::<RenderConfigRes>();
            render.width = 1280;
            render.height = 720;
        }
        app
    }

    fn marquee_drag(app: &mut TestApp, start: [f64; 2], end: [f64; 2]) {
        app.world_mut().write_message(PrimaryClick {
            x: start[0],
            y: start[1],
        });
        app.tick();
        app.world_mut().write_message(PointerMoved {
            x: end[0],
            y: end[1],
        });
        app.tick();
    }

    fn release_primary(app: &mut TestApp) {
        app.world_mut().write_message(MouseButton {
            button: 0,
            pressed: false,
        });
        app.tick();
    }

    fn drag_delta_for_screen(
        camera: &EditorCameraState,
        center: Vec3,
        screen: Vec2,
        viewport: Vec2,
    ) -> Option<Vec3> {
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return None;
        }
        let inv_view_proj = editor_camera_view_proj(camera, viewport).inverse();
        let ndc_x = (2.0 * screen.x / viewport.x) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen.y / viewport.y);
        let mut near_world = inv_view_proj * Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let mut far_world = inv_view_proj * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        if near_world.w.abs() <= f32::EPSILON || far_world.w.abs() <= f32::EPSILON {
            return None;
        }
        near_world /= near_world.w;
        far_world /= far_world.w;
        let ray_origin = near_world.truncate();
        let ray_dir = (far_world.truncate() - ray_origin).normalize();
        let target = camera.editor_camera_target();
        let camera_forward = (target - (target + camera.editor_camera_offset())).normalize();
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
        camera: &EditorCameraState,
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
                if let Some(delta) = drag_delta_for_screen(camera, center, screen, viewport) {
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
        let mut input = EditorInputState {
            marquee_start_screen: Some([100.0, 100.0]),
            marquee_current_screen: Some([102.0, 102.0]),
            ..EditorInputState::default()
        };

        assert!(!marquee_has_dragged_far_enough(&input));

        input.marquee_current_screen = Some([120.0, 120.0]);
        assert!(marquee_has_dragged_far_enough(&input));
    }

    #[test]
    fn begin_marquee_requires_editor_phase_and_supported_mode() {
        let mut input = EditorInputState {
            mode: EditorMode::Timing,
            ..EditorInputState::default()
        };

        assert!(!begin_marquee_selection_for_input(
            &mut input,
            50.0,
            50.0,
            AppPhase::Editor,
        ));

        input.mode = EditorMode::Select;
        assert!(!begin_marquee_selection_for_input(
            &mut input,
            50.0,
            50.0,
            AppPhase::Menu,
        ));

        assert!(begin_marquee_selection_for_input(
            &mut input,
            50.0,
            50.0,
            AppPhase::Editor,
        ));
        assert!(input.marquee_start_screen.is_some());
        assert!(input.marquee_current_screen.is_some());
    }

    #[test]
    fn click_only_marquee_deselects_without_requiring_drag_motion() {
        let mut app = marquee_test_app(Vec::new(), EditorMode::Select);
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_indices = vec![0];
            input.0.selected_block_index = Some(0);
        }
        marquee_drag(&mut app, [100.0, 100.0], [101.0, 101.0]);
        release_primary(&mut app);

        let input = &app.world().resource::<EditorInputRes>().0;
        assert!(input.marquee_start_screen.is_none());
        assert!(input.marquee_current_screen.is_none());
        assert!(input.selected_block_indices.is_empty());
        assert_eq!(input.selected_block_index, None);
    }

    #[test]
    fn finish_marquee_with_large_rect_selects_visible_block() {
        let mut app = marquee_test_app(vec![test_block([0.0, 0.0, 0.0])], EditorMode::Select);
        marquee_drag(&mut app, [0.0, 0.0], [2000.0, 2000.0]);
        release_primary(&mut app);

        let input = &app.world().resource::<EditorInputRes>().0;
        assert!(input.selected_block_indices.contains(&0));
        assert_eq!(input.selected_block_index, Some(0));
    }

    #[test]
    fn finish_marquee_additive_selection_avoids_duplicate_indices() {
        let mut app = marquee_test_app(
            vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])],
            EditorMode::Select,
        );
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.shift_held = true;
            input.0.selected_block_indices = vec![0];
            input.0.selected_block_index = Some(0);
        }
        marquee_drag(&mut app, [0.0, 0.0], [2000.0, 2000.0]);
        release_primary(&mut app);

        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_indices, vec![0, 1]);
        assert_eq!(input.selected_block_index, Some(0));
    }

    #[test]
    fn active_marquee_update_selects_visible_block_immediately() {
        let mut app = marquee_test_app(vec![test_block([0.0, 0.0, 0.0])], EditorMode::Select);
        marquee_drag(&mut app, [0.0, 0.0], [2000.0, 2000.0]);

        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_indices, vec![0]);
        assert_eq!(input.selected_block_index, Some(0));
        assert_eq!(input.hovered_block_index, Some(0));
    }

    #[test]
    fn marquee_selection_ignores_empty_area_inside_large_projected_bounds() {
        let viewport = default_viewport();

        let mut large_block = test_block([-6.0, 0.0, -6.0]);
        large_block.size = [12.0, 8.0, 12.0];
        large_block.rotation = [0.0, 45.0, 0.0];
        let geometry = projected_block_screen_geometry_for_view(
            &large_block,
            editor_camera_view_proj(&EditorCameraState::default(), viewport),
            viewport,
        )
        .expect("large block should project to screen");
        let (bounds_min, bounds_max) = geometry.bounds;
        let bounds_size = bounds_max - bounds_min;
        let rect_half_size = 3.0;

        let mut empty_rect = None;
        'find_rect: for y_step in 1..10 {
            for x_step in 1..10 {
                let candidate = Vec2::new(
                    bounds_min.x + bounds_size.x * x_step as f32 / 10.0,
                    bounds_min.y + bounds_size.y * y_step as f32 / 10.0,
                );
                let rect_min = candidate - Vec2::splat(rect_half_size);
                let rect_max = candidate + Vec2::splat(rect_half_size);
                if screen_bounds_overlap_rect(geometry.bounds, rect_min, rect_max)
                    && !projected_block_overlaps_rect(&geometry, rect_min, rect_max)
                {
                    empty_rect = Some((rect_min, rect_max));
                    break 'find_rect;
                }
            }
        }

        let (rect_min, rect_max) =
            empty_rect.expect("large projected bounds should include empty marquee space");
        assert!(!projected_block_overlaps_rect(
            &geometry, rect_min, rect_max
        ));
    }

    #[test]
    fn update_marquee_requires_existing_start_and_valid_phase() {
        let mut input = EditorInputState {
            mode: EditorMode::Select,
            ..EditorInputState::default()
        };

        assert!(!update_marquee_selection_for_input(
            &mut input,
            10.0,
            10.0,
            AppPhase::Editor,
        ));

        input.marquee_start_screen = Some([0.0, 0.0]);
        input.right_dragging = true;
        assert!(!update_marquee_selection_for_input(
            &mut input,
            10.0,
            10.0,
            AppPhase::Editor,
        ));

        input.right_dragging = false;
        assert!(!update_marquee_selection_for_input(
            &mut input,
            10.0,
            10.0,
            AppPhase::Menu,
        ));

        assert!(update_marquee_selection_for_input(
            &mut input,
            10.0,
            10.0,
            AppPhase::Editor,
        ));
        assert_eq!(input.marquee_current_screen, Some([10.0, 10.0]));
    }

    #[test]
    fn begin_block_drag_and_extended_drag_record_history_on_success() {
        let block = test_block([0.0, 0.0, 0.0]);
        let block_center = Vec3::new(0.5, 0.5, 0.5);
        let click = screen_for_world(block_center);

        let mut menu_app = marquee_test_app(vec![block.clone()], EditorMode::Select);
        {
            let mut input = menu_app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_indices = vec![0];
            input.0.selected_block_index = Some(0);
        }
        menu_app
            .world_mut()
            .insert_resource(State::new(AppPhase::Menu));
        menu_app.world_mut().write_message(PrimaryClick {
            x: click.x as f64,
            y: click.y as f64,
        });
        menu_app.tick();
        assert!(
            menu_app
                .world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .block_drag
                .is_none()
        );

        let mut app = marquee_test_app(vec![block], EditorMode::Select);
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_indices = vec![0];
            input.0.selected_block_index = Some(0);
        }
        let history_before = app.world().resource::<EditorHistoryRes>().0.undo.len();
        app.world_mut().write_message(PrimaryClick {
            x: click.x as f64,
            y: click.y as f64,
        });
        app.tick();
        let drag = app
            .world()
            .resource::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag
            .as_ref()
            .expect("block drag should be initialized");
        assert_eq!(drag.start_blocks.len(), 1);
        assert_eq!(drag.start_blocks[0].index, 0);
        assert_eq!(
            app.world().resource::<EditorHistoryRes>().0.undo.len(),
            history_before + 1,
            "starting a block drag should capture undo history"
        );
    }

    #[test]
    fn drag_selection_handles_missing_drag_and_zero_delta() {
        let block = test_block([0.0, 0.0, 0.0]);
        let mut app = marquee_test_app(vec![block.clone()], EditorMode::Select);
        app.world_mut()
            .write_message(PointerMoved { x: 10.0, y: 20.0 });
        app.tick();
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position,
            block.position
        );

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [10.0, 20.0],
            start_center_world: [0.5, 0.5, 0.5],
            start_drag_world: [0.5, 0.5, 0.5],
            start_cursor: [0.0, 0.0, 0.0],
            start_blocks: vec![EditorDragBlockStart {
                index: 0,
                position: block.position,
                size: block.size,
                rotation: block.rotation,
            }],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut()
            .write_message(PointerMoved { x: 10.0, y: 20.0 });
        app.tick();
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position,
            block.position
        );
    }

    #[test]
    fn drag_selection_from_screen_updates_objects_and_respects_input_guards() {
        let block = test_block([0.0, 0.0, 0.0]);
        let mut app = marquee_test_app(vec![block.clone()], EditorMode::Select);
        app.world_mut()
            .resource_mut::<super::super::super::EditorConfigRes>()
            .0
            .snap_to_grid = false;
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_world: [center.x, center.y, center.z],
            start_drag_world: [center.x, center.y, center.z],
            start_cursor: [center.x, center.y, center.z],
            start_blocks: vec![EditorDragBlockStart {
                index: 0,
                position: block.position,
                size: block.size,
                rotation: block.rotation,
            }],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: origin_screen.x as f64 + 40.0,
            y: origin_screen.y as f64,
        });
        app.tick();
        assert_ne!(
            collect_editor_objects_from_world(app.world_mut())[0].position,
            block.position
        );

        let moved = collect_editor_objects_from_world(app.world_mut())[0].position;
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .right_dragging = true;
        app.world_mut().write_message(PointerMoved {
            x: origin_screen.x as f64 + 80.0,
            y: origin_screen.y as f64,
        });
        app.tick();
        assert_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position,
            moved
        );
    }

    #[test]
    fn drag_selection_without_snap_pins_small_ground_drift_but_allows_y_lift() {
        let viewport = default_viewport();
        let block = test_block([0.0, 0.0, 0.0]);
        let mut app = marquee_test_app(vec![block.clone()], EditorMode::Select);
        {
            let mut config = app
                .world_mut()
                .resource_mut::<super::super::super::EditorConfigRes>();
            config.0.snap_to_grid = false;
            config.0.snap_step = 1.0;
        }
        let center = Vec3::new(0.5, 0.5, 0.5);
        let origin_screen = screen_for_world(center);
        let release_height = 0.5;

        let (small_drift_screen, small_drift_y) = screen_for_positive_y_delta(
            &EditorCameraState::default(),
            center,
            origin_screen,
            viewport,
            0.0,
            release_height,
        );

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_world: [center.x, center.y, center.z],
            start_drag_world: [center.x, center.y, center.z],
            start_cursor: [center.x, center.y, center.z],
            start_blocks: vec![EditorDragBlockStart {
                index: 0,
                position: block.position,
                size: block.size,
                rotation: block.rotation,
            }],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: small_drift_screen.x as f64,
            y: small_drift_screen.y as f64,
        });
        app.tick();

        assert!(small_drift_y > 0.0);
        approx::assert_abs_diff_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position[1],
            0.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            app.world().resource::<EditorInputRes>().0.cursor[1],
            0.0,
            epsilon = 1e-6
        );

        let (lift_screen, lift_y) = screen_for_positive_y_delta(
            &EditorCameraState::default(),
            center,
            origin_screen,
            viewport,
            release_height,
            f32::INFINITY,
        );
        replace_editor_objects(app.world_mut(), vec![block.clone()]);
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_world: [center.x, center.y, center.z],
            start_drag_world: [center.x, center.y, center.z],
            start_cursor: [center.x, center.y, center.z],
            start_blocks: vec![EditorDragBlockStart {
                index: 0,
                position: [0.0, 0.0, 0.0],
                size: block.size,
                rotation: block.rotation,
            }],
        });
        app.world_mut().write_message(PointerMoved {
            x: lift_screen.x as f64,
            y: lift_screen.y as f64,
        });
        app.tick();

        approx::assert_abs_diff_eq!(
            collect_editor_objects_from_world(app.world_mut())[0].position[1],
            lift_y,
            epsilon = 1e-5
        );
        approx::assert_abs_diff_eq!(
            app.world().resource::<EditorInputRes>().0.cursor[1],
            lift_y,
            epsilon = 1e-5
        );
    }

    #[test]
    fn drag_selection_raycast_ignores_dragged_block_and_lands_on_block_behind_it() {
        let mut support = test_block([-4.0, 0.0, -4.0]);
        support.size = [8.0, 1.0, 8.0];
        let floating = test_block([0.0, 5.0, 0.0]);
        let mut app = marquee_test_app(vec![support, floating.clone()], EditorMode::Select);
        {
            let mut config = app
                .world_mut()
                .resource_mut::<super::super::super::EditorConfigRes>();
            config.0.snap_to_grid = true;
            config.0.snap_step = 1.0;
        }
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_indices = vec![1];
            input.0.selected_block_index = Some(1);
        }

        let floating_center = Vec3::new(0.5, 5.5, 0.5);
        let origin_screen = screen_for_world(floating_center);
        let support_top_screen = screen_for_world(Vec3::new(0.5, 1.0, 0.5));

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [origin_screen.x as f64, origin_screen.y as f64],
            start_center_world: [floating_center.x, floating_center.y, floating_center.z],
            start_drag_world: [floating_center.x, floating_center.y, floating_center.z],
            start_cursor: [floating_center.x, floating_center.y, floating_center.z],
            start_blocks: vec![EditorDragBlockStart {
                index: 1,
                position: floating.position,
                size: floating.size,
                rotation: floating.rotation,
            }],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: support_top_screen.x as f64,
            y: support_top_screen.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());
        approx::assert_abs_diff_eq!(objects[1].position[1], 1.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(
            app.world().resource::<EditorInputRes>().0.cursor[1],
            1.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn drag_selection_maintains_relative_grab_offset_instead_of_corner_snapping() {
        let mut support = test_block([-4.0, 0.0, -4.0]);
        support.size = [8.0, 1.0, 8.0];
        let floating = test_block([0.0, 5.0, 0.0]); // block at [0, 5, 0] center is [0.5, 5.5, 0.5]
        let mut app = marquee_test_app(vec![support, floating.clone()], EditorMode::Select);
        {
            let mut config = app
                .world_mut()
                .resource_mut::<super::super::super::EditorConfigRes>();
            config.0.snap_to_grid = false;
            config.0.snap_step = 1.0;
        }
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_indices = vec![1];
            input.0.selected_block_index = Some(1);
        }

        let grab_world = Vec3::new(0.75, 5.5, 0.25); // Grab at an offset near a corner
        let grab_screen = screen_for_world(grab_world);

        let target_world = Vec3::new(2.75, 1.0, 3.25);
        let target_screen = screen_for_world(target_world);

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .block_drag = Some(EditorBlockDrag {
            start_mouse: [grab_screen.x as f64, grab_screen.y as f64],
            start_center_world: [0.5, 5.5, 0.5],
            start_drag_world: [0.75, 5.5, 0.25],
            start_cursor: [0.75, 5.5, 0.25],
            start_blocks: vec![EditorDragBlockStart {
                index: 1,
                position: floating.position,
                size: floating.size,
                rotation: floating.rotation,
            }],
        });
        app.world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .left_mouse_down = true;
        app.world_mut().write_message(PointerMoved {
            x: target_screen.x as f64,
            y: target_screen.y as f64,
        });
        app.tick();

        let objects = collect_editor_objects_from_world(app.world_mut());

        // We moved the grab cursor from X=0.75 -> 2.75 (+2.0 offset)
        // Initial position X was 0.0, so new position X should be 2.0
        // We moved the grab cursor from Z=0.25 -> 3.25 (+3.0 offset)
        // Initial position Z was 0.0, so new position Z should be 3.0

        // Height drops to sit on top of the block below (Y=1)
        approx::assert_abs_diff_eq!(objects[1].position[0], 2.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(objects[1].position[1], 1.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(objects[1].position[2], 3.0, epsilon = 1e-4);
    }

    #[test]
    fn select_block_without_shift_replaces_previous_selection() {
        let mut app = marquee_test_app(
            vec![test_block([0.0, 0.0, 0.0]), test_block([2.0, 0.0, 0.0])],
            EditorMode::Select,
        );
        let first_click = screen_for_world(Vec3::new(0.5, 0.5, 0.5));
        let second_click = screen_for_world(Vec3::new(2.5, 0.5, 0.5));

        app.world_mut().write_message(PrimaryClick {
            x: first_click.x as f64,
            y: first_click.y as f64,
        });
        app.tick();
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_index,
            Some(0)
        );
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![0]
        );
        release_primary(&mut app);

        app.world_mut().write_message(PrimaryClick {
            x: second_click.x as f64,
            y: second_click.y as f64,
        });
        app.tick();
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_index,
            Some(1)
        );
        assert_eq!(
            app.world()
                .resource::<EditorInputRes>()
                .0
                .selected_block_indices,
            vec![1]
        );
    }
}
