/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use crate::block_repository::{
    block_texture_atlas, resolve_block_definition, resolve_block_texture_layers,
};
use crate::commands::AppCommand;
use crate::editor_ui::modes::shared::{
    show_editor_property_popup, show_mode_and_snap_controls, EditorPropertyPopup,
};
use crate::state::editor_command::EditorCommand;
use crate::state::EditorUiViewModel;
use crate::triggers::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};
use crate::types::{EditorMode, LevelObject};

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

pub(crate) fn show_compose_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    _block_icon_texture_ids: &HashMap<String, egui::TextureId>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    show_mode_and_snap_controls(ui, view, commands);
}

pub(crate) fn show_recent_block_quick_strip(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    block_icon_texture_ids: &HashMap<String, egui::TextureId>,
    commands: &mut Vec<AppCommand>,
) -> bool {
    let blocks: Vec<_> = view
        .recent_block_ids
        .iter()
        .map(|id| resolve_block_definition(id))
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
            view.selected_block_id == block.id,
            block_icon_texture_ids.get(block.id.as_str()).copied(),
            egui::Vec2::splat(COMPACT_PREVIEW_BUTTON_SIZE),
            COMPACT_PREVIEW_HEIGHT,
        ) {
            commands.push(AppCommand::Editor(EditorCommand::SetBlockId(
                block.id.clone(),
            )));
            commands.push(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));
        }
    }
    true
}

