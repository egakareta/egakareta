/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorSubsystem;
use crate::editor_domain::interpolate_timeline_sample_positions;
use crate::triggers::{
    camera_trigger_eye_from_target, camera_trigger_eye_from_target_with_distance,
    default_camera_trigger_distance, default_camera_trigger_target_position,
    timed_triggers_to_camera_triggers, triggers_from_objects, CameraTrigger, TimedTrigger,
    TimedTriggerAction, TimedTriggerTarget,
};
use crate::types::{
    LevelObject, CAMERA_TRIGGER_BLOCK_ID, DEFAULT_CAMERA_TRIGGER_PITCH,
    DEFAULT_CAMERA_TRIGGER_ROTATION, TRANSFORM_TRIGGER_BLOCK_ID,
};

pub(crate) struct EditorTriggerState {
    pub(crate) selected_index: Option<usize>,
    pub(crate) simulate_trigger_hitboxes: bool,
}

fn trigger_block_id_from_action(action: &TimedTriggerAction) -> &'static str {
    match action {
        TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. } => {
            CAMERA_TRIGGER_BLOCK_ID
        }
        TimedTriggerAction::TransformObjects { .. } => TRANSFORM_TRIGGER_BLOCK_ID,
    }
}

fn camera_trigger_rotation_degrees(rotation: f32, pitch: f32) -> [f32; 3] {
    [pitch.to_degrees(), rotation.to_degrees(), 0.0]
}

fn camera_trigger_object_position_from_eye(eye: [f32; 3], size: [f32; 3]) -> [f32; 3] {
    [
        eye[0] - size[0] * 0.5,
        eye[1] - size[1] * 0.5,
        eye[2] - size[2] * 0.5,
    ]
}

fn trigger_object_from_payload(trigger: TimedTrigger) -> LevelObject {
    let (position, size, rotation_degrees) = match &trigger.action {
        TimedTriggerAction::CameraPose {
            target_position,
            rotation,
            pitch,
            ..
        } => {
            let size = [1.0, 1.0, 1.0];
            let eye = camera_trigger_eye_from_target(*target_position, *rotation, *pitch);
            (
                camera_trigger_object_position_from_eye(eye, size),
                size,
                camera_trigger_rotation_degrees(*rotation, *pitch),
            )
        }
        TimedTriggerAction::TransformObjects {
            position,
            rotation_degrees,
            size,
        } => (*position, *size, *rotation_degrees),
        TimedTriggerAction::CameraFollow {
            distance,
            rotation,
            pitch,
            ..
        } => {
            let size = [1.0, 1.0, 1.0];
            let eye = camera_trigger_eye_from_target_with_distance(
                default_camera_trigger_target_position(),
                *rotation,
                *pitch,
                *distance,
            );
            (
                camera_trigger_object_position_from_eye(eye, size),
                size,
                camera_trigger_rotation_degrees(*rotation, *pitch),
            )
        }
    };

    LevelObject {
        position,
        size,
        rotation_degrees,
        block_id: trigger_block_id_from_action(&trigger.action).to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: Some(trigger),
    }
}

impl EditorTriggerState {
    pub(crate) fn new() -> Self {
        Self {
            selected_index: None,
            simulate_trigger_hitboxes: false,
        }
    }
}

