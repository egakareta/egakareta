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

    pub(crate) fn set_editor_camera_orientation(&mut self, rotation: f32, pitch: f32) {
        self.camera.editor_rotation = rotation;
        self.camera.editor_pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
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

    pub(crate) fn set_editor_camera_orientation(&mut self, rotation: f32, pitch: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.set_editor_camera_orientation(rotation, pitch);
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
