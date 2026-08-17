/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_state::state::EditorConfigState;
use eg_types::{AppSettings, Direction, EditorMode};

use crate::UiCommandSink;

const PROPERTY_POPUP_MARGIN: f32 = 12.0;
const MODE_SELECTOR_WIDTH: f32 = 48.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorPropertyPopup {
    id: egui::Id,
    anchor: egui::Align2,
    offset: egui::Vec2,
    order: egui::Order,
    min_width: f32,
}

impl EditorPropertyPopup {
    pub(crate) fn above_bottom_bar(
        id_source: impl std::hash::Hash + std::fmt::Debug,
        bottom_bar_height: f32,
    ) -> Self {
        Self {
            id: egui::Id::new(id_source),
            anchor: egui::Align2::LEFT_BOTTOM,
            offset: egui::Vec2::new(
                PROPERTY_POPUP_MARGIN,
                -PROPERTY_POPUP_MARGIN - bottom_bar_height,
            ),
            order: egui::Order::Foreground,
            min_width: 220.0,
        }
    }

    pub(crate) fn top_left(
        id_source: impl std::hash::Hash + std::fmt::Debug,
        top_bar_height: f32,
    ) -> Self {
        Self {
            id: egui::Id::new(id_source),
            anchor: egui::Align2::LEFT_TOP,
            offset: egui::Vec2::new(
                PROPERTY_POPUP_MARGIN,
                PROPERTY_POPUP_MARGIN + top_bar_height,
            ),
            order: egui::Order::Foreground,
            min_width: 0.0,
        }
    }

    pub(crate) fn top_left_right_of(
        id_source: impl std::hash::Hash + std::fmt::Debug,
        top_bar_height: f32,
    ) -> Self {
        Self {
            id: egui::Id::new(id_source),
            anchor: egui::Align2::LEFT_TOP,
            offset: egui::Vec2::new(
                PROPERTY_POPUP_MARGIN + MODE_SELECTOR_WIDTH + PROPERTY_POPUP_MARGIN,
                PROPERTY_POPUP_MARGIN + top_bar_height,
            ),
            order: egui::Order::Foreground,
            min_width: 0.0,
        }
    }
}

pub(crate) fn show_editor_property_popup(
    ctx: &egui::Context,
    popup: EditorPropertyPopup,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Area::new(popup.id)
        .anchor(popup.anchor, popup.offset)
        .order(popup.order)
        .show(ctx, |ui| {
            egui::Frame::popup(&ctx.global_style()).show(ui, |ui| {
                ui.set_min_width(popup.min_width);
                add_contents(ui);
            });
        });
}

pub(crate) fn show_mode_selector_popup(
    ctx: &egui::Context,
    app_settings: &AppSettings,
    mode: EditorMode,
    config: &EditorConfigState,
    top_bar_height: f32,
    commands: &mut impl UiCommandSink,
) {
    let popup = EditorPropertyPopup::top_left("mode_selector_popup", top_bar_height);
    show_editor_property_popup(ctx, popup, |ui| {
        ui.spacing_mut().button_padding = egui::Vec2::new(8.0, 6.0);
        let big_icon = |icon: &str| egui::RichText::new(icon).size(20.0);
        if ui
            .selectable_label(
                mode == EditorMode::Select,
                big_icon(egui_phosphor::regular::CURSOR_CLICK),
            )
            .on_hover_text(format!("Select{}", app_settings.hotkey_hint("mode_select")))
            .clicked()
        {
            commands.set_editor_mode(EditorMode::Select);
        }
        if ui
            .selectable_label(
                mode == EditorMode::Move,
                big_icon(egui_phosphor::regular::ARROWS_OUT),
            )
            .on_hover_text(format!("Move{}", app_settings.hotkey_hint("mode_move")))
            .clicked()
        {
            commands.set_editor_mode(EditorMode::Move);
        }
        if ui
            .selectable_label(
                mode == EditorMode::Scale,
                big_icon(egui_phosphor::regular::CORNERS_OUT),
            )
            .on_hover_text(format!("Scale{}", app_settings.hotkey_hint("mode_scale")))
            .clicked()
        {
            commands.set_editor_mode(EditorMode::Scale);
        }
        if ui
            .selectable_label(
                mode == EditorMode::Rotate,
                big_icon(egui_phosphor::regular::ARROWS_CLOCKWISE),
            )
            .on_hover_text(format!("Rotate{}", app_settings.hotkey_hint("mode_rotate")))
            .clicked()
        {
            commands.set_editor_mode(EditorMode::Rotate);
        }
    });

    if mode == EditorMode::Move || mode == EditorMode::Scale || mode == EditorMode::Rotate {
        let secondary =
            EditorPropertyPopup::top_left_right_of("mode_secondary_popup", top_bar_height);
        show_editor_property_popup(ctx, secondary, |ui| {
            ui.spacing_mut().button_padding = egui::Vec2::new(4.0, 2.0);
            match mode {
                EditorMode::Move | EditorMode::Scale => {
                    show_grid_snap_controls(ui, app_settings, config, commands);
                }
                EditorMode::Rotate => {
                    show_rotation_snap_controls(ui, app_settings, config, commands);
                }
                _ => {}
            }
        });
    }
}

