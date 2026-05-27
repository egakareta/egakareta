/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::State;

const MENU_FAVICON_PNG: &[u8] = include_bytes!("../../assets/darkicon.png");

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
#[allow(deprecated)]
pub fn show_menu_topbar(ctx: &egui::Context, state: &State) {
    if !state.is_menu() {
        return;
    }

    egui::Panel::top("menu_top_bar").show(ctx, |ui| {
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

/// Shows the main-menu account status and sign-in dialog.
pub fn show_menu_auth_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_menu() {
        return;
    }

    let mut commands = Vec::new();

    egui::Area::new("menu_auth_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 7.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(name) = state.auth_display_name() {
                    ui.label(format!("{} {}", egui_phosphor::regular::USER, name));
                    if ui
                        .add_enabled(!state.auth_pending(), egui::Button::new("Sign out"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        commands.push(AppCommand::AuthSignOut);
                    }
                } else {
                    if ui
                        .add_enabled(
                            !state.auth_pending(),
                            egui::Button::new(format!(
                                "{} Sign in",
                                egui_phosphor::regular::SIGN_IN
                            )),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        commands.push(AppCommand::AuthSubmitSignIn);
                    }
                    if ui
                        .add_enabled(!state.auth_pending(), egui::Button::new("Create account"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        commands.push(AppCommand::AuthOpenSignup);
                    }
                    if state.auth_pending() {
                        ui.spinner();
                    }
                }
            });
        });

    if let Some(message) = state.auth_message() {
        egui::Area::new("menu_auth_message_area".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 42.0))
            .show(ctx, |ui| {
                ui.colored_label(ui.visuals().warn_fg_color, message);
            });
    }

    for command in commands {
        state.dispatch(command);
    }
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

#[cfg(test)]
mod tests {
    use super::{show_menu_auth_ui, show_menu_topbar};
    use crate::types::{AuthProfile, AuthSession, AuthSessionTokens, AuthUser};

    fn test_auth_session(username: Option<&str>) -> AuthSession {
        AuthSession {
            session: AuthSessionTokens {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires_at: Some(123),
                token_type: "bearer".to_string(),
            },
            user: AuthUser {
                id: "user-id".to_string(),
                email: Some("player@example.com".to_string()),
            },
            profile: username.map(|name| AuthProfile {
                id: "user-id".to_string(),
                username: Some(name.to_string()),
                avatar_url: None,
                country: "UN".to_string(),
            }),
        }
    }

    fn run_menu_auth_ui_once(state: &mut crate::State) {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            let ctx = root_ui.ctx();
            show_menu_auth_ui(ctx, state);
        });
    }

    #[test]
    fn menu_topbar_and_auth_ui_render_guest_state() {
        pollster::block_on(async {
            let mut state = crate::State::new_test().await;
            let ctx = egui::Context::default();

            let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
                let ctx = root_ui.ctx();
                show_menu_topbar(ctx, &state);
            });
            run_menu_auth_ui_once(&mut state);

            assert_eq!(state.auth_display_name(), None);
        });
    }

    #[test]
    fn menu_auth_ui_renders_pending_message_and_signed_in_state() {
        pollster::block_on(async {
            let mut state = crate::State::new_test().await;

            state.set_auth_state_for_test(
                None,
                true,
                Some("Complete sign-in in your browser.".to_string()),
            );
            run_menu_auth_ui_once(&mut state);
            assert!(state.auth_pending());
            assert_eq!(
                state.auth_message(),
                Some("Complete sign-in in your browser.")
            );

            state.set_auth_state_for_test(Some(test_auth_session(Some("player"))), false, None);
            run_menu_auth_ui_once(&mut state);
            assert_eq!(state.auth_display_name(), Some("player"));
        });
    }
}
