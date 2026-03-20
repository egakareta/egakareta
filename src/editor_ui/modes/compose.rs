use crate::block_repository::all_placeable_blocks;
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::state::EditorUiViewModel;
use crate::types::{
    EditorMode, SpawnDirection, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
};

pub(crate) fn show_compose_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal(|ui| {
        ui.label("Mode:");
        let mode = view.mode;
        if ui
            .selectable_label(
                mode == EditorMode::Select,
                format!("{} Select", egui_phosphor::regular::CURSOR_CLICK),
            )
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Select));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Move,
                format!("{} Move", egui_phosphor::regular::ARROWS_OUT),
            )
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Move));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Scale,
                format!("{} Scale", egui_phosphor::regular::CORNERS_OUT),
            )
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Scale));
        }
        if ui
            .selectable_label(mode == EditorMode::Rotate, "Rotate")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Rotate));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Place,
                format!("{} Place", egui_phosphor::regular::CUBE),
            )
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Place));
        }

        ui.separator();
        let mut snap = view.snap_to_grid;
        if ui
            .checkbox(
                &mut snap,
                format!("{} Snap to Grid", egui_phosphor::regular::GRID_FOUR),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapToGrid(snap));
        }

        ui.label("Step:");
        let mut snap_step = view.snap_step;
        if ui
            .add(
                egui::DragValue::new(&mut snap_step)
                    .speed(0.05)
                    .range(0.05..=100.0),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapStep(snap_step));
        }

        ui.separator();
        let mut snap_rotation = view.snap_rotation;
        if ui.checkbox(&mut snap_rotation, "Snap Rotation").changed() {
            commands.push(AppCommand::EditorSetSnapRotation(snap_rotation));
        }

        ui.label("Rot Step:");
        let mut snap_rotation_step = view.snap_rotation_step_degrees;
        if ui
            .add(
                egui::DragValue::new(&mut snap_rotation_step)
                    .speed(0.5)
                    .range(1.0..=180.0)
                    .suffix("°"),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapRotationStep(snap_rotation_step));
        }
    });

    match view.mode {
        EditorMode::Place => {
            ui.horizontal(|ui| {
                ui.label("Block:");

                let current = view.selected_block_id;
                for block in all_placeable_blocks() {
                    if !block.placeable {
                        continue;
                    }
                    if ui
                        .selectable_label(current == block.id, &block.display_name)
                        .clicked()
                    {
                        commands.push(AppCommand::EditorSetBlockId(block.id.clone()));
                    }
                }
            });
        }
        EditorMode::Select | EditorMode::Move | EditorMode::Scale | EditorMode::Rotate => {
            if let Some(mut selected) = view.selected_block.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Move:");
                        let mut changed = false;
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[0]).prefix("X "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[1]).prefix("Y "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[2]).prefix("Z "))
                            .changed();
                        if changed {
                            commands.push(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                                selected.clone(),
                            ));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Resize:");
                        let mut changed = false;
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[0]).prefix("W "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[1]).prefix("H "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[2]).prefix("D "))
                            .changed();
                        if changed {
                            commands.push(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                                selected.clone(),
                            ));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotate:");
                        let mut changed = false;
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut selected.rotation_degrees[0])
                                    .speed(0.5)
                                    .prefix("X ")
                                    .suffix("°"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut selected.rotation_degrees[1])
                                    .speed(0.5)
                                    .prefix("Y ")
                                    .suffix("°"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut selected.rotation_degrees[2])
                                    .speed(0.5)
                                    .prefix("Z ")
                                    .suffix("°"),
                            )
                            .changed();
                        if changed {
                            commands.push(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                                selected.clone(),
                            ));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Round:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut selected.roundness)
                                    .speed(0.01)
                                    .range(0.0..=10.0),
                            )
                            .changed()
                        {
                            commands.push(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                                selected.clone(),
                            ));
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Color:");
                    let mut color_tint = selected.color_tint;
                    if ui.color_edit_button_rgb(&mut color_tint).changed() {
                        selected.color_tint = color_tint;
                        commands.push(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                            selected.clone(),
                        ));
                    }
                });
            } else {
                ui.label("Selection mode: click a block to edit it.");
            }
        }
        EditorMode::Timing | EditorMode::Null => {} // handled separately
    }

    ui.separator();

    ui.horizontal(|ui| {
        let mut duration = view.timeline_duration_seconds;
        ui.label("Duration (s):");
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS),
            )
            .changed()
        {
            commands.push(crate::commands::AppCommand::EditorSetTimelineDuration(
                duration,
            ));
        }

        if ui.button("Add tap").clicked() {
            commands.push(crate::commands::AppCommand::EditorAddTap);
        }
        if ui.button("Remove tap").clicked() {
            commands.push(crate::commands::AppCommand::EditorRemoveTap);
        }
        if ui.button("Clear taps").clicked() {
            commands.push(crate::commands::AppCommand::EditorClearTaps);
        }
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
                let target_label = match trigger.target {
                    TimedTriggerTarget::Camera => "Camera",
                    TimedTriggerTarget::Object { .. } => "Object",
                    TimedTriggerTarget::Objects { .. } => "Objects",
                };
                let action_label = match trigger.action {
                    TimedTriggerAction::MoveTo { .. } => "Move",
                    TimedTriggerAction::RotateTo { .. } => "Rotate",
                    TimedTriggerAction::ScaleTo { .. } => "Scale",
                    TimedTriggerAction::CameraPose { .. } => "Camera Pose",
                    TimedTriggerAction::CameraFollow { .. } => "Camera Follow",
                };
                let label = format!(
                    "#{:02}  {:.2}s  {} -> {}",
                    index + 1,
                    trigger.time_seconds,
                    target_label,
                    action_label
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
        if ui
            .button("Add camera trigger at playhead (Shift+K)")
            .clicked()
        {
            commands.push(AppCommand::EditorAddCameraTrigger);
        }

        if let Some(selected_idx) = view.trigger_selected_index {
            if let Some(selected_trigger) = view.triggers.get(selected_idx) {
                let is_camera_trigger =
                    matches!(selected_trigger.target, TimedTriggerTarget::Camera)
                        && matches!(
                            selected_trigger.action,
                            TimedTriggerAction::CameraPose { .. }
                                | TimedTriggerAction::CameraFollow { .. }
                        );

                if is_camera_trigger {
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
                let target_label = match trigger.target {
                    TimedTriggerTarget::Camera => "Camera",
                    TimedTriggerTarget::Object { .. } => "Object",
                    TimedTriggerTarget::Objects { .. } => "Objects",
                };
                let action_label = match trigger.action {
                    TimedTriggerAction::MoveTo { .. } => "Move",
                    TimedTriggerAction::RotateTo { .. } => "Rotate",
                    TimedTriggerAction::ScaleTo { .. } => "Scale",
                    TimedTriggerAction::CameraPose { .. } => "Camera Pose",
                    TimedTriggerAction::CameraFollow { .. } => "Camera Follow",
                };
                ui.label(format!("Target: {}", target_label));
                ui.separator();
                ui.label(format!("Action: {}", action_label));
            });

            match &mut trigger.target {
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

            match &mut trigger.action {
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
                        ui.label("Transition:");
                        changed |= ui
                            .checkbox(use_full_segment_transition, "Full Segment")
                            .changed();
                        if !*use_full_segment_transition {
                            changed |= ui
                                .add(
                                    egui::DragValue::new(transition_interval_seconds)
                                        .speed(0.01)
                                        .range(0.0..=view.timeline_duration_seconds.max(0.1))
                                        .suffix("s"),
                                )
                                .changed();
                        }

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
                        ui.label("Transition:");
                        changed |= ui
                            .checkbox(use_full_segment_transition, "Full Segment")
                            .changed();
                        if !*use_full_segment_transition {
                            changed |= ui
                                .add(
                                    egui::DragValue::new(transition_interval_seconds)
                                        .speed(0.01)
                                        .range(0.0..=view.timeline_duration_seconds.max(0.1))
                                        .suffix("s"),
                                )
                                .changed();
                        }
                    });
                }
            }

            if changed {
                commands.push(AppCommand::EditorUpdateTrigger(selected_idx, trigger));
            }
        }
    } else {
        ui.label("Select a trigger to edit it.");
    }

    let position = view.timeline_preview_position;
    let direction = view.timeline_preview_direction;
    let direction_label = match direction {
        SpawnDirection::Forward => "Forward",
        SpawnDirection::Right => "Right",
    };
    ui.horizontal(|ui| {
        ui.label(format!(
            "Player: ({:.1}, {:.1}, {:.1}) | {}",
            position[0], position[1], position[2], direction_label
        ));
        ui.separator();
        ui.label(format!(
            "Player Camera: ({:.1}, {:.1}, {:.1}) -> ({:.1}, {:.1}, {:.1})",
            view.camera_preview_position[0],
            view.camera_preview_position[1],
            view.camera_preview_position[2],
            view.camera_preview_target[0],
            view.camera_preview_target[1],
            view.camera_preview_target[2],
        ));
        ui.separator();
        ui.label(format!(
            "Editor Camera: ({:.1}, {:.1}, {:.1})",
            view.camera_position[0], view.camera_position[1], view.camera_position[2]
        ));
        ui.separator();
        ui.label(format!("FPS: {:.0}", view.fps));
    });
}
