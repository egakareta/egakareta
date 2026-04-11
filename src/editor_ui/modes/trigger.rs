/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::editor_ui::modes::shared::{show_mode_and_snap_controls, show_player_camera_status_row};
use crate::state::EditorUiViewModel;
use crate::types::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};

fn make_trigger(
    view: &EditorUiViewModel<'_>,
    target: TimedTriggerTarget,
    action: TimedTriggerAction,
) -> TimedTrigger {
    TimedTrigger {
        time_seconds: view.timeline_time_seconds,
        duration_seconds: 0.0,
        easing: TimedTriggerEasing::Linear,
        target,
        action,
    }
}

fn selected_object_id_or_default(_view: &EditorUiViewModel<'_>) -> u32 {
    0
}

fn selected_position_or_default(view: &EditorUiViewModel<'_>) -> [f32; 3] {
    view.selected_block
        .as_ref()
        .map(|block| block.position)
        .unwrap_or([0.0, 0.0, 0.0])
}

fn selected_rotation_or_default(view: &EditorUiViewModel<'_>) -> [f32; 3] {
    view.selected_block
        .as_ref()
        .map(|block| block.rotation_degrees)
        .unwrap_or([0.0, 0.0, 0.0])
}

fn selected_size_or_default(view: &EditorUiViewModel<'_>) -> [f32; 3] {
    view.selected_block
        .as_ref()
        .map(|block| block.size)
        .unwrap_or([1.0, 1.0, 1.0])
}

fn trigger_target_label(target: &TimedTriggerTarget) -> &'static str {
    match target {
        TimedTriggerTarget::Camera => "Camera",
        TimedTriggerTarget::Object { .. } => "Object",
        TimedTriggerTarget::Objects { .. } => "Objects",
    }
}

fn trigger_action_label(action: &TimedTriggerAction) -> &'static str {
    match action {
        TimedTriggerAction::MoveTo { .. } => "Move",
        TimedTriggerAction::RotateTo { .. } => "Rotate",
        TimedTriggerAction::ScaleTo { .. } => "Scale",
        TimedTriggerAction::CameraPose { .. } => "Camera Pose",
        TimedTriggerAction::CameraFollow { .. } => "Camera Follow",
    }
}

fn is_camera_track_trigger(trigger: &TimedTrigger) -> bool {
    matches!(trigger.target, TimedTriggerTarget::Camera)
        && matches!(
            trigger.action,
            TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. }
        )
}

fn add_trigger_button(
    ui: &mut egui::Ui,
    label: &str,
    view: &EditorUiViewModel<'_>,
    target: TimedTriggerTarget,
    action: TimedTriggerAction,
    commands: &mut Vec<AppCommand>,
) {
    if ui.button(label).clicked() {
        commands.push(AppCommand::EditorAddTrigger(make_trigger(
            view, target, action,
        )));
    }
}

fn show_transition_controls(
    ui: &mut egui::Ui,
    transition_interval_seconds: &mut f32,
    use_full_segment_transition: &mut bool,
    timeline_duration_seconds: f32,
) -> bool {
    let mut changed = false;
    ui.label("Transition:");
    changed |= ui
        .checkbox(use_full_segment_transition, "Full Segment")
        .changed();
    if !*use_full_segment_transition {
        changed |= ui
            .add(
                egui::DragValue::new(transition_interval_seconds)
                    .speed(0.01)
                    .range(0.0..=timeline_duration_seconds.max(0.1))
                    .suffix("s"),
            )
            .changed();
    }
    changed
}

fn show_target_editor(ui: &mut egui::Ui, target: &mut TimedTriggerTarget) -> bool {
    let mut changed = false;
    match target {
        TimedTriggerTarget::Camera => {}
        TimedTriggerTarget::Object { object_id } => {
            ui.horizontal(|ui| {
                ui.label("Object ID:");
                changed |= ui
                    .add(egui::DragValue::new(object_id).range(0..=u32::MAX))
                    .changed();
            });
        }
        TimedTriggerTarget::Objects { object_ids } => {
            let mut text = object_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            ui.horizontal(|ui| {
                ui.label("Object IDs:");
                if ui.text_edit_singleline(&mut text).changed() {
                    let mut parsed = text
                        .split(',')
                        .filter_map(|segment| segment.trim().parse::<u32>().ok())
                        .collect::<Vec<_>>();
                    parsed.sort_unstable();
                    parsed.dedup();
                    if *object_ids != parsed {
                        *object_ids = parsed;
                        changed = true;
                    }
                }
            });
        }
    }
    changed
}

