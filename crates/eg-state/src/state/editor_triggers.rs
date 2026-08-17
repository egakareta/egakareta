/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::interpolate_timeline_sample_positions;
use eg_types::{CameraMode, CameraTrigger, LevelObject, TimedTrigger, TriggerTarget};

#[cfg(test)]
use super::EditorSubsystem;

#[derive(Default, Clone)]
pub struct EditorTriggerState {
    pub selected_index: Option<usize>,
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

/// World-space pose (position, rotation) for a camera trigger object placed so its eye
/// sits at the camera trigger's eye, looking at the target. Used when capturing a camera trigger
/// from the current editor camera.
pub(crate) fn camera_trigger_object_pose(camera: &CameraTrigger) -> ([f32; 3], [f32; 3]) {
    let size = [1.0, 1.0, 1.0];
    let eye = camera.eye();
    (
        camera_trigger_object_position_from_eye(eye, size),
        camera_trigger_rotation_degrees(camera.yaw, camera.pitch),
    )
}

pub(crate) fn sync_camera_follow_trigger_object_pose(
    object: &mut LevelObject,
    trigger: &TimedTrigger,
    target_position: [f32; 3],
) {
    let Some(camera) = &trigger.camera else {
        return;
    };
    if camera.mode != CameraMode::Follow {
        return;
    }

    let size = [1.0, 1.0, 1.0];
    let eye = CameraTrigger {
        target_position,
        yaw: camera.yaw,
        pitch: camera.pitch,
        distance: camera.distance,
        ..CameraTrigger::default()
    }
    .eye();
    object.size = size;
    object.position = camera_trigger_object_position_from_eye(eye, size);
    object.rotation = camera_trigger_rotation_degrees(camera.yaw, camera.pitch);
}

super::editor_context_methods! {
    pub fn is_camera_track_trigger(trigger: &TimedTrigger) -> bool {
        matches!(trigger.target, TriggerTarget::Camera) && trigger.camera.is_some()
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

    // TODO: avoid re-scanning self.objects on every call; index trigger objects efficiently.
    pub fn trigger_objects(&self) -> Vec<LevelObject> {
        let mut triggers = self
            .objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .cloned()
            .collect::<Vec<_>>();
        triggers.sort_by(|a, b| {
            f32::total_cmp(
                &a.trigger.as_ref().map(|t| t.time_seconds).unwrap_or(0.0),
                &b.trigger.as_ref().map(|t| t.time_seconds).unwrap_or(0.0),
            )
        });
        triggers
    }

    // TODO: avoid re-scanning self.objects on every call; index trigger objects efficiently.
    pub fn sync_trigger_selection_from_objects(&mut self) {
        let trigger_count = self.trigger_objects().len();
        self.triggers.selected_index = self
            .triggers
            .selected_index
            .filter(|index| *index < trigger_count);
    }

    pub fn selected_trigger_index(&self) -> Option<usize> {
        let trigger_count = self.trigger_objects().len();
        self.triggers
            .selected_index
            .filter(|index| *index < trigger_count)
    }

    #[cfg(test)]
    pub fn trigger_object_index_for_trigger_index(&self, trigger_index: usize) -> Option<usize> {
        let mut indexed: Vec<(usize, f32)> = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(object_index, object)| {
                let trigger = object.trigger.as_ref()?;
                Some((object_index, trigger.time_seconds))
            })
            .collect();
        indexed.sort_by(|left, right| f32::total_cmp(&left.1, &right.1));
        indexed
            .get(trigger_index)
            .map(|(object_index, _)| *object_index)
    }

    pub fn camera_trigger_markers(&self) -> Vec<(usize, CameraTrigger)> {
        self.trigger_objects()
            .iter()
            .enumerate()
            .filter_map(|(index, object)| {
                let trigger = object.trigger.as_ref()?;
                if !Self::is_camera_track_trigger(trigger) {
                    return None;
                }
                let mut camera = trigger.camera.clone()?;
                if let Some(target_position) = self
                    .timeline_position_for_camera_trigger_marker(trigger.time_seconds)
                    .filter(|_| camera.mode == CameraMode::Follow)
                {
                    camera.target_position = target_position;
                }
                Some((index, camera))
            })
            .collect()
    }

    pub fn has_object_transform_triggers(&self) -> bool {
        self.trigger_objects().iter().any(|object| {
            let Some(trigger) = object.trigger.as_ref() else {
                return false;
            };
            !matches!(trigger.target, TriggerTarget::Camera) && trigger.transform
        })
    }

    #[cfg(test)]
    pub fn has_camera_timeline_triggers(&self) -> bool {
        self.trigger_objects()
            .iter()
            .filter_map(|object| object.trigger.as_ref())
            .any(Self::is_camera_track_trigger)
    }

    /// Returns `true` when any of the given block indices are referenced as
    /// source objects by at least one transform trigger. Used to decide when
    /// the transform trigger marker overlay must be rebuilt after a block
    /// move/resize/rotate.
    pub fn any_block_is_transform_trigger_source(&self, indices: &[usize]) -> bool {
        if indices.is_empty() {
            return false;
        }
        self.trigger_objects().iter().any(|object| {
            let Some(trigger) = object.trigger.as_ref() else {
                return false;
            };
            if !trigger.transform {
                return false;
            }
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                return false;
            };
            indices
                .iter()
                .filter_map(|index| self.objects.get(*index))
                .any(|object| object_ids.contains(&object.id))
        })
    }

    #[cfg(test)]
    pub fn set_selected_block_trigger(&mut self, trigger: Option<TimedTrigger>) {
        let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < self.objects.len())
        else {
            return;
        };

