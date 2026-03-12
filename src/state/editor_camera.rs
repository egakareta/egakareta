use glam::{Mat4, Vec2, Vec3};

use super::{EditorSubsystem, State};
use crate::types::{AppPhase, CameraKeypoint, CameraKeypointEasing, CameraKeypointMode};

const EDITOR_CAMERA_BASE_DISTANCE: f32 = 24.0;
const PLAY_CAMERA_DISTANCE: f32 = 28.28;
const MIN_EDITOR_PITCH: f32 = -89.9f32.to_radians();
const MAX_EDITOR_PITCH: f32 = 89.9f32.to_radians();
const MIN_PLAYING_PITCH: f32 = 0.1f32.to_radians();
const DEFAULT_CAMERA_KEYPOINT_TRANSITION_INTERVAL_SECONDS: f32 = 1.0;
pub(crate) const DEFAULT_PLAY_CAMERA_ROTATION: f32 = -std::f32::consts::FRAC_PI_4;
pub(crate) const DEFAULT_PLAY_CAMERA_PITCH: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct CameraViewSample {
    eye: Vec3,
    target: Vec3,
}

pub(crate) struct EditorCameraState {
    pub(crate) editor_pan: [f32; 2],
    pub(crate) editor_target_z: f32,
    pub(crate) editor_rotation: f32,
    pub(crate) editor_pitch: f32,
    pub(crate) playing_rotation: f32,
    pub(crate) playing_pitch: f32,
    pub(crate) keypoints: Vec<CameraKeypoint>,
    pub(crate) selected_keypoint_index: Option<usize>,
}

fn offset_from_rotation_pitch(rotation: f32, pitch: f32, distance: f32) -> Vec3 {
    let horizontal_distance = distance * pitch.cos();
    let vertical_distance = distance * pitch.sin();
    Mat4::from_rotation_z(rotation).transform_vector3(Vec3::new(
        0.0,
        -horizontal_distance,
        vertical_distance,
    ))
}

fn editor_camera_offset_for_pose(rotation: f32, pitch: f32) -> Vec3 {
    let distance = EDITOR_CAMERA_BASE_DISTANCE;
    let pitch = pitch.clamp(MIN_EDITOR_PITCH, MAX_EDITOR_PITCH);
    offset_from_rotation_pitch(rotation, pitch, distance)
}

fn playing_camera_offset_for_angles(rotation: f32, pitch: f32) -> Vec3 {
    offset_from_rotation_pitch(rotation, pitch, PLAY_CAMERA_DISTANCE)
}

fn lerp_vec3(a: Vec3, b: Vec3, alpha: f32) -> Vec3 {
    a + (b - a) * alpha
}

fn eased_alpha(easing: CameraKeypointEasing, alpha: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    match easing {
        CameraKeypointEasing::Linear => alpha,
        CameraKeypointEasing::EaseIn => alpha * alpha,
        CameraKeypointEasing::EaseOut => 1.0 - (1.0 - alpha) * (1.0 - alpha),
        CameraKeypointEasing::EaseInOut => {
            if alpha < 0.5 {
                2.0 * alpha * alpha
            } else {
                1.0 - ((-2.0 * alpha + 2.0).powi(2) * 0.5)
            }
        }
    }
}

fn interpolate_camera_samples(
    start: CameraViewSample,
    end: CameraViewSample,
    alpha: f32,
) -> CameraViewSample {
    CameraViewSample {
        eye: lerp_vec3(start.eye, end.eye, alpha),
        target: lerp_vec3(start.target, end.target, alpha),
    }
}

impl EditorSubsystem {
    fn sanitize_camera_keypoint(&self, keypoint: &mut CameraKeypoint) {
        let duration = self.timeline.clock.duration_seconds.max(0.1);
        keypoint.time_seconds = if keypoint.time_seconds.is_finite() {
            keypoint.time_seconds.clamp(0.0, duration)
        } else {
            0.0
        };
        keypoint.target_position = keypoint.target_position.map(|component| {
            if component.is_finite() {
                component
            } else {
                0.0
            }
        });
        keypoint.rotation = if keypoint.rotation.is_finite() {
            keypoint.rotation
        } else {
            DEFAULT_PLAY_CAMERA_ROTATION
        };
        keypoint.pitch = if keypoint.pitch.is_finite() {
            keypoint.pitch.clamp(MIN_EDITOR_PITCH, MAX_EDITOR_PITCH)
        } else {
            DEFAULT_PLAY_CAMERA_PITCH
        };
        keypoint.transition_interval_seconds = if keypoint.transition_interval_seconds.is_finite() {
            keypoint.transition_interval_seconds.max(0.0)
        } else {
            DEFAULT_CAMERA_KEYPOINT_TRANSITION_INTERVAL_SECONDS
        };
        // use_full_segment_transition is a bool, no sanitization needed besides ensuring it's not forgotten
    }

