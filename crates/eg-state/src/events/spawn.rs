/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Spawn / placement preview events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Set spawn position to current cursor.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct SetSpawnHere;

impl ToLegacy for SetSpawnHere {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSpawnHere)
    }
}

/// Rotate the spawn direction.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RotateSpawnDirection;

impl ToLegacy for RotateSpawnDirection {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RotateSpawnDirection)
    }
}

/// Rotate the block placement preview.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RotatePlacementPreview;

impl ToLegacy for RotatePlacementPreview {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RotatePlacementPreview)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<SetSpawnHere>()
        .add_message::<RotateSpawnDirection>()
        .add_message::<RotatePlacementPreview>();
}

pub fn handle_set_spawn_here(
    mut events: MessageReader<SetSpawnHere>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::SetSpawnHere);
    }
}

pub fn handle_rotate_spawn_direction(
    mut events: MessageReader<RotateSpawnDirection>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::RotateSpawnDirection);
    }
}

pub fn handle_rotate_placement_preview(
    mut events: MessageReader<RotatePlacementPreview>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::RotatePlacementPreview);
    }
}
