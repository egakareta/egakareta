/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{
    Mat4, Vec2, Vec3, Vec4,
    camera::rh::{proj::opengl::perspective, view::look_at_mat4},
};

super::super::editor_context_methods! {
    #[cfg(test)]
    pub fn camera_axes_xy(&self) -> (Vec2, Vec2) {
        editor_camera_axes_xy(self.camera.editor_yaw)
    }

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
        let view = look_at_mat4(eye, target, Vec3::Y);
        let proj = perspective(45f32.to_radians(), aspect, 0.1, 10000.0);
        proj * view
    }

    pub fn world_to_screen_v(&self, world: Vec3, viewport: Vec2) -> Option<Vec2> {
        project_world_to_screen(self.view_proj(viewport), world, viewport)
    }

    #[cfg(test)]
    pub fn drag_camera_by_pixels(
        &mut self,
        dx: f64,
        dy: f64,
        phase: AppPhase,
        is_game_active: bool,
    ) {
        if !self.input.right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.004;
        const PITCH_SPEED: f32 = 0.006;

        if phase == AppPhase::Editor {
            self.camera.editor_yaw -= dx as f32 * ROTATE_SPEED;
            self.camera.editor_pitch = (self.camera.editor_pitch + dy as f32 * PITCH_SPEED)
                .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        } else if phase == AppPhase::Playing && !is_game_active {
            self.camera.playing_rotation -= dx as f32 * ROTATE_SPEED;
            self.camera.playing_pitch = (self.camera.playing_pitch + dy as f32 * PITCH_SPEED)
                .clamp(0.1f32.to_radians(), 89.9f32.to_radians());
        }
    }

    #[cfg(test)]
    pub fn set_editor_camera_orientation(
        &mut self,
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    ) {
        if let Some(duration) = transition_seconds
            && duration > 0.0
        {
            let current_rot = self.camera.editor_yaw;
            let current_pitch = self.camera.editor_pitch;

            let mut target_rot = rotation;
            let diff = target_rot - current_rot;
            if diff > std::f32::consts::PI {
                target_rot -= std::f32::consts::TAU;
            } else if diff < -std::f32::consts::PI {
                target_rot += std::f32::consts::TAU;
            }

            self.camera.transition = Some(crate::state::editor_camera::CameraTransition {
                start_yaw: current_rot,
                start_pitch: current_pitch,
                target_yaw: target_rot,
                target_pitch: pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians()),
                elapsed: 0.0,
                duration,
            });
            return;
        }

        self.camera.transition = None;
        self.camera.editor_yaw = rotation;
        self.camera.editor_pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
    }

    #[cfg(test)]
    pub fn update_camera_transition(&mut self, frame_dt: f32) -> bool {
        self.camera.update_camera_transition(frame_dt)
    }
}

#[cfg(test)]
pub(crate) fn editor_camera_axes_xy(yaw: f32) -> (Vec2, Vec2) {
    (
        Vec2::new(yaw.cos(), -yaw.sin()),
        Vec2::new(yaw.sin(), yaw.cos()),
    )
}

#[cfg(test)]
pub(crate) fn editor_camera_view_proj(
    camera: &super::super::EditorCameraState,
    viewport: Vec2,
) -> Mat4 {
    let aspect = viewport.x / viewport.y;
    let target = camera.editor_camera_target();
    let eye = target + camera.editor_camera_offset();
    let view = look_at_mat4(eye, target, Vec3::Y);
    let proj = perspective(45f32.to_radians(), aspect, 0.1, 10000.0);
    proj * view
}

pub(crate) fn project_world_to_screen(
    view_proj: Mat4,
    world: Vec3,
    viewport: Vec2,
) -> Option<Vec2> {
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

#[cfg(test)]
mod tests {
    use bevy::state::prelude::State;
    use eg_types::AppPhase;
    use glam::Vec2;

    use super::{editor_camera_axes_xy, editor_camera_view_proj, project_world_to_screen};
    use crate::{
        events::camera::SetCameraOrientation,
        state::{EditorCameraRes, EditorCameraState, test_support::TestApp},
    };

    #[test]
    fn camera_axes_follow_editor_rotation() {
        let (right, up) = editor_camera_axes_xy(0.0);
        approx::assert_abs_diff_eq!(right.x, 1.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(right.y, 0.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(up.x, 0.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(up.y, 1.0, epsilon = 1e-6);

        let (right, up) = editor_camera_axes_xy(std::f32::consts::FRAC_PI_2);
        approx::assert_abs_diff_eq!(right.x, 0.0, epsilon = 1e-5);
        approx::assert_abs_diff_eq!(right.y, -1.0, epsilon = 1e-5);
        approx::assert_abs_diff_eq!(up.x, 1.0, epsilon = 1e-5);
        approx::assert_abs_diff_eq!(up.y, 0.0, epsilon = 1e-5);
    }

    #[test]
    fn camera_offset_clamps_pitch_and_world_projection_handles_visibility() {
        let mut camera = EditorCameraState::default();

        camera.editor_yaw = 0.0;
        camera.editor_pitch = 10.0;
        let offset_clamped_high = camera.editor_camera_offset();

        camera.editor_pitch = 89.9f32.to_radians();
        let offset_at_limit = camera.editor_camera_offset();

        approx::assert_abs_diff_eq!(offset_clamped_high.x, offset_at_limit.x, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(offset_clamped_high.y, offset_at_limit.y, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(offset_clamped_high.z, offset_at_limit.z, epsilon = 1e-4);

        let viewport = Vec2::new(1280.0, 720.0);
        let target = camera.editor_camera_target();
        let view_proj = editor_camera_view_proj(&camera, viewport);

        let on_screen = project_world_to_screen(view_proj, target, viewport);
        assert!(on_screen.is_some());

        let behind_camera = target + camera.editor_camera_offset() * 2.0;
        let off_screen = project_world_to_screen(view_proj, behind_camera, viewport);
        assert!(off_screen.is_none());
    }

    #[test]
    fn camera_orientation_transition_wraps_and_interpolates() {
        let mut app = TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        {
            let mut camera = app.world_mut().resource_mut::<EditorCameraRes>();
            camera.0.editor_yaw = 3.0;
            camera.0.editor_pitch = 0.2;
        }
        app.world_mut().write_message(SetCameraOrientation {
            rotation: -3.0,
            pitch: 0.6,
            transition_seconds: Some(1.0),
        });
        app.tick();
        let camera = &mut app.world_mut().resource_mut::<EditorCameraRes>().0;
        let transition = camera.transition.as_ref().expect("transition");
        assert!(transition.target_yaw > transition.start_yaw);
        approx::assert_abs_diff_eq!(transition.target_pitch, 0.6, epsilon = 1e-6);

        assert!(camera.update_camera_transition(0.5));
        assert!(camera.transition.is_some());

        assert!(camera.update_camera_transition(0.6));
        assert!(camera.transition.is_none());
        approx::assert_abs_diff_eq!(camera.editor_pitch, 0.6, epsilon = 1e-3);
    }
}
