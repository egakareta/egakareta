/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use crate::modes::shared::{EditorPropertyPopup, show_editor_property_popup};
use eg_blocks::{BlockDefinition, BlockId, BlockTextureAtlas, BlockTextureLayers};
use eg_state::commands::AppCommand;
use eg_state::state::UiStateSnapshot;
use eg_state::state::editor_command::EditorCommand;
use eg_types::{CameraMode, EasingDirection, EditorMode, LevelObject, TimedTrigger, TriggerTarget};

const RECENT_BLOCK_STRIP_LIMIT: usize = 4;
const COMPACT_PREVIEW_BUTTON_SIZE: f32 = 40.0;
const COMPACT_PREVIEW_HEIGHT: f32 = 34.0;

const PREVIEW_BUTTON_WIDTH: f32 = 72.0;
const PREVIEW_BUTTON_HEIGHT: f32 = 72.0;
const PREVIEW_PADDING_X: f32 = 4.0;
const PREVIEW_PADDING_Y: f32 = 4.0;
const PREVIEW_HEIGHT: f32 = 64.0;
const CUBE_EDGE_WIDTH: f32 = 1.0;
const CUBE_EDGE_ALPHA: u8 = 90;
const CUBE_HALF_WIDTH_RATIO: f32 = 0.30;
const CUBE_HALF_HEIGHT_RATIO: f32 = 0.22;
const CUBE_DEPTH_RATIO: f32 = 0.34;
const FALLBACK_TOP_COLOR: egui::Color32 = egui::Color32::from_rgb(170, 170, 170);
const FALLBACK_SIDE_COLOR: egui::Color32 = egui::Color32::from_rgb(140, 140, 140);
const TOP_LIGHTEN_FACTOR: f32 = 1.05;
const ATLAS_AVERAGE_MAX_SAMPLES: usize = 1024;

pub(crate) fn show_recent_block_quick_strip(
    ui: &mut egui::Ui,
    view: &UiStateSnapshot<'_>,
    block_icon_texture_ids: &HashMap<String, egui::TextureId>,
    commands: &mut Vec<AppCommand>,
) -> bool {
    let blocks: Vec<_> = view
        .recent_block_ids
        .iter()
        .map(|id| BlockDefinition::get(id))
        .filter(|block| block.placeable)
        .take(RECENT_BLOCK_STRIP_LIMIT)
        .collect();
    if blocks.is_empty() {
        return false;
    }

    for block in blocks {
        if show_block_preview_button_sized(
            ui,
            block,
            block_icon_texture_ids.get(&block.id.to_string()).copied(),
            egui::Vec2::splat(COMPACT_PREVIEW_BUTTON_SIZE),
            COMPACT_PREVIEW_HEIGHT,
        ) {
            commands.push(AppCommand::Editor(EditorCommand::SetBlockId(BlockId::new(
                &block.id,
            ))));
            commands.push(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));
        }
    }
    true
}

pub(crate) fn show_selected_block_properties_window(
    ctx: &egui::Context,
    view: &UiStateSnapshot<'_>,
    bottom_bar_height: f32,
    commands: &mut Vec<AppCommand>,
) {
    let mode = view.mode;
    if !matches!(
        mode,
        EditorMode::Select | EditorMode::Move | EditorMode::Scale | EditorMode::Rotate
    ) {
        return;
    }

    let Some(mut selected) = view.selected_block.clone() else {
        return;
    };

    show_editor_property_popup(
        ctx,
        EditorPropertyPopup::above_bottom_bar("selected_block_properties", bottom_bar_height),
        |ui| {
            ui.vertical(|ui| {
                let field_visibility = selected.transform_field_visibility();
                let mut transform_changed = false;
                if field_visibility.position {
                    transform_changed |= show_position_controls(ui, &mut selected);
                }
                if field_visibility.rotation {
                    transform_changed |= show_rotation_controls(ui, &mut selected);
                }
                if transform_changed {
                    push_selected_block_update(commands, &selected);
                }

                if selected.trigger.is_some() {
                    if field_visibility.size && show_size_controls(ui, &mut selected) {
                        push_selected_block_update(commands, &selected);
                    }

                    let mut trigger_changed = false;
                    if let Some(trigger) = selected.trigger.as_mut() {
                        trigger_changed = show_trigger_controls(ui, trigger);
                    }
                    if trigger_changed {
                        push_selected_block_update(commands, &selected);
                    }
                } else {
                    if field_visibility.size && show_size_controls(ui, &mut selected) {
                        push_selected_block_update(commands, &selected);
                    }
                    if show_color_controls(ui, &mut selected) {
                        push_selected_block_update(commands, &selected);
                    }
                }
            });
        },
    );
}

