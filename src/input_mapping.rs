#[cfg(any(target_arch = "wasm32", test))]
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

#[cfg(not(target_arch = "wasm32"))]
pub fn key_str_from_winit(logical_key: &winit::keyboard::Key) -> String {
    match logical_key {
        winit::keyboard::Key::Named(named_key) => format!("{:?}", named_key),
        winit::keyboard::Key::Character(character) => character.to_string(),
        _ => String::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
pub fn zoom_delta_from_winit(delta: winit::event::MouseScrollDelta) -> f32 {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
        winit::event::MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.02,
    }
}

#[cfg(test)]
mod tests {
    use super::egui_key_from_key_str;

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
}