    fn static_camera_view_for_keypoint(keypoint: &CameraKeypoint) -> CameraViewSample {
        let target = Vec3::from_array(keypoint.target_position);
        CameraViewSample {
            eye: target + editor_camera_offset_for_pose(keypoint.rotation, keypoint.pitch),
            target,
        }
    }

    pub(crate) fn editor_camera_target(&self) -> Vec3 {
        Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_pan[1],
            self.camera.editor_target_z,
        )
    }

    pub(crate) fn capture_current_camera_keypoint(&self, time_seconds: f32) -> CameraKeypoint {
        let mut keypoint = CameraKeypoint {
            time_seconds,
            mode: CameraKeypointMode::Static,
            easing: CameraKeypointEasing::Linear,
            transition_interval_seconds: DEFAULT_CAMERA_KEYPOINT_TRANSITION_INTERVAL_SECONDS,
            use_full_segment_transition: false,
            target_position: self.editor_camera_target().to_array(),
            rotation: self.camera.editor_rotation,
            pitch: self.camera.editor_pitch,
        };
        self.sanitize_camera_keypoint(&mut keypoint);
        keypoint
    }

    fn insert_camera_keypoint_sorted(&mut self, mut keypoint: CameraKeypoint) -> usize {
        self.sanitize_camera_keypoint(&mut keypoint);

        if let Some(existing_index) =
            self.camera.keypoints.iter().position(|existing| {
                existing.time_seconds.to_bits() == keypoint.time_seconds.to_bits()
            })
        {
            self.camera.keypoints[existing_index] = keypoint;
            self.camera.selected_keypoint_index = Some(existing_index);
            return existing_index;
        }

        let insert_index = self
            .camera
            .keypoints
            .partition_point(|existing| existing.time_seconds < keypoint.time_seconds);
        self.camera.keypoints.insert(insert_index, keypoint);
        self.camera.selected_keypoint_index = Some(insert_index);
        insert_index
    }

    pub(crate) fn camera_keypoints(&self) -> &[CameraKeypoint] {
        &self.camera.keypoints
    }

    pub(crate) fn selected_camera_keypoint_index(&self) -> Option<usize> {
        self.camera
            .selected_keypoint_index
            .filter(|index| *index < self.camera.keypoints.len())
    }

    pub(crate) fn add_camera_keypoint(&mut self) -> usize {
        let keypoint = self.capture_current_camera_keypoint(self.timeline.clock.time_seconds);
        self.insert_camera_keypoint_sorted(keypoint)
    }

    pub(crate) fn remove_camera_keypoint(&mut self, index: usize) -> bool {
        if index >= self.camera.keypoints.len() {
            return false;
        }

        self.camera.keypoints.remove(index);
        self.camera.selected_keypoint_index = match self.camera.keypoints.is_empty() {
            true => None,
            false => Some(index.min(self.camera.keypoints.len() - 1)),
        };
        true
    }

    pub(crate) fn set_camera_keypoint_selected(&mut self, selected: Option<usize>) {
        self.camera.selected_keypoint_index =
            selected.filter(|index| *index < self.camera.keypoints.len());
    }

    pub(crate) fn update_camera_keypoint(
        &mut self,
        index: usize,
        mut keypoint: CameraKeypoint,
    ) -> Option<usize> {
        if index >= self.camera.keypoints.len() {
            return None;
        }

        self.sanitize_camera_keypoint(&mut keypoint);
        self.camera.keypoints.remove(index);
        let insert_index = self
            .camera
            .keypoints
            .partition_point(|existing| existing.time_seconds <= keypoint.time_seconds);
        self.camera.keypoints.insert(insert_index, keypoint);
        self.camera.selected_keypoint_index = Some(insert_index);
        Some(insert_index)
    }

    pub(crate) fn capture_selected_camera_keypoint(&mut self) -> Option<usize> {
        let index = self.selected_camera_keypoint_index()?;
        let mut keypoint = self.camera.keypoints.get(index)?.clone();
        let captured = self.capture_current_camera_keypoint(keypoint.time_seconds);
        keypoint.target_position = captured.target_position;
        keypoint.rotation = captured.rotation;
        keypoint.pitch = captured.pitch;
        self.update_camera_keypoint(index, keypoint)
    }

    pub(crate) fn apply_selected_camera_keypoint_to_editor_camera(&mut self) -> bool {
        let Some(index) = self.selected_camera_keypoint_index() else {
            return false;
        };
        let Some(keypoint) = self.camera.keypoints.get(index) else {
            return false;
        };

        self.camera.editor_pan = [keypoint.target_position[0], keypoint.target_position[1]];
        self.camera.editor_target_z = keypoint.target_position[2];
        self.camera.editor_rotation = keypoint.rotation;
        self.camera.editor_pitch = keypoint.pitch.clamp(MIN_EDITOR_PITCH, MAX_EDITOR_PITCH);
        true
    }

    pub(crate) fn adjust_zoom(&mut self, delta: f32) {
        let offset = self.camera_offset();
        let look_dir = -offset.normalize();
        let move_vec = look_dir * delta * 2.0;

        self.camera.editor_pan[0] += move_vec.x;
        self.camera.editor_pan[1] += move_vec.y;
        self.camera.editor_target_z += move_vec.z;
    }

    pub(crate) fn pan_by_input(&mut self, screen_x: f32, screen_y: f32) {
        let (camera_right_xy, camera_up_xy) = self.camera_axes_xy();
        let world_delta = camera_right_xy * screen_x + camera_up_xy * screen_y;

        self.camera.editor_pan[0] += world_delta.x;
        self.camera.editor_pan[1] += world_delta.y;
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
        let mut speed_multiplier = 1.0;
        if self.ui.shift_held {
            speed_multiplier = 0.3;
        }

        const PAN_SPEED_UNITS_PER_SEC: f32 = 40.0;
        let speed = PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier;

        let offset = self.camera_offset();
        let forward = -offset.normalize();

        let (right_xy, _) = self.camera_axes_xy();
        let right = Vec3::new(right_xy.x, right_xy.y, 0.0);

        let movement = right * (input.x * speed) + forward * (input.y * speed);

        self.camera.editor_pan[0] += movement.x;
        self.camera.editor_pan[1] += movement.y;
        self.camera.editor_target_z += movement.z;
    }

    fn resolve_live_follow_sample(&self, target: Vec3, is_game_active: bool) -> CameraViewSample {
        let rotation = if is_game_active {
            DEFAULT_PLAY_CAMERA_ROTATION
        } else {
            self.camera.playing_rotation
        };
        let pitch = if is_game_active {
            DEFAULT_PLAY_CAMERA_PITCH
        } else {
            self.camera
                .playing_pitch
                .clamp(MIN_PLAYING_PITCH, MAX_EDITOR_PITCH)
        };

        CameraViewSample {
            eye: target + playing_camera_offset_for_angles(rotation, pitch),
            target,
        }
    }
}