fn push_selected_block_update(commands: &mut Vec<AppCommand>, selected: &LevelObject) {
    commands.push(AppCommand::Editor(EditorCommand::UpdateSelectedBlock(
        selected.clone(),
    )));
}

fn show_position_controls(ui: &mut egui::Ui, selected: &mut LevelObject) -> bool {
    ui.horizontal(|ui| {
        ui.label("Position:");
        show_vector_drag_values(ui, &mut selected.position, ["X ", "Y ", "Z "])
    })
    .inner
}

fn show_size_controls(ui: &mut egui::Ui, selected: &mut LevelObject) -> bool {
    ui.horizontal(|ui| {
        ui.label("Size:");
        show_vector_drag_values(ui, &mut selected.size, ["W ", "H ", "D "])
    })
    .inner
}

fn show_rotation_controls(ui: &mut egui::Ui, selected: &mut LevelObject) -> bool {
    ui.horizontal(|ui| {
        ui.label("Rotation:");
        show_rotation_drag_values(ui, &mut selected.rotation)
    })
    .inner
}

fn show_color_controls(ui: &mut egui::Ui, selected: &mut LevelObject) -> bool {
    ui.horizontal(|ui| {
        ui.label("Color:");
        let mut color_tint = selected.color_tint;
        let changed = ui.color_edit_button_rgb(&mut color_tint).changed();
        if changed {
            selected.color_tint = color_tint;
        }
        changed
    })
    .inner
}

fn show_vector_drag_values(ui: &mut egui::Ui, values: &mut [f32; 3], prefixes: [&str; 3]) -> bool {
    let mut changed = false;
    for (value, prefix) in values.iter_mut().zip(prefixes) {
        changed |= ui.add(egui::DragValue::new(value).prefix(prefix)).changed();
    }
    changed
}

fn show_rotation_drag_values(ui: &mut egui::Ui, values: &mut [f32; 3]) -> bool {
    let mut changed = false;
    for (value, prefix) in values.iter_mut().zip(["X ", "Y ", "Z "]) {
        changed |= ui
            .add(
                egui::DragValue::new(value)
                    .speed(0.5)
                    .prefix(prefix)
                    .suffix("°"),
            )
            .changed();
    }
    changed
}

fn show_trigger_controls(ui: &mut egui::Ui, trigger: &mut TimedTrigger) -> bool {
    let mut changed = false;
    changed |= ui
        .horizontal(|ui| {
            ui.label("Trigger:");
            let mut row_changed = false;
            row_changed |= ui
                .add(
                    egui::DragValue::new(&mut trigger.time_seconds)
                        .speed(0.05)
                        .prefix("Time ")
                        .suffix("s"),
                )
                .changed();
            row_changed |= ui
                .add(
                    egui::DragValue::new(&mut trigger.duration_seconds)
                        .speed(0.05)
                        .prefix("Duration ")
                        .suffix("s"),
                )
                .changed();
            row_changed
        })
        .inner;

    changed |= show_trigger_easing_control(ui, trigger);
    changed |= show_trigger_action_controls(ui, trigger);
    changed
}

