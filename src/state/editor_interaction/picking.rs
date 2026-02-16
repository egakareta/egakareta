use super::super::{EditorPickResult, EditorSubsystem, PerfStage};
use crate::platform::state_host::PlatformInstant;
use glam::{Vec2, Vec3, Vec4};

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

        let unproject_started_at = PlatformInstant::now();
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
        self.perf_record(PerfStage::PickUnproject, unproject_started_at);

        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Z;
        let mut hit_found = false;
        let mut hit_block_index: Option<usize> = None;

        let raycast_started_at = PlatformInstant::now();

        if ray_dir.z.abs() > f32::EPSILON {
            let t = -ray_origin.z / ray_dir.z;
            if t >= 0.0 {
                min_t = t;
                hit_found = true;
            }
        }

        for (index, obj) in self.objects.iter().enumerate() {
            if let Some((t, normal)) = self.ray_intersect_rotated_block(ray_origin, ray_dir, obj) {
                if t < min_t {
                    min_t = t;
                    hit_found = true;
                    hit_block_index = Some(index);
                    best_hit_normal = normal;
                }
            }
        }

        if !hit_found {
            self.perf_record(PerfStage::PickRaycast, raycast_started_at);
            return None;
        }

        self.perf_record(PerfStage::PickRaycast, raycast_started_at);

        let hit = ray_origin + ray_dir * min_t;
        let target = hit + best_hit_normal * 0.01;

        let snap_enabled = self.config.snap_to_grid;
        let snap_step = self.config.snap_step.max(0.05);

        let next_cursor = if snap_enabled {
            [
                (target.x / snap_step).floor() * snap_step,
                (target.y / snap_step).floor() * snap_step,
                (target.z / snap_step).floor() * snap_step,
            ]
        } else {
            [target.x.floor(), target.y.floor(), target.z.floor()]
        };

        let next_cursor = [next_cursor[0], next_cursor[1], next_cursor[2].max(0.0)];

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
        })
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
        let inv_angle = -obj.rotation_degrees.to_radians();

        let local_origin_xy = self.rotate_vec2(
            Vec2::new(ray_origin.x - center.x, ray_origin.y - center.y),
            inv_angle,
        );
        let local_dir_xy = self.rotate_vec2(Vec2::new(ray_dir.x, ray_dir.y), inv_angle);
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

        let angle = obj.rotation_degrees.to_radians();
        let normal_xy = self.rotate_vec2(Vec2::new(normal_local.x, normal_local.y), angle);
        let normal = Vec3::new(normal_xy.x, normal_xy.y, normal_local.z);

        Some((t_hit, normal))
    }

    fn rotate_vec2(&self, v: Vec2, radians: f32) -> Vec2 {
        let (sin, cos) = radians.sin_cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }
}
