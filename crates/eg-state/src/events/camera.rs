/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Camera / zoom / trigger selection events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Adjust zoom by moving the camera along its look vector.
#[derive(Message, Debug, Clone, Copy)]
pub struct AdjustZoom(pub f32);

impl ToLegacy for AdjustZoom {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AdjustZoom(self.0))
    }
}

/// Snap the editor camera to an absolute orientation in radians.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetCameraOrientation {
    pub rotation: f32,
    pub pitch: f32,
    pub transition_seconds: Option<f32>,
}

impl ToLegacy for SetCameraOrientation {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetCameraOrientation {
            rotation: self.rotation,
            pitch: self.pitch,
            transition_seconds: self.transition_seconds,
        })
    }
}

/// Capture a new camera trigger from the current editor camera at the playhead.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraTrigger;

impl ToLegacy for AddCameraTrigger {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddCameraTrigger)
    }
}

/// Add a camera-follow trigger at the current playhead, returning to default tracking mode.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraFollowTrigger;

impl ToLegacy for AddCameraFollowTrigger {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddCameraFollowTrigger)
    }
}

/// Select or deselect a trigger in the UI.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTriggerSelected(pub Option<usize>);

impl ToLegacy for SetTriggerSelected {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTriggerSelected(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<AdjustZoom>()
        .add_message::<SetCameraOrientation>()
        .add_message::<AddCameraTrigger>()
        .add_message::<AddCameraFollowTrigger>()
        .add_message::<SetTriggerSelected>();
}

pub fn handle_adjust_zoom(
    mut events: MessageReader<AdjustZoom>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AdjustZoom(event.0));
    }
}

pub fn handle_set_camera_orientation(
    mut events: MessageReader<SetCameraOrientation>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetCameraOrientation {
                rotation: event.rotation,
                pitch: event.pitch,
                transition_seconds: event.transition_seconds,
            });
    }
}

pub fn handle_add_camera_trigger(
    mut events: MessageReader<AddCameraTrigger>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AddCameraTrigger);
    }
}

pub fn handle_add_camera_follow_trigger(
    mut events: MessageReader<AddCameraFollowTrigger>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AddCameraFollowTrigger);
    }
}

pub fn handle_set_trigger_selected(
    mut events: MessageReader<SetTriggerSelected>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTriggerSelected(event.0));
    }
}
