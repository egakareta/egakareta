/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::EditorSubsystem;
use std::collections::HashMap;

use eg_block_geometry::{
    hitbox_cuboids, ray_intersect_aabb, ray_intersect_aabb_interval, ray_intersect_sphere,
};
use eg_types::{
    CAMERA_TRIGGER_BLOCK_ID, CameraTrigger, EditorMode, EditorPickResult, EditorTapDivisionPick,
    LevelObject, TriggerAction,
};
use glam::{Vec2, Vec3, Vec4};

const CAMERA_TRIGGER_BALL_PICK_RADIUS: f32 = 0.55;
const CAMERA_TRIGGER_ARROW_PICK_RADIUS: f32 = 0.55;
const CAMERA_TRIGGER_ARROW_PICK_OFFSET: f32 = 1.4;
const PICK_GRID_CELL_SIZE: f32 = 8.0;
const PICK_GRID_MAX_CELLS_PER_OBJECT: usize = 4096;

pub(crate) struct EditorPickSpatialIndex {
    cells: HashMap<[i32; 3], Vec<usize>>,
    large_objects: Vec<usize>,
    visited: Vec<u32>,
    query_mark: u32,
    object_count: usize,
    world_min: Vec3,
    world_max: Vec3,
    dirty: bool,
}

impl Default for EditorPickSpatialIndex {
    fn default() -> Self {
        Self {
            cells: HashMap::new(),
            large_objects: Vec::new(),
            visited: Vec::new(),
            query_mark: 0,
            object_count: 0,
            world_min: Vec3::ZERO,
            world_max: Vec3::ZERO,
            dirty: true,
        }
    }
}

impl EditorPickSpatialIndex {
    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    fn ensure_current(&mut self, objects: &[LevelObject]) {
        if !self.dirty && self.object_count == objects.len() && self.visited.len() == objects.len()
        {
            return;
        }

        self.rebuild(objects);
    }

    fn rebuild(&mut self, objects: &[LevelObject]) {
        puffin::profile_scope!("PickSpatialIndexRebuild");
        self.cells.clear();
        self.large_objects.clear();
        self.visited.resize(objects.len(), 0);
        self.object_count = objects.len();
        self.world_min = Vec3::splat(f32::INFINITY);
        self.world_max = Vec3::splat(f32::NEG_INFINITY);

        for (index, object) in objects.iter().enumerate() {
            let Some((min, max)) = object_pick_bounds(object) else {
                continue;
            };

            self.world_min = self.world_min.min(min);
            self.world_max = self.world_max.max(max);

            let min_cell = Self::cell_coords(min);
            let max_cell = Self::cell_coords(max);
            let Some(cell_count) = cell_range_count(min_cell, max_cell) else {
                self.large_objects.push(index);
                continue;
            };

            if cell_count > PICK_GRID_MAX_CELLS_PER_OBJECT {
                self.large_objects.push(index);
                continue;
            }

            for x in min_cell[0]..=max_cell[0] {
                for y in min_cell[1]..=max_cell[1] {
                    for z in min_cell[2]..=max_cell[2] {
                        self.cells.entry([x, y, z]).or_default().push(index);
                    }
                }
            }
        }

        if !self.world_min.is_finite() || !self.world_max.is_finite() {
            self.world_min = Vec3::ZERO;
            self.world_max = Vec3::ZERO;
        }
        self.dirty = false;
    }

