/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) fn should_route_pointer_input(egui_consumed: bool, ui_wants_pointer: bool) -> bool {
    !(egui_consumed || ui_wants_pointer)
}

pub(crate) fn should_route_keyboard_input(egui_consumed: bool, ui_wants_keyboard: bool) -> bool {
    !(egui_consumed || ui_wants_keyboard)
}

#[cfg(test)]
mod tests {
    use super::{should_route_keyboard_input, should_route_pointer_input};

    #[test]
    fn pointer_input_routes_only_when_not_consumed_and_not_requested_by_ui() {
        assert!(should_route_pointer_input(false, false));
        assert!(!should_route_pointer_input(true, false));
        assert!(!should_route_pointer_input(false, true));
        assert!(!should_route_pointer_input(true, true));
    }

    #[test]
    fn keyboard_input_routes_only_when_not_consumed_and_not_requested_by_ui() {
        assert!(should_route_keyboard_input(false, false));
        assert!(!should_route_keyboard_input(true, false));
        assert!(!should_route_keyboard_input(false, true));
        assert!(!should_route_keyboard_input(true, true));
    }
}
