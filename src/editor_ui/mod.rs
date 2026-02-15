pub(crate) mod components;
pub(crate) mod menu;
pub(crate) mod modes;

use crate::editor_ui::components::{show_timeline_bar, show_waveform_panel, timeline_metrics};
use crate::editor_ui::modes::compose::show_compose_mode_bottom_panel;
use crate::editor_ui::modes::timing::show_timing_mode_bottom_panel;
use crate::types::EditorMode;
use crate::State;
pub use menu::{load_menu_wordmark_texture, show_menu_wordmark_ui};

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Top-level tabs: Compose / Timing
            let mode = state.editor_mode();
            let is_compose = mode == EditorMode::Select || mode == EditorMode::Place;
            if ui.selectable_label(is_compose, "Compose").clicked() && !is_compose {
                state.dispatch(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Place,
                ));
            }
            if ui
                .selectable_label(mode == EditorMode::Timing, "Timing")
                .clicked()
                && mode != EditorMode::Timing
            {
                state.dispatch(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Timing,
                ));
            }

            ui.separator();

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
                            state.dispatch(crate::commands::AppCommand::EditorLoadLevel(level));
                        }
                    }
                });

            ui.separator();

            ui.label("Name:");
            let mut name = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());
            if ui.text_edit_singleline(&mut name).changed() {
                state.dispatch(crate::commands::AppCommand::EditorRenameLevel(name));
            }

            ui.separator();

            if ui.button("Export .ldz").clicked() {
                state.dispatch(crate::commands::AppCommand::EditorExportLevel);
            }

            if ui.button("Import .ldz/JSON").clicked() {
                state.dispatch(crate::commands::AppCommand::EditorSetShowImport(true));
            }

            if ui.button("Metadata").clicked() {
                state.dispatch(crate::commands::AppCommand::EditorSetShowMetadata(true));
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
                state.dispatch(crate::commands::AppCommand::EditorRenameLevel(name));
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
                    state.dispatch(crate::commands::AppCommand::EditorTriggerAudioImport);
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
                state.dispatch(crate::commands::AppCommand::EditorUpdateMusic(music));
            }

            if ui.button("Close").clicked() {
                state.dispatch(crate::commands::AppCommand::EditorSetShowMetadata(false));
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
                state.dispatch(crate::commands::AppCommand::EditorSetImportText(text));
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    state.dispatch(crate::commands::AppCommand::EditorCompleteImport);
                }
                if ui.button("Cancel").clicked() {
                    state.dispatch(crate::commands::AppCommand::EditorSetShowImport(false));
                }
            });
        });
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let duration_seconds = timeline_metrics(state.editor_timeline_duration_seconds());

                if state.editor_mode() == EditorMode::Timing {
                    show_timing_mode_bottom_panel(ui, state, duration_seconds);
                } else {
                    show_compose_mode_bottom_panel(ui, state, duration_seconds);
                }

                // Shared timeline bar with beat lines
                show_timeline_bar(ui, state, duration_seconds);
            });
        });

    // Waveform visualization central panel (Timing mode only)
    if state.editor_mode() == EditorMode::Timing {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style()).fill(egui::Color32::from_rgb(15, 20, 28)),
            )
            .show(ctx, |ui| {
                show_waveform_panel(ui, state);
            });
    }

    if state.editor_perf_overlay_enabled() {
        egui::Area::new("editor_perf_overlay".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_black_alpha(210))
                    .show(ui, |ui| {
                        for line in state.editor_perf_overlay_lines() {
                            ui.monospace(line);
                        }
                    });
            });
    }
}