fn show_trigger_easing_control(ui: &mut egui::Ui, trigger: &mut TimedTrigger) -> bool {
    ui.horizontal(|ui| {
        ui.label("Easing:");
        let mut changed = false;
        egui::ComboBox::from_id_salt("selected_block_trigger_easing")
            .selected_text(trigger_easing_label(trigger.easing))
            .show_ui(ui, |ui| {
                for easing in [
                    EasingDirection::Linear,
                    EasingDirection::EaseIn,
                    EasingDirection::EaseOut,
                    EasingDirection::EaseInOut,
                ] {
                    changed |= ui
                        .selectable_value(&mut trigger.easing, easing, trigger_easing_label(easing))
                        .changed();
                }
            });
        changed
    })
    .inner
}

fn show_trigger_action_controls(ui: &mut egui::Ui, trigger: &mut TimedTrigger) -> bool {
    if let Some(camera) = trigger.camera.as_mut() {
        return match camera.mode {
            CameraMode::Static => show_camera_trigger_controls(
                ui,
                &mut camera.transition_interval_seconds,
                &mut camera.use_full_segment_transition,
            ),
            CameraMode::Follow => show_camera_follow_trigger_controls(
                ui,
                &mut camera.transition_interval_seconds,
                &mut camera.use_full_segment_transition,
                &mut camera.distance,
                &mut camera.yaw,
                &mut camera.pitch,
            ),
        };
    }

    if trigger.transform {
        let target_count = match &trigger.target {
            TriggerTarget::Objects { object_ids } => object_ids.len(),
            TriggerTarget::Camera => 0,
        };
        show_transform_trigger_summary(ui, target_count);
        false
    } else {
        false
    }
}

fn show_camera_trigger_controls(
    ui: &mut egui::Ui,
    transition_interval_seconds: &mut f32,
    use_full_segment_transition: &mut bool,
) -> bool {
    let mut changed = false;
    changed |= ui
        .horizontal(|ui| {
            ui.label("Camera:");
            ui.add(
                egui::DragValue::new(transition_interval_seconds)
                    .speed(0.05)
                    .prefix("Blend ")
                    .suffix("s"),
            )
            .changed()
        })
        .inner;
    changed |= ui
        .horizontal(|ui| ui.checkbox(use_full_segment_transition, "Use full segment"))
        .inner
        .changed();
    changed
}

fn show_camera_follow_trigger_controls(
    ui: &mut egui::Ui,
    transition_interval_seconds: &mut f32,
    use_full_segment_transition: &mut bool,
    distance: &mut f32,
    yaw: &mut f32,
    pitch: &mut f32,
) -> bool {
    let mut changed = false;
    changed |= ui
        .horizontal(|ui| {
            ui.label("Camera:");
            let mut row_changed = false;
            row_changed |= ui
                .add(
                    egui::DragValue::new(distance)
                        .speed(0.1)
                        .range(0.01..=200.0)
                        .prefix("Distance "),
                )
                .changed();

            let mut yaw_degrees = yaw.to_degrees();
            if ui
                .add(
                    egui::DragValue::new(&mut yaw_degrees)
                        .speed(0.5)
                        .prefix("Yaw ")
                        .suffix("°"),
                )
                .changed()
            {
                *yaw = yaw_degrees.to_radians();
                row_changed = true;
            }

            let mut pitch_degrees = pitch.to_degrees();
            if ui
                .add(
                    egui::DragValue::new(&mut pitch_degrees)
                        .speed(0.5)
                        .range(-89.9..=89.9)
                        .prefix("Pitch ")
                        .suffix("°"),
                )
                .changed()
            {
                *pitch = pitch_degrees.to_radians();
                row_changed = true;
            }

            row_changed
        })
        .inner;
    changed |=
        show_camera_trigger_controls(ui, transition_interval_seconds, use_full_segment_transition);
    changed
}

fn show_transform_trigger_summary(ui: &mut egui::Ui, target_count: usize) {
    ui.horizontal(|ui| {
        ui.label("Targets:");
        ui.label(target_count.to_string());
    });
}

