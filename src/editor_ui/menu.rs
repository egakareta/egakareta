/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::editor_ui::components::show_shadowed_label;
use crate::state::editor_command::EditorCommand;
use crate::State;

const MENU_FAVICON_PNG: &[u8] = include_bytes!("../../assets/darkicon.png");
const MENU_PLAY_BUTTON_PNG: &[u8] = include_bytes!("../../assets/circularplaybutton.png");
const MENU_PLAY_BUTTON_FILLED_PNG: &[u8] =
    include_bytes!("../../assets/circularplaybuttonfilled.png");
const MENU_GEM_ICON_SVG: &str = include_str!("../../assets/gem_icon.svg");

fn load_png_texture(
    ctx: &egui::Context,
    name: &'static str,
    png_bytes: &'static [u8],
) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(png_bytes).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
}

/// Loads the menu favicon texture from embedded PNG data.
pub fn load_menu_favicon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    load_png_texture(ctx, "menu_favicon", MENU_FAVICON_PNG)
}

/// Loads the main menu play button texture from embedded PNG data.
pub fn load_menu_play_button_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    load_png_texture(ctx, "menu_play_button", MENU_PLAY_BUTTON_PNG)
}

/// Loads the filled main menu play button texture from embedded PNG data.
pub fn load_menu_play_button_filled_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    load_png_texture(ctx, "menu_play_button_filled", MENU_PLAY_BUTTON_FILLED_PNG)
}

const MENU_LEVEL_TITLE_FONT_SIZE: f32 = 86.0;
const MENU_LEVEL_TITLE_MAX_WIDTH_PADDING: f32 = 32.0;
const MENU_LEVEL_TITLE_FONT_FAMILY: &str = "sora_thin";
const MENU_LEVEL_TITLE_SHADOW_OFFSET_X: f32 = 3.0;
const MENU_LEVEL_TITLE_SHADOW_OFFSET_Y: f32 = 3.0;
const MENU_LEVEL_PROGRESS_FONT_SIZE: f32 = 28.0;
const MENU_LEVEL_PROGRESS_SHADOW_OFFSET_X: f32 = 1.5;
const MENU_LEVEL_PROGRESS_SHADOW_OFFSET_Y: f32 = 1.5;
const MENU_LEVEL_PROGRESS_GAP_FROM_TITLE: f32 = 8.0;
const MENU_LEVEL_PROGRESS_TEXT_GAP: f32 = 10.0;
const MENU_LEVEL_PROGRESS_GEM_SIZE: f32 = 24.0;
const MENU_PLAY_BUTTON_MIN_SIZE: f32 = 88.0;
const MENU_PLAY_BUTTON_MAX_SIZE: f32 = 128.0;
const PAUSE_MENU_SIDE_BUTTON_SIZE: f32 = 70.0;
const PAUSE_MENU_PLAY_BUTTON_SIZE: f32 = 90.0;
const PAUSE_MENU_BUTTON_SPACING: f32 = 14.0;
const PAUSE_MENU_PRACTICE_GAP: f32 = 10.0;

/// Shows the selected level name in the menu hero position.
pub fn show_menu_favicon_ui(ctx: &egui::Context, state: &State, _favicon: &egui::TextureHandle) {
    if !state.is_menu() {
        return;
    }

    let level_name = state.menu_level_name().unwrap_or("No Level Selected");
    let max_width = (ctx.content_rect().width() - MENU_LEVEL_TITLE_MAX_WIDTH_PADDING).max(1.0);

    egui::Area::new("menu_favicon_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 44.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_max_width(max_width);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                show_shadowed_label(
                    ui,
                    level_name,
                    egui::FontId::new(
                        MENU_LEVEL_TITLE_FONT_SIZE,
                        egui::FontFamily::Name(MENU_LEVEL_TITLE_FONT_FAMILY.into()),
                    ),
                    ui.visuals().strong_text_color(),
                    egui::Color32::BLACK,
                    egui::vec2(
                        MENU_LEVEL_TITLE_SHADOW_OFFSET_X,
                        MENU_LEVEL_TITLE_SHADOW_OFFSET_Y,
                    ),
                    max_width,
                );
                ui.add_space(MENU_LEVEL_PROGRESS_GAP_FROM_TITLE);
                show_menu_level_progress_row(ui, state);
            });
        });
}

