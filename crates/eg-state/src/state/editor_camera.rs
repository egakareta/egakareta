/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy::{
    ecs::prelude::{Res, ResMut},
    state::prelude::State,
};
#[cfg(test)]
use eg_blocks::BlockId;
use eg_types::{
    AppPhase, CameraMode, CameraTrigger, DEFAULT_CAMERA_DISTANCE, DEFAULT_CAMERA_PITCH,
    DEFAULT_CAMERA_YAW, EasingDirection, EditorInputState, LevelObject, TimedTrigger,
};
#[cfg(test)]
use eg_types::{CAMERA_TRIGGER_BLOCK_ID, TriggerTarget};
use glam::{Mat4, Vec2, Vec3};

#[cfg(test)]
use super::EditorSubsystem;
#[cfg(test)]
use super::editor_triggers::camera_trigger_object_pose;

const PLAY_CAMERA_DISTANCE: f32 = 28.28;
const MIN_EDITOR_PITCH: f32 = -89.9f32.to_radians();
const MAX_EDITOR_PITCH: f32 = 89.9f32.to_radians();
const MIN_PLAYING_PITCH: f32 = 0.1f32.to_radians();
#[cfg(test)]
const DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CameraViewSample {
    pub(crate) eye: Vec3,
    pub(crate) target: Vec3,
}

pub struct EditorCameraState {
    pub editor_pan: [f32; 2],
    pub editor_target_z: f32,
    pub editor_yaw: f32,
    pub editor_pitch: f32,
    pub playing_rotation: f32,
    pub playing_pitch: f32,
    pub transition: Option<CameraTransition>,
}

// Manual `Clone` keeps camera resources and transition fixtures independently copyable.
impl Clone for EditorCameraState {
    fn clone(&self) -> Self {
        Self {
            editor_pan: self.editor_pan,
            editor_target_z: self.editor_target_z,
            editor_yaw: self.editor_yaw,
            editor_pitch: self.editor_pitch,
            playing_rotation: self.playing_rotation,
            playing_pitch: self.playing_pitch,
            transition: self.transition.clone(),
        }
    }
}

impl EditorCameraState {
    pub fn editor_camera_target(&self) -> Vec3 {
        Vec3::new(self.editor_pan[0], self.editor_target_z, self.editor_pan[1])
    }

    pub fn editor_camera_offset(&self) -> Vec3 {
        editor_camera_offset_for_pose(self.editor_yaw, self.editor_pitch)
    }

    /// Install a new camera transition (Phase 3.11 carve-out
    /// helper). Equivalent to constructing
    /// `CameraTransition { ... }` and assigning it to
    /// `self.transition`, but exposed as a method so callers
    /// outside the `editor_camera` module can set the transition
    /// without depending on the private `CameraTransition` type.
    pub fn set_transition(
        &mut self,
        start_yaw: f32,
        start_pitch: f32,
        target_yaw: f32,
        target_pitch: f32,
        duration: f32,
    ) {
        self.transition = Some(CameraTransition {
            start_yaw,
            start_pitch,
            target_yaw,
            target_pitch,
            elapsed: 0.0,
            duration,
        });
    }

    pub fn update_pan_from_input(&mut self, input_state: &EditorInputState, frame_dt: f32) -> bool {
        let mut input = Vec2::ZERO;
        if input_state.pan_left_held {
            input.x -= 1.0;
        }
        if input_state.pan_right_held {
            input.x += 1.0;
        }
        if input_state.pan_up_held {
            input.y += 1.0;
        }
        if input_state.pan_down_held {
            input.y -= 1.0;
        }

        if input.length_squared() <= f32::EPSILON {
            return false;
        }

        let input = input.normalize();
        let speed_multiplier = if input_state.shift_held { 0.3 } else { 1.0 };
        const PAN_SPEED_UNITS_PER_SEC: f32 = 40.0;
        let speed = PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier;

        let offset = self.editor_camera_offset();
        let forward = -offset.normalize();
        let right_xy = Vec2::new(self.editor_yaw.cos(), -self.editor_yaw.sin());
        let right = Vec3::new(right_xy.x, 0.0, right_xy.y);
        let movement = right * (-input.x * speed) + forward * (input.y * speed);

        self.editor_pan[0] += movement.x;
        self.editor_pan[1] += movement.z;
        self.editor_target_z += movement.y;
        true
    }

