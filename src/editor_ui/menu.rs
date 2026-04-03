/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::State;
use keyframe::{ease, functions::*};

const MENU_WORDMARK_PNG: &[u8] = include_bytes!("../../assets/wordmark.png");

/// Loads the menu wordmark texture from embedded PNG data.
pub fn load_menu_wordmark_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(MENU_WORDMARK_PNG).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture("menu_wordmark", color_image, egui::TextureOptions::LINEAR))
}

/// Shows the splash screen UI overlay.
pub fn show_splash_screen_ui(ctx: &egui::Context, state: &State, wordmark: &egui::TextureHandle) {
    if !state.is_splash() {
        return;
    }

    let p = state.splash_progress();

    // Background is already handled by the clear color in render_with_overlay.
    // We just need to draw the wordmark with animations.

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    // Phase 1: 0.0 -> 0.4 (Fade & Initial scale)
    // Phase 2: 0.4 -> 1.0 (Move & Scale adjustment)

    let fade_progress = (p / 0.4).min(1.0);
    let move_progress = ((p - 0.4) / 0.6).max(0.0);

    let eased_move = ease(EaseInOutCubic, 0.0, 1.0, move_progress);

    // "Scale down and up"
    let eased_scale = if p < 0.5 {
        ease(EaseOutCubic, 1.2, 0.7, p * 2.0)
    } else {
        ease(EaseInOutCubic, 0.7, 0.8, (p - 0.5) * 2.0)
    };

    let max_width = (ctx.content_rect().width() * 0.68).max(240.0);
    let menu_scale = (max_width / texture_size.x).min(1.0);
    let display_size = texture_size * menu_scale * eased_scale;

    // Calculate position
    let center = ctx.content_rect().center();
    let top_y = 28.0 + display_size.y / 2.0;
    let target_y = ease(Linear, center.y, top_y, eased_move);

    // Color animation: White -> Normal
    // If we assume wordmark is white, we can tint it.
    // To make it "completely white" initially, we use a high brightness or just white tint.
    let tint_color = if fade_progress < 1.0 {
        let white_amount = 1.0 - fade_progress;
        let white_val = (255.0 * (1.0 + white_amount * 2.0)).min(255.0f32) as u8;
        egui::Color32::from_rgb(white_val, white_val, white_val)
    } else {
        egui::Color32::WHITE
    };

    egui::Area::new("splash_wordmark_area".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(
            center.x - display_size.x / 2.0,
            target_y - display_size.y / 2.0,
        ))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)).tint(tint_color));
        });
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
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)));
        });
}
