/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

use crate::types::{LevelObject, DEFAULT_CAMERA_TRIGGER_PITCH, DEFAULT_CAMERA_TRIGGER_ROTATION};

pub(crate) const CAMERA_TRIGGER_VIEW_DISTANCE: f32 = 24.0;

pub(crate) fn camera_trigger_offset(rotation: f32, pitch: f32) -> [f32; 3] {
    let horizontal_distance = CAMERA_TRIGGER_VIEW_DISTANCE * pitch.cos();
    [
        -rotation.sin() * horizontal_distance,
        pitch.sin() * CAMERA_TRIGGER_VIEW_DISTANCE,
        -rotation.cos() * horizontal_distance,
    ]
}

pub(crate) fn camera_trigger_eye_from_target(
    target_position: [f32; 3],
    rotation: f32,
    pitch: f32,
) -> [f32; 3] {
    let offset = camera_trigger_offset(rotation, pitch);
    [
        target_position[0] + offset[0],
        target_position[1] + offset[1],
        target_position[2] + offset[2],
    ]
}

pub(crate) fn camera_trigger_eye_from_object(object: &LevelObject) -> [f32; 3] {
    [
        object.position[0] + object.size[0] * 0.5,
        object.position[1] + object.size[1] * 0.5,
        object.position[2] + object.size[2] * 0.5,
    ]
}

pub(crate) fn camera_trigger_target_from_eye(
    eye_position: [f32; 3],
    rotation: f32,
    pitch: f32,
) -> [f32; 3] {
    let offset = camera_trigger_offset(rotation, pitch);
    [
        eye_position[0] - offset[0],
        eye_position[1] - offset[1],
        eye_position[2] - offset[2],
    ]
}

pub(crate) fn camera_trigger_forward(rotation: f32, pitch: f32) -> [f32; 3] {
    let offset = camera_trigger_offset(rotation, pitch);
    [
        -offset[0] / CAMERA_TRIGGER_VIEW_DISTANCE,
        -offset[1] / CAMERA_TRIGGER_VIEW_DISTANCE,
        -offset[2] / CAMERA_TRIGGER_VIEW_DISTANCE,
    ]
}

pub(crate) fn camera_trigger_forward_from_rotation_degrees(rotation_degrees: [f32; 3]) -> [f32; 3] {
    let (rotation, pitch) = camera_trigger_rotation_pitch_from_rotation_degrees(rotation_degrees);
    camera_trigger_forward(rotation, pitch)
}

pub(crate) fn camera_trigger_rotation_pitch_from_rotation_degrees(
    rotation_degrees: [f32; 3],
) -> (f32, f32) {
    let mut pitch_degrees = normalize_degrees(rotation_degrees[0]);
    let mut rotation_degrees_y = normalize_degrees(rotation_degrees[1]);
    if pitch_degrees > 90.0 {
        pitch_degrees = 180.0 - pitch_degrees;
        rotation_degrees_y += 180.0;
    } else if pitch_degrees < -90.0 {
        pitch_degrees += 180.0;
        rotation_degrees_y += 180.0;
    }
    (
        normalize_degrees(rotation_degrees_y).to_radians(),
        pitch_degrees.to_radians(),
    )
}

fn normalize_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

pub(crate) fn default_camera_trigger_target_position() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

pub(crate) fn is_default_camera_trigger_target_position(value: &[f32; 3]) -> bool {
    value
        .iter()
        .zip(default_camera_trigger_target_position())
        .all(|(component, default)| (*component - default).abs() <= 1e-6)
}

pub(crate) fn default_camera_trigger_rotation() -> f32 {
    DEFAULT_CAMERA_TRIGGER_ROTATION
}

pub(crate) fn is_default_camera_trigger_rotation(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_TRIGGER_ROTATION).abs() <= 1e-6
}

pub(crate) fn default_camera_trigger_pitch() -> f32 {
    DEFAULT_CAMERA_TRIGGER_PITCH
}

pub(crate) fn is_default_camera_trigger_pitch(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_TRIGGER_PITCH).abs() <= 1e-6
}

pub(crate) fn default_camera_trigger_transition_interval_seconds() -> f32 {
    1.0
}

pub(crate) fn is_default_camera_trigger_transition_interval_seconds(value: &f32) -> bool {
    (*value - 1.0).abs() <= 1e-6
}

pub(crate) fn default_camera_trigger_use_full_segment_transition() -> bool {
    false
}

pub(crate) fn is_default_camera_trigger_use_full_segment_transition(value: &bool) -> bool {
    !*value
}

pub(crate) fn default_timed_trigger_duration_seconds() -> f32 {
    0.0
}

pub(crate) fn is_default_timed_trigger_duration_seconds(value: &f32) -> bool {
    value.abs() <= 1e-6
}

