/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! History (undo / redo) events + a `HistorySnapshot` carve-out Message used
//! by direct-manipulation UI to mark a "before-mutation" snapshot.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Undo the last editor action.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Undo;

impl ToLegacy for Undo {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::Undo)
    }
}

/// Redo the last undone editor action.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Redo;

impl ToLegacy for Redo {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::Redo)
    }
}

/// Marker Message emitted by direct-manipulation UI (gizmo drag, marquee,
/// block drag) to request a pre-mutation history snapshot.
///
/// Today the legacy `State::dispatch` snapshots history inline; in a
/// later phase this Message drives a `PreCommandHistorySet` system that
/// snapshots `ObjectBundle` data before any mutating handler runs. The
/// Message currently has no consumer — the legacy path remains the
/// source of truth — but defining it here unlocks that future wiring.
#[derive(Message, Debug, Clone, Default)]
pub struct HistorySnapshot {
    /// Free-form label for the snapshot (e.g. "gizmo_drag_release").
    pub label: String,
}

pub fn register_events(app: &mut App) {
    app.add_message::<Undo>()
        .add_message::<Redo>()
        .add_message::<HistorySnapshot>();
}

pub fn handle_undo(mut events: MessageReader<Undo>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::Undo);
    }
}

pub fn handle_redo(mut events: MessageReader<Redo>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::Redo);
    }
}

/// No-op handler for [`HistorySnapshot`]. Reserves the system slot in the
/// `CommandSet` so a follow-up phase can wire a real pre-mutation
/// snapshot system without re-orchestrating the schedule.
pub fn handle_history_snapshot(mut events: MessageReader<HistorySnapshot>) {
    // Drain the events so they don't accumulate frame-over-frame. No
    // command is generated — snapshots are written by a dedicated
    // pre-mutation system in a later phase.
    for _ in events.read() {}
}
