/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Navigation / phase events. One per `AppCommand` variant under
//! `AppCommand::Navigation / Phase` in the legacy enum.
//!
//! `NextLevel`/`PrevLevel` in the menu, `GameResume`, and
//! `GameSetPracticeMode` are **carve-outs** from the `PendingCommands`
//! pipeline (same pattern as `events::snap`, `events::session`, ...).
//! The next/previous level handlers mutate `MenuStateRes` directly when
//! the App phase is `Menu` and `EditorInputRes.cursor` directly when the
//! App phase is `Editor`. The game pause/practice handlers mutate
//! `SessionStateRes` directly, and the legacy `State.session` fields are
//! synced from the resource in the runtime bridge.
//!
//! `ToLegacy` impls are kept only for navigation messages that still
//! route through the legacy command drain.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_types::AppPhase;

use crate::commands::AppCommand;
use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::{AppPhaseRes, EditorConfigRes, EditorInputRes, MenuStateRes, SessionStateRes};

/// Primary action: in menu → start level, in game → turn, in editor → place block.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TurnRight;

impl ToLegacy for TurnRight {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::TurnRight)
    }
}

/// Select the next level in the menu or move editor cursor +X.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct NextLevel;

/// Select the previous level in the menu or move editor cursor −X.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PrevLevel;

/// Toggle between editor and menu/playing.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleEditor;

impl ToLegacy for ToggleEditor {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::ToggleEditor)
    }
}

/// Resume real gameplay from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameResume;

/// Restart the current real gameplay level from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameRestartLevel;

impl ToLegacy for GameRestartLevel {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameRestartLevel)
    }
}

/// Enable or disable practice mode for the current real gameplay level.
#[derive(Message, Debug, Clone, Copy)]
pub struct GameSetPracticeMode {
    pub enabled: bool,
}

/// Place a practice-mode checkpoint at the current gameplay position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameSetPracticeCheckpoint;

impl ToLegacy for GameSetPracticeCheckpoint {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameSetPracticeCheckpoint)
    }
}

/// Remove the latest practice-mode checkpoint.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameRemovePracticeCheckpoint;

impl ToLegacy for GameRemovePracticeCheckpoint {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameRemovePracticeCheckpoint)
    }
}

/// Quit the current real gameplay level from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameQuitToMenu;

impl ToLegacy for GameQuitToMenu {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameQuitToMenu)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<TurnRight>()
        .add_message::<NextLevel>()
        .add_message::<PrevLevel>()
        .add_message::<ToggleEditor>()
        .add_message::<GameResume>()
        .add_message::<GameRestartLevel>()
        .add_message::<GameSetPracticeMode>()
        .add_message::<GameSetPracticeCheckpoint>()
        .add_message::<GameRemovePracticeCheckpoint>()
        .add_message::<GameQuitToMenu>();
}

// -----------------------------------------------------------------------
// Dispatch systems. Legacy-routed messages still stage `AppCommand`s in
// `PendingCommands`; carved-out messages mutate App-world resources directly.
// -----------------------------------------------------------------------

pub fn handle_turn_right(
    mut events: MessageReader<TurnRight>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::TurnRight);
    }
}

pub fn handle_next_level(
    mut events: MessageReader<NextLevel>,
    phase: Res<AppPhaseRes>,
    config: Res<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut menu: Option<ResMut<MenuStateRes>>,
) {
    for _ in events.read() {
        match phase.current {
            AppPhase::Menu => {
                if let Some(menu) = menu.as_deref_mut() {
                    let level_count = menu.0.levels.len();
                    if level_count > 0 {
                        menu.0.selected_level = (menu.0.selected_level + 1) % level_count;
                    }
                }
            }
            AppPhase::Editor => move_editor_cursor_x(&mut input, &config, 1.0),
            _ => {}
        }
    }
}

pub fn handle_prev_level(
    mut events: MessageReader<PrevLevel>,
    phase: Res<AppPhaseRes>,
    config: Res<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut menu: Option<ResMut<MenuStateRes>>,
) {
    for _ in events.read() {
        match phase.current {
            AppPhase::Menu => {
                if let Some(menu) = menu.as_deref_mut() {
                    let level_count = menu.0.levels.len();
                    if level_count > 0 {
                        menu.0.selected_level = if menu.0.selected_level == 0 {
                            level_count - 1
                        } else {
                            menu.0.selected_level - 1
                        };
                    }
                }
            }
            AppPhase::Editor => move_editor_cursor_x(&mut input, &config, -1.0),
            _ => {}
        }
    }
}

fn move_editor_cursor_x(input: &mut EditorInputRes, config: &EditorConfigRes, direction: f32) {
    let step = if config.0.snap_to_grid && !input.0.ctrl_held {
        config.0.snap_step.max(0.05)
    } else {
        1.0
    };
    input.0.cursor[0] += direction * step;
}

pub fn handle_toggle_editor(
    mut events: MessageReader<ToggleEditor>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::ToggleEditor);
    }
}

pub fn handle_game_resume(
    mut events: MessageReader<GameResume>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.game_paused = false;
    }
}

pub fn handle_game_restart_level(
    mut events: MessageReader<GameRestartLevel>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::GameRestartLevel);
    }
}

/// `GameSetPracticeMode` is a carve-out: the handler writes the
/// enabled flag directly to `SessionStateRes.practice_mode_enabled`
/// instead of staging a legacy `AppCommand`. The legacy
/// `set_practice_mode_enabled` (in `State::command_dispatch`) has
/// heavy side effects (clears `practice_checkpoints`, calls
/// `rebuild_practice_checkpoint_vertices`, `restart_level`, and
/// `resume_game`); the change-guarded sync in
/// `Runtime::sync_app_session_state` invokes the legacy setter only
/// on real user-driven changes. The `ToLegacy` impl is preserved so
/// the legacy dispatch path remains reachable from non-message call
/// sites.
pub fn handle_game_set_practice_mode(
    mut events: MessageReader<GameSetPracticeMode>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.practice_mode_enabled = event.enabled;
    }
}

pub fn handle_game_set_practice_checkpoint(
    mut events: MessageReader<GameSetPracticeCheckpoint>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .app_commands
            .push(AppCommand::GameSetPracticeCheckpoint);
    }
}

pub fn handle_game_remove_practice_checkpoint(
    mut events: MessageReader<GameRemovePracticeCheckpoint>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .app_commands
            .push(AppCommand::GameRemovePracticeCheckpoint);
    }
}

pub fn handle_game_quit_to_menu(
    mut events: MessageReader<GameQuitToMenu>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::GameQuitToMenu);
    }
}