    fn query_ray(&mut self, ray_origin: Vec3, ray_dir: Vec3, max_t: f32) -> Vec<usize> {
        puffin::profile_scope!("PickSpatialIndexQuery");
        let mut candidates = Vec::new();
        if self.object_count == 0 {
            return candidates;
        }

        self.begin_query();
        let query_mark = self.query_mark;
        for &index in &self.large_objects {
            add_pick_candidate(index, query_mark, &mut self.visited, &mut candidates);
        }

        if self.cells.is_empty() || !self.world_min.is_finite() || !self.world_max.is_finite() {
            return candidates;
        }

        let Some((entry_t, exit_t)) =
            ray_intersect_aabb_interval(ray_origin, ray_dir, self.world_min, self.world_max)
        else {
            return candidates;
        };

        let t_start = entry_t.max(0.0);
        let t_end = exit_t.min(max_t);
        if t_end < t_start {
            return candidates;
        }

        let min_cell = Self::cell_coords(self.world_min);
        let max_cell = Self::cell_coords(self.world_max);
        let start_position = ray_origin + ray_dir * t_start;
        let mut cell = clamp_cell(Self::cell_coords(start_position), min_cell, max_cell);
        let mut step = [0; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];

        for axis in 0..3 {
            let dir_component = ray_dir[axis];
            if dir_component > f32::EPSILON {
                step[axis] = 1;
                let boundary = (cell[axis] + 1) as f32 * PICK_GRID_CELL_SIZE;
                t_max[axis] = (boundary - ray_origin[axis]) / dir_component;
                t_delta[axis] = PICK_GRID_CELL_SIZE / dir_component;
            } else if dir_component < -f32::EPSILON {
                step[axis] = -1;
                let boundary = cell[axis] as f32 * PICK_GRID_CELL_SIZE;
                t_max[axis] = (boundary - ray_origin[axis]) / dir_component;
                t_delta[axis] = -PICK_GRID_CELL_SIZE / dir_component;
            }
        }

        let mut t = t_start;
        while t <= t_end {
            add_cell_pick_candidates(
                &self.cells,
                cell,
                query_mark,
                &mut self.visited,
                &mut candidates,
            );

            let axis = min_axis(t_max);
            if !t_max[axis].is_finite() || t_max[axis] > t_end {
                break;
            }

            cell[axis] += step[axis];
            if cell[axis] < min_cell[axis] || cell[axis] > max_cell[axis] {
                break;
            }
            t = t_max[axis];
            t_max[axis] += t_delta[axis];
        }

        candidates
    }

    fn begin_query(&mut self) {
        if self.query_mark == u32::MAX {
            self.visited.fill(0);
            self.query_mark = 1;
        } else {
            self.query_mark += 1;
        }
    }

    fn cell_coords(position: Vec3) -> [i32; 3] {
        [
            (position.x / PICK_GRID_CELL_SIZE).floor() as i32,
            (position.y / PICK_GRID_CELL_SIZE).floor() as i32,
            (position.z / PICK_GRID_CELL_SIZE).floor() as i32,
        ]
    }
}

fn add_cell_pick_candidates(
    cells: &HashMap<[i32; 3], Vec<usize>>,
    cell: [i32; 3],
    query_mark: u32,
    visited: &mut [u32],
    candidates: &mut Vec<usize>,
) {
    let Some(indices) = cells.get(&cell) else {
        return;
    };

    for &index in indices {
        add_pick_candidate(index, query_mark, visited, candidates);
    }
}

fn add_pick_candidate(
    index: usize,
    query_mark: u32,
    visited: &mut [u32],
    candidates: &mut Vec<usize>,
) {
    let Some(visited) = visited.get_mut(index) else {
        return;
    };
    if *visited == query_mark {
        return;
    }

    *visited = query_mark;
    candidates.push(index);
}

fn object_pick_bounds(object: &LevelObject) -> Option<(Vec3, Vec3)> {
    if object.block_id == CAMERA_TRIGGER_BLOCK_ID {
        let eye = object.center();
        let forward = Vec3::from_array(CameraTrigger::forward_from_rotation(object.rotation));
        let arrow_direction = if forward.length_squared() > f32::EPSILON {
            forward.normalize()
        } else {
            Vec3::Z
        };
        let arrow_center = eye + arrow_direction * CAMERA_TRIGGER_ARROW_PICK_OFFSET;
        let radius = CAMERA_TRIGGER_BALL_PICK_RADIUS.max(CAMERA_TRIGGER_ARROW_PICK_RADIUS);
        let min = (eye - Vec3::splat(radius)).min(arrow_center - Vec3::splat(radius));
        let max = (eye + Vec3::splat(radius)).max(arrow_center + Vec3::splat(radius));
        return (min.is_finite() && max.is_finite()).then_some((min, max));
    }

    let half = Vec3::new(
        object.size[0].abs() * 0.5,
        object.size[1].abs() * 0.5,
        object.size[2].abs() * 0.5,
    );
    let radius = half.length();
    if radius <= f32::EPSILON {
        return None;
    }

    let center = object.center();
    let extent = Vec3::splat(radius);
    let min = center - extent;
    let max = center + extent;
    (min.is_finite() && max.is_finite()).then_some((min, max))
}

