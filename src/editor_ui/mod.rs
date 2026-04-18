/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) mod components;
pub(crate) mod menu;
pub(crate) mod modes;

use std::collections::HashMap;

use crate::commands::AppCommand;
use crate::editor_ui::components::{show_timeline_bar, show_waveform_panel, timeline_metrics};
use crate::editor_ui::modes::compose::show_compose_mode_bottom_panel;
use crate::editor_ui::modes::timing::show_timing_mode_bottom_panel;
use crate::editor_ui::modes::trigger::show_trigger_mode_bottom_panel;
use crate::types::{essential_keybind_actions, format_key_chord, EditorMode, SettingsSection};
use crate::State;
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use glam::{Mat3, Vec3};
pub use menu::{
    load_menu_favicon_texture, show_menu_favicon_ui, show_menu_play_ui, show_menu_topbar,
};

#[derive(Clone, Copy)]
struct ViewCubeFace {
    label: &'static str,
    normal: Vec3,
    indices: [usize; 4],
    rotation: f32,
    pitch: f32,
}

const VIEW_CUBE_FACES: [ViewCubeFace; 6] = [
    ViewCubeFace {
        label: "Front",
        normal: Vec3::Z,
        indices: [4, 5, 6, 7],
        rotation: std::f32::consts::PI,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Back",
        normal: Vec3::new(0.0, 0.0, -1.0),
        indices: [0, 1, 2, 3],
        rotation: 0.0,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Left",
        normal: Vec3::new(-1.0, 0.0, 0.0),
        indices: [0, 3, 7, 4],
        rotation: std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Right",
        normal: Vec3::X,
        indices: [2, 1, 5, 6],
        rotation: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Top",
        normal: Vec3::Y,
        indices: [3, 2, 6, 7],
        rotation: 0.0,
        pitch: 89.0f32.to_radians(),
    },
    ViewCubeFace {
        label: "Bottom",
        normal: Vec3::new(0.0, -1.0, 0.0),
        indices: [1, 0, 4, 5],
        rotation: 0.0,
        pitch: -89.0f32.to_radians(),
    },
];

const COMPACT_EDITOR_UI_BREAKPOINT: f32 = 720.0;
const SETTINGS_SIDEBAR_TOTAL_PADDING: f32 = 24.0;
const SETTINGS_SIDEBAR_MIN_WIDTH: f32 = 180.0;
const SETTINGS_SIDEBAR_WIDTH_SCALE: f32 = 0.78;
const RESPONSIVE_UI_SCALE_MIN: f32 = 0.8;
const RESPONSIVE_UI_SCALE_MAX: f32 = 1.35;
const UI_SCALE_BASE_WIDTH: f32 = 1280.0;
const UI_SCALE_BASE_HEIGHT: f32 = 720.0;

fn is_compact_editor_ui(viewport_width: f32) -> bool {
    viewport_width <= COMPACT_EDITOR_UI_BREAKPOINT
}

fn settings_sidebar_default_width(viewport_width: f32) -> f32 {
    // Keep a comfortable 78% default on small screens while leaving 24px total padding
    // (12px per side), and use a 180px minimum only when it still fits inside the viewport.
    let max_width = (viewport_width - SETTINGS_SIDEBAR_TOTAL_PADDING).max(0.0);
    if max_width <= SETTINGS_SIDEBAR_MIN_WIDTH {
        return max_width;
    }
    (viewport_width * SETTINGS_SIDEBAR_WIDTH_SCALE).clamp(SETTINGS_SIDEBAR_MIN_WIDTH, max_width)
}

pub(crate) fn responsive_ui_scale_multiplier(viewport_size: egui::Vec2) -> f32 {
    if !viewport_size.x.is_finite()
        || !viewport_size.y.is_finite()
        || viewport_size.x <= 0.0
        || viewport_size.y <= 0.0
    {
        return 1.0;
    }

    let width_scale = viewport_size.x / UI_SCALE_BASE_WIDTH;
    let height_scale = viewport_size.y / UI_SCALE_BASE_HEIGHT;
    width_scale
        .min(height_scale)
        .clamp(RESPONSIVE_UI_SCALE_MIN, RESPONSIVE_UI_SCALE_MAX)
}