fn show_action_editor(
    ui: &mut egui::Ui,
    action: &mut TimedTriggerAction,
    timeline_duration_seconds: f32,
) -> bool {
    let mut changed = false;

    match action {
        TimedTriggerAction::MoveTo { position } => {
            ui.horizontal(|ui| {
                ui.label("Position:");
                changed |= ui
                    .add(egui::DragValue::new(&mut position[0]).prefix("X "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut position[1]).prefix("Y "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut position[2]).prefix("Z "))
                    .changed();
            });
        }
        TimedTriggerAction::RotateTo { rotation_degrees } => {
            ui.horizontal(|ui| {
                ui.label("Rotation:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut rotation_degrees[0])
                            .prefix("X ")
                            .suffix("°"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut rotation_degrees[1])
                            .prefix("Y ")
                            .suffix("°"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut rotation_degrees[2])
                            .prefix("Z ")
                            .suffix("°"),
                    )
                    .changed();
            });
        }
        TimedTriggerAction::ScaleTo { size } => {
            ui.horizontal(|ui| {
                ui.label("Scale:");
                changed |= ui
                    .add(egui::DragValue::new(&mut size[0]).prefix("X "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut size[1]).prefix("Y "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut size[2]).prefix("Z "))
                    .changed();
            });
        }
        TimedTriggerAction::CameraPose {
            transition_interval_seconds,
            use_full_segment_transition,
            target_position,
            rotation,
            pitch,
        } => {
            ui.horizontal_wrapped(|ui| {
                changed |= show_transition_controls(
                    ui,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    timeline_duration_seconds,
                );

                ui.separator();
                ui.label("Target:");
                changed |= ui
                    .add(egui::DragValue::new(&mut target_position[0]).prefix("X "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut target_position[1]).prefix("Y "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut target_position[2]).prefix("Z "))
                    .changed();

                ui.separator();
                let mut rotation_degrees = rotation.to_degrees();
                let mut pitch_degrees = pitch.to_degrees();
                ui.label("Orientation:");
                if ui
                    .add(
                        egui::DragValue::new(&mut rotation_degrees)
                            .speed(0.5)
                            .prefix("Rot ")
                            .suffix("°"),
                    )
                    .changed()
                {
                    *rotation = rotation_degrees.to_radians();
                    changed = true;
                }
                if ui
                    .add(
                        egui::DragValue::new(&mut pitch_degrees)
                            .speed(0.5)
                            .prefix("Pitch ")
                            .suffix("°"),
                    )
                    .changed()
                {
                    *pitch = pitch_degrees.to_radians();
                    changed = true;
                }
            });
        }
        TimedTriggerAction::CameraFollow {
            transition_interval_seconds,
            use_full_segment_transition,
        } => {
            ui.horizontal_wrapped(|ui| {
                changed |= show_transition_controls(
                    ui,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    timeline_duration_seconds,
                );
            });
        }
    }

    changed
}

