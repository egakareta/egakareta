use super::super::{EditorPickResult, EditorSubsystem, PerfStage};
use crate::platform::state_host::PlatformInstant;
use glam::{Vec2, Vec3, Vec4};

const CAMERA_KEYPOINT_BALL_PICK_RADIUS: f32 = 0.55;
const CAMERA_KEYPOINT_ARROW_PICK_RADIUS: f32 = 0.55;
const CAMERA_KEYPOINT_ARROW_PICK_OFFSET: f32 = 1.4;

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
        let mut best_hit_normal = Vec3::Y;
        let mut hit_found = false;
        let mut hit_block_index: Option<usize> = None;
        let mut hit_trigger_index: Option<usize> = None;

        let raycast_started_at = PlatformInstant::now();

        if ray_dir.y.abs() > f32::EPSILON {
            let t = -ray_origin.y / ray_dir.y;
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
                    hit_trigger_index = None;
                    best_hit_normal = normal;
                }
            }
        }

        for (trigger_index, keypoint) in self.camera_trigger_markers() {
            let marker_eye = self.camera_keypoint_marker_eye(&keypoint);
            let marker_forward = self.camera_keypoint_marker_forward(&keypoint);

            let mut marker_t = self.ray_intersect_sphere(
                ray_origin,
                ray_dir,
                marker_eye,
                CAMERA_KEYPOINT_BALL_PICK_RADIUS,
            );

            let arrow_center = marker_eye + marker_forward * CAMERA_KEYPOINT_ARROW_PICK_OFFSET;
            if let Some(arrow_t) = self.ray_intersect_sphere(
                ray_origin,
                ray_dir,
                arrow_center,
                CAMERA_KEYPOINT_ARROW_PICK_RADIUS,
            ) {
                marker_t = Some(marker_t.map_or(arrow_t, |best| best.min(arrow_t)));
            }

            if let Some(t) = marker_t {
                if t < min_t {
                    min_t = t;
                    hit_found = true;
                    hit_block_index = None;
                    hit_trigger_index = Some(trigger_index);
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

        let next_cursor = [next_cursor[0], next_cursor[1].max(0.0), next_cursor[2]];

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
            hit_trigger_index,
        })
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
        let inv_angle = -obj.rotation_degrees.to_radians();

        let local_origin_xz = self.rotate_vec2(
            Vec2::new(ray_origin.x - center.x, ray_origin.z - center.z),
            inv_angle,
        );
        let local_dir_xz = self.rotate_vec2(Vec2::new(ray_dir.x, ray_dir.z), inv_angle);
        let local_origin = Vec3::new(
            local_origin_xz.x,
            ray_origin.y - center.y,
            local_origin_xz.y,
        );
        let local_dir = Vec3::new(local_dir_xz.x, ray_dir.y, local_dir_xz.y);

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
        let normal_xz = self.rotate_vec2(Vec2::new(normal_local.x, normal_local.z), angle);
        let normal = Vec3::new(normal_xz.x, normal_local.y, normal_xz.y);

        Some((t_hit, normal))
    }

    fn rotate_vec2(&self, v: Vec2, radians: f32) -> Vec2 {
        let (sin, cos) = radians.sin_cos();
        Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }
}