pub(crate) fn combined_ui_scale_factor(viewport_size: egui::Vec2, user_multiplier: f32) -> f32 {
    let responsive = responsive_ui_scale_multiplier(viewport_size);
    let user = crate::types::AppSettings::clamp_ui_scale_multiplier(user_multiplier);
    (responsive * user).clamp(0.5, 4.0)
}

fn sort_quad_by_angle(center: egui::Pos2, quad: [egui::Pos2; 4]) -> [egui::Pos2; 4] {
    let mut points = quad.to_vec();
    points.sort_by(|a, b| {
        let angle_a = (a.y - center.y).atan2(a.x - center.x);
        let angle_b = (b.y - center.y).atan2(b.x - center.x);
        angle_a.total_cmp(&angle_b)
    });
    [points[0], points[1], points[2], points[3]]
}

fn add_face_mesh(painter: &egui::Painter, quad: [egui::Pos2; 4], color: egui::Color32) {
    let mut mesh = Mesh::default();
    for pos in quad {
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

fn marquee_screen_pos_to_egui_pos(screen_pos: [f64; 2], pixels_per_point: f32) -> egui::Pos2 {
    let scale = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };

    egui::pos2(screen_pos[0] as f32 / scale, screen_pos[1] as f32 / scale)
}

/// Shows the editor UI using egui.
/// Handles the top bar, bottom panels, and other editor interface elements.
pub fn show_editor_ui(
    ctx: &egui::Context,
    state: &mut State,
    block_icon_texture_ids: &HashMap<String, egui::TextureId>,
) {
    if !state.is_editor() {
        return;
    }

    let view = state.editor_ui_view_model();
    let mut commands = Vec::<AppCommand>::new();

    let mode = view.mode;
    let last_mode = view.last_mode;
    let is_timing = mode == EditorMode::Timing
        || (mode == EditorMode::Null && last_mode == Some(EditorMode::Timing));
    let is_compose = mode.is_compose_mode() && !is_timing;
    let viewport_width = ctx.content_rect().width();
    let is_compact_ui = is_compact_editor_ui(viewport_width);

    if view.show_settings {
        egui::SidePanel::left("editor_settings_sidebar")
            .resizable(true)
            .default_width(settings_sidebar_default_width(viewport_width))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(format!("{} Settings", egui_phosphor::regular::GEAR));
                    if ui.button(egui_phosphor::regular::X).clicked() {
                        commands.push(crate::commands::AppCommand::EditorSetShowSettings(false));
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            view.settings_section == SettingsSection::Backends,
                            format!("{} Backends", egui_phosphor::regular::MONITOR),
                        )
                        .clicked()
                    {
                        commands.push(crate::commands::AppCommand::EditorSetSettingsSection(
                            SettingsSection::Backends,
                        ));
                    }

                    if ui
                        .selectable_label(
                            view.settings_section == SettingsSection::Keybinds,
                            format!("{} Keybinds", egui_phosphor::regular::KEYBOARD),
                        )
                        .clicked()
                    {
                        commands.push(crate::commands::AppCommand::EditorSetSettingsSection(
                            SettingsSection::Keybinds,
                        ));
                    }
                });

                ui.separator();

                match view.settings_section {
                    SettingsSection::Backends => {
                        ui.label(format!(
                            "{} Graphics backend",
                            egui_phosphor::regular::DESKTOP
                        ));
                        let mut graphics_choice = view.configured_graphics_backend.to_string();
                        egui::ComboBox::from_id_salt("settings_graphics_backend")
                            .selected_text(graphics_choice.as_str())
                            .show_ui(ui, |ui| {
                                for backend in view.graphics_backend_options {
                                    if ui
                                        .selectable_label(graphics_choice == *backend, backend)
                                        .clicked()
                                    {
                                        graphics_choice = backend.clone();
                                    }
                                }
                            });
                        if graphics_choice != view.configured_graphics_backend {
                            commands.push(crate::commands::AppCommand::EditorSetGraphicsBackend(
                                graphics_choice,
                            ));
                        }

                        if view.settings_restart_required {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 196, 96),
                                format!(
                                    "{} Graphics backend change will apply after restart.",
                                    egui_phosphor::regular::WARNING
                                ),
                            );
                        }

                        ui.separator();
                        ui.label(format!(
                            "{} Audio backend",
                            egui_phosphor::regular::SPEAKER_HIGH
                        ));
                        let mut audio_choice = view.configured_audio_backend.to_string();
                        egui::ComboBox::from_id_salt("settings_audio_backend")
                            .selected_text(audio_choice.as_str())
                            .show_ui(ui, |ui| {
                                for backend in view.audio_backend_options {
                                    if ui
                                        .selectable_label(audio_choice == *backend, backend)
                                        .clicked()
                                    {
                                        audio_choice = backend.clone();
                                    }
                                }
                            });
                        if audio_choice != view.configured_audio_backend {
                            commands.push(crate::commands::AppCommand::EditorSetAudioBackend(
                                audio_choice,
                            ));
                        }

                        ui.separator();
                        ui.label(format!("{} UI Scale", egui_phosphor::regular::GEAR));
                        let mut ui_scale_multiplier = view.app_settings.normalized_ui_scale_multiplier();
                        if ui
                            .add(
                                egui::DragValue::new(&mut ui_scale_multiplier)
                                    .speed(0.05)
                                    .range(0.5..=3.0)
                                    .suffix("x"),
                            )
                            .changed()
                        {
                            commands.push(crate::commands::AppCommand::EditorSetUiScaleMultiplier(
                                ui_scale_multiplier,
                            ));
                        }
                    }
                    SettingsSection::Keybinds => {
                        let mut grouped_actions: std::collections::BTreeMap<
                            &'static str,
                            Vec<&'static crate::types::KeybindActionMetadata>,
                        > = std::collections::BTreeMap::new();
                        for metadata in essential_keybind_actions() {
                            grouped_actions
                                .entry(metadata.group)
                                .or_default()
                                .push(metadata);
                        }

                        let default_keybinds = crate::types::default_essential_keybinds();

                        egui::ScrollArea::vertical()
                            .id_salt("keybinds_scroll")
                            .show(ui, |ui| {
                                for (group, actions) in grouped_actions {
                                    ui.collapsing(group, |ui| {
                                        for metadata in actions {
                                            let action = metadata.action;
                                            let label = metadata.label;
                                            let capacity = metadata.capacity;

                                            let current_chords =
                                                view.app_settings.keybinds_for_action(action);
                                            let default_chords: Vec<_> = default_keybinds
                                                .iter()
                                                .filter(|b| b.action == action)
                                                .map(|b| &b.chord)
                                                .collect();

                                            let is_not_default = current_chords != default_chords;

                                            ui.horizontal(|ui| {
                                                ui.label(label);
                                                if is_not_default
                                                    && ui
                                                        .button(
                                                            egui_phosphor::regular::ARROW_CLOCKWISE,
                                                        )
                                                        .on_hover_text("Reset to default")
                                                        .clicked()
                                                {
                                                    commands.push(
                                                        crate::commands::AppCommand::EditorResetKeybind(
                                                            action.to_string(),
                                                        ),
                                                    );
                                                    commands.push(
                                                        crate::commands::AppCommand::EditorSetKeybindCapture(
                                                            None,
                                                        ),
                                                    );
                                                }

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        for slot in (0..capacity).rev() {
                                                            let chord = current_chords.get(slot);
                                                            let key_label = chord
                                                                .map(|c| format_key_chord(c))
                                                                .unwrap_or_else(|| {
                                                                    "Unbound".to_string()
                                                                });

                                                            let is_capturing = view
                                                                .keybind_capture_action
                                                                == Some(&(
                                                                    action.to_string(),
                                                                    slot,
                                                                ));

                                                            if chord.is_some()
                                                                && ui
                                                                    .button(egui_phosphor::regular::X)
                                                                    .clicked()
                                                            {
                                                                commands.push(
                                                                    crate::commands::AppCommand::EditorClearKeybindSlot {
                                                                        action: action.to_string(),
                                                                        slot,
                                                                    },
                                                                );
                                                                commands.push(
                                                                    crate::commands::AppCommand::EditorSetKeybindCapture(
                                                                        None,
                                                                    ),
                                                                );
                                                            }

                                                            let (bg_color, display_label) =
                                                                if is_capturing {
                                                                    (
                                                                        egui::Color32::from_rgb(
                                                                            0, 120, 215,
                                                                        ),
                                                                        "...".to_string(),
                                                                    )
                                                                } else {
                                                                    (
                                                                        egui::Color32::from_gray(
                                                                            32,
                                                                        ),
                                                                        key_label,
                                                                    )
                                                                };

                                                            if ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        egui::RichText::new(
                                                                            display_label,
                                                                        )
                                                                        .monospace(),
                                                                    )
                                                                    .fill(bg_color),
                                                                )
                                                                .clicked()
                                                            {
                                                                if is_capturing {
                                                                    commands.push(crate::commands::AppCommand::EditorSetKeybindCapture(None));
                                                                } else {
                                                                    commands.push(crate::commands::AppCommand::EditorSetKeybindCapture(Some((action.to_string(), slot))));
                                                                }
                                                            }
                                                        }
                                                    },
                                                );
                                            });
                                        }
                                    });
                                }

                                ui.separator();
                                if ui.button("Reset to Defaults").clicked() {
                                    commands.push(crate::commands::AppCommand::EditorResetKeybinds);
                                }
                            });
                    }
                }
            });
    }

    egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            // Top-level tabs: Compose / Timing
            if ui.selectable_label(is_compose, "Compose").clicked() && !is_compose {
                commands.push(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Place,
                ));
                ui.ctx().request_discard("editor top tab changed");
            }
            if ui.selectable_label(is_timing, "Timing").clicked() && !is_timing {
                commands.push(crate::commands::AppCommand::EditorSetMode(
                    EditorMode::Timing,
                ));
                ui.ctx().request_discard("editor top tab changed");
            }

            ui.separator();

            ui.label(format!("{} Level:", egui_phosphor::regular::MAP_TRIFOLD));

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

            if ui
                .button(format!("{} Export .egz", egui_phosphor::regular::DOWNLOAD))
                .clicked()
            {
                commands.push(crate::commands::AppCommand::EditorExportLevel);
            }

            if ui
                .button(format!(
                    "{} Import .egz/Binary",
                    egui_phosphor::regular::UPLOAD
                ))
                .clicked()
            {
                commands.push(crate::commands::AppCommand::EditorSetShowImport(true));
            }

            if ui
                .button(format!("{} Metadata", egui_phosphor::regular::INFO))
                .clicked()
            {
                commands.push(crate::commands::AppCommand::EditorSetShowMetadata(true));
            }

            if ui
                .button(format!("{} Settings", egui_phosphor::regular::GEAR))
                .clicked()
            {
                commands.push(crate::commands::AppCommand::EditorToggleSettings);
            }
        });
    });

    if view.show_metadata {
        egui::Window::new(format!("{} Level Metadata", egui_phosphor::regular::INFO)).show(
            ctx,
            |ui| {
                ui.label(format!("{} Level Name:", egui_phosphor::regular::PENCIL));
                let mut name = view.level_name.unwrap_or("Untitled").to_string();
                if ui.text_edit_singleline(&mut name).changed() {
                    commands.push(crate::commands::AppCommand::EditorRenameLevel(name));
                }

                ui.separator();
                ui.heading(format!("{} Music", egui_phosphor::regular::MUSIC_NOTE));

                let mut music = view.music_metadata.clone();
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label(format!("{} Source:", egui_phosphor::regular::GLOBE));
                    if ui.text_edit_singleline(&mut music.source).changed() {
                        changed = true;
                    }
                    if ui
                        .button(format!(
                            "{} Import External Audio",
                            egui_phosphor::regular::FILE_AUDIO
                        ))
                        .clicked()
                    {
                        commands.push(crate::commands::AppCommand::EditorTriggerAudioImport);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(format!("{} Title:", egui_phosphor::regular::TEXT_T));
                    let mut title = music.title.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut title).changed() {
                        music.title = Some(title);
                        changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(format!("{} Author:", egui_phosphor::regular::USER));
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
            },
        );
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

                if is_timing {
                    show_timing_mode_bottom_panel(ui, &view, duration_seconds, &mut commands);
                } else if view.mode == EditorMode::Trigger {
                    show_trigger_mode_bottom_panel(ui, &view, duration_seconds, &mut commands);
                } else {
                    show_compose_mode_bottom_panel(
                        ui,
                        &view,
                        block_icon_texture_ids,
                        duration_seconds,
                        &mut commands,
                    );
                }

                // Shared timeline bar with beat lines
                show_timeline_bar(ui, &view, duration_seconds, &mut commands);
            });
        });

    // Waveform visualization central panel (Timing mode only)
    if is_timing {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style()).fill(egui::Color32::from_rgb(15, 20, 28)),
            )
            .show(ctx, |ui| {
                show_waveform_panel(ui, &view, &mut commands);
            });
    }

    if !is_timing && !is_compact_ui {
        show_view_selector_cube(ctx, view.camera_rotation, view.camera_pitch, &mut commands);
    }

    if view.mode.is_selection_mode() || view.mode == EditorMode::Trigger {
        if let Some((start, current, is_active_drag)) = view.marquee_selection_rect_screen {
            if is_active_drag {
                let pixels_per_point = ctx.pixels_per_point();
                let rect = egui::Rect::from_two_pos(
                    marquee_screen_pos_to_egui_pos(start, pixels_per_point),
                    marquee_screen_pos_to_egui_pos(current, pixels_per_point),
                );
                let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(25, 153, 255));
                let layer = egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("editor_marquee_overlay"),
                );
                ctx.layer_painter(layer).rect(
                    rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(25, 153, 255, 38),
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
                        ui.monospace(format!(
                            "{} | {} | FPS {:.1}",
                            view.graphics_backend, view.audio_backend, view.fps
                        ));
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
    const ROTATE_SPEED: f32 = 0.004;
    const PITCH_SPEED: f32 = 0.006;

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

    fn convex_hull(points: &[egui::Pos2]) -> Vec<egui::Pos2> {
        if points.len() <= 3 {
            return points.to_vec();
        }

        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));

        fn cross(o: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
            (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
        }

        let mut lower: Vec<egui::Pos2> = Vec::new();
        for p in &sorted {
            while lower.len() >= 2
                && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0
            {
                lower.pop();
            }
            lower.push(*p);
        }

        let mut upper: Vec<egui::Pos2> = Vec::new();
        for p in sorted.iter().rev() {
            while upper.len() >= 2
                && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0
            {
                upper.pop();
            }
            upper.push(*p);
        }

        lower.pop();
        upper.pop();
        lower.extend(upper);
        lower
    }

    fn polygon_winding(points: &[egui::Pos2]) -> f32 {
        let mut area = 0.0;
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            area += (b.x - a.x) * (b.y + a.y);
        }
        area
    }

    fn rounded_convex_polygon(
        points: &[egui::Pos2],
        radius: f32,
        arc_segments: usize,
    ) -> Vec<egui::Pos2> {
        if points.len() < 3 || radius <= 0.0 || arc_segments == 0 {
            return points.to_vec();
        }

        let winding = polygon_winding(points);
        let ccw = winding < 0.0;
        let mut rounded = Vec::new();

        for i in 0..points.len() {
            let prev = points[(i + points.len() - 1) % points.len()];
            let curr = points[i];
            let next = points[(i + 1) % points.len()];

            let v1 = (prev - curr).normalized();
            let v2 = (next - curr).normalized();

            let dot = (v1.x * v2.x + v1.y * v2.y).clamp(-1.0, 1.0);
            let angle = dot.acos();
            if angle <= f32::EPSILON {
                rounded.push(curr);
                continue;
            }

            let offset = radius / (angle * 0.5).tan();
            let offset = offset.min((prev - curr).length() * 0.5);
            let offset = offset.min((next - curr).length() * 0.5);

            let tangent1 = curr + v1 * offset;
            let tangent2 = curr + v2 * offset;
            let bisector = (v1 + v2).normalized();
            if bisector.length_sq() <= f32::EPSILON {
                rounded.push(curr);
                continue;
            }
            let center_distance = radius / (angle * 0.5).sin();
            let center = curr + bisector * center_distance;

            let start_angle = (tangent1.y - center.y).atan2(tangent1.x - center.x);
            let mut end_angle = (tangent2.y - center.y).atan2(tangent2.x - center.x);

            if ccw {
                if end_angle < start_angle {
                    end_angle += std::f32::consts::TAU;
                }
            } else if end_angle > start_angle {
                end_angle -= std::f32::consts::TAU;
            }

            rounded.push(tangent1);
            let step = (end_angle - start_angle) / arc_segments as f32;
            for s in 1..arc_segments {
                let angle = start_angle + step * s as f32;
                rounded.push(egui::pos2(
                    center.x + angle.cos() * radius,
                    center.y + angle.sin() * radius,
                ));
            }
            rounded.push(tangent2);
        }

        rounded
    }

    egui::Area::new("editor_view_selector_cube".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 52.0))
        .show(ctx, |ui| {
            let side = 128.0;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);

            painter.rect_filled(
                rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(16, 24, 32, 56),
            );

            let pitch = camera_pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
            let horizontal = pitch.cos();
            let offset =
                Mat3::from_rotation_y(camera_rotation) * Vec3::new(0.0, pitch.sin(), -horizontal);
            let forward = (-offset).normalize_or_zero();

            if forward.length_squared() <= f32::EPSILON {
                return;
            }

            let to_camera = -forward;
            let mut right = forward.cross(Vec3::Y);
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

            let hull = convex_hull(&projected);
            if hull.len() >= 3 {
                let rounded_hull = rounded_convex_polygon(&hull, 6.0, 4);
                painter.add(egui::Shape::convex_polygon(
                    rounded_hull,
                    egui::Color32::from_rgba_unmultiplied(182, 214, 236, 72),
                    egui::Stroke::NONE,
                ));
            }

            let mut rendered_faces = Vec::<RenderedFace>::new();
            for face in VIEW_CUBE_FACES {
                let facing = face.normal.dot(to_camera);
                if facing <= 0.02 {
                    continue;
                }

                let face_inset: f32 = 1.2;
                let raw_poly = [
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
                    (raw_poly[0].x + raw_poly[1].x + raw_poly[2].x + raw_poly[3].x) * 0.25,
                    (raw_poly[0].y + raw_poly[1].y + raw_poly[2].y + raw_poly[3].y) * 0.25,
                );

                // sort to avoid aa seam artifacts
                let mut poly = [egui::Pos2::ZERO; 4];
                for i in 0..4 {
                    let point = raw_poly[i];
                    let to_center = center - point;
                    let distance = to_center.length();
                    poly[i] = if distance > f32::EPSILON {
                        point + to_center / distance * face_inset.min(distance * 0.5)
                    } else {
                        point
                    };
                }
                let poly = sort_quad_by_angle(center, poly);
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
                let face_color = egui::Color32::from_rgba_unmultiplied(
                    182,
                    214,
                    236,
                    alpha.saturating_add(hover_boost),
                );
                add_face_mesh(&painter, face.polygon, face_color);
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

            let axis_origin = projected[0];
            let axes = [
                ("X", projected[1], egui::Color32::from_rgb(240, 104, 104)),
                ("Y", projected[3], egui::Color32::from_rgb(116, 232, 152)),
                ("Z", projected[4], egui::Color32::from_rgb(104, 154, 255)),
            ];
            for (label, tip, color) in axes {
                painter.line_segment([axis_origin, tip], egui::Stroke::new(1.6, color));

                let direction = tip - axis_origin;
                let direction_len = direction.length();
                let label_pos = if direction_len > f32::EPSILON {
                    tip + direction * (6.0 / direction_len)
                } else {
                    tip
                };
                painter.text(
                    label_pos,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    color,
                );
            }

            let dragging_cube = response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Secondary);
            if dragging_cube {
                let pointer_delta = ui.input(|input| input.pointer.delta());
                if pointer_delta != egui::Vec2::ZERO {
                    commands.push(AppCommand::EditorSetCameraOrientation {
                        rotation: camera_rotation - pointer_delta.x * ROTATE_SPEED,
                        pitch: camera_pitch + pointer_delta.y * PITCH_SPEED,
                        transition_seconds: None,
                    });
                }
            }

            if response.clicked() && !dragging_cube {
                if let Some(idx) = hovered_face {
                    let face = &rendered_faces[idx];
                    commands.push(AppCommand::EditorSetCameraOrientation {
                        rotation: face.rotation,
                        pitch: face.pitch,
                        transition_seconds: Some(0.25),
                    });
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        combined_ui_scale_factor, is_compact_editor_ui, marquee_screen_pos_to_egui_pos,
        responsive_ui_scale_multiplier, settings_sidebar_default_width, show_editor_ui,
        sort_quad_by_angle, VIEW_CUBE_FACES,
    };
    use crate::commands::AppCommand;
    use crate::test_utils::approx_eq;
    use crate::types::{EditorMode, SettingsSection};
    use glam::{Mat3, Vec3};

    fn camera_forward_from_orientation(rotation: f32, pitch: f32) -> Vec3 {
        let pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        let horizontal = pitch.cos();
        let offset = Mat3::from_rotation_y(rotation) * Vec3::new(0.0, pitch.sin(), -horizontal);
        (-offset).normalize_or_zero()
    }

    fn is_convex_quad(quad: &[egui::Pos2; 4]) -> bool {
        let mut sign = 0.0f32;
        for i in 0..4 {
            let a = quad[i];
            let b = quad[(i + 1) % 4];
            let c = quad[(i + 2) % 4];
            let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
            if cross.abs() <= 1.0e-6 {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if sign * cross < 0.0 {
                return false;
            }
        }
        sign != 0.0
    }

    fn run_editor_ui_once(state: &mut crate::State) {
        let ctx = egui::Context::default();
        let block_icon_texture_ids = std::collections::HashMap::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_editor_ui(ctx, state, &block_icon_texture_ids);
        });
    }

    #[test]
    fn sort_quad_by_angle_returns_convex_winding_for_scrambled_square() {
        let center = egui::pos2(0.0, 0.0);
        let quad = [
            egui::pos2(1.0, 1.0),
            egui::pos2(-1.0, 1.0),
            egui::pos2(1.0, -1.0),
            egui::pos2(-1.0, -1.0),
        ];
        let sorted = sort_quad_by_angle(center, quad);
        assert!(is_convex_quad(&sorted));
    }

    #[test]
    fn sort_quad_by_angle_handles_skewed_quads() {
        let center = egui::pos2(0.05, -0.12);
        let quad = [
            egui::pos2(1.3, 0.8),
            egui::pos2(-0.9, 1.1),
            egui::pos2(0.9, -1.4),
            egui::pos2(-1.1, -0.6),
        ];
        let sorted = sort_quad_by_angle(center, quad);
        assert!(is_convex_quad(&sorted));
    }

    #[test]
    fn view_cube_face_rotations_match_face_normals() {
        for face in VIEW_CUBE_FACES {
            let forward = camera_forward_from_orientation(face.rotation, face.pitch);
            let dot = forward.dot(face.normal.normalize_or_zero());
            assert!(
                dot < -0.999 || approx_eq(dot, -1.0, 0.002),
                "Face {} should align camera forward opposite the normal (dot={})",
                face.label,
                dot
            );
        }
    }

    #[test]
    fn compact_editor_ui_detects_small_viewports() {
        assert!(is_compact_editor_ui(720.0));
        assert!(is_compact_editor_ui(480.0));
        assert!(!is_compact_editor_ui(720.1));
        assert!(!is_compact_editor_ui(960.0));
    }

    #[test]
    fn settings_sidebar_width_scales_for_small_screens() {
        assert!(approx_eq(
            settings_sidebar_default_width(320.0),
            249.6,
            0.01
        ));
        assert!(approx_eq(
            settings_sidebar_default_width(200.0),
            176.0,
            0.01
        ));
        assert!(approx_eq(
            settings_sidebar_default_width(1200.0),
            936.0,
            0.01
        ));
        assert!(approx_eq(
            settings_sidebar_default_width(180.0),
            156.0,
            0.01
        ));
    }

    #[test]
    fn responsive_ui_scale_multiplier_tracks_screen_size() {
        assert!(approx_eq(
            responsive_ui_scale_multiplier(egui::vec2(1280.0, 720.0)),
            1.0,
            0.001
        ));
        assert!(approx_eq(
            responsive_ui_scale_multiplier(egui::vec2(320.0, 240.0)),
            0.8,
            0.001
        ));
        assert!(approx_eq(
            responsive_ui_scale_multiplier(egui::vec2(3840.0, 2160.0)),
            1.35,
            0.001
        ));
    }

    #[test]
    fn combined_ui_scale_factor_applies_user_multiplier_with_clamping() {
        assert!(approx_eq(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), 1.25),
            1.25,
            0.001
        ));
        assert!(approx_eq(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), 99.0),
            3.0,
            0.001
        ));
        assert!(approx_eq(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), f32::NAN),
            1.0,
            0.001
        ));
        assert!(approx_eq(
            combined_ui_scale_factor(egui::vec2(3840.0, 2160.0), 3.0),
            4.0,
            0.001
        ));
    }

    #[test]
    fn marquee_screen_pos_to_egui_pos_scales_by_pixels_per_point() {
        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], 1.5);
        assert!(approx_eq(p.x, 200.0, 0.001));
        assert!(approx_eq(p.y, 120.0, 0.001));
    }

    #[test]
    fn marquee_screen_pos_to_egui_pos_handles_invalid_scale() {
        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], f32::NAN);
        assert!(approx_eq(p.x, 300.0, 0.001));
        assert!(approx_eq(p.y, 180.0, 0.001));
    }

    #[test]
    fn show_editor_ui_returns_early_when_not_in_editor() {
        pollster::block_on(async {
            let Some(mut state) = crate::State::try_new_test().await else {
                return;
            };

            assert!(state.is_menu());
            run_editor_ui_once(&mut state);
            assert!(state.is_menu());
        });
    }

    #[test]
    fn show_editor_ui_composes_timing_compose_and_trigger_modes() {
        pollster::block_on(async {
            let Some(mut state) = crate::State::try_new_test().await else {
                return;
            };

            state.toggle_editor();
            assert!(state.is_editor());

            state.dispatch(AppCommand::EditorSetMode(EditorMode::Timing));
            state.dispatch(AppCommand::EditorSetShowSettings(true));
            state.dispatch(AppCommand::EditorSetSettingsSection(
                SettingsSection::Backends,
            ));
            state.dispatch(AppCommand::EditorSetShowMetadata(true));
            state.dispatch(AppCommand::EditorSetShowImport(true));
            state.dispatch(AppCommand::EditorTogglePerfOverlay);
            run_editor_ui_once(&mut state);

            assert_eq!(state.editor_mode(), EditorMode::Timing);
            assert!(state.editor_show_settings());
            assert_eq!(state.editor_settings_section(), SettingsSection::Backends);
            assert!(state.editor_show_metadata());
            assert!(state.editor_show_import());
            assert!(state.editor_perf_overlay_enabled());

            state.dispatch(AppCommand::EditorSetMode(EditorMode::Place));
            state.dispatch(AppCommand::EditorSetSettingsSection(
                SettingsSection::Keybinds,
            ));
            run_editor_ui_once(&mut state);

            assert_eq!(state.editor_mode(), EditorMode::Place);
            assert_eq!(state.editor_settings_section(), SettingsSection::Keybinds);

            state.dispatch(AppCommand::EditorSetMode(EditorMode::Trigger));
            run_editor_ui_once(&mut state);

            assert_eq!(state.editor_mode(), EditorMode::Trigger);
            assert!(state.is_editor());
        });
    }
}