impl EditorSubsystem {
    pub(crate) fn is_camera_track_trigger(trigger: &TimedTrigger) -> bool {
        matches!(trigger.target, TimedTriggerTarget::Camera)
            && matches!(
                trigger.action,
                TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. }
            )
    }

    fn sanitize_trigger(&self, trigger: &mut TimedTrigger) {
        let duration = self.timeline.clock.duration_seconds.max(0.1);

        trigger.time_seconds = if trigger.time_seconds.is_finite() {
            trigger.time_seconds.clamp(0.0, duration)
        } else {
            0.0
        };

        trigger.duration_seconds = if trigger.duration_seconds.is_finite() {
            trigger.duration_seconds.max(0.0)
        } else {
            0.0
        };

        match &mut trigger.target {
            TimedTriggerTarget::Camera => {}
            TimedTriggerTarget::Objects { object_ids } => {
                object_ids.sort_unstable();
                object_ids.dedup();
            }
        }

        match &mut trigger.action {
            TimedTriggerAction::TransformObjects {
                position,
                rotation_degrees,
                size,
            } => {
                *position = position.map(|component| {
                    if component.is_finite() {
                        component
                    } else {
                        0.0
                    }
                });

                *rotation_degrees = rotation_degrees.map(|component| {
                    if component.is_finite() {
                        component
                    } else {
                        0.0
                    }
                });

                *size = size.map(|component| {
                    if component.is_finite() {
                        component.max(0.01)
                    } else {
                        1.0
                    }
                });
            }
            TimedTriggerAction::CameraPose {
                transition_interval_seconds,
                target_position,
                rotation,
                pitch,
                ..
            } => {
                if !transition_interval_seconds.is_finite() {
                    *transition_interval_seconds = 1.0;
                } else {
                    *transition_interval_seconds = transition_interval_seconds.max(0.0);
                }

                *target_position = target_position.map(|component| {
                    if component.is_finite() {
                        component
                    } else {
                        0.0
                    }
                });

                if !rotation.is_finite() {
                    *rotation = DEFAULT_CAMERA_TRIGGER_ROTATION;
                }

                if !pitch.is_finite() {
                    *pitch = DEFAULT_CAMERA_TRIGGER_PITCH;
                } else {
                    *pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
                }
            }
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds,
                distance,
                rotation,
                pitch,
                ..
            } => {
                if !transition_interval_seconds.is_finite() {
                    *transition_interval_seconds = 1.0;
                } else {
                    *transition_interval_seconds = transition_interval_seconds.max(0.0);
                }

                if !distance.is_finite() {
                    *distance = default_camera_trigger_distance();
                } else {
                    *distance = distance.max(0.01);
                }

                if !rotation.is_finite() {
                    *rotation = DEFAULT_CAMERA_TRIGGER_ROTATION;
                }

                if !pitch.is_finite() {
                    *pitch = DEFAULT_CAMERA_TRIGGER_PITCH;
                } else {
                    *pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
                }
            }
        }
    }

    pub(crate) fn sync_camera_follow_trigger_object_pose(
        object: &mut LevelObject,
        trigger: &TimedTrigger,
        target_position: [f32; 3],
    ) {
        let TimedTriggerAction::CameraFollow {
            distance,
            rotation,
            pitch,
            ..
        } = trigger.action
        else {
            return;
        };

        let size = [1.0, 1.0, 1.0];
        let eye = camera_trigger_eye_from_target_with_distance(
            target_position,
            rotation,
            pitch,
            distance,
        );
        object.size = size;
        object.position = camera_trigger_object_position_from_eye(eye, size);
        object.rotation_degrees = camera_trigger_rotation_degrees(rotation, pitch);
    }

    fn timeline_position_for_camera_trigger_marker(&self, time_seconds: f32) -> Option<[f32; 3]> {
        let cache = &self.timeline.snapshot_cache;
        if cache.is_empty()
            || self.timeline.snapshot_cache_revision != self.timeline.simulation_revision
        {
            return None;
        }

        let step_seconds = self.timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
        let max_index = cache.len().saturating_sub(1);
        let sample_position = (time_seconds.max(0.0) / step_seconds).clamp(0.0, max_index as f32);
        let lower_index = sample_position.floor() as usize;
        let upper_index = (lower_index + 1).min(max_index);
        let alpha = (sample_position - lower_index as f32).clamp(0.0, 1.0);
        let lower = &cache[lower_index];
        let upper = &cache[upper_index];
        Some(interpolate_timeline_sample_positions(
            lower.position,
            lower.direction,
            upper.position,
            upper.direction,
            alpha,
        ))
    }

    pub(crate) fn triggers(&self) -> Vec<TimedTrigger> {
        triggers_from_objects(&self.objects)
    }

    pub(crate) fn sync_trigger_selection_from_objects(&mut self) {
        let trigger_count = self.triggers().len();
        self.triggers.selected_index = self
            .triggers
            .selected_index
            .filter(|index| *index < trigger_count);
    }

    pub(crate) fn selected_trigger_index(&self) -> Option<usize> {
        let triggers = self.triggers();
        self.triggers
            .selected_index
            .filter(|index| *index < triggers.len())
    }

    pub(crate) fn trigger_object_index_for_trigger_index(
        &self,
        trigger_index: usize,
    ) -> Option<usize> {
        let mut indexed_triggers = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(object_index, object)| {
                let trigger = triggers_from_objects(std::slice::from_ref(object))
                    .into_iter()
                    .next()?;
                Some((object_index, trigger))
            })
            .collect::<Vec<_>>();

        indexed_triggers
            .sort_by(|left, right| f32::total_cmp(&left.1.time_seconds, &right.1.time_seconds));
        indexed_triggers
            .get(trigger_index)
            .map(|(object_index, _)| *object_index)
    }

    pub(crate) fn camera_trigger_markers(&self) -> Vec<(usize, CameraTrigger)> {
        self.triggers()
            .iter()
            .enumerate()
            .filter_map(|(index, trigger)| {
                if !Self::is_camera_track_trigger(trigger) {
                    return None;
                }

                let mut camera_trigger =
                    timed_triggers_to_camera_triggers(std::slice::from_ref(trigger))
                        .into_iter()
                        .next()?;
                if let Some(target_position) = self
                    .timeline_position_for_camera_trigger_marker(camera_trigger.time_seconds)
                    .filter(|_| {
                        matches!(
                            camera_trigger.mode,
                            crate::triggers::CameraTriggerMode::Follow
                        )
                    })
                {
                    camera_trigger.target_position = target_position;
                }
                Some((index, camera_trigger))
            })
            .collect()
    }

    pub(crate) fn has_object_transform_triggers(&self) -> bool {
        self.triggers().iter().any(|trigger| {
            !matches!(trigger.target, TimedTriggerTarget::Camera)
                && matches!(trigger.action, TimedTriggerAction::TransformObjects { .. })
        })
    }

    pub(crate) fn has_camera_timeline_triggers(&self) -> bool {
        self.triggers().iter().any(Self::is_camera_track_trigger)
    }

    /// Returns `true` when any of the given block indices are referenced as
    /// source objects by at least one transform trigger. Used to decide when
    /// the transform trigger marker overlay must be rebuilt after a block
    /// move/resize/rotate.
    pub(crate) fn any_block_is_transform_trigger_source(&self, indices: &[usize]) -> bool {
        if indices.is_empty() {
            return false;
        }
        self.triggers().iter().any(|trigger| {
            if !matches!(trigger.action, TimedTriggerAction::TransformObjects { .. }) {
                return false;
            }
            let TimedTriggerTarget::Objects { object_ids } = &trigger.target else {
                return false;
            };
            object_ids.iter().any(|id| {
                let Ok(object_index) = usize::try_from(*id) else {
                    return false;
                };
                indices.contains(&object_index)
            })
        })
    }

    pub(crate) fn simulate_trigger_hitboxes(&self) -> bool {
        self.triggers.simulate_trigger_hitboxes
    }

    pub(crate) fn set_simulate_trigger_hitboxes(&mut self, enabled: bool) {
        self.triggers.simulate_trigger_hitboxes = enabled;
    }

    pub(crate) fn set_triggers(&mut self, mut triggers: Vec<TimedTrigger>) {
        // Sanitize
        for trigger in &mut triggers {
            self.sanitize_trigger(trigger);
        }
        triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));

        // Remove existing trigger objects from objects list
        self.objects.retain(|obj| obj.trigger.is_none());

        // Add sanitized trigger objects
        for trigger in triggers {
            self.objects.push(trigger_object_from_payload(trigger));
        }

        self.sync_trigger_selection_from_objects();
    }

    pub(crate) fn add_trigger(&mut self, trigger: TimedTrigger) -> usize {
        let mut trigger = trigger;
        self.sanitize_trigger(&mut trigger);

        let object_index = self.objects.len();
        self.objects
            .push(trigger_object_from_payload(trigger.clone()));

        self.sync_trigger_selection_from_objects();
        self.triggers.selected_index = self.trigger_index_for_object_index(object_index);
        object_index
    }

    pub(crate) fn set_selected_block_trigger(&mut self, trigger: Option<TimedTrigger>) {
        let mut trigger = trigger;
        if let Some(trigger) = &mut trigger {
            self.sanitize_trigger(trigger);
        }

        let Some(index) = self
            .ui
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        else {
            return;
        };

        self.objects[index].trigger = trigger;
        if let Some(trigger) = self.objects[index].trigger.clone() {
            Self::sync_camera_follow_trigger_object_pose(
                &mut self.objects[index],
                &trigger,
                default_camera_trigger_target_position(),
            );
        }
        self.sync_trigger_selection_from_objects();
    }

    pub(crate) fn set_trigger_selected(&mut self, selected: Option<usize>) {
        let trigger_count = self.triggers().len();
        self.triggers.selected_index = selected.filter(|index| *index < trigger_count);
    }

    fn trigger_index_for_object_index(&self, target_object_index: usize) -> Option<usize> {
        let mut indexed_triggers = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(object_index, object)| {
                let trigger = triggers_from_objects(std::slice::from_ref(object))
                    .into_iter()
                    .next()?;
                Some((object_index, trigger))
            })
            .collect::<Vec<_>>();

        indexed_triggers
            .sort_by(|left, right| f32::total_cmp(&left.1.time_seconds, &right.1.time_seconds));
        indexed_triggers
            .iter()
            .position(|(object_index, _)| *object_index == target_object_index)
    }
}

