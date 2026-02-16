pub(crate) mod components;
pub(crate) mod menu;
pub(crate) mod modes;

use crate::commands::AppCommand;
use crate::editor_ui::components::{show_timeline_bar, show_waveform_panel, timeline_metrics};
use crate::editor_ui::modes::compose::show_compose_mode_bottom_panel;
use crate::editor_ui::modes::timing::show_timing_mode_bottom_panel;
use crate::types::EditorMode;
use crate::State;
pub use menu::{load_menu_wordmark_texture, show_menu_wordmark_ui, show_splash_screen_ui};

/// Shows the editor UI using egui.
/// Handles the top bar, bottom panels, and other editor interface elements.
pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    let view = state.editor_ui_view_model();
    let mut commands = Vec::<AppCommand>::new();

    egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Top-level tabs: Compose / Timing
            let mode = view.mode;
            let is_compose = mode == EditorMode::Select || mode == EditorMode::Place;
            if ui.selectable_label(is_compose, "Compose").clicked() && !is_compose {
                commands.push(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Place,
                ));
            }
            if ui
                .selectable_label(mode == EditorMode::Timing, "Timing")
                .clicked()
                && mode != EditorMode::Timing
            {
                commands.push(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Timing,
                ));
            }

            ui.separator();

            ui.label("Level:");

            let levels = view.available_levels;
            let selected = view.level_name.unwrap_or("Untitled");

            egui::ComboBox::from_id_salt("level_select")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for level in levels {
                        if ui.selectable_label(selected == level, level).clicked() {
                            commands
                                .push(crate::commands::AppCommand::EditorLoadLevel(level.clone()));
                        }
                    }
                });

            ui.separator();

            if ui.button("Export .ldz").clicked() {
                commands.push(crate::commands::AppCommand::EditorExportLevel);
            }

            if ui.button("Import .ldz/JSON").clicked() {
                commands.push(crate::commands::AppCommand::EditorSetShowImport(true));
            }

            if ui.button("Metadata").clicked() {
                commands.push(crate::commands::AppCommand::EditorSetShowMetadata(true));
            }
        });
    });

    if view.show_metadata {
        egui::Window::new("Level Metadata").show(ctx, |ui| {
            ui.label("Level Name:");
            let mut name = view.level_name.unwrap_or("Untitled").to_string();
            if ui.text_edit_singleline(&mut name).changed() {
                commands.push(crate::commands::AppCommand::EditorRenameLevel(name));
            }

            ui.separator();
            ui.heading("Music");

            let mut music = view.music_metadata.clone();
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Source:");
                if ui.text_edit_singleline(&mut music.source).changed() {
                    changed = true;
                }
                if ui.button("Import External Audio").clicked() {
                    commands.push(crate::commands::AppCommand::EditorTriggerAudioImport);
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
                commands.push(crate::commands::AppCommand::EditorUpdateMusic(music));
            }

            if ui.button("Close").clicked() {
                commands.push(crate::commands::AppCommand::EditorSetShowMetadata(false));
            }
        });
    }

    if view.show_import {
        egui::Window::new("Import Level").show(ctx, |ui| {
            ui.label("Paste level JSON or Base64 LDZ below:");
            let mut text = view.import_text.to_string();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut text)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                )
                .changed()
            {
                commands.push(crate::commands::AppCommand::EditorSetImportText(text));
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    commands.push(crate::commands::AppCommand::EditorCompleteImport);
                }
                if ui.button("Cancel").clicked() {
                    commands.push(crate::commands::AppCommand::EditorSetShowImport(false));
                }
            });
        });
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let duration_seconds = timeline_metrics(view.timeline_duration_seconds);

                if view.mode == EditorMode::Timing {
                    show_timing_mode_bottom_panel(ui, &view, duration_seconds, &mut commands);
                } else {
                    show_compose_mode_bottom_panel(ui, &view, duration_seconds, &mut commands);
                }

                // Shared timeline bar with beat lines
                show_timeline_bar(ui, &view, duration_seconds, &mut commands);
            });
        });

    // Waveform visualization central panel (Timing mode only)
    if view.mode == EditorMode::Timing {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style()).fill(egui::Color32::from_rgb(15, 20, 28)),
            )
            .show(ctx, |ui| {
                show_waveform_panel(ui, &view, &mut commands);
            });
    }

    if view.mode == EditorMode::Select {
        if let Some((start, current, is_active_drag)) = view.marquee_selection_rect_screen {
            if is_active_drag {
                let rect = egui::Rect::from_two_pos(
                    egui::pos2(start[0] as f32, start[1] as f32),
                    egui::pos2(current[0] as f32, current[1] as f32),
                );
                let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(130, 180, 255));
                let layer = egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("editor_marquee_overlay"),
                );
                ctx.layer_painter(layer).rect(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(130, 180, 255, 38),
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }
        }
    }

    if view.perf_overlay_enabled {
        fn perf_stat_label(
            name: &str,
            last_ms: f32,
            avg_ms: f32,
            max_ms: f32,
            calls: u64,
        ) -> String {
            format!(
                "{:<22} last {:>6.2}ms | avg {:>6.2}ms | max {:>6.2}ms | n {}",
                name, last_ms, avg_ms, max_ms, calls
            )
        }

        fn show_perf_entry(ui: &mut egui::Ui, entry: &crate::state::PerfOverlayEntry) {
            if entry.children.is_empty() {
                ui.monospace(perf_stat_label(
                    entry.name,
                    entry.last_ms,
                    entry.avg_ms,
                    entry.max_ms,
                    entry.calls,
                ));
                return;
            }

            let header_text = perf_stat_label(
                entry.name,
                entry.last_ms,
                entry.avg_ms,
                entry.max_ms,
                entry.calls,
            );
            egui::CollapsingHeader::new(header_text)
                .default_open(matches!(
                    entry.name,
                    "DirtyProcess" | "BlockMeshRebuild" | "SelectClick"
                ))
                .show(ui, |ui| {
                    for child in &entry.children {
                        show_perf_entry(ui, child);
                    }
                });
        }

        egui::Area::new("editor_perf_overlay".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_black_alpha(210))
                    .show(ui, |ui| {
                        ui.monospace("Perf Overlay (Ctrl+Shift+Alt+F12)");
                        ui.monospace(format!("FPS {:.1}", view.fps));
                        for line in view.perf_overlay_lines.iter().take(1) {
                            ui.monospace(line);
                        }
                        ui.separator();
                        for entry in &view.perf_overlay_entries {
                            show_perf_entry(ui, entry);
                        }
                    });
            });
    }

    drop(view);
    for command in commands {
        state.dispatch(command);
    }
}
