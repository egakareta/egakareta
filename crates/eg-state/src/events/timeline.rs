/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Timeline / playback / tap events.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::editor_command::EditorCommand;

/// Toggle timeline playback.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleTimelinePlayback;

impl ToLegacy for ToggleTimelinePlayback {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::ToggleTimelinePlayback)
    }
}

/// Shift the timeline cursor by `delta_seconds`.
#[derive(Message, Debug, Clone, Copy)]
pub struct ShiftTimeline(pub f32);

impl ToLegacy for ShiftTimeline {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::ShiftTimeline(self.0))
    }
}

/// Set the timeline cursor to an absolute time in seconds.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimelineTime(pub f32);

impl ToLegacy for SetTimelineTime {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimelineTime(self.0))
    }
}

/// Set the total duration of the timeline (seconds).
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimelineDuration(pub f32);

impl ToLegacy for SetTimelineDuration {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTimelineDuration(self.0))
    }
}

/// Add a tap at the current timeline position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddTap;

impl ToLegacy for AddTap {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddTap)
    }
}

/// Remove a tap at the current timeline position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RemoveTap;

impl ToLegacy for RemoveTap {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RemoveTap)
    }
}

/// Remove a tap at a specific timeline position.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemoveTapAt(pub f32);

impl ToLegacy for RemoveTapAt {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::RemoveTapAt(self.0))
    }
}

/// Select or deselect a tap by index.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSelectedTap(pub Option<usize>);

impl ToLegacy for SetSelectedTap {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSelectedTap(self.0))
    }
}

/// Update the selected tap's timestamp.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSelectedTapTime(pub f32);

impl ToLegacy for SetSelectedTapTime {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetSelectedTapTime(self.0))
    }
}

/// Remove all taps from the level.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ClearTaps;

impl ToLegacy for ClearTaps {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::ClearTaps)
    }
}

/// Update the playback speed multiplier.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPlaybackSpeed(pub f32);

impl ToLegacy for SetPlaybackSpeed {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetPlaybackSpeed(self.0))
    }
}

/// Set waveform view zoom.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetWaveformZoom(pub f32);

impl ToLegacy for SetWaveformZoom {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetWaveformZoom(self.0))
    }
}

/// Set waveform view scroll offset.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetWaveformScroll(pub f32);

impl ToLegacy for SetWaveformScroll {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetWaveformScroll(self.0))
    }
}

/// Start playtesting from the current editor state.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Playtest;

impl ToLegacy for Playtest {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::Playtest)
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<ToggleTimelinePlayback>()
        .add_message::<ShiftTimeline>()
        .add_message::<SetTimelineTime>()
        .add_message::<SetTimelineDuration>()
        .add_message::<AddTap>()
        .add_message::<RemoveTap>()
        .add_message::<RemoveTapAt>()
        .add_message::<SetSelectedTap>()
        .add_message::<SetSelectedTapTime>()
        .add_message::<ClearTaps>()
        .add_message::<SetPlaybackSpeed>()
        .add_message::<SetWaveformZoom>()
        .add_message::<SetWaveformScroll>()
        .add_message::<Playtest>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_toggle_timeline_playback(
    mut events: MessageReader<ToggleTimelinePlayback>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::ToggleTimelinePlayback);
    }
}

pub fn handle_shift_timeline(
    mut events: MessageReader<ShiftTimeline>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::ShiftTimeline(event.0));
    }
}

pub fn handle_set_timeline_time(
    mut events: MessageReader<SetTimelineTime>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimelineTime(event.0));
    }
}

pub fn handle_set_timeline_duration(
    mut events: MessageReader<SetTimelineDuration>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetTimelineDuration(event.0));
    }
}

pub fn handle_add_tap(mut events: MessageReader<AddTap>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::AddTap);
    }
}

pub fn handle_remove_tap(
    mut events: MessageReader<RemoveTap>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::RemoveTap);
    }
}

pub fn handle_remove_tap_at(
    mut events: MessageReader<RemoveTapAt>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::RemoveTapAt(event.0));
    }
}

pub fn handle_set_selected_tap(
    mut events: MessageReader<SetSelectedTap>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSelectedTap(event.0));
    }
}

pub fn handle_set_selected_tap_time(
    mut events: MessageReader<SetSelectedTapTime>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetSelectedTapTime(event.0));
    }
}

pub fn handle_clear_taps(
    mut events: MessageReader<ClearTaps>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::ClearTaps);
    }
}

pub fn handle_set_playback_speed(
    mut events: MessageReader<SetPlaybackSpeed>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetPlaybackSpeed(event.0));
    }
}

pub fn handle_set_waveform_zoom(
    mut events: MessageReader<SetWaveformZoom>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetWaveformZoom(event.0));
    }
}

pub fn handle_set_waveform_scroll(
    mut events: MessageReader<SetWaveformScroll>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::SetWaveformScroll(event.0));
    }
}

pub fn handle_playtest(mut events: MessageReader<Playtest>, mut pending: ResMut<PendingCommands>) {
    for _ in events.read() {
        pending.editor_commands.push(EditorCommand::Playtest);
    }
}