fn cell_range_count(min_cell: [i32; 3], max_cell: [i32; 3]) -> Option<usize> {
    let x = usize::try_from(max_cell[0].checked_sub(min_cell[0])?.checked_add(1)?).ok()?;
    let y = usize::try_from(max_cell[1].checked_sub(min_cell[1])?.checked_add(1)?).ok()?;
    let z = usize::try_from(max_cell[2].checked_sub(min_cell[2])?.checked_add(1)?).ok()?;
    x.checked_mul(y)?.checked_mul(z)
}

fn clamp_cell(cell: [i32; 3], min_cell: [i32; 3], max_cell: [i32; 3]) -> [i32; 3] {
    [
        cell[0].clamp(min_cell[0], max_cell[0]),
        cell[1].clamp(min_cell[1], max_cell[1]),
        cell[2].clamp(min_cell[2], max_cell[2]),
    ]
}

fn min_axis(values: [f32; 3]) -> usize {
    if values[0] <= values[1] && values[0] <= values[2] {
        0
    } else if values[1] <= values[2] {
        1
    } else {
        2
    }
}

impl EditorSubsystem {
    pub fn pick_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> Option<EditorPickResult> {
        if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
            return None;
        }

        let (ray_origin, ray_dir) = {
            puffin::profile_scope!("PickUnproject");
            let view_proj = self.view_proj(viewport_size);
            let inv_view_proj = view_proj.inverse();

            let ndc_x = (2.0 * x as f32 / viewport_size.x) - 1.0;
            let ndc_y = 1.0 - (2.0 * y as f32 / viewport_size.y);

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
            (ray_origin, ray_dir)
        };

        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Y;
        let mut hit_found = false;
        let mut hit_block_index: Option<usize> = None;
        let mut hit_trigger_index: Option<usize> = None;
        let mut hit_tap_index: Option<usize> = None;
        let mut hit_tap_division: Option<EditorTapDivisionPick> = None;
        let mut cursor_override: Option<[f32; 3]> = None;
        let ground_t = if ray_dir.y.abs() > f32::EPSILON {
            let t = -ray_origin.y / ray_dir.y;
            (t >= 0.0).then_some(t)
        } else {
            None
        };

        {
            puffin::profile_scope!("PickRaycast");

            if self.ui.mode == EditorMode::Tapping {
                if let Some((tap_index, tap_t, tap_position)) =
                    self.ray_intersect_tap_indicator(ray_origin, ray_dir, min_t)
                {
                    min_t = tap_t;
                    hit_found = true;
                    hit_block_index = None;
                    hit_trigger_index = None;
                    hit_tap_index = Some(tap_index);
                    hit_tap_division = None;
                    cursor_override = Some(tap_position);
                    best_hit_normal = Vec3::Y;
                }

                if let Some((division, division_t)) =
                    self.ray_intersect_tap_division(ray_origin, ray_dir, min_t)
                {
                    min_t = division_t;
                    hit_found = true;
                    hit_block_index = None;
                    hit_trigger_index = None;
                    hit_tap_index = None;
                    hit_tap_division = Some(division);
                    cursor_override = Some(division.indicator_position);
                    best_hit_normal = Vec3::Y;
                }
            }

            let object_indices = self.pick_candidate_object_indices(
                ray_origin,
                ray_dir,
                ground_t.unwrap_or(f32::INFINITY),
            );
            for index in object_indices {
                let Some(obj) = self.objects.get(index) else {
                    continue;
                };
                if obj.block_id == CAMERA_TRIGGER_BLOCK_ID {
                    if let Some((t, normal)) =
                        self.ray_intersect_camera_trigger_block(ray_origin, ray_dir, obj)
                    {
                        if t < min_t {
                            min_t = t;
                            hit_found = true;
                            hit_block_index = Some(index);
                            hit_trigger_index = None;
                            hit_tap_index = None;
                            hit_tap_division = None;
                            cursor_override = None;
                            best_hit_normal = normal;
                        }
                    }
                    continue;
                }

                if !Self::ray_may_hit_block_bounds(ray_origin, ray_dir, obj, min_t) {
                    continue;
                }

                if let Some((t, normal)) =
                    self.ray_intersect_rotated_block(ray_origin, ray_dir, obj)
                {
                    if t < min_t {
                        min_t = t;
                        hit_found = true;
                        hit_block_index = Some(index);
                        hit_trigger_index = None;
                        hit_tap_index = None;
                        hit_tap_division = None;
                        cursor_override = None;
                        best_hit_normal = normal;
                    }
                }
            }

            for (trigger_index, trigger) in self.triggers().iter().enumerate() {
                let Some(target) = transform_trigger_target_object(&trigger.action) else {
                    continue;
                };
                if !Self::ray_may_hit_block_bounds(ray_origin, ray_dir, &target, min_t) {
                    continue;
                }

                if let Some((t, _normal)) =
                    self.ray_intersect_rotated_block(ray_origin, ray_dir, &target)
                {
                    if t < min_t {
                        min_t = t;
                        hit_found = true;
                        hit_trigger_index = Some(trigger_index);
                        hit_tap_index = None;
                        hit_tap_division = None;
                        cursor_override = None;
                    }
                }
            }

            for (trigger_index, camera_trigger) in self.camera_trigger_markers() {
                let marker_eye = Vec3::from_array(camera_trigger.eye());
                let marker_forward = Vec3::from_array(camera_trigger.forward());

                let mut marker_t = ray_intersect_sphere(
                    ray_origin,
                    ray_dir,
                    marker_eye,
                    CAMERA_TRIGGER_BALL_PICK_RADIUS,
                );

                let arrow_center = marker_eye + marker_forward * CAMERA_TRIGGER_ARROW_PICK_OFFSET;
                if let Some(arrow_t) = ray_intersect_sphere(
                    ray_origin,
                    ray_dir,
                    arrow_center,
                    CAMERA_TRIGGER_ARROW_PICK_RADIUS,
                ) {
                    marker_t = Some(marker_t.map_or(arrow_t, |best| best.min(arrow_t)));
                }

                if let Some(t) = marker_t {
                    if t < min_t {
                        min_t = t;
                        hit_found = true;
                        hit_block_index = None;
                        hit_trigger_index = Some(trigger_index);
                        hit_tap_index = None;
                        hit_tap_division = None;
                        cursor_override = None;
                    }
                }
            }
        }

