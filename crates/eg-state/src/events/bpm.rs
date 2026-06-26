/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! BPM tapping events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Record a BPM heart-beat tap.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct BpmTap;

impl ToLegacy for BpmTap {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::BpmTap)
    }
}

/// Reset the BPM tapping state.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct BpmTapReset;

impl ToLegacy for BpmTapReset {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::BpmTapReset)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<BpmTap>().add_message::<BpmTapReset>();
}

pub fn handle_bpm_tap(mut events: MessageReader<BpmTap>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::BpmTap);
    }
}

pub fn handle_bpm_tap_reset(
    mut events: MessageReader<BpmTapReset>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::BpmTapReset);
    }
}
