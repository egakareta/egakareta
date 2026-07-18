/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::UiCommandSink;
use crate::modes::shared::{
    EditorPropertyPopup, show_editor_property_popup, show_player_camera_status_row,
};
use eg_state::commands::AppCommand;
use eg_state::state::editor_command::EditorCommand;

pub(crate) fn show_tapping_mode_bottom_panel(
    ui: &mut egui::Ui,
    timeline_preview_position: [f32; 3],
    timeline_preview_direction: eg_types::Direction,
    camera_preview_position: [f32; 3],
    camera_preview_target: [f32; 3],
    commands: &mut impl UiCommandSink,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(format!(
                "{} Add tap at current time",
                egui_phosphor::regular::PLUS_CIRCLE
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::AddTap));
        }
        if ui
            .button(format!("{} Clear all taps", egui_phosphor::regular::BROOM))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::ClearTaps));
        }

        ui.separator();

        show_player_camera_status_row(
            ui,
            timeline_preview_position,
            timeline_preview_direction,
            camera_preview_position,
            camera_preview_target,
        );
    });
}

pub(crate) fn show_selected_tap_properties_window(
    ctx: &egui::Context,
    selected_tap: Option<(usize, f32, [f32; 3])>,
    timeline_duration_seconds: f32,
    bottom_bar_height: f32,
    commands: &mut impl UiCommandSink,
) {
    let Some((selected_tap_index, selected_tap_time_seconds, selected_tap_position)) = selected_tap
    else {
        return;
    };

    show_editor_property_popup(
        ctx,
        EditorPropertyPopup::above_bottom_bar("selected_tap_properties", bottom_bar_height),
        |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Tap #{}", selected_tap_index + 1));
                    if ui
                        .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                        .clicked()
                    {
                        commands.push(AppCommand::Editor(EditorCommand::RemoveTap));
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Time (s):");
                    let mut time_seconds = selected_tap_time_seconds;
                    if ui
                        .add(
                            egui::DragValue::new(&mut time_seconds)
                                .speed(0.01)
                                .range(0.0..=timeline_duration_seconds.max(0.0)),
                        )
                        .changed()
                    {
                        commands.push(AppCommand::Editor(EditorCommand::SetSelectedTapTime(
                            time_seconds,
                        )));
                    }
                });

                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    selected_tap_position[0], selected_tap_position[1], selected_tap_position[2]
                ));
            });
        },
    );
}