    pub fn update_camera_transition(&mut self, frame_dt: f32) -> bool {
        let Some(mut transition) = self.transition.take() else {
            return false;
        };

        transition.elapsed += frame_dt;
        let alpha = (transition.elapsed / transition.duration).clamp(0.0, 1.0);
        self.editor_yaw =
            transition.start_yaw + (transition.target_yaw - transition.start_yaw) * alpha;
        self.editor_pitch =
            transition.start_pitch + (transition.target_pitch - transition.start_pitch) * alpha;

        if alpha < 1.0 {
            self.transition = Some(transition);
        }
        true
    }
}

impl Default for EditorCameraState {
    fn default() -> Self {
        Self {
            editor_pan: [0.0, 0.0],
            editor_target_z: 0.0,
            editor_yaw: DEFAULT_CAMERA_YAW,
            editor_pitch: DEFAULT_CAMERA_PITCH,
            playing_rotation: DEFAULT_CAMERA_YAW,
            playing_pitch: DEFAULT_CAMERA_PITCH,
            transition: None,
        }
    }
}

pub struct CameraTransition {
    pub start_yaw: f32,
    pub start_pitch: f32,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub elapsed: f32,
    pub duration: f32,
}

impl Clone for CameraTransition {
    fn clone(&self) -> Self {
        Self {
            start_yaw: self.start_yaw,
            start_pitch: self.start_pitch,
            target_yaw: self.target_yaw,
            target_pitch: self.target_pitch,
            elapsed: self.elapsed,
            duration: self.duration,
        }
    }
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
    let pitch = pitch.clamp(MIN_EDITOR_PITCH, MAX_EDITOR_PITCH);
    offset_from_rotation_pitch(rotation, pitch, DEFAULT_CAMERA_DISTANCE)
}

fn playing_camera_offset_for_angles(rotation: f32, pitch: f32) -> Vec3 {
    offset_from_rotation_pitch(rotation, pitch, PLAY_CAMERA_DISTANCE)
}

fn lerp_vec3(a: Vec3, b: Vec3, alpha: f32) -> Vec3 {
    a + (b - a) * alpha
}

