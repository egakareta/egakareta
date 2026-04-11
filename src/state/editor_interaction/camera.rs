/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::{EditorSubsystem, State};
use crate::types::AppPhase;
use glam::{Mat4, Vec2, Vec3};

impl EditorSubsystem {
    pub(crate) fn camera_axes_xy(&self) -> (Vec2, Vec2) {
        let right = Vec2::new(
            self.camera.editor_rotation.cos(),
            -self.camera.editor_rotation.sin(),
        );
        let up = Vec2::new(
            self.camera.editor_rotation.sin(),
            self.camera.editor_rotation.cos(),
        );
        (right, up)
    }

    pub(crate) fn camera_offset(&self) -> Vec3 {
        let distance = 24.0;
        let pitch = self
            .camera
            .editor_pitch
            .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_y(self.camera.editor_rotation).transform_vector3(Vec3::new(
            0.0,
            vertical_distance,
            -horizontal_distance,
        ))
    }

    pub(crate) fn view_proj(&self, viewport: Vec2) -> Mat4 {
        let aspect = viewport.x / viewport.y;
        let target = Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_target_z,
            self.camera.editor_pan[1],
        );
        let eye = target + self.camera_offset();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        proj * view
    }

    pub(crate) fn world_to_screen_v(&self, world: Vec3, viewport: Vec2) -> Option<Vec2> {
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

    pub(crate) fn drag_camera_by_pixels(
        &mut self,
        dx: f64,
        dy: f64,
        phase: AppPhase,
        is_game_active: bool,
    ) {
        if !self.ui.right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.004;
        const PITCH_SPEED: f32 = 0.006;

        if phase == AppPhase::Editor {
            self.camera.editor_rotation -= dx as f32 * ROTATE_SPEED;
            self.camera.editor_pitch = (self.camera.editor_pitch + dy as f32 * PITCH_SPEED)
                .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        } else if phase == AppPhase::Playing && !is_game_active {
            self.camera.playing_rotation -= dx as f32 * ROTATE_SPEED;
            self.camera.playing_pitch = (self.camera.playing_pitch + dy as f32 * PITCH_SPEED)
                .clamp(0.1f32.to_radians(), 89.9f32.to_radians());
        }
    }

    pub(crate) fn set_editor_camera_orientation(
        &mut self,
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    ) {
        if let Some(duration) = transition_seconds {
            if duration > 0.0 {
                let current_rot = self.camera.editor_rotation;
                let current_pitch = self.camera.editor_pitch;

                let mut target_rot = rotation;
                let diff = target_rot - current_rot;
                if diff > std::f32::consts::PI {
                    target_rot -= std::f32::consts::TAU;
                } else if diff < -std::f32::consts::PI {
                    target_rot += std::f32::consts::TAU;
                }

                self.camera.transition = Some(crate::state::editor_camera::CameraTransition {
                    start_rotation: current_rot,
                    start_pitch: current_pitch,
                    target_rotation: target_rot,
                    target_pitch: pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians()),
                    elapsed: 0.0,
                    duration,
                });
                return;
            }
        }

        self.camera.transition = None;
        self.camera.editor_rotation = rotation;
        self.camera.editor_pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
    }

    pub(crate) fn update_camera_transition(&mut self, frame_dt: f32) -> bool {
        let Some(mut transition) = self.camera.transition.take() else {
            return false;
        };

        transition.elapsed += frame_dt;
        let alpha = (transition.elapsed / transition.duration).clamp(0.0, 1.0);

        self.camera.editor_rotation = transition.start_rotation
            + (transition.target_rotation - transition.start_rotation) * alpha;
        self.camera.editor_pitch =
            transition.start_pitch + (transition.target_pitch - transition.start_pitch) * alpha;

        if alpha < 1.0 {
            self.camera.transition = Some(transition);
        }

        true
    }
}

impl State {
    pub(crate) fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        let is_game_active = self.gameplay.state.started && !self.gameplay.state.game_over;
        self.editor
            .drag_camera_by_pixels(dx, dy, self.phase, is_game_active);