fn trigger_easing_label(easing: EasingDirection) -> &'static str {
    match easing {
        EasingDirection::Linear => "Linear",
        EasingDirection::EaseIn => "Ease In",
        EasingDirection::EaseOut => "Ease Out",
        EasingDirection::EaseInOut => "Ease In Out",
    }
}

pub(crate) fn show_place_block_properties_window(
    ctx: &egui::Context,
    view: &UiStateSnapshot<'_>,
    bottom_bar_height: f32,
) {
    if view.mode != EditorMode::Place {
        return;
    }

    let block = BlockDefinition::get(view.selected_block_id);
    show_editor_property_popup(
        ctx,
        EditorPropertyPopup::above_bottom_bar("place_block_properties", bottom_bar_height),
        |ui| {
            ui.vertical(|ui| {
                ui.label(block.display_name.as_str());
                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    view.place_preview_position[0],
                    view.place_preview_position[1],
                    view.place_preview_position[2]
                ));
                ui.label(format!(
                    "Size: ({:.2}, {:.2}, {:.2})",
                    view.place_preview_size[0],
                    view.place_preview_size[1],
                    view.place_preview_size[2]
                ));
            });
        },
    );
}

pub(crate) fn show_block_preview_button(
    ui: &mut egui::Ui,
    block: &eg_blocks::BlockDefinition,
    icon_texture_id: Option<egui::TextureId>,
) -> bool {
    let button_size = egui::vec2(PREVIEW_BUTTON_WIDTH, PREVIEW_BUTTON_HEIGHT);
    show_block_preview_button_sized(ui, block, icon_texture_id, button_size, PREVIEW_HEIGHT)
}

fn show_block_preview_button_sized(
    ui: &mut egui::Ui,
    block: &eg_blocks::BlockDefinition,
    icon_texture_id: Option<egui::TextureId>,
    button_size: egui::Vec2,
    preview_height: f32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

    let visuals = ui.style().interact_selectable(&response, false);
    ui.painter().rect(
        rect,
        4.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Outside,
    );

    let preview_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(PREVIEW_PADDING_X, PREVIEW_PADDING_Y),
        egui::vec2(rect.width() - PREVIEW_PADDING_X * 2.0, preview_height),
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
        draw_block_preview_cube(ui.painter(), preview_rect, &block.id.to_string());
    }

    let clicked = response.clicked();
    response.on_hover_text_at_pointer(block.display_name.as_str());
    clicked
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
    let atlas = BlockTextureAtlas::get();
    let layers = BlockTextureLayers::get(block_id);
    let top = atlas_average_color(atlas, layers.top).unwrap_or(FALLBACK_TOP_COLOR);
    let side = atlas_average_color(atlas, layers.side).unwrap_or(FALLBACK_SIDE_COLOR);
    (scale_rgb(top, TOP_LIGHTEN_FACTOR), side)
}