fn eased_alpha(easing: EasingDirection, alpha: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    match easing {
        EasingDirection::Linear => alpha,
        EasingDirection::EaseIn => alpha * alpha,
        EasingDirection::EaseOut => 1.0 - (1.0 - alpha) * (1.0 - alpha),
        EasingDirection::EaseInOut => {
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

fn live_follow_sample_for_camera(
    camera: &EditorCameraState,
    target: Vec3,
    is_game_active: bool,
) -> CameraViewSample {
    let rotation = if is_game_active {
        DEFAULT_CAMERA_YAW
    } else {
        camera.playing_rotation
    };
    let pitch = if is_game_active {
        DEFAULT_CAMERA_PITCH
    } else {
        camera
            .playing_pitch
            .clamp(MIN_PLAYING_PITCH, MAX_EDITOR_PITCH)
    };

    CameraViewSample {
        eye: target + playing_camera_offset_for_angles(rotation, pitch),
        target,
    }
}

fn resolve_camera_trigger_sample(
    camera_trigger: &CameraTrigger,
    live_follow_sample: CameraViewSample,
) -> CameraViewSample {
    match camera_trigger.mode {
        CameraMode::Follow => {
            follow_camera_view_for_trigger(camera_trigger, live_follow_sample.target)
        }
        CameraMode::Static => static_camera_view_for_trigger(camera_trigger),
    }
}

pub(crate) fn evaluate_camera_track_for_objects(
    camera: &EditorCameraState,
    objects: &[LevelObject],
    time_seconds: f32,
    live_follow_target: Vec3,
    is_game_active: bool,
) -> CameraViewSample {
    let live_follow_sample =
        live_follow_sample_for_camera(camera, live_follow_target, is_game_active);
    let mut camera_triggers = objects
        .iter()
        .filter_map(|object| {
            let trigger = object.trigger.as_ref()?;
            let camera = trigger.camera.as_ref()?;
            Some((trigger, camera))
        })
        .collect::<Vec<(&TimedTrigger, &CameraTrigger)>>();
    camera_triggers
        .sort_by(|left, right| f32::total_cmp(&left.0.time_seconds, &right.0.time_seconds));
    if camera_triggers.is_empty() {
        return live_follow_sample;
    }

    let clamped_time = time_seconds.max(0.0);
    let next_index =
        camera_triggers.partition_point(|(trigger, _)| trigger.time_seconds < clamped_time);

    if next_index == 0 {
        let (first_t, first_c) = camera_triggers[0];
        if first_t.time_seconds <= 1e-6 {
            return resolve_camera_trigger_sample(first_c, live_follow_sample);
        }

        let end_sample = resolve_camera_trigger_sample(first_c, live_follow_sample);
        let transition_start = if first_c.use_full_segment_transition {
            0.0
        } else {
            (first_t.time_seconds - first_c.transition_interval_seconds.max(0.0)).max(0.0)
        };
        if clamped_time <= transition_start {
            return live_follow_sample;
        }

        let transition_duration = (first_t.time_seconds - transition_start).max(1e-6);
        let alpha = eased_alpha(
            first_t.easing,
            (clamped_time - transition_start) / transition_duration,
        );
        return interpolate_camera_samples(live_follow_sample, end_sample, alpha);
    }

    if next_index >= camera_triggers.len() {
        let (_, last_c) = *camera_triggers.last().expect("camera triggers not empty");
        return resolve_camera_trigger_sample(last_c, live_follow_sample);
    }

    let (previous_t, previous_c) = camera_triggers[next_index - 1];
    let (next_t, next_c) = camera_triggers[next_index];
    let start_sample = resolve_camera_trigger_sample(previous_c, live_follow_sample);
    let end_sample = resolve_camera_trigger_sample(next_c, live_follow_sample);

    let transition_start = if next_c.use_full_segment_transition {
        previous_t.time_seconds
    } else {
        (next_t.time_seconds - next_c.transition_interval_seconds.max(0.0))
            .max(previous_t.time_seconds)
    };
    if clamped_time <= transition_start {
        return start_sample;
    }

    let transition_duration = (next_t.time_seconds - transition_start).max(1e-6);
    let local_alpha = (clamped_time - transition_start) / transition_duration;
    let eased = eased_alpha(next_t.easing, local_alpha);
    interpolate_camera_samples(start_sample, end_sample, eased)
}

/// Returns the editor preview camera pose for presentation code that owns no
/// editor state itself.
pub fn editor_camera_preview_for_objects(
    camera: &EditorCameraState,
    objects: &[LevelObject],
    time_seconds: f32,
    live_follow_target: [f32; 3],
    is_game_active: bool,
) -> ([f32; 3], [f32; 3]) {
    let sample = evaluate_camera_track_for_objects(
        camera,
        objects,
        time_seconds,
        Vec3::from_array(live_follow_target),
        is_game_active,
    );
    (sample.eye.to_array(), sample.target.to_array())
}

fn static_camera_view_for_trigger(camera_trigger: &CameraTrigger) -> CameraViewSample {
    let target = Vec3::from_array(camera_trigger.target_position);
    CameraViewSample {
        eye: target + editor_camera_offset_for_pose(camera_trigger.yaw, camera_trigger.pitch),
        target,
    }
}

fn follow_camera_view_for_trigger(
    camera_trigger: &CameraTrigger,
    live_follow_target: Vec3,
) -> CameraViewSample {
    let pitch = camera_trigger
        .pitch
        .clamp(MIN_PLAYING_PITCH, MAX_EDITOR_PITCH);
    CameraViewSample {
        eye: live_follow_target
            + offset_from_rotation_pitch(
                camera_trigger.yaw,
                pitch,
                camera_trigger.distance.max(0.01),
            ),
        target: live_follow_target,
    }
}

super::editor_context_methods! {
    #[cfg(test)]
    pub(crate) fn evaluate_camera_track(
        &self,
        time_seconds: f32,
        live_follow_target: Vec3,
        is_game_active: bool,
    ) -> CameraViewSample {
        evaluate_camera_track_for_objects(
            &self.camera,
            &self.objects,
            time_seconds,
            live_follow_target,
            is_game_active,
        )
    }

    #[cfg(test)]
    pub fn capture_current_camera_trigger(&self, time_seconds: f32) -> LevelObject {
        let camera = CameraTrigger {
            mode: CameraMode::Static,
            transition_interval_seconds: DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS,
            use_full_segment_transition: false,
            target_position: self.camera.editor_camera_target().to_array(),
            yaw: self.camera.editor_yaw,
            pitch: self.camera.editor_pitch,
            ..CameraTrigger::default()
        };
        let (position, rotation) = camera_trigger_object_pose(&camera);
        LevelObject {
            position,
            rotation,
            size: [1.0, 1.0, 1.0],
            block_id: BlockId::new(CAMERA_TRIGGER_BLOCK_ID),
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                camera: Some(camera),
                ..TimedTrigger::default()
            }),
            ..LevelObject::default()
        }
    }

    #[cfg(test)]
    pub fn adjust_zoom(&mut self, delta: f32) {
        let offset = self.camera_offset();
        let look_dir = -offset.normalize();
        let move_vec = look_dir * delta * 2.0;

        self.camera.editor_pan[0] += move_vec.x;
        self.camera.editor_pan[1] += move_vec.z;
        self.camera.editor_target_z += move_vec.y;
    }

    #[cfg(test)]
    pub fn pan_by_input(&mut self, screen_x: f32, screen_y: f32) {
        let (camera_right_xy, camera_up_xy) = self.camera_axes_xy();
        let world_delta = camera_right_xy * screen_x + camera_up_xy * screen_y;

        self.camera.editor_pan[0] += world_delta.x;
        self.camera.editor_pan[1] += world_delta.y;
    }

    #[cfg(test)]
    pub fn update_pan_from_keys(&mut self, frame_dt: f32) {
        self.camera.update_pan_from_input(&self.input, frame_dt);
    }
}

/// Advance editor camera key panning from App-owned input and camera state.
pub fn update_editor_camera_pan_system(
    phase: Res<State<AppPhase>>,
    frame_time: Res<crate::state::FrameTimeRes>,
    input: Res<crate::state::EditorInputRes>,
    mut camera: ResMut<crate::state::EditorCameraRes>,
) {
    if *phase.get() == AppPhase::Editor {
        camera.0.update_camera_transition(frame_time.delta_seconds);
        camera
            .0
            .update_pan_from_input(&input.0, frame_time.delta_seconds);
    }
}

#[cfg(test)]
mod tests {
    use eg_types::{
        CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection, LevelObject,
        TimedTrigger, TriggerTarget,
    };
    use glam::Vec3;

    use super::{
        CameraViewSample, DEFAULT_CAMERA_PITCH, DEFAULT_CAMERA_YAW, EditorSubsystem,
        MIN_PLAYING_PITCH, eased_alpha, editor_camera_offset_for_pose, interpolate_camera_samples,
        live_follow_sample_for_camera, offset_from_rotation_pitch,
        playing_camera_offset_for_angles,
    };

    fn assert_vec3_close(actual: Vec3, expected: Vec3) {
        assert!(
            (actual.x - expected.x).abs() <= 1e-5,
            "x mismatch: {actual:?} vs {expected:?}"
        );
        assert!(
            (actual.y - expected.y).abs() <= 1e-5,
            "y mismatch: {actual:?} vs {expected:?}"
        );
        assert!(
            (actual.z - expected.z).abs() <= 1e-5,
            "z mismatch: {actual:?} vs {expected:?}"
        );
    }

    fn static_trigger(
        time_seconds: f32,
        target_position: [f32; 3],
        yaw: f32,
        pitch: f32,
        transition_interval_seconds: f32,
        use_full_segment_transition: bool,
        easing: EasingDirection,
    ) -> LevelObject {
        LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Static,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position,
                    yaw,
                    pitch,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    fn follow_trigger(
        time_seconds: f32,
        transition_interval_seconds: f32,
        use_full_segment_transition: bool,
    ) -> LevelObject {
        follow_trigger_with_orbit(
            time_seconds,
            transition_interval_seconds,
            use_full_segment_transition,
            CameraTrigger::default().distance,
            CameraTrigger::default().yaw,
            CameraTrigger::default().pitch,
        )
    }

    fn follow_trigger_with_orbit(
        time_seconds: f32,
        transition_interval_seconds: f32,
        use_full_segment_transition: bool,
        distance: f32,
        yaw: f32,
        pitch: f32,
    ) -> LevelObject {
        LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Follow,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    distance,
                    yaw,
                    pitch,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    fn new_editor() -> EditorSubsystem {
        EditorSubsystem::default()
    }

    #[test]
    fn eased_alpha_clamps_bounds_for_all_curves() {
        for easing in [
            EasingDirection::Linear,
            EasingDirection::EaseIn,
            EasingDirection::EaseOut,
            EasingDirection::EaseInOut,
        ] {
            assert_eq!(eased_alpha(easing, -1.0), 0.0);
            assert_eq!(eased_alpha(easing, 2.0), 1.0);
        }
    }

    #[test]
    fn ease_in_out_alpha_is_symmetric() {
        let first_half = eased_alpha(EasingDirection::EaseInOut, 0.25);
        let second_half = eased_alpha(EasingDirection::EaseInOut, 0.75);
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
    fn resolve_live_follow_sample_uses_default_angles_when_game_is_active() {
        let mut editor = new_editor();
        editor.camera.playing_rotation = 1.2;
        editor.camera.playing_pitch = -0.8;

        let target = Vec3::new(2.0, 3.0, 4.0);
        let sample = live_follow_sample_for_camera(&editor.camera, target, true);
        let expected_eye =
            target + playing_camera_offset_for_angles(DEFAULT_CAMERA_YAW, DEFAULT_CAMERA_PITCH);

        assert_vec3_close(sample.target, target);
        assert_vec3_close(sample.eye, expected_eye);
    }

    #[test]
    fn resolve_live_follow_sample_clamps_pitch_when_not_active() {
        let mut editor = new_editor();
        editor.camera.playing_rotation = 0.35;
        editor.camera.playing_pitch = -5.0;

        let target = Vec3::new(1.0, 0.5, -2.0);
        let sample = live_follow_sample_for_camera(&editor.camera, target, false);
        let expected_eye = target
            + playing_camera_offset_for_angles(editor.camera.playing_rotation, MIN_PLAYING_PITCH);

        assert_vec3_close(sample.target, target);
        assert_vec3_close(sample.eye, expected_eye);
    }

    #[test]
    fn evaluate_camera_track_without_triggers_returns_live_follow() {
        let mut editor = new_editor();
        editor.objects.clear();
        editor.camera.playing_rotation = 0.6;
        editor.camera.playing_pitch = 0.4;

        let target = Vec3::new(2.5, 1.0, -0.5);
        let expected = live_follow_sample_for_camera(&editor.camera, target, false);
        let sample = editor.evaluate_camera_track(3.0, target, false);

        assert_vec3_close(sample.eye, expected.eye);
        assert_vec3_close(sample.target, expected.target);
    }

    #[test]
    fn evaluate_camera_track_interpolates_into_first_trigger_window() {
        let mut editor = new_editor();
        editor.objects = vec![static_trigger(
            5.0,
            [10.0, 1.0, -2.0],
            0.8,
            0.3,
            2.0,
            false,
            EasingDirection::Linear,
        )];

        let live_target = Vec3::new(0.0, 0.0, 0.0);
        let live = live_follow_sample_for_camera(&editor.camera, live_target, false);
        let trigger_target = Vec3::new(10.0, 1.0, -2.0);
        let end = CameraViewSample {
            eye: trigger_target + editor_camera_offset_for_pose(0.8, 0.3),
            target: trigger_target,
        };
        let expected = interpolate_camera_samples(live, end, 0.5);

        let sample = editor.evaluate_camera_track(4.0, live_target, false);
        assert_vec3_close(sample.eye, expected.eye);
        assert_vec3_close(sample.target, expected.target);
    }

    #[test]
    fn evaluate_camera_track_between_follow_segments_uses_follow_orbit() {
        let mut editor = new_editor();
        editor.objects = vec![
            follow_trigger(2.0, 1.0, false),
            follow_trigger(4.0, 1.0, false),
        ];

        let target = Vec3::new(1.0, 2.0, 3.0);
        let expected = CameraViewSample {
            eye: target
                + offset_from_rotation_pitch(
                    CameraTrigger::default().yaw,
                    CameraTrigger::default().pitch,
                    CameraTrigger::default().distance,
                ),
            target,
        };
        let sample = editor.evaluate_camera_track(3.0, target, false);

        assert_vec3_close(sample.eye, expected.eye);
        assert_vec3_close(sample.target, expected.target);
    }

    #[test]
    fn evaluate_camera_track_uses_custom_follow_orbit() {
        let mut editor = new_editor();
        editor.objects = vec![follow_trigger_with_orbit(0.0, 1.0, false, 12.0, 0.25, 0.35)];

        let target = Vec3::new(4.0, 1.0, -2.0);
        let sample = editor.evaluate_camera_track(1.0, target, false);
        let expected_eye = target + offset_from_rotation_pitch(0.25, 0.35, 12.0);

        assert_vec3_close(sample.eye, expected_eye);
        assert_vec3_close(sample.target, target);
    }

    #[test]
    fn evaluate_camera_track_between_static_triggers_handles_pre_and_active_transition() {
        let mut editor = new_editor();
        editor.objects = vec![
            static_trigger(
                2.0,
                [0.0, 0.0, 0.0],
                0.0,
                0.2,
                0.0,
                false,
                EasingDirection::Linear,
            ),
            static_trigger(
                6.0,
                [6.0, 1.0, -3.0],
                1.0,
                0.4,
                1.0,
                false,
                EasingDirection::EaseIn,
            ),
        ];

        let live_target = Vec3::new(3.0, 0.0, 2.0);
        let previous_sample = CameraViewSample {
            eye: Vec3::new(0.0, 0.0, 0.0) + editor_camera_offset_for_pose(0.0, 0.2),
            target: Vec3::new(0.0, 0.0, 0.0),
        };
        let next_sample = CameraViewSample {
            eye: Vec3::new(6.0, 1.0, -3.0) + editor_camera_offset_for_pose(1.0, 0.4),
            target: Vec3::new(6.0, 1.0, -3.0),
        };

        let pre_transition = editor.evaluate_camera_track(4.0, live_target, false);
        assert_vec3_close(pre_transition.eye, previous_sample.eye);
        assert_vec3_close(pre_transition.target, previous_sample.target);

        let active = editor.evaluate_camera_track(5.5, live_target, false);
        let expected = interpolate_camera_samples(
            previous_sample,
            next_sample,
            eased_alpha(EasingDirection::EaseIn, 0.5),
        );
        assert_vec3_close(active.eye, expected.eye);
        assert_vec3_close(active.target, expected.target);
    }

    #[test]
    fn evaluate_camera_track_after_last_trigger_uses_last_sample() {
        let mut editor = new_editor();
        editor.objects = vec![static_trigger(
            1.0,
            [4.0, 2.0, 1.0],
            0.3,
            0.5,
            0.5,
            false,
            EasingDirection::Linear,
        )];

        let sample = editor.evaluate_camera_track(50.0, Vec3::ZERO, false);
        let expected_target = Vec3::new(4.0, 2.0, 1.0);
        let expected_eye = expected_target + editor_camera_offset_for_pose(0.3, 0.5);

        assert_vec3_close(sample.eye, expected_eye);
        assert_vec3_close(sample.target, expected_target);
    }

    #[test]
    fn capture_current_camera_trigger_uses_editor_pose() {
        let mut editor = new_editor();
        editor.camera.editor_pan = [12.0, -7.0];
        editor.camera.editor_target_z = 4.5;
        editor.camera.editor_yaw = 0.9;
        editor.camera.editor_pitch = 0.3;

        let trigger_object = editor.capture_current_camera_trigger(8.0);
        let trigger = trigger_object.trigger.expect("camera trigger payload");
        assert_eq!(trigger.time_seconds, 8.0);
        let camera = trigger.camera.expect("camera payload");
        assert_eq!(camera.mode, CameraMode::Static);
        assert_eq!(camera.target_position, [12.0, 4.5, -7.0]);
        assert_eq!(camera.yaw, 0.9);
        assert_eq!(camera.pitch, 0.3);
        assert_eq!(camera.transition_interval_seconds, 1.0);
        assert!(!camera.use_full_segment_transition);
    }
}
