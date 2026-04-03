/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
pub(crate) fn should_route_pointer_input(egui_consumed: bool, ui_wants_pointer: bool) -> bool {
    !(egui_consumed || ui_wants_pointer)
}

pub(crate) fn should_route_keyboard_input(egui_consumed: bool, ui_wants_keyboard: bool) -> bool {
    !(egui_consumed || ui_wants_keyboard)
}
