/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) mod components;
pub(crate) mod flame_graph;
pub(crate) mod menu;
pub(crate) mod modes;

use std::collections::HashMap;

use crate::commands::AppCommand;
use crate::editor_ui::components::{
    show_perf_profiler_header, show_timeline_bar, show_waveform_panel, timeline_metrics,
};
use crate::editor_ui::flame_graph::show_perf_flame_graph_panel;
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
const VIEW_CUBE_TOP_MARGIN: f32 = 12.0;

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

fn view_cube_top_offset_y(top_panel_height: f32) -> f32 {
    top_panel_height + VIEW_CUBE_TOP_MARGIN
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

fn perf_frame_bar_color(frame_ms: f32) -> egui::Color32 {
    if frame_ms <= 16.7 {
        egui::Color32::from_rgb(56, 173, 95)
    } else if frame_ms <= 33.3 {
        egui::Color32::from_rgb(219, 173, 68)
    } else {
        egui::Color32::from_rgb(214, 84, 84)
    }
}

fn perf_histogram_bar_dimensions(zoom: f32) -> (f32, f32) {
    let step = (4.0 * zoom).clamp(1.0, 22.0);
    let width = (step - 1.0).clamp(1.0, 18.0);
    (step, width)
}

fn perf_visible_history_range(
    history_len: usize,
    visible_bars: usize,
    follow_latest: bool,
    focus_index: Option<usize>,
) -> (isize, isize) {
    if history_len == 0 {
        return (0, 0);
    }

    let visible = visible_bars.max(1) as isize;
    let latest = history_len.saturating_sub(1) as isize;
    let center = if follow_latest {
        latest
    } else {
        focus_index.unwrap_or(latest as usize).min(latest as usize) as isize
    };

    let start = center - (visible / 2);
    let end = start + visible;
    (start, end)
}

fn perf_history_index_from_pointer(
    rect: egui::Rect,
    view_start: isize,
    visible_bars: usize,
    history_len: usize,
    pointer: egui::Pos2,
    bar_step: f32,
) -> Option<usize> {
    if rect.width() <= 0.0 || bar_step <= 0.0 {
        return None;
    }

    let offset_x = pointer.x - rect.left();
    if offset_x < 0.0 {
        return None;
    }

    let slot = (offset_x / bar_step).floor() as isize;
    if slot < 0 || slot >= visible_bars as isize {
        return None;
    }

    let index = view_start + slot;
    if index < 0 {
        return None;
    }

    let index = index as usize;
    if index < history_len {
        Some(index)
    } else {
        None
    }
}

fn show_perf_microprofiler_overlay(
    ctx: &egui::Context,
    view: &crate::state::EditorUiViewModel<'_>,
    is_compact_ui: bool,
    commands: &mut Vec<AppCommand>,
) {
    let viewport = ctx.content_rect();
    if viewport.width() <= 1.0 || viewport.height() <= 1.0 {
        return;
    }

    egui::Area::new("editor_perf_microprofiler_overlay".into())
        .order(egui::Order::Foreground)
        .interactable(true)
        .fixed_pos(viewport.min)
        .show(ctx, |ui| {
            ui.set_min_size(viewport.size());
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(8, 12, 16, 234))
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_min_size(viewport.size());

                    show_perf_profiler_header(ui, view, commands);

                    let content_margin = if is_compact_ui { 8 } else { 12 };
                    egui::Frame::default()
                        .inner_margin(egui::Margin {
                            left: 0,
                            right: 0,
                            top: 0,
                            bottom: content_margin,
                        })
                        .show(ui, |ui| {
                            let history = &view.perf_frame_history;
                            let graph_height = if is_compact_ui {
                                (viewport.height() * 0.065).clamp(30.0, 60.0)
                            } else {
                                (viewport.height() * 0.085).clamp(40.0, 90.0)
                            };
                            // Keep histogram width tied to the actual viewport to avoid
                            // unconstrained UI width growth pushing bars off-screen.
                            let graph_width = viewport.width().max(180.0);
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(graph_width, graph_height),
                                egui::Sense::click_and_drag(),
                            );
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 24, 30));

                            if history.is_empty() {
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "No frames captured yet",
                                    egui::FontId::monospace(12.0),
                                    egui::Color32::from_gray(185),
                                );
                            } else {
                                let (bar_step, bar_width) = perf_histogram_bar_dimensions(1.0);
                                let visible_bars = ((rect.width() / bar_step).floor() as usize)
                                    .max(1);
                                let (view_start, view_end) = perf_visible_history_range(
                                    history.len(),
                                    visible_bars,
                                    view.perf_histogram_follow_latest,
                                    view.perf_histogram_focus_index,
                                );
                                let visible_start = view_start.max(0) as usize;
                                let visible_end = view_end
                                    .min(history.len() as isize)
                                    .max(view_start.max(0)) as usize;
                                let max_ms = 33.33f32;

                                for guide_ms in [16.67, 33.33] {
                                    if guide_ms > max_ms {
                                        continue;
                                    }

                                    let ratio = (guide_ms / max_ms).clamp(0.0, 1.0);
                                    let y = egui::lerp(rect.bottom()..=rect.top(), ratio);
                                    painter.line_segment(
                                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                                        egui::Stroke::new(1.0, egui::Color32::from_gray(90)),
                                    );
                                    painter.text(
                                        egui::pos2(rect.left() + 4.0, y - 1.0),
                                        egui::Align2::LEFT_BOTTOM,
                                        format!("{guide_ms:.1}ms"),
                                        egui::FontId::monospace(9.0),
                                        egui::Color32::from_gray(145),
                                    );
                                }

                                if visible_end > visible_start {
                                    for (offset, frame) in history
                                        .range(visible_start..=visible_end - 1)
                                        .enumerate()
                                    {
                                        let history_index = visible_start + offset;
                                        let slot = (history_index as isize - view_start) as usize;
                                        let x_min = rect.left() + slot as f32 * bar_step;
                                        let x_max = (x_min + bar_width).min(rect.right() - 1.0);
                                        if x_max <= x_min {
                                            continue;
                                        }

                                        let ratio = (frame.frame_time_ms / max_ms).clamp(0.0, 1.0);
                                        let y_min = egui::lerp(rect.bottom()..=rect.top(), ratio);
                                        let bar_rect = egui::Rect::from_min_max(
                                            egui::pos2(x_min, y_min),
                                            egui::pos2(x_max, rect.bottom() - 1.0),
                                        );
                                        painter.rect_filled(
                                            bar_rect,
                                            0.0,
                                            perf_frame_bar_color(frame.frame_time_ms),
                                        );
                                    }
                                }

                                if let Some((range_start, range_end)) = view.perf_selected_history_range {
                                    let in_view_start = range_start.max(visible_start);
                                    let in_view_end = range_end.min(visible_end.saturating_sub(1));

                                    if visible_end > visible_start && in_view_start <= in_view_end {
                                        let slot_start =
                                            (in_view_start as isize - view_start) as usize;
                                        let slot_end = (in_view_end as isize - view_start) as usize;

                                        let x_min = rect.left() + slot_start as f32 * bar_step;
                                        let x_max =
                                            (rect.left() + slot_end as f32 * bar_step + bar_width)
                                                .min(rect.right() - 1.0);

                                        let selection_rect = egui::Rect::from_min_max(
                                            egui::pos2(x_min, rect.top() + 1.0),
                                            egui::pos2(x_max, rect.bottom() - 1.0),
                                        );

                                        painter.rect_filled(
                                            selection_rect,
                                            0.0,
                                            egui::Color32::from_rgba_unmultiplied(
                                                255, 220, 115, 30,
                                            ),
                                        );
                                    }
                                }

                                if let Some(single_index) = view.perf_selected_history_index {
                                    if single_index >= visible_start && single_index < visible_end {
                                        let slot = (single_index as isize - view_start) as usize;
                                        let x_min = rect.left() + slot as f32 * bar_step;
                                        let x_max = (x_min + bar_width).min(rect.right() - 1.0);

                                        let selection_rect = egui::Rect::from_min_max(
                                            egui::pos2(x_min, rect.top() + 1.0),
                                            egui::pos2(x_max, rect.bottom() - 1.0),
                                        );

                                        painter.rect_filled(
                                            selection_rect,
                                            0.0,
                                            egui::Color32::from_rgba_unmultiplied(
                                                214, 231, 255, 30,
                                            ),
                                        );
                                    }
                                }

                                if response.hovered() {
                                    let scroll_y = ui.input(|input| input.raw_scroll_delta.y);
                                    if scroll_y.abs() > 0.0 {
                                        let history_len = history.len();
                                        if history_len > 0 {
                                            let last = history_len - 1;

                                            // Determine current range
                                            let (current_start, current_end) = if let Some(range) =
                                                view.perf_selected_history_range
                                            {
                                                (range.0.min(last), range.1.min(last))
                                            } else if let Some(index) = view.perf_selected_history_index
                                            {
                                                let clamped = index.min(last);
                                                (clamped, clamped)
                                            } else {
                                                let center = if view.perf_histogram_follow_latest {
                                                    last
                                                } else {
                                                    view.perf_histogram_focus_index
                                                        .unwrap_or(last)
                                                        .min(last)
                                                };
                                                let chunk_size = 16.min(history_len).max(1);
                                                let start = center.saturating_sub(chunk_size / 2);
                                                let end =
                                                    (start + chunk_size).saturating_sub(1).min(last);
                                                (start, end)
                                            };

                                            let current_width =
                                                current_end.saturating_sub(current_start) + 1;

                                            // Keep one stable anchor frame for zoom operations.
                                            // This prevents cumulative left/right drift when toggling zoom levels.
                                            let anchor_index = if let Some(index) =
                                                view.perf_selected_history_index
                                            {
                                                index.min(last)
                                            } else if let Some(index) =
                                                view.perf_histogram_focus_index
                                            {
                                                index.min(last)
                                            } else {
                                                (current_start + current_end) / 2
                                            };

                                            // Adjust width based on scroll
                                            // Up (pos) -> smaller width -> Zoom IN
                                            let zoom_factor = if scroll_y > 0.0 {
                                                1.0 + scroll_y * 0.005
                                            } else {
                                                1.0 / (1.0 - scroll_y * 0.005)
                                            };
                                            let next_width =
                                                (current_width as f32 / zoom_factor)
                                                    .clamp(1.0, 1024.0);
                                            let next_width_i = if scroll_y > 0.0 {
                                                next_width.floor() as usize
                                            } else {
                                                next_width.ceil() as usize
                                            }
                                            .clamp(1, 1024);

                                            if next_width_i != current_width {
                                                let left_count = (next_width_i - 1) / 2;
                                                let right_count = next_width_i - 1 - left_count;

                                                let mut clamped_start =
                                                    anchor_index.saturating_sub(left_count);
                                                let mut clamped_end =
                                                    anchor_index.saturating_add(right_count).min(last);

                                                let clamped_width =
                                                    clamped_end.saturating_sub(clamped_start) + 1;
                                                if clamped_width < next_width_i {
                                                    if clamped_start == 0 {
                                                        clamped_end = (next_width_i - 1).min(last);
                                                    } else {
                                                        clamped_start =
                                                            last.saturating_add(1).saturating_sub(next_width_i);
                                                        clamped_end = last;
                                                    }
                                                }

                                                commands.push(
                                                    AppCommand::EditorSetPerfFollowLatest(false),
                                                );
                                                commands.push(
                                                    AppCommand::EditorSelectPerfHistoryRange {
                                                        start: clamped_start,
                                                        end: clamped_end,
                                                    },
                                                );

                                                // Keep viewport centered on the same stable anchor.
                                                commands.push(
                                                    AppCommand::EditorFocusPerfHistogramIndex(
                                                        anchor_index,
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }

                                if response.dragged() {
                                    let delta_x = ui.input(|input| input.pointer.delta().x);
                                    let frames_to_pan = (-delta_x / bar_step).trunc() as i32;
                                    if frames_to_pan != 0 {
                                        commands.push(AppCommand::EditorSetPerfFollowLatest(false));
                                        commands
                                            .push(AppCommand::EditorPanPerfHistogram(frames_to_pan));
                                    }
                                }

                                if response.clicked() {
                                    if let Some(pointer) = response.interact_pointer_pos() {
                                        if let Some(clicked_index) = perf_history_index_from_pointer(
                                            rect,
                                            view_start,
                                            visible_bars,
                                            history.len(),
                                            pointer,
                                            bar_step,
                                        ) {
                                            let alt_held = ui.input(|input| input.modifiers.alt);
                                            if alt_held {
                                                commands.push(AppCommand::EditorSelectPerfHistoryIndex(
                                                    clicked_index,
                                                ));
                                            } else {
                                                let chunk_size = 16.min(history.len()).max(1);
                                                let mut start =
                                                    clicked_index.saturating_sub(chunk_size / 2);
                                                if start + chunk_size > history.len() {
                                                    start = history.len().saturating_sub(chunk_size);
                                                }
                                                let end = start + chunk_size.saturating_sub(1);

                                                commands.push(AppCommand::EditorSetPerfFollowLatest(false));
                                                commands.push(AppCommand::EditorSelectPerfHistoryRange {
                                                    start,
                                                    end,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                });

                    egui::Frame::default()
                        .inner_margin(egui::Margin::symmetric(content_margin, 0))
                        .show(ui, |ui| {
                            ui.separator();

                            if let Some(summary) = &view.perf_active_range_summary {
                                let label = if summary.frame_count > 1 {
                                    "Chunk"
                                } else {
                                    "Frame"
                                };
                                ui.horizontal_wrapped(|ui| {
                                    ui.monospace(format!(
                                        "{} #{}..#{} | {} frames",
                                        label,
                                        summary.start_frame_index,
                                        summary.end_frame_index,
                                        summary.frame_count,
                                    ));
                                    ui.separator();
                                    ui.monospace(format!(
                                        "Avg {:.2}ms ({:.1} FPS) | Worst {:.2}ms",
                                        summary.average_frame_time_ms,
                                        summary.average_fps,
                                        summary.worst_frame_time_ms
                                    ));
                                    ui.separator();
                                    ui.monospace(format!(
                                        "Dominant: {}",
                                        summary
                                            .dominant_stage
                                            .map(|stage| stage.name())
                                            .unwrap_or("none")
                                    ));
                                });
                            }

                            if view.perf_selected_history_index.is_some() {
                                ui.separator();
                                egui::CollapsingHeader::new("Selected Frame Drilldown")
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        if let Some(frame) = &view.perf_selected_frame {
                                            ui.monospace(format!(
                                                "Frame #{} | {:.2}ms ({:.1} FPS) | Dominant: {}",
                                                frame.frame_index,
                                                frame.frame_time_ms,
                                                1000.0 / frame.frame_time_ms.max(0.001),
                                                frame
                                                    .dominant_stage
                                                    .map(|stage| stage.name())
                                                    .unwrap_or("none")
                                            ));
                                        }
                                    });
                            }

                            show_perf_flame_graph_panel(
                                ui,
                                view,
                                viewport.width(),
                                viewport.height(),
                                commands,
                            );
                        });
                });
        });
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
    let mut top_panel_height = 0.0;

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

    let editor_top_panel = egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
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
                commands.push(crate::commands::AppCommand::EditorCompleteImport);
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
    top_panel_height += editor_top_panel.response.rect.height();

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

    if !view.perf_overlay_enabled && !is_timing && !is_compact_ui {
        show_view_selector_cube(
            ctx,
            view.camera_rotation,
            view.camera_pitch,
            view_cube_top_offset_y(top_panel_height),
            &mut commands,
        );
    }

    if view.perf_overlay_enabled {
        show_perf_microprofiler_overlay(ctx, &view, is_compact_ui, &mut commands);
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

    drop(view);
    for command in commands {
        state.dispatch(command);
    }
}

fn show_view_selector_cube(
    ctx: &egui::Context,
    camera_rotation: f32,
    camera_pitch: f32,
    top_offset_y: f32,
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
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, top_offset_y))
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
        perf_history_index_from_pointer, perf_visible_history_range,
        responsive_ui_scale_multiplier, settings_sidebar_default_width, show_editor_ui,
        sort_quad_by_angle, view_cube_top_offset_y, VIEW_CUBE_FACES,
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
    fn perf_follow_latest_keeps_latest_slot_centered() {
        let visible_bars = 120;
        let expected_slot = (visible_bars / 2) as isize;

        for history_len in [1usize, 2, 16, 120, 500] {
            let (view_start, _) = perf_visible_history_range(history_len, visible_bars, true, None);
            let latest_slot = (history_len as isize - 1) - view_start;
            assert_eq!(latest_slot, expected_slot);
        }
    }

    #[test]
    fn perf_pointer_mapping_handles_virtual_window_offsets() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 20.0));
        let bar_step = 2.0;
        let visible_bars = 100;
        let history_len = 10;
        let (view_start, _) = perf_visible_history_range(history_len, visible_bars, true, None);

        let pointer_at_latest = egui::pos2((visible_bars as f32 / 2.0) * bar_step + 0.1, 10.0);
        let latest_index = perf_history_index_from_pointer(
            rect,
            view_start,
            visible_bars,
            history_len,
            pointer_at_latest,
            bar_step,
        );
        assert_eq!(latest_index, Some(history_len - 1));

        let pointer_in_future_slot = egui::pos2((visible_bars as f32 - 1.0) * bar_step, 10.0);
        let future_index = perf_history_index_from_pointer(
            rect,
            view_start,
            visible_bars,
            history_len,
            pointer_in_future_slot,
            bar_step,
        );
        assert_eq!(future_index, None);
    }

    #[test]
    fn view_cube_top_offset_tracks_top_panel_stack_height() {
        let offset_without_profiler = view_cube_top_offset_y(36.0);
        let offset_with_profiler = view_cube_top_offset_y(36.0 + 112.0);

        assert!(approx_eq(offset_without_profiler, 48.0, 0.001));
        assert!(approx_eq(offset_with_profiler, 160.0, 0.001));
        assert!(approx_eq(
            offset_with_profiler - offset_without_profiler,
            112.0,
            0.001
        ));
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
    fn show_editor_ui_renders_marquee_selection() {
        pollster::block_on(async {
            let Some(mut state) = crate::State::try_new_test().await else {
                return;
            };

            state.toggle_editor();
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            // Inject a marquee drag large enough to be considered active.
            state.process_input_event(crate::commands::InputEvent::MouseButton {
                button: 1, // Left click
                pressed: true,
            });
            state.process_input_event(crate::commands::InputEvent::PointerMoved {
                x: 10.0,
                y: 10.0,
            });
            state.process_input_event(crate::commands::InputEvent::PointerMoved {
                x: 100.0,
                y: 100.0,
            });

            run_editor_ui_once(&mut state);

            // If we successfully run the UI loop without panicking, the marquee rendering path is covered.
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
            state.dispatch(AppCommand::EditorTogglePerfOverlay);
            run_editor_ui_once(&mut state);

            assert_eq!(state.editor_mode(), EditorMode::Timing);
            assert!(state.editor_show_settings());
            assert_eq!(state.editor_settings_section(), SettingsSection::Backends);
            assert!(state.editor_show_metadata());
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