impl State {
    pub(super) fn editor_camera_axes_xy(&self) -> (Vec2, Vec2) {
        self.editor.camera_axes_xy()
    }

    pub(super) fn editor_camera_offset(&self) -> Vec3 {
        self.editor.camera_offset()
    }

    fn resolve_camera_keypoint_sample(
        &self,
        keypoint: &CameraKeypoint,
        live_follow_sample: CameraViewSample,
    ) -> CameraViewSample {
        match keypoint.mode {
            CameraKeypointMode::Follow => live_follow_sample,
            CameraKeypointMode::Static => {
                EditorSubsystem::static_camera_view_for_keypoint(keypoint)
            }
        }
    }

    fn evaluate_camera_track(
        &self,
        time_seconds: f32,
        live_follow_target: Vec3,
        is_game_active: bool,
    ) -> CameraViewSample {
        let live_follow_sample = self
            .editor
            .resolve_live_follow_sample(live_follow_target, is_game_active);
        let keypoints = &self.editor.camera.keypoints;
        if keypoints.is_empty() {
            return live_follow_sample;
        }

        let clamped_time = time_seconds.max(0.0);
        let next_index = keypoints.partition_point(|keypoint| keypoint.time_seconds < clamped_time);

        if next_index == 0 {
            let first = &keypoints[0];
            if first.time_seconds <= 1e-6 {
                return self.resolve_camera_keypoint_sample(first, live_follow_sample);
            }

            let end_sample = self.resolve_camera_keypoint_sample(first, live_follow_sample);
            let transition_start = if first.use_full_segment_transition {
                0.0
            } else {
                (first.time_seconds - first.transition_interval_seconds.max(0.0)).max(0.0)
            };
            if clamped_time <= transition_start {
                return live_follow_sample;
            }

            let transition_duration = (first.time_seconds - transition_start).max(1e-6);
            let alpha = eased_alpha(
                first.easing,
                (clamped_time - transition_start) / transition_duration,
            );
            return interpolate_camera_samples(live_follow_sample, end_sample, alpha);
        }

        if next_index >= keypoints.len() {
            let last = keypoints.last().expect("camera keypoints not empty");
            return self.resolve_camera_keypoint_sample(last, live_follow_sample);
        }

        let previous = &keypoints[next_index - 1];
        let next = &keypoints[next_index];
        let start_sample = self.resolve_camera_keypoint_sample(previous, live_follow_sample);
        let end_sample = self.resolve_camera_keypoint_sample(next, live_follow_sample);

        if previous.mode == CameraKeypointMode::Follow && next.mode == CameraKeypointMode::Follow {
            return live_follow_sample;
        }

        let transition_start = if next.use_full_segment_transition {
            previous.time_seconds
        } else {
            (next.time_seconds - next.transition_interval_seconds.max(0.0))
                .max(previous.time_seconds)
        };
        if clamped_time <= transition_start {
            return start_sample;
        }

        let transition_duration = (next.time_seconds - transition_start).max(1e-6);
        let local_alpha = (clamped_time - transition_start) / transition_duration;
        let eased = eased_alpha(next.easing, local_alpha);
        interpolate_camera_samples(start_sample, end_sample, eased)
    }

