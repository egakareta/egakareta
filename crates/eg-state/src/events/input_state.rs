/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Held-state input events (Shift / Ctrl / Alt / WASD panning).
//!
//! These are **deprecated as events** per the migration plan. They
//! represent `EditorInput` state, not commands. A follow-up phase will
//! replace them with direct writes to the `EditorInput` resource from a
//! dedicated `InputSet` system. The events are kept here for now so the
//! 1:1 command→Message mapping is complete and test parity is preserved
//! (the legacy `EditorCommand::SetShiftHeld` etc. handlers stay).

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Track Shift held state for editor shortcuts.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetShiftHeld(pub bool);

impl ToLegacy for SetShiftHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetShiftHeld(self.0))
    }
}

/// Track Ctrl held state for editor shortcuts.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetCtrlHeld(pub bool);

impl ToLegacy for SetCtrlHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetCtrlHeld(self.0))
    }
}

/// Track Alt held state for editor shortcuts.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetAltHeld(pub bool);

impl ToLegacy for SetAltHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetAltHeld(self.0))
    }
}

/// Track W-pan held state in editor.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPanUpHeld(pub bool);

impl ToLegacy for SetPanUpHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetPanUpHeld(self.0))
    }
}

/// Track S-pan held state in editor.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPanDownHeld(pub bool);

impl ToLegacy for SetPanDownHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetPanDownHeld(self.0))
    }
}

/// Track A-pan held state in editor.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPanLeftHeld(pub bool);

impl ToLegacy for SetPanLeftHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetPanLeftHeld(self.0))
    }
}

/// Track D-pan held state in editor.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPanRightHeld(pub bool);

impl ToLegacy for SetPanRightHeld {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetPanRightHeld(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<SetShiftHeld>()
        .add_message::<SetCtrlHeld>()
        .add_message::<SetAltHeld>()
        .add_message::<SetPanUpHeld>()
        .add_message::<SetPanDownHeld>()
        .add_message::<SetPanLeftHeld>()
        .add_message::<SetPanRightHeld>();
}

pub fn handle_set_shift_held(
    mut events: MessageReader<SetShiftHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetShiftHeld(event.0));
    }
}

pub fn handle_set_ctrl_held(
    mut events: MessageReader<SetCtrlHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetCtrlHeld(event.0));
    }
}

pub fn handle_set_alt_held(
    mut events: MessageReader<SetAltHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetAltHeld(event.0));
    }
}

pub fn handle_set_pan_up_held(
    mut events: MessageReader<SetPanUpHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetPanUpHeld(event.0));
    }
}

pub fn handle_set_pan_down_held(
    mut events: MessageReader<SetPanDownHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetPanDownHeld(event.0));
    }
}

pub fn handle_set_pan_left_held(
    mut events: MessageReader<SetPanLeftHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetPanLeftHeld(event.0));
    }
}

pub fn handle_set_pan_right_held(
    mut events: MessageReader<SetPanRightHeld>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetPanRightHeld(event.0));
    }
}
