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
            .selectable_label(mode == EditorMode::Select, "Select")
            .clicked()
        {
            commands.push(AppCommand::EditorSetMode(EditorMode::Select));
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
        EditorMode::Select => {
            ui.label("Tip: Shift+Click to select multiple blocks.");
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
                            .add(egui::DragValue::new(&mut selected.size[1]).prefix("D "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[2]).prefix("H "))
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
                    for block in all_placeable_blocks() {
                        if !block.placeable {
                            continue;
                        }
                        if ui
                            .selectable_label(selected.block_id == block.id, &block.display_name)
                            .clicked()
                        {
                            let mut next = selected.clone();
                            next.block_id = block.id.clone();
                            commands
                                .push(crate::commands::AppCommand::EditorUpdateSelectedBlock(next));
                        }
                    }
                });
            } else {
                ui.label("Select mode: click a block to edit it.");
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
        ui.label(format!("FPS: {:.0}", view.fps));
    });
}
