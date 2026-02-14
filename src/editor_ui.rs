use crate::types::{EditorMode, SpawnDirection};
use crate::{BlockKind, State};

const MIN_TIMELINE_LENGTH: u32 = 1;
const MAX_TIMELINE_LENGTH: u32 = 512;
const MENU_WORDMARK_PNG: &[u8] = include_bytes!("../assets/wordmark.png");

pub fn load_menu_wordmark_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(MENU_WORDMARK_PNG).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture("menu_wordmark", color_image, egui::TextureOptions::LINEAR))
}

pub fn show_menu_wordmark_ui(ctx: &egui::Context, state: &State, wordmark: &egui::TextureHandle) {
    if !state.is_menu() {
        return;
    }

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    let max_width = (ctx.screen_rect().width() * 0.68).max(240.0);
    let scale = (max_width / texture_size.x).min(1.0);
    let display_size = texture_size * scale;

    egui::Area::new("menu_wordmark_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)));
        });
}

fn timeline_step_metrics(length: u32) -> (u32, f32) {
    let max_step = length.saturating_sub(1);
    let divisor = max_step.max(1) as f32;
    (max_step, divisor)
}

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Level:");

            let levels = state.available_levels().to_vec();
            let selected = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());

            egui::ComboBox::from_id_salt("level_select")
                .selected_text(&selected)
                .show_ui(ui, |ui| {
                    for level in levels {
                        if ui.selectable_label(selected == level, &level).clicked() {
                            state.load_builtin_level_into_editor(&level);
                        }
                    }
                });

            ui.separator();

            ui.label("Name:");
            let mut name = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());
            if ui.text_edit_singleline(&mut name).changed() {
                state.set_editor_level_name(name);
            }

            ui.separator();

            if ui.button("Export .ldz").clicked() {
                state.trigger_level_export();
            }

            if ui.button("Import .ldz/JSON").clicked() {
                state.set_editor_show_import(true);
            }

            if ui.button("Metadata").clicked() {
                state.set_editor_show_metadata(true);
            }
        });
    });

    if state.editor_show_metadata() {
        egui::Window::new("Level Metadata").show(ctx, |ui| {
            ui.label("Level Name:");
            let mut name = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());
            if ui.text_edit_singleline(&mut name).changed() {
                state.set_editor_level_name(name);
            }

            ui.separator();
            ui.heading("Music");

            let mut music = state.editor_music_metadata().clone();
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Source:");
                if ui.text_edit_singleline(&mut music.source).changed() {
                    changed = true;
                }
                if ui.button("Import External Audio").clicked() {
                    state.trigger_audio_import();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Title:");
                let mut title = music.title.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut title).changed() {
                    music.title = Some(title);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Author:");
                let mut author = music.author.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut author).changed() {
                    music.author = Some(author);
                    changed = true;
                }
            });

            if changed {
                state.set_editor_music_metadata(music);
            }

            if ui.button("Close").clicked() {
                state.set_editor_show_metadata(false);
            }
        });
    }

    if state.editor_show_import() {
        egui::Window::new("Import Level").show(ctx, |ui| {
            ui.label("Paste level JSON or Base64 LDZ below:");
            let mut text = state.editor_import_text().to_string();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut text)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                )
                .changed()
            {
                state.set_editor_import_text(text);
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    state.complete_import();
                }
                if ui.button("Cancel").clicked() {
                    state.set_editor_show_import(false);
                }
            });
        });
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let (max_step, divisor) = timeline_step_metrics(state.editor_timeline_length());

                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    let mode = state.editor_mode();
                    if ui
                        .selectable_label(mode == EditorMode::Select, "Select")
                        .clicked()
                    {
                        state.set_editor_mode(EditorMode::Select);
                    }
                    if ui
                        .selectable_label(mode == EditorMode::Place, "Place")
                        .clicked()
                    {
                        state.set_editor_mode(EditorMode::Place);
                    }

                    ui.separator();
                    let mut snap = state.editor_snap_to_grid();
                    if ui.checkbox(&mut snap, "Snap to Grid").changed() {
                        state.set_editor_snap_to_grid(snap);
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
                        state.set_editor_snap_step(snap_step);
                    }
                });

                match state.editor_mode() {
                    EditorMode::Place => {
                        ui.horizontal(|ui| {
                            ui.label("Block:");

                            let current = state.editor_selected_block_kind();
                            for (name, kind) in [
                                ("Standard", BlockKind::Standard),
                                ("Grass", BlockKind::Grass),
                                ("Dirt", BlockKind::Dirt),
                                ("Void", BlockKind::Void),
                                ("Speed Portal", BlockKind::SpeedPortal),
                            ] {
                                if ui.selectable_label(current == kind, name).clicked() {
                                    state.set_editor_block_kind(kind);
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
                                        .add(
                                            egui::DragValue::new(&mut selected.position[0])
                                                .prefix("X "),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut selected.position[1])
                                                .prefix("Y "),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut selected.position[2])
                                                .prefix("Z "),
                                        )
                                        .changed();
                                    if changed {
                                        state.set_editor_selected_block_position(selected.position);
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Resize:");
                                    let mut changed = false;
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut selected.size[0])
                                                .prefix("W "),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut selected.size[1])
                                                .prefix("D "),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut selected.size[2])
                                                .prefix("H "),
                                        )
                                        .changed();
                                    if changed {
                                        state.set_editor_selected_block_size(selected.size);
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
                                        state.set_editor_selected_block_rotation(
                                            selected.rotation_degrees,
                                        );
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
                                        state.set_editor_selected_block_roundness(
                                            selected.roundness,
                                        );
                                    }
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                for (name, kind) in [
                                    ("Standard", BlockKind::Standard),
                                    ("Grass", BlockKind::Grass),
                                    ("Dirt", BlockKind::Dirt),
                                    ("Void", BlockKind::Void),
                                    ("Speed Portal", BlockKind::SpeedPortal),
                                ] {
                                    if ui.selectable_label(selected.kind == kind, name).clicked() {
                                        state.set_editor_selected_block_kind(kind);
                                    }
                                }
                            });
                        } else {
                            ui.label("Select mode: click a block to edit it.");
                        }
                    }
                }

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