pub(crate) fn default_timed_trigger_target() -> TimedTriggerTarget {
    TimedTriggerTarget::Camera
}

pub(crate) fn is_default_timed_trigger_target(value: &TimedTriggerTarget) -> bool {
    matches!(value, TimedTriggerTarget::Camera)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CameraTriggerMode {
    Follow,
    #[default]
    Static,
}

fn is_default_camera_trigger_mode(value: &CameraTriggerMode) -> bool {
    matches!(value, CameraTriggerMode::Static)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimedTriggerEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

fn is_default_camera_trigger_easing(value: &TimedTriggerEasing) -> bool {
    matches!(value, TimedTriggerEasing::Linear)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct CameraTrigger {
    pub(crate) time_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_mode")]
    pub(crate) mode: CameraTriggerMode,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub(crate) easing: TimedTriggerEasing,
    #[serde(
        default = "default_camera_trigger_transition_interval_seconds",
        skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
    )]
    pub(crate) transition_interval_seconds: f32,
    #[serde(
        default = "default_camera_trigger_use_full_segment_transition",
        skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
    )]
    pub(crate) use_full_segment_transition: bool,
    #[serde(
        default = "default_camera_trigger_target_position",
        skip_serializing_if = "is_default_camera_trigger_target_position"
    )]
    pub(crate) target_position: [f32; 3],
    #[serde(
        default = "default_camera_trigger_rotation",
        skip_serializing_if = "is_default_camera_trigger_rotation"
    )]
    pub(crate) rotation: f32,
    #[serde(
        default = "default_camera_trigger_pitch",
        skip_serializing_if = "is_default_camera_trigger_pitch"
    )]
    pub(crate) pitch: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TimedTriggerTarget {
    Camera,
    Objects { object_ids: Vec<u32> },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TimedTriggerAction {
    TransformObjects {
        position: [f32; 3],
        rotation_degrees: [f32; 3],
        size: [f32; 3],
    },
    CameraPose {
        #[serde(
            default = "default_camera_trigger_transition_interval_seconds",
            skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
        )]
        transition_interval_seconds: f32,
        #[serde(
            default = "default_camera_trigger_use_full_segment_transition",
            skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
        )]
        use_full_segment_transition: bool,
        #[serde(
            default = "default_camera_trigger_target_position",
            skip_serializing_if = "is_default_camera_trigger_target_position"
        )]
        target_position: [f32; 3],
        #[serde(
            default = "default_camera_trigger_rotation",
            skip_serializing_if = "is_default_camera_trigger_rotation"
        )]
        rotation: f32,
        #[serde(
            default = "default_camera_trigger_pitch",
            skip_serializing_if = "is_default_camera_trigger_pitch"
        )]
        pitch: f32,
    },
    CameraFollow {
        #[serde(
            default = "default_camera_trigger_transition_interval_seconds",
            skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
        )]
        transition_interval_seconds: f32,
        #[serde(
            default = "default_camera_trigger_use_full_segment_transition",
            skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
        )]
        use_full_segment_transition: bool,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct TimedTrigger {
    pub(crate) time_seconds: f32,
    #[serde(
        default = "default_timed_trigger_duration_seconds",
        skip_serializing_if = "is_default_timed_trigger_duration_seconds"
    )]
    pub(crate) duration_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub(crate) easing: TimedTriggerEasing,
    #[serde(
        default = "default_timed_trigger_target",
        skip_serializing_if = "is_default_timed_trigger_target"
    )]
    pub(crate) target: TimedTriggerTarget,
    pub(crate) action: TimedTriggerAction,
}

#[cfg(test)]
pub(crate) fn camera_triggers_to_timed_triggers(
    camera_triggers: &[CameraTrigger],
) -> Vec<TimedTrigger> {
    let mut triggers = Vec::with_capacity(camera_triggers.len());
    for camera_trigger in camera_triggers {
        let action = match camera_trigger.mode {
            CameraTriggerMode::Follow => TimedTriggerAction::CameraFollow {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
            },
            CameraTriggerMode::Static => TimedTriggerAction::CameraPose {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
                target_position: camera_trigger.target_position,
                rotation: camera_trigger.rotation,
                pitch: camera_trigger.pitch,
            },
        };

        triggers.push(TimedTrigger {
            time_seconds: camera_trigger.time_seconds,
            duration_seconds: 0.0,
            easing: camera_trigger.easing,
            target: TimedTriggerTarget::Camera,
            action,
        });
    }

    triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    triggers
}

