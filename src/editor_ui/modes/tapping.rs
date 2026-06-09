/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::editor_ui::modes::shared::{
    show_editor_property_popup, show_player_camera_status_row, EditorPropertyPopup,
};
use crate::state::EditorUiViewModel;

pub(crate) fn show_tapping_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(format!(
                "{} Add tap at current time",
                egui_phosphor::regular::PLUS_CIRCLE
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(
                crate::state::editor_command::EditorCommand::AddTap,
            ));
        }
        if ui
            .button(format!("{} Clear all taps", egui_phosphor::regular::BROOM))
            .clicked()
        {
            commands.push(AppCommand::Editor(
                crate::state::editor_command::EditorCommand::ClearTaps,
            ));
        }

        ui.separator();

        show_player_camera_status_row(ui, view);
    });
}

pub(crate) fn show_selected_tap_properties_window(
    ctx: &egui::Context,
    view: &EditorUiViewModel<'_>,
    bottom_bar_height: f32,
    commands: &mut Vec<AppCommand>,
) {
    let Some(selected_tap) = view.selected_tap else {
        return;
    };

    show_editor_property_popup(
        ctx,
        EditorPropertyPopup::above_bottom_bar("selected_tap_properties", bottom_bar_height),
        |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Tap #{}", selected_tap.index + 1));
                    if ui
                        .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                        .clicked()
                    {
                        commands.push(AppCommand::Editor(
                            crate::state::editor_command::EditorCommand::RemoveTap,
                        ));
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Time (s):");
                    let mut time_seconds = selected_tap.time_seconds;
                    if ui
                        .add(
                            egui::DragValue::new(&mut time_seconds)
                                .speed(0.01)
                                .range(0.0..=view.timeline_duration_seconds.max(0.0)),
                        )
                        .changed()
                    {
                        commands.push(AppCommand::Editor(
                            crate::state::editor_command::EditorCommand::SetSelectedTapTime(
                                time_seconds,
                            ),
                        ));
                    }
                });

                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    selected_tap.position[0], selected_tap.position[1], selected_tap.position[2]
                ));
            });
        },
    );
}
