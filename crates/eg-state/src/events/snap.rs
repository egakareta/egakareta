/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Snap / grid events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Set whether to snap to grid.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapToGrid(pub bool);

impl ToLegacy for SetSnapToGrid {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSnapToGrid(self.0))
    }
}

/// Set the grid snap step.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapStep(pub f32);

impl ToLegacy for SetSnapStep {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSnapStep(self.0))
    }
}

/// Set whether rotation uses snapping.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapRotation(pub bool);

impl ToLegacy for SetSnapRotation {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSnapRotation(self.0))
    }
}

/// Set the rotation snap step in degrees.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapRotationStep(pub f32);

impl ToLegacy for SetSnapRotationStep {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSnapRotationStep(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<SetSnapToGrid>()
        .add_message::<SetSnapStep>()
        .add_message::<SetSnapRotation>()
        .add_message::<SetSnapRotationStep>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_set_snap_to_grid(
    mut events: MessageReader<SetSnapToGrid>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSnapToGrid(event.0));
    }
}

pub fn handle_set_snap_step(
    mut events: MessageReader<SetSnapStep>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSnapStep(event.0));
    }
}

pub fn handle_set_snap_rotation(
    mut events: MessageReader<SetSnapRotation>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSnapRotation(event.0));
    }
}

pub fn handle_set_snap_rotation_step(
    mut events: MessageReader<SetSnapRotationStep>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSnapRotationStep(event.0));
    }
}
