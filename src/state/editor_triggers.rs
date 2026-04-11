/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorSubsystem;
use crate::types::{
    timed_triggers_to_camera_triggers, CameraTrigger, TimedTrigger, TimedTriggerAction,
    TimedTriggerTarget,
};

pub(crate) struct EditorTriggerState {
    pub(crate) items: Vec<TimedTrigger>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) simulate_trigger_hitboxes: bool,
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
            TimedTriggerTarget::Object { .. } => {}
            TimedTriggerTarget::Objects { object_ids } => {
                object_ids.sort_unstable();
                object_ids.dedup();
            }
        }

        match &mut trigger.action {
            TimedTriggerAction::MoveTo { position } => {
                *position = position.map(|component| {
                    if component.is_finite() {
                        component
                    } else {
                        0.0
                    }
                });
            }
            TimedTriggerAction::RotateTo { rotation_degrees } => {
                *rotation_degrees = rotation_degrees.map(|component| {
                    if component.is_finite() {
                        component
                    } else {
                        0.0
                    }
                });
            }
            TimedTriggerAction::ScaleTo { size } => {
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
                    *rotation = -45.0f32.to_radians();
                }

                if !pitch.is_finite() {
                    *pitch = 45.0f32.to_radians();
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

    fn insert_trigger_sorted(&mut self, mut trigger: TimedTrigger) -> usize {
        self.sanitize_trigger(&mut trigger);
        let insert_index = self
            .triggers
            .items
            .partition_point(|existing| existing.time_seconds < trigger.time_seconds);
        self.triggers.items.insert(insert_index, trigger);
        self.triggers.selected_index = Some(insert_index);
        insert_index
    }

    pub(crate) fn triggers(&self) -> &[TimedTrigger] {
        &self.triggers.items
    }

    pub(crate) fn selected_trigger_index(&self) -> Option<usize> {
        self.triggers
            .selected_index
            .filter(|index| *index < self.triggers.items.len())
    }

    pub(crate) fn camera_trigger_markers(&self) -> Vec<(usize, CameraTrigger)> {
        self.triggers
            .items
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
        self.triggers.items.iter().any(|trigger| {
            !matches!(trigger.target, TimedTriggerTarget::Camera)
                && matches!(
                    trigger.action,
                    TimedTriggerAction::MoveTo { .. }
                        | TimedTriggerAction::RotateTo { .. }
                        | TimedTriggerAction::ScaleTo { .. }
                )
        })
    }

    pub(crate) fn simulate_trigger_hitboxes(&self) -> bool {
        self.triggers.simulate_trigger_hitboxes
    }

    pub(crate) fn set_simulate_trigger_hitboxes(&mut self, enabled: bool) {
        self.triggers.simulate_trigger_hitboxes = enabled;
    }

    pub(crate) fn set_triggers(&mut self, mut triggers: Vec<TimedTrigger>) {
        for trigger in &mut triggers {
            self.sanitize_trigger(trigger);
        }
        triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
        self.triggers.items = triggers;
        self.triggers.selected_index = self
            .triggers
            .selected_index
            .filter(|index| *index < self.triggers.items.len());
    }

    pub(crate) fn add_trigger(&mut self, trigger: TimedTrigger) -> usize {
        self.insert_trigger_sorted(trigger)
    }

    pub(crate) fn remove_trigger(&mut self, index: usize) -> bool {
        if index >= self.triggers.items.len() {
            return false;
        }

        self.triggers.items.remove(index);
        self.triggers.selected_index = if self.triggers.items.is_empty() {
            None
        } else {
            Some(index.min(self.triggers.items.len() - 1))
        };
        true
    }

    pub(crate) fn update_trigger(
        &mut self,
        index: usize,
        mut trigger: TimedTrigger,
    ) -> Option<usize> {
        if index >= self.triggers.items.len() {
            return None;
        }

        self.sanitize_trigger(&mut trigger);
        self.triggers.items.remove(index);
        let insert_index = self
            .triggers
            .items
            .partition_point(|existing| existing.time_seconds <= trigger.time_seconds);
        self.triggers.items.insert(insert_index, trigger);
        self.triggers.selected_index = Some(insert_index);
        Some(insert_index)
    }

    pub(crate) fn set_trigger_selected(&mut self, selected: Option<usize>) {
        self.triggers.selected_index = selected.filter(|index| *index < self.triggers.items.len());
    }
}