        if self.phase == AppPhase::Editor {
            self.editor.mark_dirty(crate::state::EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..Default::default()
            });
        }
    }

    pub(crate) fn set_editor_camera_orientation(
        &mut self,
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    ) {
        if self.phase == AppPhase::Editor {
            self.editor
                .set_editor_camera_orientation(rotation, pitch, transition_seconds);
            self.editor.mark_dirty(crate::state::EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..Default::default()
            });
        }
    }

    pub(crate) fn update_editor_camera_transition(&mut self, frame_dt: f32) {
        if self.phase == AppPhase::Editor && self.editor.update_camera_transition(frame_dt) {
            self.editor.mark_dirty(crate::state::EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..Default::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use crate::test_utils::assert_approx_eq as approx_eq;
    use crate::types::AppPhase;
    use glam::{Vec2, Vec3};

    #[test]
    fn camera_axes_follow_editor_rotation() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.camera.editor_rotation = 0.0;
            let (right, up) = state.editor.camera_axes_xy();
            approx_eq(right.x, 1.0, 1e-6);
            approx_eq(right.y, 0.0, 1e-6);
            approx_eq(up.x, 0.0, 1e-6);
            approx_eq(up.y, 1.0, 1e-6);

            state.editor.camera.editor_rotation = std::f32::consts::FRAC_PI_2;
            let (right, up) = state.editor.camera_axes_xy();
            approx_eq(right.x, 0.0, 1e-5);
            approx_eq(right.y, -1.0, 1e-5);
            approx_eq(up.x, 1.0, 1e-5);
            approx_eq(up.y, 0.0, 1e-5);
        });
    }

    #[test]
    fn camera_offset_clamps_pitch_and_world_projection_handles_visibility() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.camera.editor_rotation = 0.0;
            state.editor.camera.editor_pitch = 10.0;
            let offset_clamped_high = state.editor.camera_offset();

            state.editor.camera.editor_pitch = 89.9f32.to_radians();
            let offset_at_limit = state.editor.camera_offset();

            approx_eq(offset_clamped_high.x, offset_at_limit.x, 1e-4);
            approx_eq(offset_clamped_high.y, offset_at_limit.y, 1e-4);
            approx_eq(offset_clamped_high.z, offset_at_limit.z, 1e-4);

            let viewport = Vec2::new(1280.0, 720.0);
            let target = Vec3::new(
                state.editor.camera.editor_pan[0],
                state.editor.camera.editor_target_z,
                state.editor.camera.editor_pan[1],
            );

            let on_screen = state.editor.world_to_screen_v(target, viewport);
            assert!(on_screen.is_some());

            let behind_camera = target + state.editor.camera_offset() * 2.0;
            let off_screen = state.editor.world_to_screen_v(behind_camera, viewport);
            assert!(off_screen.is_none());
        });
    }

    #[test]
    fn camera_orientation_transition_wraps_and_interpolates() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.camera.editor_rotation = 3.0;
            state.editor.camera.editor_pitch = 0.2;

            state
                .editor
                .set_editor_camera_orientation(-3.0, 0.6, Some(1.0));
            let transition = state.editor.camera.transition.as_ref().expect("transition");
            assert!(transition.target_rotation > transition.start_rotation);
            approx_eq(transition.target_pitch, 0.6, 1e-6);

            assert!(state.editor.update_camera_transition(0.5));
            assert!(state.editor.camera.transition.is_some());

            assert!(state.editor.update_camera_transition(0.6));
            assert!(state.editor.camera.transition.is_none());
            approx_eq(state.editor.camera.editor_pitch, 0.6, 1e-3);
        });
    }

    #[test]
    fn state_camera_orientation_api_only_applies_in_editor_phase() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            let baseline_rotation = state.editor.camera.editor_rotation;
            state.phase = AppPhase::Menu;
            state.set_editor_camera_orientation(1.2, 0.3, None);
            approx_eq(state.editor.camera.editor_rotation, baseline_rotation, 1e-6);

            state.phase = AppPhase::Editor;
            state.set_editor_camera_orientation(1.2, 0.3, None);
            approx_eq(state.editor.camera.editor_rotation, 1.2, 1e-6);
            approx_eq(state.editor.camera.editor_pitch, 0.3, 1e-6);
        });
    }
}
