/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Misc editor command events: hitbox / perf overlay / place window /
//! escape.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Toggle editor hitbox visualization.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleHitboxVisualization;

impl ToLegacy for ToggleHitboxVisualization {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::ToggleHitboxVisualization)
    }
}

/// Toggle the performance overlay.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePerfOverlay;

impl ToLegacy for TogglePerfOverlay {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::TogglePerfOverlay)
    }
}

/// Toggle the floating place window with block catalog.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePlaceWindow;

impl ToLegacy for TogglePlaceWindow {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::TogglePlaceWindow)
    }
}

/// Escape key context-sensitive (stop playback → reset timeline → back to menu).
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Escape;

impl ToLegacy for Escape {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::Escape)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<ToggleHitboxVisualization>()
        .add_message::<TogglePerfOverlay>()
        .add_message::<TogglePlaceWindow>()
        .add_message::<Escape>();
}

pub fn handle_toggle_hitbox_visualization(
    mut events: MessageReader<ToggleHitboxVisualization>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::ToggleHitboxVisualization);
    }
}

pub fn handle_toggle_perf_overlay(
    mut events: MessageReader<TogglePerfOverlay>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::TogglePerfOverlay);
    }
}

pub fn handle_toggle_place_window(
    mut events: MessageReader<TogglePlaceWindow>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::TogglePlaceWindow);
    }
}

pub fn handle_escape(mut events: MessageReader<Escape>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::Escape);
    }
}