    pub(super) fn playing_camera_view(&self) -> (Vec3, Vec3) {
        let target = Vec3::from_array(self.gameplay.state.position);
        let sample = self.evaluate_camera_track(
            self.gameplay.state.elapsed_seconds,
            target,
            self.gameplay.state.started && !self.gameplay.state.game_over,
        );
        (sample.eye, sample.target)
    }

    pub(crate) fn editor_preview_camera_view(&self) -> ([f32; 3], [f32; 3]) {
        let target = Vec3::from_array(self.editor.timeline.preview.position);
        let sample =
            self.evaluate_camera_track(self.editor.timeline.clock.time_seconds, target, true);
        (sample.eye.to_array(), sample.target.to_array())
    }

    /// Adjusts the zoom level of the editor camera by moving its position.
    pub fn adjust_editor_zoom(&mut self, delta: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.adjust_zoom(delta);
        }
    }

    /// Pans the editor camera based on screen-space input.
    ///
    /// This is typically called in response to mouse dragging or touch gestures.
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

    /// Moves the editor cursor one unit up in the grid.
    pub fn move_editor_up(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, 1);
        }
    }

    /// Moves the editor cursor one unit down in the grid.
    pub fn move_editor_down(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, -1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{eased_alpha, interpolate_camera_samples, CameraViewSample};
    use crate::types::{CameraKeypoint, CameraKeypointEasing, CameraKeypointMode};
    use glam::Vec3;

    #[test]
    fn ease_in_out_alpha_is_symmetric() {
        let first_half = eased_alpha(CameraKeypointEasing::EaseInOut, 0.25);
        let second_half = eased_alpha(CameraKeypointEasing::EaseInOut, 0.75);
        assert!((first_half - (1.0 - second_half)).abs() <= 1e-6);
    }

    #[test]
    fn interpolate_camera_samples_lerps_eye_and_target() {
        let start = CameraViewSample {
            eye: Vec3::new(0.0, 0.0, 0.0),
            target: Vec3::new(1.0, 1.0, 1.0),
        };
        let end = CameraViewSample {
            eye: Vec3::new(10.0, 20.0, 30.0),
            target: Vec3::new(4.0, 5.0, 6.0),
        };
        let mid = interpolate_camera_samples(start, end, 0.5);
        assert_eq!(mid.eye, Vec3::new(5.0, 10.0, 15.0));
        assert_eq!(mid.target, Vec3::new(2.5, 3.0, 3.5));
    }

    #[test]
    fn camera_keypoint_modes_are_distinct() {
        let follow = CameraKeypoint {
            time_seconds: 1.0,
            mode: CameraKeypointMode::Follow,
            easing: CameraKeypointEasing::Linear,
            transition_interval_seconds: 1.0,
            use_full_segment_transition: false,
            target_position: [0.0, 0.0, 0.0],
            rotation: 0.0,
            pitch: 0.0,
        };
        let static_keypoint = CameraKeypoint {
            mode: CameraKeypointMode::Static,
            ..follow.clone()
        };

        assert_ne!(follow.mode, static_keypoint.mode);
    }
}