#[cfg(test)]
mod tests {
    use super::EditorSubsystem;
    use crate::state::State;
    use crate::triggers::{
        camera_trigger_eye_from_object, camera_trigger_eye_from_target,
        default_camera_trigger_distance, CameraTriggerMode, TimedTrigger, TimedTriggerAction,
        TimedTriggerEasing, TimedTriggerTarget,
    };
    use crate::types::{DEFAULT_CAMERA_TRIGGER_PITCH, DEFAULT_CAMERA_TRIGGER_ROTATION};

    fn object_move_trigger(time_seconds: f32) -> TimedTrigger {
        TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Objects {
                object_ids: vec![0],
            },
            action: TimedTriggerAction::TransformObjects {
                position: [1.0, 2.0, 3.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
            },
        }
    }

    fn camera_pose_trigger(time_seconds: f32) -> TimedTrigger {
        TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Camera,
            action: TimedTriggerAction::CameraPose {
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [0.0, 0.0, 0.0],
                rotation: 0.0,
                pitch: 0.0,
            },
        }
    }

    async fn new_editor_state() -> State {
        let mut state = State::new_test().await;
        state.enter_editor_phase("Test Level".to_string());
        state
    }

    #[test]
    fn camera_track_trigger_detection_matches_camera_target_and_action() {
        let pose = camera_pose_trigger(0.0);
        let follow = TimedTrigger {
            action: TimedTriggerAction::CameraFollow {
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                distance: default_camera_trigger_distance(),
                rotation: DEFAULT_CAMERA_TRIGGER_ROTATION,
                pitch: DEFAULT_CAMERA_TRIGGER_PITCH,
            },
            ..camera_pose_trigger(1.0)
        };
        let object = object_move_trigger(2.0);

        assert!(EditorSubsystem::is_camera_track_trigger(&pose));
        assert!(EditorSubsystem::is_camera_track_trigger(&follow));
        assert!(!EditorSubsystem::is_camera_track_trigger(&object));
    }

    #[test]
    fn add_trigger_sanitizes_values_and_keeps_items_sorted() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.timeline.clock.duration_seconds = 5.0;

            let index = state.editor.add_trigger(TimedTrigger {
                time_seconds: f32::INFINITY,
                duration_seconds: f32::NAN,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![3, 1, 3, 2],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [f32::NAN, 2.0, f32::INFINITY],
                    rotation_degrees: [f32::NEG_INFINITY, 90.0, f32::NAN],
                    size: [f32::NAN, -9.0, 2.0],
                },
            });
            assert_eq!(index, 0);

            state.editor.add_trigger(camera_pose_trigger(1.0));
            assert_eq!(state.editor.triggers()[0].time_seconds, 0.0);
            assert_eq!(state.editor.triggers()[1].time_seconds, 1.0);

            let first = &state.editor.triggers()[0];
            assert_eq!(first.duration_seconds, 0.0);
            match &first.target {
                TimedTriggerTarget::Objects { object_ids } => {
                    assert_eq!(object_ids, &vec![1, 2, 3]);
                }
                _ => panic!("expected object list target"),
            }
            match first.action {
                TimedTriggerAction::TransformObjects {
                    position,
                    rotation_degrees,
                    size,
                } => {
                    assert_eq!(position, [0.0, 2.0, 0.0]);
                    assert_eq!(rotation_degrees, [0.0, 90.0, 0.0]);
                    assert_eq!(size[0], 1.0);
                    assert_eq!(size[1], 0.01);
                    assert_eq!(size[2], 2.0);
                }
                _ => panic!("expected transform action"),
            }
        });
    }

    #[test]
    fn set_triggers_sanitizes_camera_pose_and_follow_and_bounds_selection() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.triggers.selected_index = Some(4);

            state.editor.set_triggers(vec![
                TimedTrigger {
                    time_seconds: 9.0,
                    duration_seconds: -5.0,
                    easing: TimedTriggerEasing::EaseIn,
                    target: TimedTriggerTarget::Camera,
                    action: TimedTriggerAction::CameraPose {
                        transition_interval_seconds: f32::NAN,
                        use_full_segment_transition: false,
                        target_position: [f32::NAN, 2.0, f32::NEG_INFINITY],
                        rotation: f32::NAN,
                        pitch: 99.0,
                    },
                },
                TimedTrigger {
                    time_seconds: -1.0,
                    duration_seconds: 1.0,
                    easing: TimedTriggerEasing::Linear,
                    target: TimedTriggerTarget::Camera,
                    action: TimedTriggerAction::CameraFollow {
                        transition_interval_seconds: -3.0,
                        use_full_segment_transition: true,
                        distance: f32::NAN,
                        rotation: f32::NAN,
                        pitch: 99.0,
                    },
                },
            ]);

            assert_eq!(state.editor.triggers().len(), 2);
            assert_eq!(state.editor.triggers.selected_index, None);
            assert_eq!(state.editor.triggers()[0].time_seconds, 0.0);
            assert_eq!(state.editor.triggers()[1].time_seconds, 8.0);

            match state.editor.triggers()[0].action {
                TimedTriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    distance,
                    rotation,
                    pitch,
                } => {
                    assert_eq!(transition_interval_seconds, 0.0);
                    assert!(use_full_segment_transition);
                    assert_eq!(distance, default_camera_trigger_distance());
                    assert_eq!(rotation, DEFAULT_CAMERA_TRIGGER_ROTATION);
                    assert!((pitch - 89.9f32.to_radians()).abs() <= 1e-6);
                }
                _ => panic!("expected camera follow"),
            }

            match state.editor.triggers()[1].action {
                TimedTriggerAction::CameraPose {
                    transition_interval_seconds,
                    target_position,
                    rotation,
                    pitch,
                    ..
                } => {
                    assert_eq!(transition_interval_seconds, 1.0);
                    for (actual, expected) in target_position.iter().zip([0.0, 2.0, 0.0]) {
                        assert!((*actual - expected).abs() <= 1e-5);
                    }
                    assert_eq!(rotation, -45.0f32.to_radians());
                    assert!((pitch - 89.9f32.to_radians()).abs() <= 1e-6);
                }
                _ => panic!("expected camera pose"),
            }
        });
    }

    #[test]
    fn set_triggers_centers_camera_trigger_object_on_eye() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            let trigger = camera_pose_trigger(0.0);
            let TimedTriggerAction::CameraPose {
                target_position,
                rotation,
                pitch,
                ..
            } = trigger.action
            else {
                panic!("expected camera pose");
            };
            let expected_eye = camera_trigger_eye_from_target(target_position, rotation, pitch);

            state.editor.set_triggers(vec![trigger]);

            let object = state
                .editor
                .objects
                .iter()
                .find(|object| object.trigger.is_some())
                .expect("expected camera trigger object");
            let actual_eye = camera_trigger_eye_from_object(object);
            for (actual, expected) in actual_eye.iter().zip(expected_eye) {
                assert!((*actual - expected).abs() <= 1e-6);
            }
        });
    }

    #[test]
    fn trigger_selection_and_marker_queries_reflect_filtered_camera_track() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.set_triggers(vec![
                camera_pose_trigger(0.5),
                object_move_trigger(0.75),
                TimedTrigger {
                    action: TimedTriggerAction::CameraFollow {
                        transition_interval_seconds: 1.5,
                        use_full_segment_transition: false,
                        distance: default_camera_trigger_distance(),
                        rotation: DEFAULT_CAMERA_TRIGGER_ROTATION,
                        pitch: DEFAULT_CAMERA_TRIGGER_PITCH,
                    },
                    ..camera_pose_trigger(1.0)
                },
            ]);

            state.editor.set_trigger_selected(Some(99));
            assert_eq!(state.editor.selected_trigger_index(), None);
            state.editor.set_trigger_selected(Some(2));
            assert_eq!(state.editor.selected_trigger_index(), Some(2));

            let markers = state.editor.camera_trigger_markers();
            assert_eq!(markers.len(), 2);
            assert_eq!(markers[0].0, 0);
            assert_eq!(markers[1].0, 2);
            assert_eq!(markers[0].1.mode, CameraTriggerMode::Static);
            assert_eq!(markers[1].1.mode, CameraTriggerMode::Follow);
        });
    }

    #[test]
    fn object_transform_detection_and_hitbox_toggle_cover_simple_state_helpers() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.editor.set_triggers(vec![camera_pose_trigger(0.0)]);
            assert!(!state.editor.has_object_transform_triggers());

            state.editor.add_trigger(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![1],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [0.0, 0.0, 0.0],
                    rotation_degrees: [10.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            });
            assert!(state.editor.has_object_transform_triggers());

            assert!(!state.editor.simulate_trigger_hitboxes());
            state.editor.set_simulate_trigger_hitboxes(true);
            assert!(state.editor.simulate_trigger_hitboxes());
        });
    }

    #[test]
    fn derived_triggers_use_trigger_block_pose_as_source_of_truth() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects.clear();
            state.editor.set_triggers(vec![object_move_trigger(0.0)]);

            assert_eq!(state.editor.triggers().len(), 1);
            state.editor.objects[0].position = [9.0, 8.0, 7.0];
            state.editor.objects[0].size = [2.0, 3.0, 4.0];
            state.editor.objects[0].rotation_degrees = [10.0, 20.0, 30.0];

            let trigger = state.editor.triggers().remove(0);
            let TimedTriggerAction::TransformObjects {
                position,
                rotation_degrees,
                size,
            } = trigger.action
            else {
                panic!("expected transform trigger");
            };
            assert_eq!(position, [9.0, 8.0, 7.0]);
            assert_eq!(rotation_degrees, [10.0, 20.0, 30.0]);
            assert_eq!(size, [2.0, 3.0, 4.0]);
        });
    }
}
