/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(test)]
pub(crate) fn egui_key_from_key_str(key: &str) -> Option<egui::Key> {
    match key {
        "ArrowUp" => Some(egui::Key::ArrowUp),
        "ArrowDown" => Some(egui::Key::ArrowDown),
        "ArrowLeft" => Some(egui::Key::ArrowLeft),
        "ArrowRight" => Some(egui::Key::ArrowRight),
        "Enter" => Some(egui::Key::Enter),
        "Backspace" => Some(egui::Key::Backspace),
        "Delete" => Some(egui::Key::Delete),
        "Tab" => Some(egui::Key::Tab),
        _ => None,
    }
}

/// Converts a winit key to a string representation.
pub fn key_str_from_winit(logical_key: &winit::keyboard::Key) -> String {
    match logical_key {
        winit::keyboard::Key::Named(named_key) => format!("{:?}", named_key),
        winit::keyboard::Key::Character(character) => character.to_string(),
        _ => String::new(),
    }
}

/// Converts a winit mouse button to an index.
pub fn mouse_button_index_from_winit(button: winit::event::MouseButton) -> u32 {
    match button {
        winit::event::MouseButton::Left => 0,
        winit::event::MouseButton::Right => 2,
        winit::event::MouseButton::Middle => 1,
        winit::event::MouseButton::Back => 3,
        winit::event::MouseButton::Forward => 4,
        winit::event::MouseButton::Other(index) => index as u32,
    }
}

/// Converts a winit mouse scroll delta to a zoom delta.
pub fn zoom_delta_from_winit(delta: winit::event::MouseScrollDelta) -> f32 {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
        winit::event::MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.02,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        egui_key_from_key_str, key_str_from_winit, mouse_button_index_from_winit,
        zoom_delta_from_winit,
    };

    #[test]
    fn maps_supported_egui_keys() {
        assert_eq!(egui_key_from_key_str("ArrowUp"), Some(egui::Key::ArrowUp));
        assert_eq!(egui_key_from_key_str("Tab"), Some(egui::Key::Tab));
    }

    #[test]
    fn returns_none_for_unknown_keys() {
        assert_eq!(egui_key_from_key_str("x"), None);
        assert_eq!(egui_key_from_key_str("Space"), None);
    }

    #[test]
    fn maps_winit_named_and_character_keys_to_strings() {
        let named = winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter);
        assert_eq!(key_str_from_winit(&named), "Enter");

        let character = winit::keyboard::Key::Character("k".into());
        assert_eq!(key_str_from_winit(&character), "k");
    }

    #[test]
    fn maps_mouse_button_indices() {
        assert_eq!(
            mouse_button_index_from_winit(winit::event::MouseButton::Left),
            0
        );
        assert_eq!(
            mouse_button_index_from_winit(winit::event::MouseButton::Middle),
            1
        );
        assert_eq!(
            mouse_button_index_from_winit(winit::event::MouseButton::Right),
            2
        );
        assert_eq!(
            mouse_button_index_from_winit(winit::event::MouseButton::Other(9)),
            9
        );
    }

    #[test]
    fn maps_scroll_delta_to_zoom_delta() {
        let line = winit::event::MouseScrollDelta::LineDelta(0.0, 2.5);
        assert_eq!(zoom_delta_from_winit(line), 2.5);

        let pixel = winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            0.0, 50.0,
        ));
        assert!((zoom_delta_from_winit(pixel) - 1.0).abs() < 1e-6);
    }
}
