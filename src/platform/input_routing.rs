pub(crate) fn should_route_pointer_input(egui_consumed: bool, ui_wants_pointer: bool) -> bool {
    !(egui_consumed || ui_wants_pointer)
}

pub(crate) fn should_route_keyboard_input(egui_consumed: bool, ui_wants_keyboard: bool) -> bool {
    !(egui_consumed || ui_wants_keyboard)
}