fn atlas_average_color(atlas: &eg_blocks::BlockTextureAtlas, layer: u32) -> Option<egui::Color32> {
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
        FALLBACK_SIDE_COLOR, FALLBACK_TOP_COLOR, TOP_LIGHTEN_FACTOR, atlas_average_color,
        block_preview_colors, scale_channel, scale_rgb, show_block_preview_button,
        show_place_block_properties_window, show_selected_block_properties_window,
    };
    use crate::test_utils::editor_ui_snapshot;
    use eg_blocks::{BlockDefinition, BlockTextureAtlas, BlockTextureLayers};
    use eg_types::{
        AppSettings, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EditorMode, LevelObject,
        MusicMetadata, TRANSFORM_TRIGGER_BLOCK_ID,
    };
    use eg_types::{EasingDirection, TimedTrigger, TriggerTarget};

    fn make_compose_view<'a>(
        app_settings: &'a AppSettings,
        music_metadata: &'a MusicMetadata,
        mode: EditorMode,
    ) -> eg_state::state::UiStateSnapshot<'a> {
        let mut view = editor_ui_snapshot(app_settings, music_metadata);
        view.mode = mode;
        view.last_mode = Some(mode);
        view.level_name = Some("Compose Test");
        view.place_preview_position = [2.0, 1.0, 3.0];
        view.place_preview_size = [1.0, 2.0, 1.0];
        view
    }

    #[test]
    fn atlas_average_color_returns_some_for_known_default_layer() {
        let atlas = BlockTextureAtlas::get();
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
        let atlas = BlockTextureAtlas::get();
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
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block");
        let atlas = BlockTextureAtlas::get();
        let layers = BlockTextureLayers::get(&block.id.to_string());

        let expected_top = scale_rgb(
            atlas_average_color(atlas, layers.top).unwrap_or(FALLBACK_TOP_COLOR),
            TOP_LIGHTEN_FACTOR,
        );
        let expected_side = atlas_average_color(atlas, layers.side).unwrap_or(FALLBACK_SIDE_COLOR);

        assert_eq!(
            block_preview_colors(&block.id.to_string()),
            (expected_top, expected_side)
        );
    }

    #[test]
    fn show_block_preview_button_handles_texture_and_fallback_paths() {
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block");
        let ctx = egui::Context::default();
        let mut clicked_without_texture = true;
        let mut clicked_with_texture = true;

        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                clicked_without_texture = show_block_preview_button(ui, block, None);
                clicked_with_texture =
                    show_block_preview_button(ui, block, Some(egui::TextureId::Managed(1)));
            });
        });

        assert!(!clicked_without_texture);
        assert!(!clicked_with_texture);
    }

    #[test]
    fn show_place_block_properties_window_handles_place_and_other_modes() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let place_view = make_compose_view(&app_settings, &music_metadata, EditorMode::Place);
        let select_view = make_compose_view(&app_settings, &music_metadata, EditorMode::Select);
        let ctx = egui::Context::default();

        let _ = ctx.run_ui(egui::RawInput::default(), |_root_ui| {
            show_place_block_properties_window(&ctx, &place_view, 48.0);
            show_place_block_properties_window(&ctx, &select_view, 48.0);
        });
    }

    #[test]
    fn show_selected_block_properties_window_handles_normal_and_trigger_blocks() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let mut normal_view = make_compose_view(&app_settings, &music_metadata, EditorMode::Select);
        normal_view.selected_block = Some(LevelObject {
            position: [1.0, 2.0, 3.0],
            ..LevelObject::default()
        });

        let mut trigger_view =
            make_compose_view(&app_settings, &music_metadata, EditorMode::Select);
        trigger_view.selected_block = Some(LevelObject {
            id: "trigger".to_string(),
            position: [1.0, 0.0, 1.0],
            rotation: [0.0, 90.0, 0.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.5,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects {
                    object_ids: vec!["target".to_string()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        });

        let ctx = egui::Context::default();
        let mut commands = Vec::new();
        let _ = ctx.run_ui(egui::RawInput::default(), |_root_ui| {
            show_selected_block_properties_window(&ctx, &normal_view, 48.0, &mut commands);
            show_selected_block_properties_window(&ctx, &trigger_view, 48.0, &mut commands);
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn selected_block_properties_identifies_camera_follow_triggers() {
        let camera_follow = LevelObject {
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Camera,
                transform: false,
                camera: Some(CameraTrigger {
                    mode: CameraMode::Follow,
                    ..CameraTrigger::default()
                }),
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        };

        let transform_trigger = LevelObject {
            id: "trigger".to_string(),
            position: [1.0, 0.0, 1.0],
            rotation: [0.0, 90.0, 0.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.5,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects {
                    object_ids: vec!["target".to_string()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        };

        assert_eq!(
            camera_follow.transform_field_visibility(),
            eg_types::TransformVisibility::none()
        );
        assert_eq!(
            transform_trigger.transform_field_visibility(),
            eg_types::TransformVisibility::all()
        );
    }
}