/// Shows the menu play button UI overlay.
pub fn show_menu_play_ui(
    ctx: &egui::Context,
    state: &mut State,
    play_button: Option<&egui::TextureHandle>,
    play_button_filled: Option<&egui::TextureHandle>,
) {
    if !state.is_menu() {
        return;
    }

    let ui_scale = ctx.pixels_per_point();
    let screen_height = ctx.content_rect().height();
    let icon_size = screen_height * 0.07 * ui_scale;
    let button_size = (screen_height * 0.25 * ui_scale)
        .clamp(MENU_PLAY_BUTTON_MIN_SIZE, MENU_PLAY_BUTTON_MAX_SIZE);
    let padding = egui::vec2(24.0, 16.0);
    let offset_y = 40.0;

    egui::Area::new("menu_play_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, offset_y))
        .show(ctx, |ui| {
            if let Some(play_button) = play_button {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(button_size, button_size),
                    egui::Sense::click(),
                );
                let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                let hover_alpha = ctx.animate_bool(response.id, response.hovered());

                if ui.is_rect_visible(rect) {
                    ui.painter().image(
                        play_button.id(),
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );

                    if let Some(play_button_filled) = play_button_filled {
                        ui.painter().image(
                            play_button_filled.id(),
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::from_white_alpha((hover_alpha * 255.0).round() as u8),
                        );
                    }
                }

                if response.clicked() {
                    state.turn_right();
                }
                return;
            }

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

/// Shows the in-game pause menu overlay for real gameplay sessions.
pub fn show_pause_menu_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_game_paused() {
        return;
    }

    let screen_rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        "pause_menu_scrim".into(),
    ));
    painter.rect_filled(
        screen_rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(5, 8, 12, 172),
    );

    let mut commands = Vec::new();
    let pause_menu_width = PAUSE_MENU_SIDE_BUTTON_SIZE * 2.0
        + PAUSE_MENU_PLAY_BUTTON_SIZE
        + PAUSE_MENU_BUTTON_SPACING * 2.0;

    egui::Area::new("pause_menu_controls".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(pause_menu_width);
            ui.spacing_mut().item_spacing.x = PAUSE_MENU_BUTTON_SPACING;

            ui.vertical_centered(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(pause_menu_width, PAUSE_MENU_PLAY_BUTTON_SIZE),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        if ui
                            .add_sized(
                                egui::vec2(
                                    PAUSE_MENU_SIDE_BUTTON_SIZE,
                                    PAUSE_MENU_SIDE_BUTTON_SIZE,
                                ),
                                egui::Button::new(
                                    egui::RichText::new(
                                        egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                                    )
                                    .size(36.0),
                                ),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            commands.push(AppCommand::GameRestartLevel);
                        }
                        if ui
                            .add_sized(
                                egui::vec2(
                                    PAUSE_MENU_PLAY_BUTTON_SIZE,
                                    PAUSE_MENU_PLAY_BUTTON_SIZE,
                                ),
                                egui::Button::new(
                                    egui::RichText::new(egui_phosphor::regular::PLAY).size(36.0),
                                ),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            commands.push(AppCommand::GameResume);
                        }
                        if ui
                            .add_sized(
                                egui::vec2(
                                    PAUSE_MENU_SIDE_BUTTON_SIZE,
                                    PAUSE_MENU_SIDE_BUTTON_SIZE,
                                ),
                                egui::Button::new(
                                    egui::RichText::new(egui_phosphor::regular::HOUSE).size(36.0),
                                ),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            commands.push(AppCommand::GameQuitToMenu);
                        }
                    },
                );

                ui.add_space(PAUSE_MENU_PRACTICE_GAP);

                let mut practice_enabled = state.is_practice_mode_enabled();
                ui.horizontal_centered(|ui| {
                    if ui
                        .checkbox(
                            &mut practice_enabled,
                            format!("{} {}", egui_phosphor::regular::FLAG, "Practice"),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .changed()
                    {
                        commands.push(AppCommand::GameSetPracticeMode(practice_enabled));
                    }
                });
            });
        });

    for command in commands {
        state.dispatch(command);
    }
}