pub(crate) fn show_trigger_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    show_mode_and_snap_controls(ui, view, commands);

    ui.horizontal(|ui| {
        let mut simulate_hitboxes = view.simulate_trigger_hitboxes;
        if ui
            .checkbox(
                &mut simulate_hitboxes,
                "Timed Object Triggers Move Hitboxes During Play",
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSimulateTriggerHitboxes(
                simulate_hitboxes,
            ));
        }
    });

    ui.separator();

    ui.horizontal_wrapped(|ui| {
        let object_id = selected_object_id_or_default(view);
        let position = selected_position_or_default(view);
        let rotation_degrees = selected_rotation_or_default(view);
        let size = selected_size_or_default(view);

        if ui.button("Add camera pose trigger (Shift+K)").clicked() {
            commands.push(AppCommand::EditorAddCameraTrigger);
        }

        add_trigger_button(
            ui,
            "Add camera follow trigger",
            view,
            TimedTriggerTarget::Camera,
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
            },
            commands,
        );

        add_trigger_button(
            ui,
            "Add object move trigger",
            view,
            TimedTriggerTarget::Object { object_id },
            TimedTriggerAction::MoveTo { position },
            commands,
        );
        add_trigger_button(
            ui,
            "Add object rotate trigger",
            view,
            TimedTriggerTarget::Object { object_id },
            TimedTriggerAction::RotateTo { rotation_degrees },
            commands,
        );
        add_trigger_button(
            ui,
            "Add object scale trigger",
            view,
            TimedTriggerTarget::Object { object_id },
            TimedTriggerAction::ScaleTo { size },
            commands,
        );

        add_trigger_button(
            ui,
            "Add objects move trigger",
            view,
            TimedTriggerTarget::Objects {
                object_ids: vec![object_id],
            },
            TimedTriggerAction::MoveTo { position },
            commands,
        );
        add_trigger_button(
            ui,
            "Add objects rotate trigger",
            view,
            TimedTriggerTarget::Objects {
                object_ids: vec![object_id],
            },
            TimedTriggerAction::RotateTo { rotation_degrees },
            commands,
        );
        add_trigger_button(
            ui,
            "Add objects scale trigger",
            view,
            TimedTriggerTarget::Objects {
                object_ids: vec![object_id],
            },
            TimedTriggerAction::ScaleTo { size },
            commands,
        );
    });

    ui.separator();

    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Triggers: {}", view.triggers.len()));

        if let Some(selected_idx) = view.trigger_selected_index {
            if ui.button("Remove selected trigger").clicked() {
                commands.push(AppCommand::EditorRemoveTrigger(selected_idx));
            }

            if ui.button("Use playhead time (trigger)").clicked() {
                if let Some(selected_trigger) = view.triggers.get(selected_idx).cloned() {
                    let mut updated = selected_trigger;
                    updated.time_seconds = view.timeline_time_seconds;
                    commands.push(AppCommand::EditorUpdateTrigger(selected_idx, updated));
                }
            }
        }
    });

    egui::ScrollArea::horizontal()
        .max_height(72.0)
        .show(ui, |ui| {
            for (index, trigger) in view.triggers.iter().enumerate() {
                let label = format!(
                    "#{:02}  {:.2}s  {} -> {}",
                    index + 1,
                    trigger.time_seconds,
                    trigger_target_label(&trigger.target),
                    trigger_action_label(&trigger.action)
                );
                if ui
                    .selectable_label(view.trigger_selected_index == Some(index), label)
                    .clicked()
                {
                    commands.push(AppCommand::EditorSetTriggerSelected(Some(index)));
                    commands.push(AppCommand::EditorSetTimelineTime(
                        trigger
                            .time_seconds
                            .clamp(0.0, view.timeline_duration_seconds.max(0.1)),
                    ));
                }
            }

            if view.triggers.is_empty() {
                ui.label("No triggers yet.");
            }
        });

    ui.separator();

    ui.horizontal_wrapped(|ui| {
        if let Some(selected_idx) = view.trigger_selected_index {
            if let Some(selected_trigger) = view.triggers.get(selected_idx) {
                if is_camera_track_trigger(selected_trigger) {
                    if ui.button("Capture current camera pose").clicked() {
                        commands.push(AppCommand::EditorCaptureSelectedCameraTrigger);
                    }
                    if ui.button("Jump editor camera").clicked() {
                        commands.push(AppCommand::EditorApplySelectedCameraTrigger);
                    }
                }
            }
        }
    });

    ui.separator();

    if let Some(selected_idx) = view.trigger_selected_index {
        if let Some(selected_trigger) = view.triggers.get(selected_idx).cloned() {
            let mut trigger = selected_trigger;
            let mut changed = false;

            ui.label(format!("Editing Trigger #{}", selected_idx + 1));

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                ui.label("Time:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut trigger.time_seconds)
                            .speed(0.01)
                            .range(0.0..=view.timeline_duration_seconds.max(0.1)),
                    )
                    .changed();

                ui.separator();

                ui.label("Duration:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut trigger.duration_seconds)
                            .speed(0.01)
                            .range(0.0..=view.timeline_duration_seconds.max(0.1))
                            .suffix("s"),
                    )
                    .changed();

                ui.separator();

                ui.label("Easing:");
                let mut easing = trigger.easing;
                egui::ComboBox::from_id_salt("trigger_easing")
                    .selected_text(match easing {
                        TimedTriggerEasing::Linear => "Linear",
                        TimedTriggerEasing::EaseIn => "Ease In",
                        TimedTriggerEasing::EaseOut => "Ease Out",
                        TimedTriggerEasing::EaseInOut => "Ease In/Out",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut easing, TimedTriggerEasing::Linear, "Linear");
                        ui.selectable_value(&mut easing, TimedTriggerEasing::EaseIn, "Ease In");
                        ui.selectable_value(&mut easing, TimedTriggerEasing::EaseOut, "Ease Out");
                        ui.selectable_value(
                            &mut easing,
                            TimedTriggerEasing::EaseInOut,
                            "Ease In/Out",
                        );
                    });
                if easing != trigger.easing {
                    trigger.easing = easing;
                    changed = true;
                }
            });

            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Target: {}", trigger_target_label(&trigger.target)));
                ui.separator();
                ui.label(format!("Action: {}", trigger_action_label(&trigger.action)));
            });

            changed |= show_target_editor(ui, &mut trigger.target);
            changed |= show_action_editor(ui, &mut trigger.action, view.timeline_duration_seconds);

            if changed {
                commands.push(AppCommand::EditorUpdateTrigger(selected_idx, trigger));
            }
        }
    } else {
        ui.label("Select a trigger to edit it.");
    }

    show_player_camera_status_row(ui, view);
}
