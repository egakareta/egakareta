use crate::block_repository::all_placeable_blocks;
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::state::EditorUiViewModel;
use crate::types::{EditorMode, SpawnDirection};

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
        if ui
            .selectable_label(
                mode == EditorMode::Trigger,
                format!("{} Trigger", egui_phosphor::regular::LIGHTNING),
            )
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Trigger));
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
        EditorMode::Trigger | EditorMode::Timing | EditorMode::Null => {} // handled separately
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