        if !hit_found {
            if let Some(t) = ground_t {
                min_t = t;
                hit_found = true;
                best_hit_normal = Vec3::Y;
            }
        }

        if !hit_found {
            return None;
        }

        let next_cursor = if self.ui.mode == EditorMode::Tapping && cursor_override.is_none() {
            let target = ray_origin + ray_dir * min_t + best_hit_normal * 0.01;
            self.tap_path_cursor_near_world([target.x, target.y.max(0.0), target.z])
        } else {
            cursor_override.unwrap_or_else(|| {
                self.cursor_from_ray_hit(ray_origin + ray_dir * min_t, best_hit_normal)
            })
        };

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
            hit_trigger_index,
            hit_tap_index,
            hit_tap_division,
        })
    }

    pub fn pick_block_cursor_from_screen_excluding(
        &mut self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let (ray_origin, ray_dir) = self.screen_to_ray(x, y, viewport_size)?;
        self.pick_block_cursor_from_ray_excluding(ray_origin, ray_dir, excluded_indices)
    }

    /// Like `pick_block_cursor_from_screen_excluding` but returns the raw surface
    /// hit position (without the cursor nudge). Used for drag positioning where
    /// the 0.01 surface offset would cause blocks to float above surfaces.
    pub fn pick_block_surface_from_screen_excluding(
        &mut self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let (ray_origin, ray_dir) = self.screen_to_ray(x, y, viewport_size)?;
        self.pick_block_surface_from_ray_excluding(ray_origin, ray_dir, excluded_indices)
    }

    fn pick_block_surface_from_ray_excluding(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let mut min_t = f32::INFINITY;
        let mut hit_found = false;

        for index in self.pick_candidate_object_indices(ray_origin, ray_dir, f32::INFINITY) {
            let Some(obj) = self.objects.get(index) else {
                continue;
            };
            if excluded_indices.contains(&index)
                || !Self::ray_may_hit_block_bounds(ray_origin, ray_dir, obj, min_t)
            {
                continue;
            }

            if let Some((t, _normal)) = self.ray_intersect_rotated_block(ray_origin, ray_dir, obj) {
                if t < min_t {
                    min_t = t;
                    hit_found = true;
                }
            }
        }

        if hit_found {
            let hit = ray_origin + ray_dir * min_t;
            Some([hit.x, hit.y.max(0.0), hit.z])
        } else {
            None
        }
    }

    fn pick_block_cursor_from_ray_excluding(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Y;
        let mut hit_found = false;

        for index in self.pick_candidate_object_indices(ray_origin, ray_dir, f32::INFINITY) {
            let Some(obj) = self.objects.get(index) else {
                continue;
            };
            if excluded_indices.contains(&index)
                || !Self::ray_may_hit_block_bounds(ray_origin, ray_dir, obj, min_t)
            {
                continue;
            }

            if let Some((t, normal)) = self.ray_intersect_rotated_block(ray_origin, ray_dir, obj) {
                if t < min_t {
                    min_t = t;
                    best_hit_normal = normal;
                    hit_found = true;
                }
            }
        }

        if hit_found {
            Some(self.cursor_from_ray_hit(ray_origin + ray_dir * min_t, best_hit_normal))
        } else {
            None
        }
    }

    fn cursor_from_ray_hit(&self, hit: Vec3, hit_normal: Vec3) -> [f32; 3] {
        let target = hit + hit_normal * 0.01;
        let snap_enabled = self.effective_snap_to_grid();
        let snap_step = self.config.snap_step.max(0.05);
        let footprint_centered =
            self.ui.mode == EditorMode::Place && (!snap_enabled || snap_step < 1.0);
        let selected_size = self.selected_block_default_size();
        let cursor_target = if footprint_centered {
            Vec3::new(
                target.x - selected_size[0] * 0.5,
                target.y,
                target.z - selected_size[2] * 0.5,
            )
        } else {
            target
        };

        let next_cursor = if snap_enabled {
            [
                (cursor_target.x / snap_step).floor() * snap_step,
                (cursor_target.y / snap_step).floor() * snap_step,
                (cursor_target.z / snap_step).floor() * snap_step,
            ]
        } else {
            [cursor_target.x, cursor_target.y, cursor_target.z]
        };

        [next_cursor[0], next_cursor[1].max(0.0), next_cursor[2]]
    }

    fn pick_candidate_object_indices(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_t: f32,
    ) -> Vec<usize> {
        self.pick_spatial_index.ensure_current(&self.objects);
        self.pick_spatial_index
            .query_ray(ray_origin, ray_dir, max_t)
    }

    fn ray_intersect_tap_indicator(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_t: f32,
    ) -> Option<(usize, f32, [f32; 3])> {
        if ray_dir.y.abs() <= f32::EPSILON {
            return None;
        }

        let mut best_hit: Option<(usize, f32, f32, [f32; 3])> = None;
        for (index, position) in self
            .timeline
            .taps
            .tap_indicator_positions
            .iter()
            .enumerate()
        {
            let plane_y = position[1] + 0.1;
            let t = (plane_y - ray_origin.y) / ray_dir.y;
            if t < 0.0 || t >= max_t {
                continue;
            }

            let hit = ray_origin + ray_dir * t;
            if hit.x < position[0]
                || hit.x > position[0] + 1.0
                || hit.z < position[2]
                || hit.z > position[2] + 1.0
            {
                continue;
            }

            let timeline_distance = self
                .timeline
                .taps
                .tap_times
                .get(index)
                .map(|time| (*time - self.timeline.clock.time_seconds).abs())
                .unwrap_or(f32::INFINITY);

            match best_hit {
                Some((_, best_t, best_timeline_distance, _))
                    if t > best_t + f32::EPSILON
                        || ((t - best_t).abs() <= f32::EPSILON
                            && timeline_distance >= best_timeline_distance) => {}
                _ => best_hit = Some((index, t, timeline_distance, *position)),
            }
        }

        best_hit.map(|(index, t, _, position)| (index, t, position))
    }

    fn ray_intersect_tap_division(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_t: f32,
    ) -> Option<(EditorTapDivisionPick, f32)> {
        if ray_dir.y.abs() <= f32::EPSILON {
            return None;
        }

        let mut best_hit: Option<(EditorTapDivisionPick, f32, f32)> = None;
        let current_time = self.timeline.clock.time_seconds;
        let previews: Vec<_> = self.timing_division_tap_previews().to_vec();
        for division in previews {
            let position = division.indicator_position;
            let plane_y = position[1] + 0.1;
            let t = (plane_y - ray_origin.y) / ray_dir.y;
            if t < 0.0 || t >= max_t {
                continue;
            }

            let hit = ray_origin + ray_dir * t;
            if hit.x < position[0]
                || hit.x > position[0] + 1.0
                || hit.z < position[2]
                || hit.z > position[2] + 1.0
            {
                continue;
            }

            let timeline_distance = (division.time_seconds - current_time).abs();
            let pick = EditorTapDivisionPick {
                time_seconds: division.time_seconds,
                indicator_position: division.indicator_position,
            };

            match best_hit {
                Some((_, best_t, best_timeline_distance))
                    if t > best_t + f32::EPSILON
                        || ((t - best_t).abs() <= f32::EPSILON
                            && timeline_distance >= best_timeline_distance) => {}
                _ => best_hit = Some((pick, t, timeline_distance)),
            }
        }

        best_hit.map(|(pick, t, _)| (pick, t))
    }

    fn ray_may_hit_block_bounds(
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &eg_types::LevelObject,
        max_t: f32,
    ) -> bool {
        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let radius = half.length();
        if radius <= f32::EPSILON {
            return false;
        }

        let to_center = center - ray_origin;
        let center_t = to_center.dot(ray_dir);
        if center_t + radius < 0.0 {
            return false;
        }

        let nearest_possible_t = (center_t - radius).max(0.0);
        if nearest_possible_t > max_t {
            return false;
        }

        let closest_distance_sq = (to_center.length_squared() - center_t * center_t).max(0.0);
        closest_distance_sq <= radius * radius
    }

    fn ray_intersect_camera_trigger_block(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &eg_types::LevelObject,
    ) -> Option<(f32, Vec3)> {
        let eye = obj.center();
        let forward = Vec3::from_array(CameraTrigger::forward_from_rotation(obj.rotation));
        let arrow_direction = if forward.length_squared() > f32::EPSILON {
            forward.normalize()
        } else {
            Vec3::Z
        };

        let mut marker_t =
            ray_intersect_sphere(ray_origin, ray_dir, eye, CAMERA_TRIGGER_BALL_PICK_RADIUS);

        let arrow_center = eye + arrow_direction * CAMERA_TRIGGER_ARROW_PICK_OFFSET;
        if let Some(arrow_t) = ray_intersect_sphere(
            ray_origin,
            ray_dir,
            arrow_center,
            CAMERA_TRIGGER_ARROW_PICK_RADIUS,
        ) {
            marker_t = Some(marker_t.map_or(arrow_t, |best| best.min(arrow_t)));
        }

        marker_t.map(|t| (t, Vec3::Y))
    }

    pub fn ray_intersect_rotated_block(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &eg_types::LevelObject,
    ) -> Option<(f32, Vec3)> {
        let center = obj.center();
        let rotation = obj.basis();
        let inv_rotation = rotation.transpose();

        let local_origin_world = center + inv_rotation * (ray_origin - center);
        let local_dir = inv_rotation * ray_dir;
        let mut best_hit: Option<(f32, Vec3)> = None;

        for cuboid in hitbox_cuboids(obj) {
            let min = Vec3::from(cuboid.min);
            let max = Vec3::from(cuboid.max);
            if let Some((t_hit, normal_local)) =
                ray_intersect_aabb(local_origin_world, local_dir, min, max)
            {
                if best_hit.map(|(best_t, _)| t_hit < best_t).unwrap_or(true) {
                    best_hit = Some((t_hit, rotation * normal_local));
                }
            }
        }

        best_hit
    }
}

