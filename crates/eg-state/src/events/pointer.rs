/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Pointer / drag / cursor events. Deprecated as commands per the
//! migration plan; see the module-level docs of
//! [`crate::events::mod`] for the input-state carve-out. The
//! events remain routed through `dispatch_editor` for now to preserve
//! test parity.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Mouse button state update for editor interaction routing.
#[derive(Message, Debug, Clone, Copy)]
pub struct MouseButton {
    pub button: u32,
    pub pressed: bool,
}

impl ToLegacy for MouseButton {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::MouseButton {
            button: self.button,
            pressed: self.pressed,
        })
    }
}

/// Primary click action at screen coordinates.
#[derive(Message, Debug, Clone, Copy)]
pub struct PrimaryClick {
    pub x: f64,
    pub y: f64,
}

impl ToLegacy for PrimaryClick {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::PrimaryClick {
            x: self.x,
            y: self.y,
        })
    }
}

/// Pointer moved to screen coordinates.
#[derive(Message, Debug, Clone, Copy)]
pub struct PointerMoved {
    pub x: f64,
    pub y: f64,
}

impl ToLegacy for PointerMoved {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::PointerMoved {
            x: self.x,
            y: self.y,
        })
    }
}

/// Update the editor cursor from screen coordinates without triggering a click.
#[derive(Message, Debug, Clone, Copy)]
pub struct UpdateCursorFromScreen {
    pub x: f64,
    pub y: f64,
}

impl ToLegacy for UpdateCursorFromScreen {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::UpdateCursorFromScreen {
            x: self.x,
            y: self.y,
        })
    }
}

/// Camera drag delta in screen pixels.
#[derive(Message, Debug, Clone, Copy)]
pub struct CameraDrag {
    pub dx: f64,
    pub dy: f64,
}

impl ToLegacy for CameraDrag {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::CameraDrag {
            dx: self.dx,
            dy: self.dy,
        })
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<MouseButton>()
        .add_message::<PrimaryClick>()
        .add_message::<PointerMoved>()
        .add_message::<UpdateCursorFromScreen>()
        .add_message::<CameraDrag>();
}

pub fn handle_mouse_button(
    mut events: MessageReader<MouseButton>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::MouseButton {
            button: event.button,
            pressed: event.pressed,
        });
    }
}

pub fn handle_primary_click(
    mut events: MessageReader<PrimaryClick>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::PrimaryClick {
            x: event.x,
            y: event.y,
        });
    }
}

pub fn handle_pointer_moved(
    mut events: MessageReader<PointerMoved>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::PointerMoved {
            x: event.x,
            y: event.y,
        });
    }
}

pub fn handle_update_cursor_from_screen(
    mut events: MessageReader<UpdateCursorFromScreen>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::UpdateCursorFromScreen {
                x: event.x,
                y: event.y,
            });
    }
}

pub fn handle_camera_drag(
    mut events: MessageReader<CameraDrag>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::CameraDrag {
            dx: event.dx,
            dy: event.dy,
        });
    }
}
