/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::interpolate_timeline_sample_positions;
use eg_types::{CameraMode, CameraTrigger, LevelObject, TimedTrigger, TriggerTarget};

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

/// Whether a timed trigger drives the editor camera track.
pub(crate) fn is_camera_track_trigger(trigger: &TimedTrigger) -> bool {
    matches!(trigger.target, TriggerTarget::Camera) && trigger.camera.is_some()
}

/// Return authored trigger objects ordered by their timeline time.
pub(crate) fn trigger_objects_sorted(objects: &[LevelObject]) -> Vec<LevelObject> {
    let mut triggers = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .cloned()
        .collect::<Vec<_>>();
    triggers.sort_by(|a, b| {
        f32::total_cmp(
            &a.trigger
                .as_ref()
                .map(|trigger| trigger.time_seconds)
                .unwrap_or(0.0),
            &b.trigger
                .as_ref()
                .map(|trigger| trigger.time_seconds)
                .unwrap_or(0.0),
        )
    });
    triggers
}

/// Whether authored objects contain an object-transform trigger.
pub(crate) fn has_object_transform_triggers(objects: &[LevelObject]) -> bool {
    trigger_objects_sorted(objects).iter().any(|object| {
        let Some(trigger) = object.trigger.as_ref() else {
            return false;
        };
        !matches!(trigger.target, TriggerTarget::Camera) && trigger.transform
    })
}

/// Return ordered camera-track marker data with indices relative to the full
/// ordered trigger list.
pub(crate) fn camera_track_markers(objects: &[LevelObject]) -> Vec<(usize, CameraTrigger, f32)> {
    trigger_objects_sorted(objects)
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let trigger = object.trigger.as_ref()?;
            if !is_camera_track_trigger(trigger) {
                return None;
            }
            Some((index, trigger.camera.clone()?, trigger.time_seconds))
        })
        .collect()
}

/// Whether authored objects contain a camera-track trigger.
#[cfg(test)]
pub(crate) fn has_camera_timeline_triggers(objects: &[LevelObject]) -> bool {
    trigger_objects_sorted(objects)
        .iter()
        .filter_map(|object| object.trigger.as_ref())
        .any(is_camera_track_trigger)
}

super::editor_context_methods! {
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
        trigger_objects_sorted(&self.objects)
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
        camera_track_markers(&self.objects)
            .into_iter()
            .map(|(index, mut camera, time_seconds)| {
                if let Some(target_position) = self
                    .timeline_position_for_camera_trigger_marker(time_seconds)
                    .filter(|_| camera.mode == CameraMode::Follow)
                {
                    camera.target_position = target_position;
                }
                (index, camera)
            })
            .collect()
    }

    pub fn has_object_transform_triggers(&self) -> bool {
        has_object_transform_triggers(&self.objects)
    }

    #[cfg(test)]
    pub fn has_camera_timeline_triggers(&self) -> bool {
        has_camera_timeline_triggers(&self.objects)
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

    use super::{
        camera_track_markers, has_camera_timeline_triggers, has_object_transform_triggers,
        is_camera_track_trigger, trigger_objects_sorted,
    };

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

        assert!(is_camera_track_trigger(&pose));
        assert!(is_camera_track_trigger(&follow));
        assert!(!is_camera_track_trigger(&object));
    }

    #[test]
    fn trigger_objects_are_sorted_trigger_objects() {
        let objects = vec![
            test_block(99.0),
            camera_pose_trigger(2.0),
            object_move_trigger(0.5),
            camera_follow_trigger(1.0),
        ];

        let triggers = trigger_objects_sorted(&objects);
        assert_eq!(triggers.len(), 3);
        assert_eq!(triggers[0].id, "transform_0.5");
        assert_eq!(triggers[1].id, "camera_follow_1");
        assert_eq!(triggers[2].id, "camera_pose_2");
    }

    #[test]
    fn trigger_selection_and_marker_queries_reflect_filtered_camera_track() {
        let objects = vec![
            camera_pose_trigger(0.5),
            object_move_trigger(0.75),
            camera_follow_trigger(1.0),
        ];

        let markers = camera_track_markers(&objects);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].0, 0);
        assert_eq!(markers[1].0, 2);
        assert_eq!(markers[0].1.mode, CameraMode::Static);
        assert_eq!(markers[1].1.mode, CameraMode::Follow);
    }

    #[test]
    fn object_transform_detection_and_hitbox_toggle_cover_simple_state_helpers() {
        let mut objects = vec![camera_pose_trigger(0.0)];
        assert!(!has_object_transform_triggers(&objects));

        objects.push(object_move_trigger(1.0));
        assert!(has_object_transform_triggers(&objects));
    }

    #[test]
    fn derived_triggers_use_trigger_block_pose_as_source_of_truth() {
        let mut objects = vec![object_move_trigger(0.0)];

        objects[0].position = [9.0, 8.0, 7.0];
        objects[0].size = [2.0, 3.0, 4.0];
        objects[0].rotation = [10.0, 20.0, 30.0];

        let trigger_object = trigger_objects_sorted(&objects).remove(0);
        assert_eq!(trigger_object.position, [9.0, 8.0, 7.0]);
        assert_eq!(trigger_object.rotation, [10.0, 20.0, 30.0]);
        assert_eq!(trigger_object.size, [2.0, 3.0, 4.0]);
    }

    #[test]
    fn trigger_queries_skip_non_trigger_objects_after_index_sync() {
        let mut objects = (0..10_000)
            .map(|index| test_block(index as f32))
            .collect::<Vec<_>>();
        objects.push(camera_pose_trigger(1.0));
        objects.push(test_block(10_001.0));

        assert_eq!(trigger_objects_sorted(&objects).len(), 1);
        assert!(has_camera_timeline_triggers(&objects));
    }
}