/// Shows the practice-mode checkpoint placement button during real gameplay.
pub fn show_practice_checkpoint_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_practice_mode_enabled() || state.is_game_paused() {
        return;
    }

    let mut place_checkpoint = false;
    let mut remove_checkpoint = false;

    egui::Area::new("practice_checkpoint_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let checkpoint_icon_size = 36.0;
                if ui
                    .add_sized(
                        egui::vec2(70.0, 70.0),
                        egui::Button::new(
                            egui::RichText::new(egui_phosphor::regular::FLAG)
                                .size(checkpoint_icon_size),
                        ),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    place_checkpoint = true;
                }
                if ui
                    .add_sized(
                        egui::vec2(70.0, 70.0),
                        egui::Button::new(
                            egui::RichText::new(egui_phosphor::regular::TRASH)
                                .size(checkpoint_icon_size),
                        ),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    remove_checkpoint = true;
                }
            });
        });

    if place_checkpoint {
        state.dispatch(AppCommand::GameSetPracticeCheckpoint);
    }
    if remove_checkpoint {
        state.dispatch(AppCommand::GameRemovePracticeCheckpoint);
    }
}

fn show_menu_level_progress_row(ui: &mut egui::Ui, state: &State) {
    let Some(progress) = state.menu_selected_level_progress() else {
        return;
    };

    let percent_text = format!("{:.0}%", progress.progress_percent);
    let gems_text = format!("{}/{}", progress.gems_collected, progress.gems_max);

    show_shadowed_gem_progress(
        ui,
        percent_text,
        gems_text,
        egui::FontId::new(
            MENU_LEVEL_PROGRESS_FONT_SIZE,
            egui::FontFamily::Name(MENU_LEVEL_TITLE_FONT_FAMILY.into()),
        ),
        ui.visuals().strong_text_color(),
        egui::Color32::BLACK,
        egui::vec2(
            MENU_LEVEL_PROGRESS_SHADOW_OFFSET_X,
            MENU_LEVEL_PROGRESS_SHADOW_OFFSET_Y,
        ),
    );
}

fn show_shadowed_gem_progress(
    ui: &mut egui::Ui,
    percent_text: String,
    gems_text: String,
    font_id: egui::FontId,
    text_color: egui::Color32,
    shadow_color: egui::Color32,
    shadow_offset: egui::Vec2,
) -> egui::Response {
    debug_assert!(MENU_GEM_ICON_SVG.contains("<svg"));

    let percent_galley =
        ui.painter()
            .layout_no_wrap(percent_text, font_id.clone(), egui::Color32::PLACEHOLDER);
    let gems_galley = ui
        .painter()
        .layout_no_wrap(gems_text, font_id, egui::Color32::PLACEHOLDER);
    let shadow_extent = egui::vec2(shadow_offset.x.abs(), shadow_offset.y.abs());
    let content_width = percent_galley.size().x
        + MENU_LEVEL_PROGRESS_TEXT_GAP
        + MENU_LEVEL_PROGRESS_GEM_SIZE
        + MENU_LEVEL_PROGRESS_TEXT_GAP
        + gems_galley.size().x;
    let content_height = percent_galley
        .size()
        .y
        .max(gems_galley.size().y)
        .max(MENU_LEVEL_PROGRESS_GEM_SIZE);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(content_width, content_height) + shadow_extent,
        egui::Sense::hover(),
    );
    let content_min = rect.min
        + egui::vec2(
            shadow_offset.x.min(0.0).abs(),
            shadow_offset.y.min(0.0).abs(),
        );
    let center_y = content_min.y + content_height * 0.5;

    let percent_pos = egui::pos2(content_min.x, center_y - percent_galley.size().y * 0.5);
    let gem_rect = egui::Rect::from_min_size(
        egui::pos2(
            percent_pos.x + percent_galley.size().x + MENU_LEVEL_PROGRESS_TEXT_GAP,
            center_y - MENU_LEVEL_PROGRESS_GEM_SIZE * 0.5,
        ),
        egui::vec2(MENU_LEVEL_PROGRESS_GEM_SIZE, MENU_LEVEL_PROGRESS_GEM_SIZE),
    );
    let gems_pos = egui::pos2(
        gem_rect.right() + MENU_LEVEL_PROGRESS_TEXT_GAP,
        center_y - gems_galley.size().y * 0.5,
    );

    ui.painter().galley(
        percent_pos + shadow_offset,
        percent_galley.clone(),
        shadow_color,
    );
    paint_menu_gem_icon(ui.painter(), gem_rect.translate(shadow_offset), true);
    ui.painter()
        .galley(gems_pos + shadow_offset, gems_galley.clone(), shadow_color);

    ui.painter().galley(percent_pos, percent_galley, text_color);
    paint_menu_gem_icon(ui.painter(), gem_rect, false);
    ui.painter().galley(gems_pos, gems_galley, text_color);

    response
}