pub(crate) fn show_selected_block_properties_window(
    ctx: &egui::Context,
    view: &EditorUiViewModel<'_>,
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
                if show_position_controls(ui, &mut selected)
                    || show_rotation_controls(ui, &mut selected)
                {
                    push_selected_block_update(commands, &selected);
                }

                if selected.trigger.is_some() {
                    if selected_has_transform_trigger(&selected)
                        && show_size_controls(ui, &mut selected)
                    {
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
                    if show_size_controls(ui, &mut selected) {
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

fn selected_has_transform_trigger(selected: &LevelObject) -> bool {
    matches!(
        selected.trigger.as_ref().map(|trigger| &trigger.action),
        Some(TimedTriggerAction::TransformObjects { .. })
    )
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
        show_rotation_drag_values(ui, &mut selected.rotation_degrees)
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
                    TimedTriggerEasing::Linear,
                    TimedTriggerEasing::EaseIn,
                    TimedTriggerEasing::EaseOut,
                    TimedTriggerEasing::EaseInOut,
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
    match &mut trigger.action {
        TimedTriggerAction::CameraPose {
            transition_interval_seconds,
            use_full_segment_transition,
            ..
        }
        | TimedTriggerAction::CameraFollow {
            transition_interval_seconds,
            use_full_segment_transition,
        } => show_camera_trigger_controls(
            ui,
            transition_interval_seconds,
            use_full_segment_transition,
        ),
        TimedTriggerAction::TransformObjects { .. } => show_transform_trigger_summary(ui, trigger),
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

fn show_transform_trigger_summary(ui: &mut egui::Ui, trigger: &TimedTrigger) -> bool {
    let target_count = match &trigger.target {
        TimedTriggerTarget::Objects { object_ids } => object_ids.len(),
        TimedTriggerTarget::Camera => 0,
    };
    ui.horizontal(|ui| {
        ui.label("Targets:");
        ui.label(target_count.to_string());
    });
    false
}

fn trigger_easing_label(easing: TimedTriggerEasing) -> &'static str {
    match easing {
        TimedTriggerEasing::Linear => "Linear",
        TimedTriggerEasing::EaseIn => "Ease In",
        TimedTriggerEasing::EaseOut => "Ease Out",
        TimedTriggerEasing::EaseInOut => "Ease In Out",
    }
}

pub(crate) fn show_place_block_properties_window(
    ctx: &egui::Context,
    view: &EditorUiViewModel<'_>,
    bottom_bar_height: f32,
) {
    if view.mode != EditorMode::Place {
        return;
    }

    let block = resolve_block_definition(view.selected_block_id);
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
    block: &crate::block_repository::BlockDefinition,
    selected: bool,
    icon_texture_id: Option<egui::TextureId>,
) -> bool {
    let button_size = egui::vec2(PREVIEW_BUTTON_WIDTH, PREVIEW_BUTTON_HEIGHT);
    show_block_preview_button_sized(
        ui,
        block,
        selected,
        icon_texture_id,
        button_size,
        PREVIEW_HEIGHT,
    )
}

fn show_block_preview_button_sized(
    ui: &mut egui::Ui,
    block: &crate::block_repository::BlockDefinition,
    selected: bool,
    icon_texture_id: Option<egui::TextureId>,
    button_size: egui::Vec2,
    preview_height: f32,
) -> bool {
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
        draw_block_preview_cube(ui.painter(), preview_rect, block.id.as_str());
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
        show_block_preview_button, show_place_block_properties_window,
        show_selected_block_properties_window, FALLBACK_SIDE_COLOR, FALLBACK_TOP_COLOR,
        TOP_LIGHTEN_FACTOR,
    };
    use crate::block_repository::{
        all_placeable_blocks, block_texture_atlas, resolve_block_texture_layers,
    };
    use crate::state::EditorUiViewModel;
    use crate::triggers::{
        TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
    };
    use crate::types::{
        AppSettings, EditorMode, LevelObject, MusicMetadata, SettingsSection, SpawnDirection,
        TRANSFORM_TRIGGER_BLOCK_ID,
    };

    fn make_view<'a>(
        app_settings: &'a AppSettings,
        music_metadata: &'a MusicMetadata,
        mode: EditorMode,
    ) -> EditorUiViewModel<'a> {
        EditorUiViewModel {
            mode,
            last_mode: Some(mode),
            available_levels: &[],
            level_name: Some("Compose Test"),
            show_metadata: false,
            show_place_window: false,
            show_settings: false,
            settings_section: SettingsSection::Backends,
            keybind_capture_action: None,
            music_metadata,
            creator_metadata: crate::types::LevelCreatorMetadata::default(),
            sky_color: crate::types::default_sky_color(),
            app_settings,
            configured_graphics_backend: "Auto",
            configured_audio_backend: "Default",
            graphics_backend_options: &[],
            audio_backend_options: &[],
            settings_restart_required: false,
            snap_to_grid: true,
            snap_step: 1.0,
            snap_rotation: true,
            snap_rotation_step_degrees: 15.0,
            selected_block_id: "core/stone",
            place_preview_position: [2.0, 1.0, 3.0],
            place_preview_size: [1.0, 2.0, 1.0],
            recent_block_ids: &[],
            selected_block: None,
            selected_block_count: 0,
            clipboard_block_count: 0,
            can_undo: false,
            can_redo: false,
            playing: false,
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            tap_times: &[],
            selected_tap: None,
            timeline_preview_position: [0.0, 0.0, 0.0],
            timeline_preview_direction: SpawnDirection::Forward,
            timing_points: &[],
            playback_speed: 1.0,
            timing_selected_index: None,
            waveform_zoom: 1.0,
            waveform_scroll: 0.0,
            waveform_samples: &[],
            waveform_sample_rate: 0,
            waveform_window_size: crate::audio_service::WAVEFORM_WINDOW,
            waveform_loading: false,
            waveform_complete: false,
            bpm_tap_result: None,
            triggers: vec![],
            trigger_selected_index: None,
            simulate_trigger_hitboxes: false,
            camera_position: [0.0, 0.0, 0.0],
            camera_preview_position: [0.0, 0.0, 0.0],
            camera_preview_target: [0.0, 0.0, 0.0],
            camera_rotation: 0.0,
            camera_pitch: 0.0,
            fps: 60.0,
            marquee_selection_rect_screen: None,
            object_count: 0,
        }
    }

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

        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                clicked_without_texture = show_block_preview_button(ui, block, false, None);
                clicked_with_texture =
                    show_block_preview_button(ui, block, true, Some(egui::TextureId::Managed(1)));
            });
        });

        assert!(!clicked_without_texture);
        assert!(!clicked_with_texture);
    }

    #[test]
    fn show_place_block_properties_window_handles_place_and_other_modes() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let place_view = make_view(&app_settings, &music_metadata, EditorMode::Place);
        let select_view = make_view(&app_settings, &music_metadata, EditorMode::Select);
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
        let mut normal_view = make_view(&app_settings, &music_metadata, EditorMode::Select);
        normal_view.selected_block = Some(crate::test_utils::stone(1.0, 2.0, 3.0));

        let mut trigger_view = make_view(&app_settings, &music_metadata, EditorMode::Select);
        trigger_view.selected_block = Some(LevelObject {
            position: [1.0, 0.0, 1.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 90.0, 0.0],
            block_id: TRANSFORM_TRIGGER_BLOCK_ID.to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.5,
                duration_seconds: 1.0,
                easing: TimedTriggerEasing::EaseInOut,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![0],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [1.0, 0.0, 1.0],
                    rotation_degrees: [0.0, 90.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            }),
        });

        let ctx = egui::Context::default();
        let mut commands = Vec::new();
        let _ = ctx.run_ui(egui::RawInput::default(), |_root_ui| {
            show_selected_block_properties_window(&ctx, &normal_view, 48.0, &mut commands);
            show_selected_block_properties_window(&ctx, &trigger_view, 48.0, &mut commands);
        });

        assert!(commands.is_empty());
    }
}
