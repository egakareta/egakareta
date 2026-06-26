/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Block operations events: remove, duplicate, copy, paste, update.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_types::LevelObject;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Remove the block under cursor or selected blocks.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RemoveBlock;

impl ToLegacy for RemoveBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RemoveBlock)
    }
}

/// Duplicate selected blocks in place.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct DuplicateBlock;

impl ToLegacy for DuplicateBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::DuplicateBlock)
    }
}

/// Copy selected blocks to clipboard.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct CopyBlock;

impl ToLegacy for CopyBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::CopyBlock)
    }
}

/// Paste clipboard blocks.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PasteBlock;

impl ToLegacy for PasteBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::PasteBlock)
    }
}

/// Update the properties of the currently selected block.
#[derive(Message, Debug, Clone)]
pub struct UpdateSelectedBlock(pub LevelObject);

impl ToLegacy for UpdateSelectedBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::UpdateSelectedBlock(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<RemoveBlock>()
        .add_message::<DuplicateBlock>()
        .add_message::<CopyBlock>()
        .add_message::<PasteBlock>()
        .add_message::<UpdateSelectedBlock>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_remove_block(
    mut events: MessageReader<RemoveBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::RemoveBlock);
    }
}

pub fn handle_duplicate_block(
    mut events: MessageReader<DuplicateBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::DuplicateBlock);
    }
}

pub fn handle_copy_block(
    mut events: MessageReader<CopyBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::CopyBlock);
    }
}

pub fn handle_paste_block(
    mut events: MessageReader<PasteBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::PasteBlock);
    }
}

pub fn handle_update_selected_block(
    mut events: MessageReader<UpdateSelectedBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::UpdateSelectedBlock(event.0.clone()));
    }
}