pub(crate) fn timed_triggers_to_camera_triggers(triggers: &[TimedTrigger]) -> Vec<CameraTrigger> {
    let mut camera_triggers = Vec::new();

    for trigger in triggers {
        if !matches!(trigger.target, TimedTriggerTarget::Camera) {
            continue;
        }

        match trigger.action {
            TimedTriggerAction::CameraPose {
                transition_interval_seconds,
                use_full_segment_transition,
                target_position,
                rotation,
                pitch,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraTriggerMode::Static,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position,
                    rotation,
                    pitch,
                });
            }
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds,
                use_full_segment_transition,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraTriggerMode::Follow,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: default_camera_trigger_target_position(),
                    rotation: default_camera_trigger_rotation(),
                    pitch: default_camera_trigger_pitch(),
                });
            }
            TimedTriggerAction::TransformObjects { .. } => {}
        }
    }

    camera_triggers.retain(|trigger| trigger.time_seconds.is_finite());
    camera_triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    camera_triggers
}

fn timed_trigger_eased_alpha(easing: TimedTriggerEasing, alpha: f32) -> f32 {
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

fn timed_trigger_progress(trigger: &TimedTrigger, time_seconds: f32) -> Option<f32> {
    let time_seconds = time_seconds.max(0.0);
    if !trigger.time_seconds.is_finite() {
        return None;
    }

    if trigger.duration_seconds <= 1e-6 {
        return (time_seconds + 1e-6 >= trigger.time_seconds).then_some(1.0);
    }

    let start = trigger.time_seconds;
    let end = start + trigger.duration_seconds.max(0.0);
    if time_seconds + 1e-6 < start {
        return None;
    }

    if time_seconds >= end {
        return Some(1.0);
    }

    let alpha = (time_seconds - start) / trigger.duration_seconds.max(1e-6);
    Some(timed_trigger_eased_alpha(trigger.easing, alpha))
}

pub(crate) fn apply_timed_triggers_to_objects(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    let mut objects = base_objects.to_vec();
    if objects.is_empty() || triggers.is_empty() {
        return objects;
    }

    for trigger in triggers {
        if !trigger.time_seconds.is_finite() {
            continue;
        }

        let Some(progress) = timed_trigger_progress(trigger, time_seconds) else {
            continue;
        };
        let object_count = objects.len();

        match &trigger.target {
            TimedTriggerTarget::Camera => {}
            TimedTriggerTarget::Objects { object_ids } => {
                let mut seen: Vec<u32> = Vec::new();
                for &object_id in object_ids {
                    if object_id as usize >= object_count {
                        continue;
                    }
                    if seen.contains(&object_id) {
                        continue;
                    }
                    seen.push(object_id);
                    apply_trigger_action_to_object(
                        &mut objects[object_id as usize],
                        &trigger.action,
                        progress,
                    );
                }
            }
        }
    }

    objects
}

fn apply_trigger_action_to_object(
    object: &mut LevelObject,
    action: &TimedTriggerAction,
    progress: f32,
) {
    match action {
        TimedTriggerAction::TransformObjects {
            position,
            rotation_degrees,
            size,
        } => {
            for (current, target) in object.position.iter_mut().zip(position.iter()) {
                *current += (*target - *current) * progress;
            }
            for (current, target) in object
                .rotation_degrees
                .iter_mut()
                .zip(rotation_degrees.iter())
            {
                *current += (*target - *current) * progress;
            }
            for (current, target) in object.size.iter_mut().zip(size.iter()) {
                let current_value = (*current).max(0.01);
                let target_value = (*target).max(0.01);
                *current = current_value + (target_value - current_value) * progress;
            }
        }
        TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. } => {}
    }
}

pub(crate) fn triggers_from_objects(objects: &[LevelObject]) -> Vec<TimedTrigger> {
    let mut triggers = objects
        .iter()
        .filter_map(|object| {
            let mut trigger = object.trigger.clone()?;
            trigger.action = match trigger.action {
                TimedTriggerAction::TransformObjects {
                    position: _,
                    rotation_degrees: _,
                    size: _,
                } => TimedTriggerAction::TransformObjects {
                    position: object.position,
                    rotation_degrees: object.rotation_degrees,
                    size: object.size,
                },
                TimedTriggerAction::CameraPose {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: _,
                    rotation: _,
                    pitch: _,
                } => {
                    let (rotation, pitch) = camera_trigger_rotation_pitch_from_rotation_degrees(
                        object.rotation_degrees,
                    );
                    TimedTriggerAction::CameraPose {
                        transition_interval_seconds,
                        use_full_segment_transition,
                        target_position: camera_trigger_target_from_eye(
                            camera_trigger_eye_from_object(object),
                            rotation,
                            pitch,
                        ),
                        rotation,
                        pitch,
                    }
                }
                TimedTriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                } => TimedTriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                },
            };
            Some(trigger)
        })
        .filter(|trigger| trigger.time_seconds.is_finite())
        .collect::<Vec<_>>();
    triggers.sort_by(|left, right| f32::total_cmp(&left.time_seconds, &right.time_seconds));
    triggers
}
