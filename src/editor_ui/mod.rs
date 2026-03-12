pub(crate) mod components;
pub(crate) mod menu;
pub(crate) mod modes;

use crate::commands::AppCommand;
use crate::editor_ui::components::{show_timeline_bar, show_waveform_panel, timeline_metrics};
use crate::editor_ui::modes::compose::show_compose_mode_bottom_panel;
use crate::editor_ui::modes::timing::show_timing_mode_bottom_panel;
use crate::types::EditorMode;
use crate::State;
use glam::{Mat3, Vec3};
pub use menu::{load_menu_wordmark_texture, show_menu_wordmark_ui, show_splash_screen_ui};

#[derive(Clone, Copy)]
struct ViewCubeFace {
    label: &'static str,
    normal: Vec3,
    indices: [usize; 4],
    rotation: f32,
    pitch: f32,
}

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

            if ui.button("Export .egz").clicked() {
                commands.push(crate::commands::AppCommand::EditorExportLevel);
            }

            if ui.button("Import .egz/JSON").clicked() {
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
            ui.label("Paste level JSON or Base64 egz below:");
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

    if view.mode != EditorMode::Timing {
        show_view_selector_cube(ctx, view.camera_rotation, view.camera_pitch, &mut commands);
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

fn show_view_selector_cube(
    ctx: &egui::Context,
    camera_rotation: f32,
    camera_pitch: f32,
    commands: &mut Vec<AppCommand>,
) {
    const FACE_SET: [ViewCubeFace; 6] = [
        ViewCubeFace {
            label: "Front",
            normal: Vec3::Y,
            indices: [3, 2, 6, 7],
            rotation: 0.0,
            pitch: 0.0,
        },
        ViewCubeFace {
            label: "Back",
            normal: Vec3::new(0.0, -1.0, 0.0),
            indices: [1, 0, 4, 5],
            rotation: std::f32::consts::PI,
            pitch: 0.0,
        },
        ViewCubeFace {
            label: "Left",
            normal: Vec3::new(-1.0, 0.0, 0.0),
            indices: [0, 3, 7, 4],
            rotation: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
        },
        ViewCubeFace {
            label: "Right",
            normal: Vec3::X,
            indices: [2, 1, 5, 6],
            rotation: std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
        },
        ViewCubeFace {
            label: "Top",
            normal: Vec3::Z,
            indices: [7, 6, 5, 4],
            rotation: 0.0,
            pitch: 89.0f32.to_radians(),
        },
        ViewCubeFace {
            label: "Bottom",
            normal: Vec3::new(0.0, 0.0, -1.0),
            indices: [0, 1, 2, 3],
            rotation: 0.0,
            pitch: -89.0f32.to_radians(),
        },
    ];

    struct RenderedFace {
        label: &'static str,
        polygon: [egui::Pos2; 4],
        center: egui::Pos2,
        avg_depth: f32,
        facing: f32,
        rotation: f32,
        pitch: f32,
    }

    fn point_in_quad(point: egui::Pos2, quad: &[egui::Pos2; 4]) -> bool {
        let mut last_non_zero_sign = 0.0;
        for i in 0..4 {
            let a = quad[i];
            let b = quad[(i + 1) % 4];
            let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
            if cross.abs() <= f32::EPSILON {
                continue;
            }
            if last_non_zero_sign == 0.0 {
                last_non_zero_sign = cross.signum();
                continue;
            }
            if last_non_zero_sign * cross < 0.0 {
                return false;
            }
        }
        true
    }

    egui::Area::new("editor_view_selector_cube".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 52.0))
        .show(ctx, |ui| {
            let side = 128.0;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
            let painter = ui.painter_at(rect);

            painter.rect_filled(
                rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(16, 24, 32, 56),
            );

            let pitch = camera_pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
            let horizontal = pitch.cos();
            let offset =
                Mat3::from_rotation_z(camera_rotation) * Vec3::new(0.0, -horizontal, pitch.sin());
            let forward = (-offset).normalize_or_zero();

            if forward.length_squared() <= f32::EPSILON {
                return;
            }

            let to_camera = -forward;
            let mut right = forward.cross(Vec3::Z);
            if right.length_squared() <= f32::EPSILON {
                right = Vec3::X;
            } else {
                right = right.normalize();
            }
            let up = right.cross(forward).normalize();

            let cube_vertices = [
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(-1.0, -1.0, 1.0),
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(-1.0, 1.0, 1.0),
            ];

            let center = rect.center();
            let cube_radius = side * 0.23;
            let perspective = 3.6;

            let mut projected = [egui::Pos2::ZERO; 8];
            let mut depth = [0.0; 8];
            for (idx, vertex) in cube_vertices.iter().enumerate() {
                let local_x = vertex.dot(right);
                let local_y = vertex.dot(up);
                let local_z = vertex.dot(to_camera);
                let scale = perspective / (perspective - local_z);
                projected[idx] = egui::pos2(
                    center.x + local_x * cube_radius * scale,
                    center.y - local_y * cube_radius * scale,
                );
                depth[idx] = local_z;
            }

            let mut rendered_faces = Vec::<RenderedFace>::new();
            for face in FACE_SET {
                let facing = face.normal.dot(to_camera);
                if facing <= 0.02 {
                    continue;
                }

                let poly = [
                    projected[face.indices[0]],
                    projected[face.indices[1]],
                    projected[face.indices[2]],
                    projected[face.indices[3]],
                ];
                let avg_depth = (depth[face.indices[0]]
                    + depth[face.indices[1]]
                    + depth[face.indices[2]]
                    + depth[face.indices[3]])
                    * 0.25;
                let center = egui::pos2(
                    (poly[0].x + poly[1].x + poly[2].x + poly[3].x) * 0.25,
                    (poly[0].y + poly[1].y + poly[2].y + poly[3].y) * 0.25,
                );
                rendered_faces.push(RenderedFace {
                    label: face.label,
                    polygon: poly,
                    center,
                    avg_depth,
                    facing,
                    rotation: face.rotation,
                    pitch: face.pitch,
                });
            }

            rendered_faces.sort_by(|a, b| a.avg_depth.total_cmp(&b.avg_depth));

            let pointer_pos = response.interact_pointer_pos();
            let mut hovered_face: Option<usize> = None;
            if let Some(pointer) = pointer_pos {
                for idx in (0..rendered_faces.len()).rev() {
                    if point_in_quad(pointer, &rendered_faces[idx].polygon) {
                        hovered_face = Some(idx);
                        break;
                    }
                }
            }

            for (idx, face) in rendered_faces.iter().enumerate() {
                let hover_boost = if hovered_face == Some(idx) { 28 } else { 0 };
                let alpha = (40.0 + face.facing * 90.0).round() as u8;
                painter.add(egui::Shape::convex_polygon(
                    face.polygon.to_vec(),
                    egui::Color32::from_rgba_unmultiplied(
                        182,
                        214,
                        236,
                        alpha.saturating_add(hover_boost),
                    ),
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(220, 240, 255, 180),
                    ),
                ));
                if face.facing > 0.2 {
                    painter.text(
                        face.center,
                        egui::Align2::CENTER_CENTER,
                        face.label,
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgba_unmultiplied(235, 246, 255, 220),
                    );
                }
            }

            let axis_origin = egui::pos2(rect.left() + 22.0, rect.bottom() - 22.0);
            let axis_len = 16.0;
            let axes = [
                ("X", Vec3::X, egui::Color32::from_rgb(240, 104, 104)),
                ("Y", Vec3::Y, egui::Color32::from_rgb(116, 232, 152)),
                ("Z", Vec3::Z, egui::Color32::from_rgb(104, 154, 255)),
            ];
            for (label, axis, color) in axes {
                let tip = axis_origin + egui::vec2(axis.dot(right), -axis.dot(up)) * axis_len;
                painter.line_segment([axis_origin, tip], egui::Stroke::new(1.6, color));
                painter.text(
                    tip,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    color,
                );
            }

            if response.clicked() {
                if let Some(idx) = hovered_face {
                    let face = &rendered_faces[idx];
                    commands.push(AppCommand::EditorSetCameraOrientation {
                        rotation: face.rotation,
                        pitch: face.pitch,
                    });
                }
            }
        });
}
