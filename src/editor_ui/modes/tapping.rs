/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::EditorUiViewModel;

pub(crate) fn show_tapping_mode_bottom_panel(
    ui: &mut egui::Ui,
    _view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Tapping:");
        if ui.button("Add tap").clicked() {
            commands.push(AppCommand::EditorAddTap);
        }
        if ui.button("Remove tap").clicked() {
            commands.push(AppCommand::EditorRemoveTap);
        }
        if ui.button("Clear taps").clicked() {
            commands.push(AppCommand::EditorClearTaps);
        }
    });
}
