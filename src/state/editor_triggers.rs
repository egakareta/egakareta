/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorSubsystem;
use crate::triggers::{
    camera_trigger_eye_from_target, default_camera_trigger_target_position,
    timed_triggers_to_camera_triggers, triggers_from_objects, CameraTrigger, TimedTrigger,
    TimedTriggerAction, TimedTriggerTarget,
};
use crate::types::{
    LevelObject, CAMERA_TRIGGER_BLOCK_ID, DEFAULT_CAMERA_TRIGGER_PITCH,
    DEFAULT_CAMERA_TRIGGER_ROTATION, TRANSFORM_TRIGGER_BLOCK_ID,
};

pub(crate) struct EditorTriggerState {
    pub(crate) items: Vec<TimedTrigger>,
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

fn trigger_object_from_payload(trigger: TimedTrigger) -> LevelObject {
    let (position, size, rotation_degrees) = match &trigger.action {
        TimedTriggerAction::CameraPose {
            target_position,
            rotation,
            pitch,
            ..
        } => (
            camera_trigger_eye_from_target(*target_position, *rotation, *pitch),
            [1.0, 1.0, 1.0],
            camera_trigger_rotation_degrees(*rotation, *pitch),
        ),
        TimedTriggerAction::TransformObjects {
            position,
            rotation_degrees,
            size,
        } => (*position, *size, *rotation_degrees),
        TimedTriggerAction::CameraFollow { .. } => (
            camera_trigger_eye_from_target(
                default_camera_trigger_target_position(),
                DEFAULT_CAMERA_TRIGGER_ROTATION,
                DEFAULT_CAMERA_TRIGGER_PITCH,
            ),
            [1.0, 1.0, 1.0],
            camera_trigger_rotation_degrees(
                DEFAULT_CAMERA_TRIGGER_ROTATION,
                DEFAULT_CAMERA_TRIGGER_PITCH,
            ),
        ),
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
            items: Vec::new(),
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
                ..
            } => {
                if !transition_interval_seconds.is_finite() {
                    *transition_interval_seconds = 1.0;
                } else {
                    *transition_interval_seconds = transition_interval_seconds.max(0.0);
                }
            }
        }
    }

    pub(crate) fn triggers(&self) -> Vec<TimedTrigger> {
        triggers_from_objects(&self.objects)
    }

    pub(crate) fn selected_trigger_index(&self) -> Option<usize> {
        let triggers = self.triggers();
        self.triggers
            .selected_index
            .filter(|index| *index < triggers.len())
    }

    pub(crate) fn camera_trigger_markers(&self) -> Vec<(usize, CameraTrigger)> {
        self.triggers()
            .iter()
            .enumerate()
            .filter_map(|(index, trigger)| {
                if !Self::is_camera_track_trigger(trigger) {
                    return None;
                }

                let camera_trigger =
                    timed_triggers_to_camera_triggers(std::slice::from_ref(trigger))
                        .into_iter()
                        .next()?;
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

    /// Returns `true` when any of the given block indices are referenced as
    /// source objects by at least one transform trigger. Used to decide when
    /// the transform trigger marker overlay must be rebuilt after a block
    /// move/resize/rotate.
    pub(crate) fn any_block_is_transform_trigger_source(&self, indices: &[usize]) -> bool {
        if indices.is_empty() {
            return false;
        }
        self.triggers.items.iter().any(|trigger| {
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

        // Sync cache for selected_index tracking
        self.triggers.items = self.triggers();
        self.triggers.selected_index = self
            .triggers
            .selected_index
            .filter(|index| *index < self.triggers.items.len());
    }

    pub(crate) fn add_trigger(&mut self, trigger: TimedTrigger) -> usize {
        let mut trigger = trigger;
        self.sanitize_trigger(&mut trigger);

        let object_index = self.objects.len();
        self.objects
            .push(trigger_object_from_payload(trigger.clone()));

        // Sync cache for selected_index tracking
        self.triggers.items = self.triggers();
        let cache_index = self
            .triggers
            .items
            .iter()
            .position(|t| t.time_seconds == trigger.time_seconds)
            .unwrap_or(0);
        self.triggers.selected_index = Some(cache_index);
        object_index
    }

    pub(crate) fn set_trigger_selected(&mut self, selected: Option<usize>) {
        self.triggers.selected_index = selected.filter(|index| *index < self.triggers.items.len());
    }
}

#[cfg(test)]
mod tests {
    use super::EditorSubsystem;
    use crate::state::State;
    use crate::triggers::{
        CameraTriggerMode, TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
    };

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
                } => {
                    assert_eq!(transition_interval_seconds, 0.0);
                    assert!(use_full_segment_transition);
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
                    assert_eq!(target_position, [0.0, 2.0, 0.0]);
                    assert_eq!(rotation, -45.0f32.to_radians());
                    assert!((pitch - 89.9f32.to_radians()).abs() <= 1e-6);
                }
                _ => panic!("expected camera pose"),
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
}
