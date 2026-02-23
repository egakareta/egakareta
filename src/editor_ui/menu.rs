use crate::commands::AppCommand;
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

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    let fade_progress = (p / 0.4).min(1.0);
    let move_progress = ((p - 0.4) / 0.6).max(0.0);

    let eased_move = ease(EaseInOutCubic, 0.0, 1.0, move_progress);

    let eased_scale = if p < 0.5 {
        ease(EaseOutCubic, 1.2, 0.7, p * 2.0)
    } else {
        ease(EaseInOutCubic, 0.7, 0.8, (p - 0.5) * 2.0)
    };

    let max_width = (ctx.content_rect().width() * 0.68).max(240.0);
    let menu_scale = (max_width / texture_size.x).min(1.0);
    let display_size = texture_size * menu_scale * eased_scale;

    let center = ctx.content_rect().center();
    let top_y = 28.0 + display_size.y / 2.0;
    let target_y = ease(Linear, center.y, top_y, eased_move);

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

/// Shows the menu wordmark UI overlay with a PLAY button.
/// Returns any commands triggered by UI interactions.
pub fn show_menu_wordmark_ui(
    ctx: &egui::Context,
    state: &mut State,
    wordmark: &egui::TextureHandle,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();

    if !state.is_menu() {
        return commands;
    }

    // Don't show if we're in level select mode (we show a different UI there)
    if state.is_level_select() {
        return commands;
    }

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return commands;
    }

    let max_width = (ctx.content_rect().width() * 0.68).max(240.0);
    let scale = (max_width / texture_size.x).min(1.0) * 0.8;
    let display_size = texture_size * scale;

    // Calculate wordmark position
    let wordmark_y = 28.0 + display_size.y / 2.0;

    // Show the wordmark
    egui::Area::new("menu_wordmark_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)));
        });

    // Show PLAY button below the wordmark
    let button_area = egui::Area::new("menu_play_button_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, wordmark_y + 20.0));

    button_area.show(ctx, |ui| {
        // Large PLAY button
        let button_size = egui::vec2(200.0, 60.0);
        let button_response = ui.add_sized(
            button_size,
            egui::Button::new("PLAY").fill(egui::Color32::from_rgb(0x2D, 0x7D, 0x32)),
        );

        if button_response.clicked() {
            commands.push(AppCommand::EnterLevelSelect);
        }
    });

    commands
}

/// Shows the level select UI with level preview and navigation.
/// Returns any commands triggered by UI interactions.
pub fn show_level_select_ui(ctx: &egui::Context, state: &mut State) -> Vec<AppCommand> {
    let mut commands = Vec::new();

    if !state.is_level_select() {
        return commands;
    }

    let levels = state.menu_levels();
    let selected_index = state.level_select_index();

    if levels.is_empty() {
        return commands;
    }

    let level_count = levels.len();
    let selected_level_name = levels.get(selected_index).cloned().unwrap_or_default();

    // Show level name at the top
    egui::Area::new("level_select_title".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.heading(&selected_level_name);
        });

    // Show navigation arrows and level indicator
    let center = ctx.content_rect().center();
    let nav_y = center.y;

    egui::Area::new("level_select_nav".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, nav_y - 30.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Previous button
                if ui.button("◄").clicked() {
                    commands.push(AppCommand::LevelSelectPrevLevel);
                }

                ui.label(format!("{}/{}", selected_index + 1, level_count));

                // Next button
                if ui.button("►").clicked() {
                    commands.push(AppCommand::LevelSelectNextLevel);
                }
            });
        });

    // Show back and play buttons at the bottom
    let button_y = ctx.content_rect().max.y - 100.0;

    egui::Area::new("level_select_buttons".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, button_y))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Back button
                if ui.button("BACK").clicked() {
                    commands.push(AppCommand::ExitLevelSelect);
                }

                ui.add_space(40.0);

                // Play button
                if ui.button("PLAY").clicked() {
                    commands.push(AppCommand::LevelSelectPlay);
                }
            });
        });

    commands
}
