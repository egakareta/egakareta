/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::block_repository::all_placeable_blocks;
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::editor_ui::modes::shared::{show_mode_and_snap_controls, show_player_camera_status_row};
use crate::state::EditorUiViewModel;
use crate::types::EditorMode;

pub(crate) fn show_compose_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    show_mode_and_snap_controls(ui, view, commands);

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

    show_player_camera_status_row(ui, view);
}
