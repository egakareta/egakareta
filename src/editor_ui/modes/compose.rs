/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::collections::HashMap;

use crate::block_repository::{
    all_placeable_blocks, block_texture_atlas, resolve_block_texture_layers,
};
use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::editor_ui::modes::shared::{show_mode_and_snap_controls, show_player_camera_status_row};
use crate::state::EditorUiViewModel;
use crate::types::EditorMode;

const PREVIEW_BUTTON_WIDTH: f32 = 72.0;
const PREVIEW_BUTTON_HEIGHT: f32 = 88.0;
const PREVIEW_PADDING_X: f32 = 8.0;
const PREVIEW_PADDING_Y: f32 = 6.0;
const PREVIEW_HEIGHT: f32 = 54.0;
const PREVIEW_TEXT_Y_OFFSET: f32 = 13.0;
const CUBE_EDGE_WIDTH: f32 = 1.0;
const CUBE_EDGE_ALPHA: u8 = 90;
const CUBE_HALF_WIDTH_RATIO: f32 = 0.30;
const CUBE_HALF_HEIGHT_RATIO: f32 = 0.22;
const CUBE_DEPTH_RATIO: f32 = 0.34;
const FALLBACK_TOP_COLOR: egui::Color32 = egui::Color32::from_rgb(170, 170, 170);
const FALLBACK_SIDE_COLOR: egui::Color32 = egui::Color32::from_rgb(140, 140, 140);
const TOP_LIGHTEN_FACTOR: f32 = 1.05;
const ATLAS_AVERAGE_MAX_SAMPLES: usize = 1024;