fn transform_trigger_target_object(action: &TriggerAction) -> Option<LevelObject> {
    let TriggerAction::TransformObjects {
        position,
        rotation: rotation_degrees,
        size,
        ..
    } = action
    else {
        return None;
    };

    Some(LevelObject {
        position: *position,
        size: *size,
        rotation: *rotation_degrees,
        ..LevelObject::default()
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use eg_types::{BlockId, CAMERA_TRIGGER_BLOCK_ID, LevelObject};
    use glam::{Vec2, Vec3};

    #[tokio::test]
    async fn ray_intersect_rotated_block_hits_and_misses() {
        let state = State::new_test().await;
        let block = LevelObject {
            rotation: [0.0, 45.0, 0.0],
            ..LevelObject::default()
        };

        let hit =
            state
                .editor
                .ray_intersect_rotated_block(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, &block);
        assert!(hit.is_some());

        let miss =
            state
                .editor
                .ray_intersect_rotated_block(Vec3::new(5.0, 5.0, -5.0), Vec3::Z, &block);
        assert!(miss.is_none());
    }

    #[tokio::test]
    async fn ray_intersect_rotated_block_uses_json_hitboxes() {
        let state = State::new_test().await;
        let fence = LevelObject::new("core/wooden_fence");

        let rail_hit =
            state
                .editor
                .ray_intersect_rotated_block(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, &fence);
        assert!(rail_hit.is_some());

        let gap_miss =
            state
                .editor
                .ray_intersect_rotated_block(Vec3::new(0.2, 0.5, -5.0), Vec3::Z, &fence);
        assert!(gap_miss.is_none());
    }

    #[tokio::test]
    async fn camera_trigger_block_pick_uses_arrow_visual_outside_old_cube_bounds() {
        let state = State::new_test().await;
        let camera_trigger = LevelObject::new(CAMERA_TRIGGER_BLOCK_ID);

        let ray_origin = Vec3::new(0.5, 5.0, 0.5 + super::CAMERA_TRIGGER_ARROW_PICK_OFFSET);
        let ray_dir = -Vec3::Y;

        assert!(
            state
                .editor
                .ray_intersect_rotated_block(ray_origin, ray_dir, &camera_trigger)
                .is_none()
        );
        assert!(
            state
                .editor
                .ray_intersect_camera_trigger_block(ray_origin, ray_dir, &camera_trigger)
                .is_some()
        );
    }

    #[tokio::test]
    async fn pick_from_screen_rejects_invalid_viewport() {
        let mut state = State::new_test().await;

        let pick = state
            .editor
            .pick_from_screen(0.0, 0.0, Vec2::new(0.0, 720.0));
        assert!(pick.is_none());

        let pick = state
            .editor
            .pick_from_screen(0.0, 0.0, Vec2::new(1280.0, 0.0));
        assert!(pick.is_none());
    }

    #[tokio::test]
    async fn pick_from_screen_center_prefers_block_hit() {
        let mut state = State::new_test().await;

        state.editor.objects.clear();
        state.editor.objects.push(LevelObject::default());

        let viewport = Vec2::new(1280.0, 720.0);
        let pick = state
            .editor
            .pick_from_screen(
                (viewport.x * 0.5) as f64,
                (viewport.y * 0.5) as f64,
                viewport,
            )
            .expect("expected pick result");

        assert!(pick.hit_trigger_index.is_none());
        assert!(pick.cursor[1] >= 0.0);
    }

    #[tokio::test]
    async fn cursor_from_ray_hit_centers_footprint_when_snap_disabled() {
        let mut state = State::new_test().await;
        state.editor.config.snap_to_grid = false;

        let hit = Vec3::new(1.234, 0.5, 5.678);
        let normal = Vec3::Y;

        let cursor = state.editor.cursor_from_ray_hit(hit, normal);

        approx::assert_abs_diff_eq!(cursor[0], 0.734, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[1], 0.51, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[2], 5.178, epsilon = 1e-4);
    }

    #[tokio::test]
    async fn cursor_from_ray_hit_preserves_unit_snap_flooring() {
        let mut state = State::new_test().await;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 1.0;

        let hit = Vec3::new(1.234, 0.5, 5.678);
        let normal = Vec3::Y;

        let cursor = state.editor.cursor_from_ray_hit(hit, normal);

        approx::assert_abs_diff_eq!(cursor[0], 1.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[1], 0.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[2], 5.0, epsilon = 1e-4);
    }

    #[tokio::test]
    async fn cursor_from_ray_hit_centers_footprint_before_subunit_snap() {
        let mut state = State::new_test().await;
        state.editor.config.snap_to_grid = true;
        state.editor.config.snap_step = 0.25;

        let hit = Vec3::new(1.234, 0.5, 5.678);
        let normal = Vec3::Y;

        let cursor = state.editor.cursor_from_ray_hit(hit, normal);

        approx::assert_abs_diff_eq!(cursor[0], 0.5, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[1], 0.5, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[2], 5.0, epsilon = 1e-4);
    }

    #[tokio::test]
    async fn cursor_from_ray_hit_centers_selected_block_size_when_snap_disabled() {
        let mut state = State::new_test().await;
        state.editor.config.snap_to_grid = false;
        state.editor.config.selected_block_id = BlockId::new("core/speedportal");

        let hit = Vec3::new(4.0, 0.25, 9.0);
        let normal = Vec3::Y;

        let cursor = state.editor.cursor_from_ray_hit(hit, normal);

        approx::assert_abs_diff_eq!(cursor[0], 3.0, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[1], 0.26, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(cursor[2], 8.5, epsilon = 1e-4);
    }
}
