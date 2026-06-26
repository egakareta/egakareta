/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Navigation / phase events. One per `AppCommand` variant under
//! `AppCommand::Navigation / Phase` in the legacy enum.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::commands::AppCommand;
use crate::events::{LegacyCommand, PendingCommands, ToLegacy};

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

impl ToLegacy for NextLevel {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::NextLevel)
    }
}

/// Select the previous level in the menu or move editor cursor −X.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PrevLevel;

impl ToLegacy for PrevLevel {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::PrevLevel)
    }
}

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

impl ToLegacy for GameResume {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameResume)
    }
}

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

impl ToLegacy for GameSetPracticeMode {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::GameSetPracticeMode(self.enabled))
    }
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
// Dispatch systems. Each one drains a single `MessageReader<T>` and writes
// the corresponding `AppCommand` to `PendingCommands`. These systems are
// the only Bevy-side dispatch path; the legacy `State::dispatch` is still
// the source of truth for command handling.
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
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::NextLevel);
    }
}

pub fn handle_prev_level(
    mut events: MessageReader<PrevLevel>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::PrevLevel);
    }
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
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::GameResume);
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

pub fn handle_game_set_practice_mode(
    mut events: MessageReader<GameSetPracticeMode>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .app_commands
            .push(AppCommand::GameSetPracticeMode(event.enabled));
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