pub(crate) fn show_compose_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    block_icon_texture_ids: &HashMap<String, egui::TextureId>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    show_mode_and_snap_controls(ui, view, commands);

    match view.mode {
        EditorMode::Place => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Block:");

                let current = view.selected_block_id;
                for block in all_placeable_blocks() {
                    if !block.placeable {
                        continue;
                    }
                    if show_block_preview_button(
                        ui,
                        block,
                        current == block.id,
                        block_icon_texture_ids.get(block.id.as_str()).copied(),
                    ) {
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

    ui.horizontal_wrapped(|ui| {
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

fn show_block_preview_button(
    ui: &mut egui::Ui,
    block: &crate::block_repository::BlockDefinition,
    selected: bool,
    icon_texture_id: Option<egui::TextureId>,
) -> bool {
    let button_size = egui::vec2(PREVIEW_BUTTON_WIDTH, PREVIEW_BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter().rect(
        rect,
        4.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Outside,
    );

    let preview_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(PREVIEW_PADDING_X, PREVIEW_PADDING_Y),
        egui::vec2(rect.width() - PREVIEW_PADDING_X * 2.0, PREVIEW_HEIGHT),
    );
    if let Some(texture_id) = icon_texture_id {
        let image_rect = preview_rect.shrink2(egui::vec2(2.0, 2.0));
        ui.painter().image(
            texture_id,
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        draw_block_preview_cube(ui.painter(), preview_rect, block.id.as_str());
    }

    let text_pos = egui::pos2(rect.center().x, rect.max.y - PREVIEW_TEXT_Y_OFFSET);
    ui.painter().text(
        text_pos,
        egui::Align2::CENTER_CENTER,
        block.display_name.as_str(),
        egui::TextStyle::Small.resolve(ui.style()),
        visuals.text_color(),
    );

    response.clicked()
}

fn draw_block_preview_cube(painter: &egui::Painter, rect: egui::Rect, block_id: &str) {
    let (top_color, side_color) = block_preview_colors(block_id);
    let left_side_color = scale_rgb(side_color, 0.8);
    let edge_stroke = egui::Stroke::new(
        CUBE_EDGE_WIDTH,
        egui::Color32::from_black_alpha(CUBE_EDGE_ALPHA),
    );

    let cx = rect.center().x;
    let top_y = rect.top() + 2.0;
    let half_w = rect.width() * CUBE_HALF_WIDTH_RATIO;
    let half_h = rect.height() * CUBE_HALF_HEIGHT_RATIO;
    let depth = rect.height() * CUBE_DEPTH_RATIO;

    let top = [
        egui::pos2(cx, top_y),
        egui::pos2(cx + half_w, top_y + half_h),
        egui::pos2(cx, top_y + half_h * 2.0),
        egui::pos2(cx - half_w, top_y + half_h),
    ];
    let right = [
        top[1],
        top[2],
        egui::pos2(top[2].x, top[2].y + depth),
        egui::pos2(top[1].x, top[1].y + depth),
    ];
    let left = [
        top[3],
        top[2],
        egui::pos2(top[2].x, top[2].y + depth),
        egui::pos2(top[3].x, top[3].y + depth),
    ];

    painter.add(egui::Shape::convex_polygon(
        left.to_vec(),
        left_side_color,
        edge_stroke,
    ));
    painter.add(egui::Shape::convex_polygon(
        right.to_vec(),
        side_color,
        edge_stroke,
    ));
    painter.add(egui::Shape::convex_polygon(
        top.to_vec(),
        top_color,
        edge_stroke,
    ));
}

fn block_preview_colors(block_id: &str) -> (egui::Color32, egui::Color32) {
    let atlas = block_texture_atlas();
    let layers = resolve_block_texture_layers(block_id);
    let top = atlas_average_color(atlas, layers.top).unwrap_or(FALLBACK_TOP_COLOR);
    let side = atlas_average_color(atlas, layers.side).unwrap_or(FALLBACK_SIDE_COLOR);
    (scale_rgb(top, TOP_LIGHTEN_FACTOR), side)
}

fn atlas_average_color(
    atlas: &crate::block_repository::BlockTextureAtlas,
    layer: u32,
) -> Option<egui::Color32> {
    let rgba = atlas.layers.get(layer as usize)?.rgba.as_slice();
    if rgba.is_empty() {
        return None;
    }

    let total_pixels = rgba.len() / 4;
    if total_pixels == 0 {
        return None;
    }
    let step = (total_pixels / ATLAS_AVERAGE_MAX_SAMPLES.max(1)).max(1);
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let mut count: u64 = 0;
    for pixel in rgba.chunks_exact(4).step_by(step) {
        sum_r += pixel[0] as u64;
        sum_g += pixel[1] as u64;
        sum_b += pixel[2] as u64;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some(egui::Color32::from_rgb(
        (sum_r / count) as u8,
        (sum_g / count) as u8,
        (sum_b / count) as u8,
    ))
}

fn scale_rgb(color: egui::Color32, factor: f32) -> egui::Color32 {
    let scale = factor.max(0.0);
    let r = scale_channel(color.r(), scale);
    let g = scale_channel(color.g(), scale);
    let b = scale_channel(color.b(), scale);
    egui::Color32::from_rgb(r, g, b)
}

fn scale_channel(value: u8, factor: f32) -> u8 {
    ((value as f32) * factor).clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::{
        atlas_average_color, block_preview_colors, scale_channel, scale_rgb,
        show_block_preview_button, FALLBACK_SIDE_COLOR, FALLBACK_TOP_COLOR, TOP_LIGHTEN_FACTOR,
    };
    use crate::block_repository::{
        all_placeable_blocks, block_texture_atlas, resolve_block_texture_layers,
    };

    #[test]
    fn atlas_average_color_returns_some_for_known_default_layer() {
        let atlas = block_texture_atlas();
        let color = atlas_average_color(atlas, atlas.default_layer());
        assert!(color.is_some());
    }

    #[test]
    fn scale_rgb_clamps_to_channel_bounds() {
        let color = egui::Color32::from_rgb(240, 120, 60);
        let brightened = scale_rgb(color, 2.0);
        assert_eq!(brightened.r(), 255);
        assert_eq!(brightened.g(), 240);
        assert_eq!(brightened.b(), 120);

        let darkened = scale_rgb(color, 0.0);
        assert_eq!(darkened.r(), 0);
        assert_eq!(darkened.g(), 0);
        assert_eq!(darkened.b(), 0);
    }

    #[test]
    fn atlas_average_color_returns_none_for_out_of_bounds_layer() {
        let atlas = block_texture_atlas();
        let color = atlas_average_color(atlas, u32::MAX);
        assert!(color.is_none());
    }

    #[test]
    fn scale_channel_clamps_low_and_high_bounds() {
        assert_eq!(scale_channel(100, -2.0), 0);
        assert_eq!(scale_channel(200, 2.0), 255);
    }

    #[test]
    fn block_preview_colors_match_texture_layer_sampling_for_placeable_block() {
        let block = all_placeable_blocks()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block");
        let atlas = block_texture_atlas();
        let layers = resolve_block_texture_layers(block.id.as_str());

        let expected_top = scale_rgb(
            atlas_average_color(atlas, layers.top).unwrap_or(FALLBACK_TOP_COLOR),
            TOP_LIGHTEN_FACTOR,
        );
        let expected_side = atlas_average_color(atlas, layers.side).unwrap_or(FALLBACK_SIDE_COLOR);

        assert_eq!(
            block_preview_colors(block.id.as_str()),
            (expected_top, expected_side)
        );
    }

    #[test]
    fn show_block_preview_button_handles_texture_and_fallback_paths() {
        let block = all_placeable_blocks()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block");
        let ctx = egui::Context::default();
        let mut clicked_without_texture = true;
        let mut clicked_with_texture = true;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                clicked_without_texture = show_block_preview_button(ui, block, false, None);
                clicked_with_texture =
                    show_block_preview_button(ui, block, true, Some(egui::TextureId::Managed(1)));
            });
        });

        assert!(!clicked_without_texture);
        assert!(!clicked_with_texture);
    }
}
