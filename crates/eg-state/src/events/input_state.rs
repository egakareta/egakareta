/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Held-state input events (Shift / Ctrl / Alt / WASD panning).
//!
//! These represent `EditorInput` state, not commands. The handlers write
//! directly to the mirrored `EditorInputRes` resource so the App-world
//! input-state path no longer needs to route these through legacy editor
//! dispatch.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, ToLegacy};
use crate::state::EditorInputRes;
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
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.shift_held = event.0;
    }
}

pub fn handle_set_ctrl_held(
    mut events: MessageReader<SetCtrlHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.ctrl_held = event.0;
    }
}

pub fn handle_set_alt_held(
    mut events: MessageReader<SetAltHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.alt_held = event.0;
    }
}

pub fn handle_set_pan_up_held(
    mut events: MessageReader<SetPanUpHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pan_up_held = event.0;
    }
}

pub fn handle_set_pan_down_held(
    mut events: MessageReader<SetPanDownHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pan_down_held = event.0;
    }
}

pub fn handle_set_pan_left_held(
    mut events: MessageReader<SetPanLeftHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pan_left_held = event.0;
    }
}

pub fn handle_set_pan_right_held(
    mut events: MessageReader<SetPanRightHeld>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pan_right_held = event.0;
    }
}
