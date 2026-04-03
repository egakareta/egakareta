/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use glam::{Mat4, Vec2, Vec3};

use super::{EditorSubsystem, State};
use crate::types::{
    timed_triggers_to_camera_triggers, AppPhase, CameraTrigger, CameraTriggerMode, TimedTrigger,
    TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
};

const EDITOR_CAMERA_BASE_DISTANCE: f32 = 24.0;
const PLAY_CAMERA_DISTANCE: f32 = 28.28;
const MIN_EDITOR_PITCH: f32 = -89.9f32.to_radians();
const MAX_EDITOR_PITCH: f32 = 89.9f32.to_radians();
const MIN_PLAYING_PITCH: f32 = 0.1f32.to_radians();
const DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS: f32 = 1.0;
pub(crate) const DEFAULT_PLAY_CAMERA_ROTATION: f32 = std::f32::consts::FRAC_PI_4;
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
    pub(crate) transition: Option<CameraTransition>,
}

pub(crate) struct CameraTransition {
    pub(crate) start_rotation: f32,
    pub(crate) start_pitch: f32,
    pub(crate) target_rotation: f32,
    pub(crate) target_pitch: f32,
    pub(crate) elapsed: f32,
    pub(crate) duration: f32,
}

fn offset_from_rotation_pitch(rotation: f32, pitch: f32, distance: f32) -> Vec3 {
    let horizontal_distance = distance * pitch.cos();
    let vertical_distance = distance * pitch.sin();
    Mat4::from_rotation_y(rotation).transform_vector3(Vec3::new(
        0.0,
        vertical_distance,
        -horizontal_distance,
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

fn eased_alpha(easing: TimedTriggerEasing, alpha: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    match easing {
        TimedTriggerEasing::Linear => alpha,
        TimedTriggerEasing::EaseIn => alpha * alpha,
        TimedTriggerEasing::EaseOut => 1.0 - (1.0 - alpha) * (1.0 - alpha),
        TimedTriggerEasing::EaseInOut => {
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
    fn static_camera_view_for_trigger(camera_trigger: &CameraTrigger) -> CameraViewSample {
        let target = Vec3::from_array(camera_trigger.target_position);
        CameraViewSample {
            eye: target
                + editor_camera_offset_for_pose(camera_trigger.rotation, camera_trigger.pitch),
            target,
        }
    }

    pub(crate) fn camera_trigger_marker_eye(&self, camera_trigger: &CameraTrigger) -> Vec3 {
        Vec3::from_array(camera_trigger.target_position)
            + editor_camera_offset_for_pose(camera_trigger.rotation, camera_trigger.pitch)
    }

    pub(crate) fn camera_trigger_marker_forward(&self, camera_trigger: &CameraTrigger) -> Vec3 {
        let eye = self.camera_trigger_marker_eye(camera_trigger);
        let to_target = Vec3::from_array(camera_trigger.target_position) - eye;
        if to_target.length_squared() <= f32::EPSILON {
            Vec3::Z
        } else {
            to_target.normalize()
        }
    }

    pub(crate) fn editor_camera_target(&self) -> Vec3 {
        Vec3::new(
            self.camera.editor_pan[0],
            self.camera.editor_target_z,
            self.camera.editor_pan[1],
        )
    }

    pub(crate) fn capture_current_camera_trigger(&self, time_seconds: f32) -> TimedTrigger {
        TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Camera,
            action: TimedTriggerAction::CameraPose {
                transition_interval_seconds: DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS,
                use_full_segment_transition: false,
                target_position: self.editor_camera_target().to_array(),
                rotation: self.camera.editor_rotation,
                pitch: self.camera.editor_pitch,
            },
        }
    }

    pub(crate) fn apply_selected_camera_trigger_to_editor_camera(&mut self) -> bool {
        let Some(index) = self.selected_trigger_index() else {
            return false;
        };
        let Some(trigger) = self.triggers().get(index) else {
            return false;
        };
        let Some(camera_trigger) = timed_triggers_to_camera_triggers(std::slice::from_ref(trigger))
            .into_iter()
            .next()
        else {
            return false;
        };

        self.camera.editor_pan = [
            camera_trigger.target_position[0],
            camera_trigger.target_position[2],
        ];
        self.camera.editor_target_z = camera_trigger.target_position[1];
        self.camera.editor_rotation = camera_trigger.rotation;
        self.camera.editor_pitch = camera_trigger
            .pitch
            .clamp(MIN_EDITOR_PITCH, MAX_EDITOR_PITCH);
        true
    }

    pub(crate) fn adjust_zoom(&mut self, delta: f32) {
        let offset = self.camera_offset();
        let look_dir = -offset.normalize();
        let move_vec = look_dir * delta * 2.0;

        self.camera.editor_pan[0] += move_vec.x;
        self.camera.editor_pan[1] += move_vec.z;
        self.camera.editor_target_z += move_vec.y;
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
        let right = Vec3::new(right_xy.x, 0.0, right_xy.y);

        let movement = right * (-input.x * speed) + forward * (input.y * speed);

        self.camera.editor_pan[0] += movement.x;
        self.camera.editor_pan[1] += movement.z;
        self.camera.editor_target_z += movement.y;
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

    fn resolve_camera_trigger_sample(
        &self,
        camera_trigger: &CameraTrigger,
        live_follow_sample: CameraViewSample,
    ) -> CameraViewSample {
        match camera_trigger.mode {
            CameraTriggerMode::Follow => live_follow_sample,
            CameraTriggerMode::Static => {
                EditorSubsystem::static_camera_view_for_trigger(camera_trigger)
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
        let camera_triggers = timed_triggers_to_camera_triggers(self.editor.triggers());
        if camera_triggers.is_empty() {
            return live_follow_sample;
        }

        let clamped_time = time_seconds.max(0.0);
        let next_index =
            camera_triggers.partition_point(|trigger| trigger.time_seconds < clamped_time);

        if next_index == 0 {
            let first = &camera_triggers[0];
            if first.time_seconds <= 1e-6 {
                return self.resolve_camera_trigger_sample(first, live_follow_sample);
            }

            let end_sample = self.resolve_camera_trigger_sample(first, live_follow_sample);
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

        if next_index >= camera_triggers.len() {
            let last = camera_triggers.last().expect("camera triggers not empty");
            return self.resolve_camera_trigger_sample(last, live_follow_sample);
        }

        let previous = &camera_triggers[next_index - 1];
        let next = &camera_triggers[next_index];
        let start_sample = self.resolve_camera_trigger_sample(previous, live_follow_sample);
        let end_sample = self.resolve_camera_trigger_sample(next, live_follow_sample);

        if previous.mode == CameraTriggerMode::Follow && next.mode == CameraTriggerMode::Follow {
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
            self.editor.mark_dirty(crate::state::EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..Default::default()
            });
        }
    }

    /// Pans the editor camera based on screen-space input.
    ///
    /// This is typically called in response to mouse dragging or touch gestures.
    pub fn pan_editor_camera_by_input(&mut self, screen_x: f32, screen_y: f32) {
        if self.phase == AppPhase::Editor {
            self.editor.pan_by_input(screen_x, screen_y);
            self.editor.mark_dirty(crate::state::EditorDirtyFlags {
                rebuild_selection_overlays: true,
                rebuild_cursor: true,
                rebuild_tap_indicators: true,
                rebuild_preview_player: true,
                ..Default::default()
            });
        }
    }

    pub(super) fn update_editor_pan_from_keys(&mut self, frame_dt: f32) {
        if self.phase == AppPhase::Editor {
            let previous_pan = self.editor.camera.editor_pan;
            self.editor.update_pan_from_keys(frame_dt);
            if previous_pan != self.editor.camera.editor_pan {
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
    use crate::types::{CameraTrigger, CameraTriggerMode, TimedTriggerEasing};
    use glam::Vec3;

    #[test]
    fn ease_in_out_alpha_is_symmetric() {
        let first_half = eased_alpha(TimedTriggerEasing::EaseInOut, 0.25);
        let second_half = eased_alpha(TimedTriggerEasing::EaseInOut, 0.75);
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
    fn camera_trigger_modes_are_distinct() {
        let follow = CameraTrigger {
            time_seconds: 1.0,
            mode: CameraTriggerMode::Follow,
            easing: TimedTriggerEasing::Linear,
            transition_interval_seconds: 1.0,
            use_full_segment_transition: false,
            target_position: [0.0, 0.0, 0.0],
            rotation: 0.0,
            pitch: 0.0,
        };
        let static_trigger = CameraTrigger {
            mode: CameraTriggerMode::Static,
            ..follow.clone()
        };

        assert_ne!(follow.mode, static_trigger.mode);
    }
}