        self.objects[index].trigger = trigger;
        if let Some(trigger) = self.objects[index].trigger.clone() {
            sync_camera_follow_trigger_object_pose(
                &mut self.objects[index],
                &trigger,
                CameraTrigger::default().target_position,
            );
        }
        self.sync_trigger_selection_from_objects();
    }

    pub fn set_trigger_selected(&mut self, selected: Option<usize>) {
        let trigger_count = self.trigger_objects().len();
        self.triggers.selected_index = selected.filter(|index| *index < trigger_count);
    }
}

#[cfg(test)]
mod tests {
    use eg_types::{
        CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection, LevelObject,
        TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
    };

    use super::EditorSubsystem;

    fn test_block(x: f32) -> LevelObject {
        LevelObject {
            id: format!("obj_{:06}", x as i32 + 1),
            position: [x, 0.0, 0.0],
            ..LevelObject::default()
        }
    }

    fn object_move_trigger(time_seconds: f32) -> LevelObject {
        LevelObject {
            id: format!("transform_{time_seconds}"),
            position: [1.0, 2.0, 3.0],
            rotation: [10.0, 20.0, 30.0],
            size: [2.0, 3.0, 4.0],
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: vec!["obj_000001".to_string()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    fn camera_pose_trigger(time_seconds: f32) -> LevelObject {
        LevelObject {
            id: format!("camera_pose_{time_seconds}"),
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Static,
                    transition_interval_seconds: 1.0,
                    use_full_segment_transition: false,
                    target_position: [0.0, 0.0, 0.0],
                    yaw: 0.0,
                    pitch: 0.0,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    fn camera_follow_trigger(time_seconds: f32) -> LevelObject {
        LevelObject {
            id: format!("camera_follow_{time_seconds}"),
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Follow,
                    transition_interval_seconds: 1.5,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    fn object_move_timed_trigger(time_seconds: f32) -> TimedTrigger {
        TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: EasingDirection::Linear,
            target: TriggerTarget::Objects {
                object_ids: vec!["obj_000001".to_string()],
            },
            transform: true,
            camera: None,
        }
    }

    #[test]
    fn camera_track_trigger_detection_matches_camera_target_and_action() {
        let pose = camera_pose_trigger(0.0).trigger.unwrap();
        let follow = camera_follow_trigger(1.0).trigger.unwrap();
        let object = object_move_timed_trigger(2.0);

        assert!(EditorSubsystem::is_camera_track_trigger(&pose));
        assert!(EditorSubsystem::is_camera_track_trigger(&follow));
        assert!(!EditorSubsystem::is_camera_track_trigger(&object));
    }

    #[test]
    fn trigger_objects_are_sorted_trigger_objects() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![
            test_block(99.0),
            camera_pose_trigger(2.0),
            object_move_trigger(0.5),
            camera_follow_trigger(1.0),
        ];

        let triggers = editor.trigger_objects();
        assert_eq!(triggers.len(), 3);
        assert_eq!(triggers[0].id, "transform_0.5");
        assert_eq!(triggers[1].id, "camera_follow_1");
        assert_eq!(triggers[2].id, "camera_pose_2");
    }

    #[test]
    fn trigger_selection_and_marker_queries_reflect_filtered_camera_track() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![
            camera_pose_trigger(0.5),
            object_move_trigger(0.75),
            camera_follow_trigger(1.0),
        ];

        editor.set_trigger_selected(Some(99));
        assert_eq!(editor.selected_trigger_index(), None);
        editor.set_trigger_selected(Some(2));
        assert_eq!(editor.selected_trigger_index(), Some(2));

        let markers = editor.camera_trigger_markers();
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].0, 0);
        assert_eq!(markers[1].0, 2);
        assert_eq!(markers[0].1.mode, CameraMode::Static);
        assert_eq!(markers[1].1.mode, CameraMode::Follow);
    }

    #[test]
    fn object_transform_detection_and_hitbox_toggle_cover_simple_state_helpers() {
        let mut editor = EditorSubsystem::default();

        editor.objects = vec![camera_pose_trigger(0.0)];
        assert!(!editor.has_object_transform_triggers());

        editor.objects.push(object_move_trigger(1.0));
        assert!(editor.has_object_transform_triggers());
    }

    #[test]
    fn derived_triggers_use_trigger_block_pose_as_source_of_truth() {
        let mut editor = EditorSubsystem::default();
        editor.objects = vec![object_move_trigger(0.0)];

        editor.objects[0].position = [9.0, 8.0, 7.0];
        editor.objects[0].size = [2.0, 3.0, 4.0];
        editor.objects[0].rotation = [10.0, 20.0, 30.0];

        let trigger_object = editor.trigger_objects().remove(0);
        assert_eq!(trigger_object.position, [9.0, 8.0, 7.0]);
        assert_eq!(trigger_object.rotation, [10.0, 20.0, 30.0]);
        assert_eq!(trigger_object.size, [2.0, 3.0, 4.0]);
    }

    #[test]
    fn trigger_queries_skip_non_trigger_objects_after_index_sync() {
        let mut editor = EditorSubsystem::default();
        editor.objects = (0..10_000).map(|index| test_block(index as f32)).collect();
        editor.objects.push(camera_pose_trigger(1.0));
        editor.objects.push(test_block(10_001.0));

        assert_eq!(editor.trigger_objects().len(), 1);
        assert!(editor.has_camera_timeline_triggers());

        editor.sync_objects();

        assert_eq!(editor.trigger_objects().len(), 1);
        assert!(editor.has_camera_timeline_triggers());
    }
}
