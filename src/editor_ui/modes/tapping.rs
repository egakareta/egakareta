/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::editor_ui::modes::shared::show_player_camera_status_row;
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
            commands.push(AppCommand::EditorAddTap);
        }
        if ui
            .button(format!("{} Clear all taps", egui_phosphor::regular::BROOM))
            .clicked()
        {
            commands.push(AppCommand::EditorClearTaps);
        }

        ui.separator();

        show_player_camera_status_row(ui, view);
    });

    if let Some(selected_tap) = view.selected_tap {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Tap #{}", selected_tap.index + 1));
            if ui
                .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                .clicked()
            {
                commands.push(AppCommand::EditorRemoveTap);
            }
            ui.separator();
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
                commands.push(AppCommand::EditorSetSelectedTapTime(time_seconds));
            }
            ui.separator();
            ui.label(format!(
                "Position: ({:.2}, {:.2}, {:.2})",
                selected_tap.position[0], selected_tap.position[1], selected_tap.position[2]
            ));
        });
    }
}
