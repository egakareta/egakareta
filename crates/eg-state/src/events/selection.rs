/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Selection / transform events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Snap selected blocks or tap to the nearest grid cell.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct SnapSelectionToGrid;

impl ToLegacy for SnapSelectionToGrid {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SnapSelectionToGrid)
    }
}

/// Focus the editor camera on the selected or preview target.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct FocusCameraTarget;

impl ToLegacy for FocusCameraTarget {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::FocusCameraTarget)
    }
}

/// Add transform-object triggers for the selected blocks.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddTransformTrigger;

impl ToLegacy for AddTransformTrigger {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddTransformTrigger)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<SnapSelectionToGrid>()
        .add_message::<FocusCameraTarget>()
        .add_message::<AddTransformTrigger>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_snap_selection_to_grid(
    mut events: MessageReader<SnapSelectionToGrid>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SnapSelectionToGrid);
    }
}

pub fn handle_focus_camera_target(
    mut events: MessageReader<FocusCameraTarget>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::FocusCameraTarget);
    }
}

pub fn handle_add_transform_trigger(
    mut events: MessageReader<AddTransformTrigger>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AddTransformTrigger);
    }
}
