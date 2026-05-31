/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::EditorUiViewModel;

pub(crate) fn show_tapping_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Tapping:");
        if ui.button("Add tap at current time").clicked() {
            commands.push(AppCommand::EditorAddTap);
        }
        if ui.button("Remove tap").clicked() {
            commands.push(AppCommand::EditorRemoveTap);
        }
        if ui.button("Clear taps").clicked() {
            commands.push(AppCommand::EditorClearTaps);
        }
    });

    if let Some(selected_tap) = view.selected_tap {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Selected tap #{}", selected_tap.index + 1));
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
