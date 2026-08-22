/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{Mat4, Vec2, Vec3, Vec4};

super::super::editor_context_methods! {
    pub fn screen_to_ray(&self, x: f64, y: f64, viewport: Vec2) -> Option<(Vec3, Vec3)> {
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return None;
        }
        let view_proj = self.view_proj(viewport);
        let inv_view_proj = view_proj.inverse();

        let ndc_x = (2.0 * x as f32 / viewport.x) - 1.0;
        let ndc_y = 1.0 - (2.0 * y as f32 / viewport.y);

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
        Some((ray_origin, ray_dir))
    }

    pub fn camera_forward(&self) -> Vec3 {
        let target = Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_target_z,
            self.camera.editor_pan[1],
        );
        let eye = target + self.camera_offset();
        (target - eye).normalize()
    }

    pub fn camera_offset(&self) -> Vec3 {
        let distance = 24.0;
        let pitch = self
            .camera
            .editor_pitch
            .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_y(self.camera.editor_yaw).transform_vector3(Vec3::new(
            0.0,
            vertical_distance,
            -horizontal_distance,
        ))
    }

    pub fn view_proj(&self, viewport: Vec2) -> Mat4 {
        let aspect = viewport.x / viewport.y;
        let target = Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_target_z,
            self.camera.editor_pan[1],
        );
        let eye = target + self.camera_offset();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = glam::camera::rh::view::look_at_mat4(eye, target, up);
        let proj = glam::camera::rh::proj::opengl::perspective(
            45f32.to_radians(),
            aspect,
            0.1,
            10000.0,
        );
        proj * view
    }

    pub fn world_to_screen_v(&self, world: Vec3, viewport: Vec2) -> Option<Vec2> {
        let view_proj = self.view_proj(viewport);
        let clip = view_proj * world.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }

        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            return None;
        }

        let screen_x = (ndc.x + 1.0) * 0.5 * viewport.x;
        let screen_y = (1.0 - ndc.y) * 0.5 * viewport.y;
        Some(Vec2::new(screen_x, screen_y))
    }

}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use crate::state::{test_support::TestApp, with_editor_interaction_context};

    #[test]
    fn camera_offset_clamps_pitch_and_world_projection_handles_visibility() {
        let viewport = Vec2::new(1280.0, 720.0);
        let mut app = TestApp::minimal();
        let (offset_clamped_high, offset_at_limit, on_screen, off_screen) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.camera.editor_yaw = 0.0;
                editor.camera.editor_pitch = 10.0;
                let offset_clamped_high = editor.camera_offset();

                editor.camera.editor_pitch = 89.9f32.to_radians();
                let offset_at_limit = editor.camera_offset();
                let target = editor.camera.editor_camera_target();
                let on_screen = editor.world_to_screen_v(target, viewport);
                let behind_camera = target + offset_at_limit * 2.0;
                let off_screen = editor.world_to_screen_v(behind_camera, viewport);

                (offset_clamped_high, offset_at_limit, on_screen, off_screen)
            });

        approx::assert_abs_diff_eq!(offset_clamped_high.x, offset_at_limit.x, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(offset_clamped_high.y, offset_at_limit.y, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(offset_clamped_high.z, offset_at_limit.z, epsilon = 1e-4);
        assert!(on_screen.is_some());
        assert!(off_screen.is_none());
    }

    #[test]
    fn camera_transition_interpolates_and_finishes() {
        let mut app = TestApp::minimal();
        let (active, transition_active, finished, transition_finished, pitch) =
            with_editor_interaction_context(app.world_mut(), |editor| {
                editor.camera.editor_yaw = 3.0;
                editor.camera.editor_pitch = 0.2;
                editor.camera.set_transition(3.0, 0.2, 3.5, 0.6, 1.0);

                let active = editor.camera.update_camera_transition(0.5);
                let transition_active = editor.camera.transition.is_some();
                let finished = editor.camera.update_camera_transition(0.6);
                let transition_finished = editor.camera.transition.is_none();
                (
                    active,
                    transition_active,
                    finished,
                    transition_finished,
                    editor.camera.editor_pitch,
                )
            });

        assert!(active);
        assert!(transition_active);
        assert!(finished);
        assert!(transition_finished);
        approx::assert_abs_diff_eq!(pitch, 0.6, epsilon = 1e-3);
    }
}
