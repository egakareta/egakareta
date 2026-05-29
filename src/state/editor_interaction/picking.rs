/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::EditorSubsystem;
use crate::types::{EditorMode, EditorPickResult, EditorTapDivisionPick};
use glam::{EulerRot, Mat3, Vec2, Vec3, Vec4};

const CAMERA_TRIGGER_BALL_PICK_RADIUS: f32 = 0.55;
const CAMERA_TRIGGER_ARROW_PICK_RADIUS: f32 = 0.55;
const CAMERA_TRIGGER_ARROW_PICK_OFFSET: f32 = 1.4;

impl EditorSubsystem {
    pub(crate) fn pick_from_screen(
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

        {
            puffin::profile_scope!("PickRaycast");

            if ray_dir.y.abs() > f32::EPSILON {
                let t = -ray_origin.y / ray_dir.y;
                if t >= 0.0 {
                    min_t = t;
                    hit_found = true;
                }
            }

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

            for (index, obj) in self.objects.iter().enumerate() {
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

            for (trigger_index, camera_trigger) in self.camera_trigger_markers() {
                let marker_eye = self.camera_trigger_marker_eye(&camera_trigger);
                let marker_forward = self.camera_trigger_marker_forward(&camera_trigger);

                let mut marker_t = self.ray_intersect_sphere(
                    ray_origin,
                    ray_dir,
                    marker_eye,
                    CAMERA_TRIGGER_BALL_PICK_RADIUS,
                );

                let arrow_center = marker_eye + marker_forward * CAMERA_TRIGGER_ARROW_PICK_OFFSET;
                if let Some(arrow_t) = self.ray_intersect_sphere(
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
            return None;
        }

        let next_cursor = cursor_override.unwrap_or_else(|| {
            self.cursor_from_ray_hit(ray_origin + ray_dir * min_t, best_hit_normal)
        });

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
            hit_trigger_index,
            hit_tap_index,
            hit_tap_division,
        })
    }

    pub(crate) fn pick_block_cursor_from_screen_excluding(
        &self,
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
    pub(crate) fn pick_block_surface_from_screen_excluding(
        &self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let (ray_origin, ray_dir) = self.screen_to_ray(x, y, viewport_size)?;
        self.pick_block_surface_from_ray_excluding(ray_origin, ray_dir, excluded_indices)
    }

    fn pick_block_surface_from_ray_excluding(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let mut min_t = f32::INFINITY;
        let mut hit_found = false;

        for (index, obj) in self.objects.iter().enumerate() {
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
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        excluded_indices: &[usize],
    ) -> Option<[f32; 3]> {
        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Y;
        let mut hit_found = false;

        for (index, obj) in self.objects.iter().enumerate() {
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

        let next_cursor = if snap_enabled {
            [
                (target.x / snap_step).floor() * snap_step,
                (target.y / snap_step).floor() * snap_step,
                (target.z / snap_step).floor() * snap_step,
            ]
        } else {
            [target.x, target.y, target.z]
        };

        [next_cursor[0], next_cursor[1].max(0.0), next_cursor[2]]
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
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_t: f32,
    ) -> Option<(EditorTapDivisionPick, f32)> {
        if ray_dir.y.abs() <= f32::EPSILON {
            return None;
        }

        let mut best_hit: Option<(EditorTapDivisionPick, f32, f32)> = None;
        for division in self.timing_division_tap_previews() {
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

            let timeline_distance =
                (division.time_seconds - self.timeline.clock.time_seconds).abs();
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
        obj: &crate::types::LevelObject,
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

    fn ray_intersect_sphere(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        center: Vec3,
        radius: f32,
    ) -> Option<f32> {
        let a = ray_dir.length_squared();
        if a <= f32::EPSILON {
            return None;
        }

        let oc = ray_origin - center;
        let b = 2.0 * oc.dot(ray_dir);
        let c = oc.length_squared() - radius * radius;
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }

        let sqrt_discriminant = discriminant.sqrt();
        let inv_two_a = 0.5 / a;
        let t0 = (-b - sqrt_discriminant) * inv_two_a;
        let t1 = (-b + sqrt_discriminant) * inv_two_a;

        if t0 >= 0.0 {
            Some(t0)
        } else if t1 >= 0.0 {
            Some(t1)
        } else {
            None
        }
    }

    pub(crate) fn ray_intersect_rotated_block(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        obj: &crate::types::LevelObject,
    ) -> Option<(f32, Vec3)> {
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
        let inv_rotation = rotation.transpose();

        let local_origin = inv_rotation * (ray_origin - center);
        let local_dir = inv_rotation * ray_dir;

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

        let normal = rotation * normal_local;

        Some((t_hit, normal))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use crate::test_utils::assert_approx_eq as approx_eq;
    use crate::types::LevelObject;
    use glam::{Vec2, Vec3};

    fn sample_block(rotation_degrees: [f32; 3]) -> LevelObject {
        LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees,
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn ray_intersect_sphere_returns_expected_distance() {
        pollster::block_on(async {
            let state = State::new_test().await;

            let t = state
                .editor
                .ray_intersect_sphere(Vec3::new(0.0, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 1.0)
                .expect("expected sphere hit");
            approx_eq(t, 4.0, 1e-5);

            let miss = state.editor.ray_intersect_sphere(
                Vec3::new(5.0, 0.0, -5.0),
                Vec3::Z,
                Vec3::ZERO,
                1.0,
            );
            assert!(miss.is_none());
        });
    }

    #[test]
    fn ray_intersect_rotated_block_hits_and_misses() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let block = sample_block([0.0, 45.0, 0.0]);

            let hit = state.editor.ray_intersect_rotated_block(
                Vec3::new(0.5, 0.5, -5.0),
                Vec3::Z,
                &block,
            );
            assert!(hit.is_some());

            let miss = state.editor.ray_intersect_rotated_block(
                Vec3::new(5.0, 5.0, -5.0),
                Vec3::Z,
                &block,
            );
            assert!(miss.is_none());
        });
    }

    #[test]
    fn pick_from_screen_rejects_invalid_viewport() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            let pick = state
                .editor
                .pick_from_screen(0.0, 0.0, Vec2::new(0.0, 720.0));
            assert!(pick.is_none());

            let pick = state
                .editor
                .pick_from_screen(0.0, 0.0, Vec2::new(1280.0, 0.0));
            assert!(pick.is_none());
        });
    }

    #[test]
    fn pick_from_screen_center_prefers_block_hit() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.objects.clear();
            state.editor.objects.push(sample_block([0.0, 0.0, 0.0]));

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
        });
    }

    #[test]
    fn cursor_from_ray_hit_does_not_snap_when_disabled() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.config.snap_to_grid = false;

            let hit = Vec3::new(1.234, 0.5, 5.678);
            let normal = Vec3::Y;

            let cursor = state.editor.cursor_from_ray_hit(hit, normal);

            approx_eq(cursor[0], 1.234, 1e-4);
            approx_eq(cursor[1], 0.51, 1e-4);
            approx_eq(cursor[2], 5.678, 1e-4);
        });
    }
}
