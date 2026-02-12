use crate::types::SpawnDirection;
use crate::{BlockKind, State};

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Block:");

                    let current = state.editor_selected_block_kind();
                    for (name, kind) in [
                        ("Standard", BlockKind::Standard),
                        ("Grass", BlockKind::Grass),
                        ("Dirt", BlockKind::Dirt),
                    ] {
                        if ui.selectable_label(current == kind, name).clicked() {
                            state.set_editor_block_kind(kind);
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Timeline:");

                    let max_step = state.editor_timeline_length().saturating_sub(1);
                    let mut step = state.editor_timeline_step();
                    let slider = egui::Slider::new(&mut step, 0..=max_step)
                        .text("Step")
                        .show_value(true);
                    if ui.add(slider).changed() {
                        state.set_editor_timeline_step(step);
                    }

                    let mut length = state.editor_timeline_length();
                    if ui
                        .add(egui::DragValue::new(&mut length).range(1..=512))
                        .changed()
                    {
                        state.set_editor_timeline_length(length);
                    }

                    if ui.button("Add tap").clicked() {
                        state.editor_add_tap();
                    }
                    if ui.button("Remove tap").clicked() {
                        state.editor_remove_tap();
                    }
                    if ui.button("Clear taps").clicked() {
                        state.editor_clear_taps();
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
                });

                let available_width = ui.available_width();
                let timeline_height = 18.0;
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(available_width, timeline_height),
                    egui::Sense::click(),
                );

                let painter = ui.painter();
                let center_y = rect.center().y;
                let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(160));
                painter.line_segment(
                    [
                        egui::pos2(rect.left(), center_y),
                        egui::pos2(rect.right(), center_y),
                    ],
                    stroke,
                );

                let max_step = state.editor_timeline_length().saturating_sub(1);
                let denom = max_step.max(1) as f32;
                for tap in state.editor_tap_steps() {
                    let t = (*tap as f32 / denom).clamp(0.0, 1.0);
                    let x = rect.left() + rect.width() * t;
                    painter.circle_filled(
                        egui::pos2(x, center_y),
                        3.0,
                        egui::Color32::from_rgb(255, 170, 64),
                    );
                }

                let current_t = state.editor_timeline_step() as f32 / denom;
                let current_x = rect.left() + rect.width() * current_t.clamp(0.0, 1.0);
                painter.line_segment(
                    [
                        egui::pos2(current_x, rect.top()),
                        egui::pos2(current_x, rect.bottom()),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 200, 255)),
                );

                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                        let step = (t * denom).round() as u32;
                        state.set_editor_timeline_step(step);
                    }
                }
            });
        });
}