fn show_grid_snap_controls(
    ui: &mut egui::Ui,
    app_settings: &AppSettings,
    config: &EditorConfigState,
    commands: &mut impl UiCommandSink,
) {
    ui.horizontal(|ui| {
        let mut snap = config.snap_to_grid;
        let saved_icon_width = ui.spacing().icon_width;
        ui.spacing_mut().icon_width = ui.spacing().interact_size.y;
        let checkbox = ui.add(egui::Checkbox::without_text(&mut snap));
        ui.spacing_mut().icon_width = saved_icon_width;
        let changed = checkbox.changed();
        checkbox.on_hover_text(format!(
            "Grid Snap{}",
            app_settings.hotkey_hint("toggle_grid_snap")
        ));
        if changed {
            commands.set_snap_to_grid(snap);
        }
        let mut snap_step = config.snap_step;
        let drag: egui::Response = ui
            .add(
                egui::DragValue::new(&mut snap_step)
                    .speed(0.05)
                    .range(0.05..=100.0)
                    .prefix(format!("{} ", egui_phosphor::regular::GRID_FOUR)),
            )
            .on_hover_text("Grid Snap Step");
        if drag.changed() {
            commands.set_snap_step(snap_step);
        }
    });
}

fn show_rotation_snap_controls(
    ui: &mut egui::Ui,
    app_settings: &AppSettings,
    config: &EditorConfigState,
    commands: &mut impl UiCommandSink,
) {
    ui.horizontal(|ui| {
        let mut snap_rotation = config.snap_rotation;
        let saved_icon_width = ui.spacing().icon_width;
        ui.spacing_mut().icon_width = ui.spacing().interact_size.y;
        let checkbox = ui.add(egui::Checkbox::without_text(&mut snap_rotation));
        ui.spacing_mut().icon_width = saved_icon_width;
        let changed = checkbox.changed();
        checkbox.on_hover_text(format!(
            "Rotation Snap{}",
            app_settings.hotkey_hint("toggle_rotation_snap")
        ));
        if changed {
            commands.set_snap_rotation(snap_rotation);
        }

        let mut snap_rotation_step = config.snap_rotation_step_degrees;
        let drag = ui
            .add(
                egui::DragValue::new(&mut snap_rotation_step)
                    .speed(0.5)
                    .range(1.0..=180.0)
                    .prefix(format!("{} ", egui_phosphor::regular::ARROWS_CLOCKWISE))
                    .suffix("°"),
            )
            .on_hover_text("Rotation Snap Step");
        if drag.changed() {
            commands.set_snap_rotation_step(snap_rotation_step);
        }
    });
}

