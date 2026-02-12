use crate::types::SpawnDirection;
use crate::{BlockKind, State};

const MIN_TIMELINE_LENGTH: u32 = 1;
const MAX_TIMELINE_LENGTH: u32 = 512;

fn timeline_step_metrics(length: u32) -> (u32, f32) {
    let max_step = length.saturating_sub(1);
    let divisor = max_step.max(1) as f32;
    (max_step, divisor)
}

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let (max_step, divisor) = timeline_step_metrics(state.editor_timeline_length());

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

                    let mut step = state.editor_timeline_step();
                    let slider = egui::Slider::new(&mut step, 0..=max_step)
                        .text("Step")
                        .show_value(true);
                    if ui.add(slider).changed() {
                        state.set_editor_timeline_step(step);
                    }

                    let mut length = state.editor_timeline_length();
                    ui.label("Length:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut length)
                                .range(MIN_TIMELINE_LENGTH..=MAX_TIMELINE_LENGTH),
                        )
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

                let max_step_f32 = max_step as f32;
                for tap in state.editor_tap_steps() {
                    let t = (*tap as f32 / divisor).clamp(0.0, 1.0);
                    let x = rect.left() + rect.width() * t;
                    painter.circle_filled(
                        egui::pos2(x, center_y),
                        3.0,
                        egui::Color32::from_rgb(255, 170, 64),
                    );
                }

                let current_t = state.editor_timeline_step() as f32 / divisor;
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
                        let step = if max_step == 0 {
                            0
                        } else {
                            (t * max_step_f32).round() as u32
                        };
                        state.set_editor_timeline_step(step);
                    }
                }
            });
        });
}
