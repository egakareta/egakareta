/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::State;

const MENU_FAVICON_PNG: &[u8] = include_bytes!("../../assets/favicon.png");

/// Loads the menu favicon texture from embedded PNG data.
pub fn load_menu_favicon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(MENU_FAVICON_PNG).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture("menu_favicon", color_image, egui::TextureOptions::LINEAR))
}

/// Shows the menu favicon UI overlay.
pub fn show_menu_favicon_ui(ctx: &egui::Context, state: &State, favicon: &egui::TextureHandle) {
    if !state.is_menu() {
        return;
    }

    let texture_size = favicon.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    let ui_scale = ctx.pixels_per_point();
    let display_height = ctx.content_rect().height() * 0.2 * ui_scale;
    let scale = display_height / texture_size.y;
    let display_size = texture_size * scale;

    egui::Area::new("menu_favicon_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((favicon.id(), display_size)));
        });
}

/// Shows the menu topbar with the current time.
pub fn show_menu_topbar(ctx: &egui::Context, state: &State) {
    if !state.is_menu() {
        return;
    }

    egui::TopBottomPanel::top("menu_top_bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::GAME_CONTROLLER)
                    .size(18.0)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.label(egui::RichText::new("egakareta").strong());

            ui.separator();

            let level_name = state.menu_level_name().unwrap_or("No Level Selected");

            ui.label(format!(
                "{} {}",
                egui_phosphor::regular::MAP_TRIFOLD,
                level_name
            ));

            ui.separator();
            ui.label(egui_phosphor::regular::CLOCK);
            ui.label(egui::RichText::new(get_current_time_str()).monospace());
        });
    });
}

/// Shows the menu play button UI overlay.
pub fn show_menu_play_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_menu() {
        return;
    }

    let ui_scale = ctx.pixels_per_point();
    let screen_height = ctx.content_rect().height();
    let icon_size = screen_height * 0.07 * ui_scale;
    let padding = egui::vec2(24.0, 16.0);
    let offset_y = 40.0;

    egui::Area::new("menu_play_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, offset_y))
        .show(ctx, |ui| {
            ui.spacing_mut().button_padding = padding;

            let play_text = egui::RichText::new(egui_phosphor::regular::PLAY)
                .size(icon_size)
                .strong();

            let button = egui::Button::new(play_text)
                .corner_radius(16.0)
                .stroke(egui::Stroke::new(2.0, ui.visuals().strong_text_color()))
                .fill(ui.visuals().window_fill());

            if ui
                .add(button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                state.turn_right();
            }
        });
}

fn get_current_time_str() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let hours = (now / 3600) % 24;
        let minutes = (now / 60) % 60;
        format!("{:02}:{:02} UTC", hours, minutes)
    }
}
