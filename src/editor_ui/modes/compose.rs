use crate::block_repository::all_placeable_blocks;
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::state::EditorUiViewModel;
use crate::types::{
    camera_keypoints_to_timed_triggers, CameraKeypointEasing, CameraKeypointMode, EditorMode,
    SpawnDirection, TimedTriggerAction, TimedTriggerTarget,
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
            .selectable_label(mode == EditorMode::Select, "Select")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Select));
        }
        if ui
            .selectable_label(mode == EditorMode::Move, "Move")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Move));
        }
        if ui
            .selectable_label(mode == EditorMode::Scale, "Scale")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Scale));
        }
        if ui
            .selectable_label(mode == EditorMode::Place, "Place")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Place));
        }

        ui.separator();
        let mut snap = view.snap_to_grid;
        if ui.checkbox(&mut snap, "Snap to Grid").changed() {
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
        EditorMode::Select | EditorMode::Move | EditorMode::Scale => {
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
                        ui.label("Angle:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut selected.rotation_degrees)
                                    .speed(0.5)
                                    .suffix("°"),
                            )
                            .changed()
                        {
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
        EditorMode::Timing => {} // handled separately
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

    ui.collapsing("Camera Track", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Triggers: {}", view.triggers.len()));

            if ui.button("Add trigger from selected keypoint").clicked() {
                if let Some(selected_idx) = view.camera_selected_index {
                    if let Some(keypoint) = view.camera_keypoints.get(selected_idx) {
                        if let Some(trigger) =
                            camera_keypoints_to_timed_triggers(std::slice::from_ref(keypoint))
                                .into_iter()
                                .next()
                        {
                            commands.push(AppCommand::EditorAddTrigger(trigger));
                        }
                    }
                }
            }

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

        egui::ScrollArea::vertical().max_height(72.0).show(ui, |ui| {
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

        ui.horizontal(|ui| {
            if ui.button("Add at playhead (Shift+K)").clicked() {
                commands.push(AppCommand::EditorAddCameraKeypoint);
            }

            if let Some(selected_idx) = view.camera_selected_index {
                if ui.button("Remove").clicked() {
                    commands.push(AppCommand::EditorRemoveCameraKeypoint(selected_idx));
                }
                if ui.button("Use current camera").clicked() {
                    commands.push(AppCommand::EditorCaptureSelectedCameraKeypoint);
                }
                if ui.button("Jump editor camera").clicked() {
                    commands.push(AppCommand::EditorApplySelectedCameraKeypoint);
                }
                if let Some(selected_keypoint) = view.camera_keypoints.get(selected_idx).cloned() {
                    if ui.button("Use playhead time").clicked() {
                        let mut updated = selected_keypoint;
                        updated.time_seconds = view.timeline_time_seconds;
                        commands.push(AppCommand::EditorUpdateCameraKeypoint(
                            selected_idx,
                            updated,
                        ));
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Keypoints:");
                egui::ScrollArea::vertical()
                    .max_height(110.0)
                    .show(ui, |ui| {
                        for (index, keypoint) in view.camera_keypoints.iter().enumerate() {
                            let label = format!(
                                "#{}: {:.2}s | {} | {}",
                                index + 1,
                                keypoint.time_seconds,
                                match keypoint.mode {
                                    CameraKeypointMode::Follow => "Follow",
                                    CameraKeypointMode::Static => "Static",
                                },
                                match keypoint.easing {
                                    CameraKeypointEasing::Linear => "Linear",
                                    CameraKeypointEasing::EaseIn => "Ease In",
                                    CameraKeypointEasing::EaseOut => "Ease Out",
                                    CameraKeypointEasing::EaseInOut => "Ease In/Out",
                                }
                            );
                            if ui
                                .selectable_label(view.camera_selected_index == Some(index), label)
                                .clicked()
                            {
                                commands.push(AppCommand::EditorSetCameraKeypointSelected(Some(index)));
                            }
                        }

                        if view.camera_keypoints.is_empty() {
                            ui.label("No camera keypoints yet.");
                        }
                    });
            });

            ui.separator();

            ui.vertical(|ui| {
                if let Some(selected_idx) = view.camera_selected_index {
                    if let Some(selected_keypoint) = view.camera_keypoints.get(selected_idx).cloned() {
                        let mut keypoint = selected_keypoint;
                        ui.label(format!("Editing Camera Keypoint #{}", selected_idx + 1));

                        let mut changed = false;

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            ui.label("Time:");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut keypoint.time_seconds)
                                        .speed(0.01)
                                        .range(0.0..=view.timeline_duration_seconds.max(0.1)),
                                )
                                .changed();

                            ui.separator();

                            ui.label("Mode:");
                            let mut mode = keypoint.mode;
                            egui::ComboBox::from_id_salt("camera_keypoint_mode")
                                .selected_text(match mode {
                                    CameraKeypointMode::Follow => "Follow",
                                    CameraKeypointMode::Static => "Static",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut mode, CameraKeypointMode::Follow, "Follow");
                                    ui.selectable_value(&mut mode, CameraKeypointMode::Static, "Static");
                                });
                            if mode != keypoint.mode {
                                keypoint.mode = mode;
                                changed = true;
                            }

                            ui.separator();

                            ui.label("Easing:");
                            let mut easing = keypoint.easing;
                            egui::ComboBox::from_id_salt("camera_keypoint_easing")
                                .selected_text(match easing {
                                    CameraKeypointEasing::Linear => "Linear",
                                    CameraKeypointEasing::EaseIn => "Ease In",
                                    CameraKeypointEasing::EaseOut => "Ease Out",
                                    CameraKeypointEasing::EaseInOut => "Ease In/Out",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut easing, CameraKeypointEasing::Linear, "Linear");
                                    ui.selectable_value(&mut easing, CameraKeypointEasing::EaseIn, "Ease In");
                                    ui.selectable_value(&mut easing, CameraKeypointEasing::EaseOut, "Ease Out");
                                    ui.selectable_value(&mut easing, CameraKeypointEasing::EaseInOut, "Ease In/Out");
                                });
                            if easing != keypoint.easing {
                                keypoint.easing = easing;
                                changed = true;
                            }

                            ui.separator();

                            ui.label("Transition:");
                            changed |= ui
                                .checkbox(&mut keypoint.use_full_segment_transition, "Full Segment")
                                .changed();

                            if !keypoint.use_full_segment_transition {
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut keypoint.transition_interval_seconds)
                                            .speed(0.01)
                                            .range(0.0..=view.timeline_duration_seconds.max(0.1))
                                            .suffix("s"),
                                    )
                                    .changed();
                            }

                            ui.separator();

                            ui.label("Target:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut keypoint.target_position[0]).prefix("X "))
                                .changed();
                            changed |= ui
                                .add(egui::DragValue::new(&mut keypoint.target_position[1]).prefix("Y "))
                                .changed();
                            changed |= ui
                                .add(egui::DragValue::new(&mut keypoint.target_position[2]).prefix("Z "))
                                .changed();

                            ui.separator();

                            let mut rotation_degrees = keypoint.rotation.to_degrees();
                            let mut pitch_degrees = keypoint.pitch.to_degrees();
                            ui.label("Orientation:");
                            if ui
                                .add(egui::DragValue::new(&mut rotation_degrees).speed(0.5).prefix("Rot ").suffix("°"))
                                .changed()
                            {
                                keypoint.rotation = rotation_degrees.to_radians();
                                changed = true;
                            }
                            if ui
                                .add(egui::DragValue::new(&mut pitch_degrees).speed(0.5).prefix("Pitch ").suffix("°"))
                                .changed()
                            {
                                keypoint.pitch = pitch_degrees.to_radians();
                                changed = true;
                            }
                        });

                        if keypoint.mode == CameraKeypointMode::Follow {
                            ui.label("Follow blends toward the live player-follow camera during this segment.");
                        }

                        if changed {
                            commands.push(AppCommand::EditorUpdateCameraKeypoint(selected_idx, keypoint));
                        }
                    }
                } else {
                    ui.label("Select a camera keypoint to edit it.");
                }
            });
        });
    });

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
