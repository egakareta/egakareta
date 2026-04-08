/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::State;

const MENU_WORDMARK_PNG: &[u8] = include_bytes!("../../assets/wordmark.png");

/// Loads the menu wordmark texture from embedded PNG data.
pub fn load_menu_wordmark_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(MENU_WORDMARK_PNG).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture("menu_wordmark", color_image, egui::TextureOptions::LINEAR))
}

/// Shows the menu wordmark UI overlay.
pub fn show_menu_wordmark_ui(ctx: &egui::Context, state: &State, wordmark: &egui::TextureHandle) {
    if !state.is_menu() {
        return;
    }

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    let max_width = (ctx.content_rect().width() * 0.68).max(240.0);
    let scale = (max_width / texture_size.x).min(1.0) * 0.8;
    let display_size = texture_size * scale;

    egui::Area::new("menu_wordmark_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)));
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
