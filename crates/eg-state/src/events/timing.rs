/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Timing point events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Add a new timing point.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddTimingPoint {
    pub time_seconds: f32,
    pub bpm: f32,
}

impl ToLegacy for AddTimingPoint {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddTimingPoint {
            time_seconds: self.time_seconds,
            bpm: self.bpm,
        })
    }
}

/// Remove an existing timing point by index.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemoveTimingPoint(pub usize);

impl ToLegacy for RemoveTimingPoint {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RemoveTimingPoint(self.0))
    }
}

/// Update an existing timing point's timestamp.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointTime {
    pub index: usize,
    pub time: f32,
}

impl ToLegacy for SetTimingPointTime {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimingPointTime(self.index, self.time))
    }
}

/// Update an existing timing point's BPM.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointBpm {
    pub index: usize,
    pub bpm: f32,
}

impl ToLegacy for SetTimingPointBpm {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimingPointBpm(self.index, self.bpm))
    }
}

/// Update an existing timing point's time signature.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointTimeSignature {
    pub index: usize,
    pub numerator: u32,
    pub denominator: u32,
}

impl ToLegacy for SetTimingPointTimeSignature {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimingPointTimeSignature(
            self.index,
            self.numerator,
            self.denominator,
        ))
    }
}

/// Select or deselect a timing point in the UI.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingSelected(pub Option<usize>);

impl ToLegacy for SetTimingSelected {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimingSelected(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<AddTimingPoint>()
        .add_message::<RemoveTimingPoint>()
        .add_message::<SetTimingPointTime>()
        .add_message::<SetTimingPointBpm>()
        .add_message::<SetTimingPointTimeSignature>()
        .add_message::<SetTimingSelected>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_add_timing_point(
    mut events: MessageReader<AddTimingPoint>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending.editor_commands.push(EditorCommand::AddTimingPoint {
            time_seconds: event.time_seconds,
            bpm: event.bpm,
        });
    }
}

pub fn handle_remove_timing_point(
    mut events: MessageReader<RemoveTimingPoint>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::RemoveTimingPoint(event.0));
    }
}

pub fn handle_set_timing_point_time(
    mut events: MessageReader<SetTimingPointTime>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimingPointTime(event.index, event.time));
    }
}

pub fn handle_set_timing_point_bpm(
    mut events: MessageReader<SetTimingPointBpm>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimingPointBpm(event.index, event.bpm));
    }
}

pub fn handle_set_timing_point_time_signature(
    mut events: MessageReader<SetTimingPointTimeSignature>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimingPointTimeSignature(
                event.index,
                event.numerator,
                event.denominator,
            ));
    }
}

pub fn handle_set_timing_selected(
    mut events: MessageReader<SetTimingSelected>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimingSelected(event.0));
    }
}
