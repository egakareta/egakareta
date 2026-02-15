use crate::block_repository::all_placeable_blocks;
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::types::{EditorMode, SpawnDirection};
use crate::State;

pub(crate) fn show_compose_mode_bottom_panel(
    ui: &mut egui::Ui,
    state: &mut State,
    duration_seconds: f32,
) {
    ui.horizontal(|ui| {
        ui.label("Mode:");
        let mode = state.editor_mode();
        if ui
            .selectable_label(mode == EditorMode::Select, "Select")
            .clicked()
        {
            state.dispatch(AppCommand::EditorSetMode(EditorMode::Select));
        }
        if ui
            .selectable_label(mode == EditorMode::Place, "Place")
            .clicked()
        {
            state.dispatch(AppCommand::EditorSetMode(EditorMode::Place));
        }

        ui.separator();
        let mut snap = state.editor_snap_to_grid();
        if ui.checkbox(&mut snap, "Snap to Grid").changed() {
            state.dispatch(AppCommand::EditorSetSnapToGrid(snap));
        }

        ui.label("Step:");
        let mut snap_step = state.editor_snap_step();
        if ui
            .add(
                egui::DragValue::new(&mut snap_step)
                    .speed(0.05)
                    .range(0.05..=100.0),
            )
            .changed()
        {
            state.dispatch(AppCommand::EditorSetSnapStep(snap_step));
        }
    });

    match state.editor_mode() {
        EditorMode::Place => {
            ui.horizontal(|ui| {
                ui.label("Block:");

                let current = state.editor_selected_block_id().to_string();
                for block in all_placeable_blocks() {
                    if !block.placeable {
                        continue;
                    }
                    if ui
                        .selectable_label(current == block.id, &block.display_name)
                        .clicked()
                    {
                        state.dispatch(AppCommand::EditorSetBlockId(block.id.clone()));
                    }
                }
            });
        }
        EditorMode::Select => {
            ui.label("Tip: Shift+Click to select multiple blocks.");
            if let Some(mut selected) = state.editor_selected_block() {
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
                            state.dispatch(crate::commands::AppCommand::EditorUpdateSelectedBlock(
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
                            state.dispatch(crate::commands::AppCommand::EditorUpdateSelectedBlock(
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
                            state.dispatch(crate::commands::AppCommand::EditorUpdateSelectedBlock(
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
                            state.dispatch(crate::commands::AppCommand::EditorUpdateSelectedBlock(
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
                            state.dispatch(crate::commands::AppCommand::EditorUpdateSelectedBlock(
                                next,
                            ));
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
        ui.label("Timeline:");

        let mut time_seconds = state.editor_timeline_time_seconds();
        let slider = egui::Slider::new(&mut time_seconds, 0.0..=duration_seconds)
            .text("Time")
            .show_value(true);
        if ui.add(slider).changed() {
            state.dispatch(crate::commands::AppCommand::EditorSetTimelineTime(
                time_seconds,
            ));
        }

        let mut duration = state.editor_timeline_duration_seconds();
        ui.label("Duration (s):");
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS),
            )
            .changed()
        {
            state.dispatch(crate::commands::AppCommand::EditorSetTimelineDuration(
                duration,
            ));
        }

        if ui.button("Add tap").clicked() {
            state.dispatch(crate::commands::AppCommand::EditorAddTap);
        }
        if ui.button("Remove tap").clicked() {
            state.dispatch(crate::commands::AppCommand::EditorRemoveTap);
        }
        if ui.button("Clear taps").clicked() {
            state.dispatch(crate::commands::AppCommand::EditorClearTaps);
        }
    });

    let (position, direction) = state.editor_timeline_preview();
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
        ui.label(format!("FPS: {:.0}", state.editor_fps()));
    });
}
