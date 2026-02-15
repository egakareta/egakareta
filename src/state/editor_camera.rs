use glam::{Mat4, Vec2, Vec3};

use super::{EditorSubsystem, State};
use crate::types::AppPhase;

impl EditorSubsystem {
    pub(crate) fn adjust_zoom(&mut self, delta: f32) {
        const ZOOM_SENSITIVITY: f32 = 0.12;
        let factor = (1.0 + delta * ZOOM_SENSITIVITY).max(0.1);
        self.camera.editor_zoom = (self.camera.editor_zoom * factor).clamp(0.35, 4.0);
    }

    pub(crate) fn pan_by_input(&mut self, screen_x: f32, screen_y: f32) {
        let (camera_right_xy, camera_up_xy) = self.camera_axes_xy();
        let world_delta = camera_right_xy * screen_x + camera_up_xy * screen_y;

        let max_pan = self.ui.bounds as f32;
        self.camera.editor_pan[0] =
            (self.camera.editor_pan[0] + world_delta.x).clamp(-max_pan, max_pan);
        self.camera.editor_pan[1] =
            (self.camera.editor_pan[1] + world_delta.y).clamp(-max_pan, max_pan);
    }

    pub(crate) fn update_pan_from_keys(&mut self, frame_dt: f32) {
        let mut input = Vec2::ZERO;
        if self.ui.pan_left_held {
            input.x -= 1.0;
        }
        if self.ui.pan_right_held {
            input.x += 1.0;
        }
        if self.ui.pan_up_held {
            input.y += 1.0;
        }
        if self.ui.pan_down_held {
            input.y -= 1.0;
        }

        if input.length_squared() <= f32::EPSILON {
            return;
        }

        let input = input.normalize();
        let pitch = self
            .camera
            .editor_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_factor = pitch.cos();
        let vertical_factor = pitch.sin();

        let mut speed_multiplier = 1.0;
        if self.ui.shift_held {
            speed_multiplier = 0.3;
        }

        const PAN_SPEED_UNITS_PER_SEC: f32 = 40.0;
        self.pan_by_input(
            input.x * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
            input.y * horizontal_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );

        self.adjust_zoom(
            input.y * vertical_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );
    }
}

impl State {
    pub(super) fn editor_camera_axes_xy(&self) -> (Vec2, Vec2) {
        self.editor.camera_axes_xy()
    }

    pub(super) fn editor_camera_offset(&self) -> Vec3 {
        self.editor.camera_offset()
    }

    pub(super) fn playing_camera_offset(&self) -> Vec3 {
        let distance = 28.28;
        let rotation = if self.gameplay.state.game_over || !self.gameplay.state.started {
            self.editor.camera.playing_rotation
        } else {
            -45.0f32.to_radians()
        };
        let pitch = if self.gameplay.state.game_over || !self.gameplay.state.started {
            self.editor.camera.playing_pitch
        } else {
            45.0f32.to_radians()
        };

        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
    }

    pub fn adjust_editor_zoom(&mut self, delta: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.adjust_zoom(delta);
        }
    }

    pub fn pan_editor_camera_by_input(&mut self, screen_x: f32, screen_y: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.pan_by_input(screen_x, screen_y);
        }
    }

    pub(super) fn update_editor_pan_from_keys(&mut self, frame_dt: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.update_pan_from_keys(frame_dt);
        }
    }

    pub fn move_editor_up(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, 1);
        }
    }

    pub fn move_editor_down(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, -1);
        }
    }
}