fn paint_menu_gem_icon(painter: &egui::Painter, rect: egui::Rect, shadow: bool) {
    let gem_point = |x: f32, y: f32| {
        egui::pos2(
            rect.left() + x / 24.0 * rect.width(),
            rect.top() + y / 24.0 * rect.height(),
        )
    };

    let top_left = gem_point(6.5, 3.5);
    let top_right = gem_point(17.5, 3.5);
    let middle_left = gem_point(2.5, 9.0);
    let middle_inner_left = gem_point(8.25, 9.0);
    let middle_inner_right = gem_point(15.75, 9.0);
    let middle_right = gem_point(21.5, 9.0);
    let top_center = gem_point(12.0, 3.5);
    let bottom = gem_point(12.0, 21.0);

    let outer_points = vec![top_left, top_right, middle_right, bottom, middle_left];
    if shadow {
        painter.add(egui::Shape::convex_polygon(
            outer_points,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 190),
            egui::Stroke::NONE,
        ));
        return;
    }

    let outline = egui::Stroke::new(1.7, egui::Color32::WHITE);
    let facet_line =
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(74, 95, 114, 184));

    painter.add(egui::Shape::convex_polygon(
        vec![middle_left, middle_inner_left, bottom],
        egui::Color32::from_rgb(200, 241, 255),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![middle_inner_left, middle_inner_right, bottom],
        egui::Color32::from_rgb(157, 231, 255),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![middle_inner_right, middle_right, bottom],
        egui::Color32::from_rgb(126, 220, 255),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![top_left, top_right, middle_right, middle_left],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![middle_inner_left, top_center, middle_inner_right],
        egui::Color32::from_rgb(247, 253, 255),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        outer_points,
        egui::Color32::TRANSPARENT,
        outline,
    ));

    for points in [
        [middle_left, middle_right],
        [middle_inner_left, bottom],
        [middle_inner_right, bottom],
        [middle_inner_left, top_center],
        [middle_inner_right, top_center],
    ] {
        painter.line_segment(points, facet_line);
    }
    painter.circle_filled(
        gem_point(7.0, 6.0),
        rect.width() * 0.07,
        egui::Color32::WHITE,
    );
}

/// Shows the main-menu topbar.
pub fn show_menu_topbar_ui(root_ui: &mut egui::Ui, state: &mut State) {
    if !state.is_menu() {
        return;
    }

    let ctx = root_ui.ctx().clone();
    let mut commands = Vec::new();

    egui::Panel::top("menu_top_bar").show_inside(root_ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(4.0);
            ui.label(egui_phosphor::regular::CLOCK);
            ui.label(egui::RichText::new(get_current_time_str()).monospace());

            if ui
                .button(format!("{} Settings", egui_phosphor::regular::GEAR))
                .on_hover_text(format!(
                    "Settings{}",
                    state.app_settings().hotkey_hint("toggle_settings")
                ))
                .clicked()
            {
                commands.push(AppCommand::Editor(EditorCommand::ToggleSettings));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });

    if let Some(message) = state.auth_message() {
        egui::Area::new("menu_auth_message_area".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 42.0))
            .show(&ctx, |ui| {
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
    use super::show_menu_topbar_ui;
    use crate::auth_types::{AuthProfile, AuthSession, AuthSessionTokens, AuthUser};

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
            show_menu_topbar_ui(root_ui, state);
        });
    }

    #[test]
    fn menu_topbar_and_auth_ui_render_guest_state() {
        pollster::block_on(async {
            let mut state = crate::State::new_test().await;

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
