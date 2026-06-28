/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Editor mode / block-id / pick events.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_blocks::BlockId;
use eg_types::EditorMode;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::EditorConfigRes;
use crate::state::EditorInputRes;
use crate::state::editor_command::EditorCommand;

/// Switch the editor mode.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetEditorMode(pub EditorMode);

impl ToLegacy for SetEditorMode {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetMode(self.0))
    }
}

/// Set the current block ID for placement.
#[derive(Message, Debug, Clone)]
pub struct SetBlockId(pub BlockId);

impl ToLegacy for SetBlockId {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetBlockId(self.0))
    }
}

/// Select the Nth recent placeable block and switch to Place mode.
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectRecentBlock(pub usize);

impl ToLegacy for SelectRecentBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SelectRecentBlock(self.0))
    }
}

/// Pick the selected block and enter place mode with that block type.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PickSelectedBlock;

impl ToLegacy for PickSelectedBlock {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::PickSelectedBlock)
    }
}

/// Pick the block under the pointer and enter place mode with that block type.
#[derive(Message, Debug, Clone, Copy)]
pub struct PickBlockAt {
    pub x: f64,
    pub y: f64,
}

impl ToLegacy for PickBlockAt {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::PickBlockAt {
            x: self.x,
            y: self.y,
        })
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<SetEditorMode>()
        .add_message::<SetBlockId>()
        .add_message::<SelectRecentBlock>()
        .add_message::<PickSelectedBlock>()
        .add_message::<PickBlockAt>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

// `SetEditorMode` is a carve-out from the `PendingCommands` →
// `State::dispatch_editor` pipeline (same pattern as `events::snap`,
// `events::bpm`, and `events::misc`): the handler mutates
// `EditorInputRes.0.mode` directly, and the legacy
// `State.editor.input.mode` is synced from the resource by
// `Runtime::sync_app_editor_input_state` before any dispatch in the
// same frame. The `ToLegacy` impl is preserved so the legacy
// dispatch path remains reachable from non-message call sites (the
// existing `editor_dispatch` tests continue to drive
// `State::dispatch_editor(EditorCommand::SetMode(...))` directly).
//
// The legacy `set_editor_mode` has heavy side effects (rebuilds
// gizmo/hover/selection outline vertices, marks
// `EditorDirtyFlags::tap_timeline_changed()`). The change guard in
// the runtime ensures those side effects fire only on real
// user-driven changes. The legacy dispatch path
// (`EditorCommand::SetMode`) goes through `dispatch_editor` which
// also clears `editor.timeline.taps.selected_index` on
// Tapping→other transitions and calls
// `refresh_editor_tapping_preview_on_mode_entry` on
// other→Tapping transitions. The sync helper invokes
// `set_editor_mode` (the same setter used by
// `set_editor_playback_effective_mode` after a mode change), so
// the side effects fire through the same path.
pub fn handle_set_editor_mode(
    mut events: MessageReader<SetEditorMode>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.mode = event.0;
    }
}

// `SetBlockId` is a carve-out from the `PendingCommands` ->
// `State::dispatch_editor` pipeline (same pattern as `events::snap`):
// the handler mutates `EditorConfigRes.0.selected_block_id` directly,
// and the legacy `State.editor.config.selected_block_id` is synced from
// the resource by `Runtime::sync_app_editor_config_state` before any
// dispatch in the same frame. The `ToLegacy` impl is kept so the legacy
// dispatch path remains reachable from non-message call sites (the
// existing `dispatch_editor_set_block_id_updates_recent` test still
// drives `State::dispatch_editor(EditorCommand::SetBlockId)` directly).
pub fn handle_set_block_id(
    mut events: MessageReader<SetBlockId>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.selected_block_id = event.0.clone();
    }
}

pub fn handle_select_recent_block(
    mut events: MessageReader<SelectRecentBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SelectRecentBlock(event.0));
    }
}

pub fn handle_pick_selected_block(
    mut events: MessageReader<PickSelectedBlock>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::PickSelectedBlock);
    }
}

pub fn handle_pick_block_at(
    mut events: MessageReader<PickBlockAt>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::PickBlockAt {
            x: event.x,
            y: event.y,
        });
    }
}