pub(crate) fn show_player_camera_status_row(
    ui: &mut egui::Ui,
    position: [f32; 3],
    direction: Direction,
    camera_preview_position: [f32; 3],
    camera_preview_target: [f32; 3],
) {
    let direction_label = match direction {
        Direction::Forward => "Forward",
        Direction::Right => "Right",
    };
    ui.horizontal(|ui| {
        ui.label(format!(
            "Player: ({:.1}, {:.1}, {:.1}) | {}",
            position[0], position[1], position[2], direction_label
        ));
        ui.separator();
        ui.label(format!(
            "Player Camera: ({:.1}, {:.1}, {:.1}) -> ({:.1}, {:.1}, {:.1})",
            camera_preview_position[0],
            camera_preview_position[1],
            camera_preview_position[2],
            camera_preview_target[0],
            camera_preview_target[1],
            camera_preview_target[2],
        ));
    });
}

#[cfg(test)]
mod tests {
    use eg_state::state::EditorConfigState;
    use eg_types::{AppSettings, Direction, EditorMode};

    use super::{
        EditorPropertyPopup, PROPERTY_POPUP_MARGIN, show_editor_property_popup,
        show_mode_selector_popup, show_player_camera_status_row,
    };
    use crate::test_utils::with_world_command_sink;

    #[test]
    fn property_popup_runs_content_closure_with_bottom_bar_offset() {
        let popup = EditorPropertyPopup::above_bottom_bar("test_property_popup", 48.0);
        assert_eq!(popup.anchor, egui::Align2::LEFT_BOTTOM);
        assert_eq!(popup.offset.x, PROPERTY_POPUP_MARGIN);
        assert_eq!(popup.offset.y, -PROPERTY_POPUP_MARGIN - 48.0);

        let ctx = egui::Context::default();
        let mut rendered = false;
        let _ = ctx.run_ui(egui::RawInput::default(), |_root_ui| {
            show_editor_property_popup(&ctx, popup, |ui| {
                rendered = true;
                ui.label("Property popup");
            });
        });

        assert!(rendered);
    }

    #[test]
    fn show_player_camera_status_row_renders_for_forward_and_right_directions() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                show_player_camera_status_row(
                    ui,
                    [2.0, 1.0, 0.0],
                    Direction::Forward,
                    [3.0, 2.0, 1.0],
                    [0.0, 0.0, 0.0],
                );
                show_player_camera_status_row(
                    ui,
                    [2.0, 1.0, 0.0],
                    Direction::Right,
                    [3.0, 2.0, 1.0],
                    [0.0, 0.0, 0.0],
                );
            });
        });
    }

    #[test]
    fn mode_selector_popup_uses_top_left_anchor_and_renders_modes() {
        let app_settings = AppSettings::default();
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |_root_ui| {
                show_mode_selector_popup(
                    &ctx,
                    &app_settings,
                    EditorMode::Select,
                    &EditorConfigState::default(),
                    28.0,
                    commands,
                );
            })
        });
    }

    #[test]
    fn mode_selector_popup_shows_secondary_for_move_mode() {
        let app_settings = AppSettings::default();
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |_root_ui| {
                show_mode_selector_popup(
                    &ctx,
                    &app_settings,
                    EditorMode::Move,
                    &EditorConfigState::default(),
                    28.0,
                    commands,
                );
            })
        });
    }

    #[test]
    fn mode_selector_popup_shows_secondary_for_scale_mode() {
        let app_settings = AppSettings::default();
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |_root_ui| {
                show_mode_selector_popup(
                    &ctx,
                    &app_settings,
                    EditorMode::Scale,
                    &EditorConfigState::default(),
                    28.0,
                    commands,
                );
            })
        });
    }

    #[test]
    fn mode_selector_popup_shows_secondary_for_rotate_mode() {
        let app_settings = AppSettings::default();
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |_root_ui| {
                show_mode_selector_popup(
                    &ctx,
                    &app_settings,
                    EditorMode::Rotate,
                    &EditorConfigState::default(),
                    28.0,
                    commands,
                );
            })
        });
    }

    #[test]
    fn top_left_right_of_positions_to_the_right() {
        let popup = EditorPropertyPopup::top_left_right_of("test_right_popup", 28.0);
        assert_eq!(popup.anchor, egui::Align2::LEFT_TOP);
        assert!(
            popup.offset.x > PROPERTY_POPUP_MARGIN,
            "Secondary popup offset should be beyond the primary popup"
        );
        assert_eq!(popup.offset.y, PROPERTY_POPUP_MARGIN + 28.0);
    }
}
