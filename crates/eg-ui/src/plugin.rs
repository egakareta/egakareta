/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy UI plugin and command bridge.
//!
//! REFACTOR POINT(full-bevy): `EguiContextRes` and the manual command bridge
//! are replaced by `bevy_egui` systems writing Bevy input/events directly.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use eg_state::commands::AppCommand;
use eg_state::events::{
    auth, block, bpm, camera, editor_mode, history, input_state, misc, navigation, pointer,
    selection, session, snap, spawn, timeline, timing,
};
use eg_state::state::editor_command::EditorCommand;

/// Current egui context for UI systems.
#[derive(Resource, Default, Clone)]
pub struct EguiContextRes(pub Option<egui::Context>);

/// Per-frame queue of UI-originated commands waiting to enter the Bevy message bus.
#[derive(Resource, Default, Debug)]
pub struct UiCommandQueue {
    commands: Vec<AppCommand>,
}

impl UiCommandQueue {
    /// Push a UI command into the queue.
    pub fn push(&mut self, command: AppCommand) {
        self.commands.push(command);
    }

    /// Push multiple UI commands into the queue, preserving order.
    pub fn extend(&mut self, commands: impl IntoIterator<Item = AppCommand>) {
        self.commands.extend(commands);
    }

    /// Returns true when no commands are queued.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Number of queued commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    fn drain(&mut self) -> impl Iterator<Item = AppCommand> + '_ {
        self.commands.drain(..)
    }
}

/// Registers egui-facing resources and the UI command bridge system.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        eg_state::events::register_command_events(app);
        app.init_resource::<EguiContextRes>()
            .init_resource::<UiCommandQueue>()
            .add_systems(Update, drain_ui_command_queue_system);
    }
}

/// Register UI resources on a bare World without adding App-level systems.
pub fn register_ui_resources_world(world: &mut World) {
    world.init_resource::<EguiContextRes>();
    world.init_resource::<UiCommandQueue>();
}

/// Mirror the current egui context into the App World for UI systems.
pub fn sync_egui_context(world: &mut World, ctx: &egui::Context) {
    world.resource_mut::<EguiContextRes>().0 = Some(ctx.clone());
}

/// Queue one UI command directly into a world-backed command queue.
pub fn enqueue_ui_command(world: &mut World, command: AppCommand) {
    world.resource_mut::<UiCommandQueue>().push(command);
}

/// Queue UI commands directly into a world-backed command queue.
pub fn enqueue_ui_commands(world: &mut World, commands: impl IntoIterator<Item = AppCommand>) {
    world.resource_mut::<UiCommandQueue>().extend(commands);
}

/// Immediately write UI commands to the Bevy message bus.
pub fn write_ui_commands(world: &mut World, commands: impl IntoIterator<Item = AppCommand>) {
    for command in commands {
        write_ui_command(world, command);
    }
}

/// Immediately write one UI command to the Bevy message bus.
pub fn write_ui_command(world: &mut World, command: AppCommand) {
    match command {
        AppCommand::TurnRight => {
            world.write_message(navigation::TurnRight);
        }
        AppCommand::NextLevel => {
            world.write_message(navigation::NextLevel);
        }
        AppCommand::PrevLevel => {
            world.write_message(navigation::PrevLevel);
        }
        AppCommand::ToggleEditor => {
            world.write_message(navigation::ToggleEditor);
        }
        AppCommand::GameResume => {
            world.write_message(navigation::GameResume);
        }
        AppCommand::GameRestartLevel => {
            world.write_message(navigation::GameRestartLevel);
        }
        AppCommand::GameSetPracticeMode(enabled) => {
            world.write_message(navigation::GameSetPracticeMode { enabled });
        }
        AppCommand::GameSetPracticeCheckpoint => {
            world.write_message(navigation::GameSetPracticeCheckpoint);
        }
        AppCommand::GameRemovePracticeCheckpoint => {
            world.write_message(navigation::GameRemovePracticeCheckpoint);
        }
        AppCommand::GameQuitToMenu => {
            world.write_message(navigation::GameQuitToMenu);
        }
        AppCommand::AuthSubmitSignIn => {
            world.write_message(auth::AuthSubmitSignIn);
        }
        AppCommand::AuthSignOut => {
            world.write_message(auth::AuthSignOut);
        }
        AppCommand::AuthOpenSignup => {
            world.write_message(auth::AuthOpenSignup);
        }
        AppCommand::CopyLatestPerfFrame => {
            world.write_message(auth::CopyLatestPerfFrame);
        }
        AppCommand::ResizeSurface { width, height } => {
            world.write_message(auth::ResizeSurface { width, height });
        }
        AppCommand::Editor(command) => write_editor_command(world, command),
    }
}

fn drain_ui_command_queue_system(world: &mut World) {
    drain_ui_command_queue(world);
}

/// Drain queued UI commands into Bevy messages immediately.
pub fn drain_ui_command_queue(world: &mut World) {
    let commands = {
        let mut queue = world.resource_mut::<UiCommandQueue>();
        queue.drain().collect::<Vec<_>>()
    };
    write_ui_commands(world, commands);
}

fn write_editor_command(world: &mut World, command: EditorCommand) {
    match command {
        EditorCommand::SetMode(mode) => {
            world.write_message(editor_mode::SetEditorMode(mode));
        }
        EditorCommand::SetBlockId(block_id) => {
            world.write_message(editor_mode::SetBlockId(block_id));
        }
        EditorCommand::SelectRecentBlock(index) => {
            world.write_message(editor_mode::SelectRecentBlock(index));
        }
        EditorCommand::PickSelectedBlock => {
            world.write_message(editor_mode::PickSelectedBlock);
        }
        EditorCommand::PickBlockAt { x, y } => {
            world.write_message(editor_mode::PickBlockAt { x, y });
        }
        EditorCommand::SetSnapToGrid(enabled) => {
            world.write_message(snap::SetSnapToGrid(enabled));
        }
        EditorCommand::SetSnapStep(step) => {
            world.write_message(snap::SetSnapStep(step));
        }
        EditorCommand::SetSnapRotation(enabled) => {
            world.write_message(snap::SetSnapRotation(enabled));
        }
        EditorCommand::SetSnapRotationStep(step) => {
            world.write_message(snap::SetSnapRotationStep(step));
        }
        EditorCommand::RemoveBlock => {
            world.write_message(block::RemoveBlock);
        }
        EditorCommand::DuplicateBlock => {
            world.write_message(block::DuplicateBlock);
        }
        EditorCommand::CopyBlock => {
            world.write_message(block::CopyBlock);
        }
        EditorCommand::PasteBlock => {
            world.write_message(block::PasteBlock);
        }
        EditorCommand::UpdateSelectedBlock(object) => {
            world.write_message(block::UpdateSelectedBlock(object));
        }
        EditorCommand::SnapSelectionToGrid => {
            world.write_message(selection::SnapSelectionToGrid);
        }
        EditorCommand::FocusCameraTarget => {
            world.write_message(selection::FocusCameraTarget);
        }
        EditorCommand::AddTransformTrigger => {
            world.write_message(selection::AddTransformTrigger);
        }
        EditorCommand::ToggleTimelinePlayback => {
            world.write_message(timeline::ToggleTimelinePlayback);
        }
        EditorCommand::ShiftTimeline(delta_seconds) => {
            world.write_message(timeline::ShiftTimeline(delta_seconds));
        }
        EditorCommand::SetTimelineTime(time_seconds) => {
            world.write_message(timeline::SetTimelineTime(time_seconds));
        }
        EditorCommand::SetTimelineDuration(duration_seconds) => {
            world.write_message(timeline::SetTimelineDuration(duration_seconds));
        }
        EditorCommand::AddTap => {
            world.write_message(timeline::AddTap);
        }
        EditorCommand::RemoveTap => {
            world.write_message(timeline::RemoveTap);
        }
        EditorCommand::RemoveTapAt(time_seconds) => {
            world.write_message(timeline::RemoveTapAt(time_seconds));
        }
        EditorCommand::SetSelectedTap(index) => {
            world.write_message(timeline::SetSelectedTap(index));
        }
        EditorCommand::SetSelectedTapTime(time_seconds) => {
            world.write_message(timeline::SetSelectedTapTime(time_seconds));
        }
        EditorCommand::ClearTaps => {
            world.write_message(timeline::ClearTaps);
        }
        EditorCommand::SetPlaybackSpeed(speed) => {
            world.write_message(timeline::SetPlaybackSpeed(speed));
        }
        EditorCommand::SetWaveformZoom(zoom) => {
            world.write_message(timeline::SetWaveformZoom(zoom));
        }
        EditorCommand::SetWaveformScroll(scroll) => {
            world.write_message(timeline::SetWaveformScroll(scroll));
        }
        EditorCommand::Playtest => {
            world.write_message(timeline::Playtest);
        }
        EditorCommand::AddTimingPoint { time_seconds, bpm } => {
            world.write_message(timing::AddTimingPoint { time_seconds, bpm });
        }
        EditorCommand::RemoveTimingPoint(index) => {
            world.write_message(timing::RemoveTimingPoint(index));
        }
        EditorCommand::SetTimingPointTime(index, time) => {
            world.write_message(timing::SetTimingPointTime { index, time });
        }
        EditorCommand::SetTimingPointBpm(index, bpm) => {
            world.write_message(timing::SetTimingPointBpm { index, bpm });
        }
        EditorCommand::SetTimingPointTimeSignature(index, numerator, denominator) => {
            world.write_message(timing::SetTimingPointTimeSignature {
                index,
                numerator,
                denominator,
            });
        }
        EditorCommand::SetTimingSelected(index) => {
            world.write_message(timing::SetTimingSelected(index));
        }
        EditorCommand::BpmTap => {
            world.write_message(bpm::BpmTap);
        }
        EditorCommand::BpmTapReset => {
            world.write_message(bpm::BpmTapReset);
        }
        EditorCommand::SetSpawnHere => {
            world.write_message(spawn::SetSpawnHere);
        }
        EditorCommand::RotateSpawnDirection => {
            world.write_message(spawn::RotateSpawnDirection);
        }
        EditorCommand::RotatePlacementPreview => {
            world.write_message(spawn::RotatePlacementPreview);
        }
        EditorCommand::Undo => {
            world.write_message(history::Undo);
        }
        EditorCommand::Redo => {
            world.write_message(history::Redo);
        }
        EditorCommand::AdjustZoom(delta) => {
            world.write_message(camera::AdjustZoom(delta));
        }
        EditorCommand::SetCameraOrientation {
            rotation,
            pitch,
            transition_seconds,
        } => {
            world.write_message(camera::SetCameraOrientation {
                rotation,
                pitch,
                transition_seconds,
            });
        }
        EditorCommand::AddCameraTrigger => {
            world.write_message(camera::AddCameraTrigger);
        }
        EditorCommand::AddCameraFollowTrigger => {
            world.write_message(camera::AddCameraFollowTrigger);
        }
        EditorCommand::SetTriggerSelected(index) => {
            world.write_message(camera::SetTriggerSelected(index));
        }
        EditorCommand::ToggleHitboxVisualization => {
            world.write_message(misc::ToggleHitboxVisualization);
        }
        EditorCommand::TogglePerfOverlay => {
            world.write_message(misc::TogglePerfOverlay);
        }
        EditorCommand::LoadLevel(name) => {
            world.write_message(session::LoadLevel(name));
        }
        EditorCommand::RenameLevel(name) => {
            world.write_message(session::RenameLevel(name));
        }
        EditorCommand::ExportLevel => {
            world.write_message(session::ExportLevel);
        }
        EditorCommand::SetShowMetadata(show) => {
            world.write_message(session::SetShowMetadata(show));
        }
        EditorCommand::ToggleSettings => {
            world.write_message(session::ToggleSettings);
        }
        EditorCommand::SetShowSettings(show) => {
            world.write_message(session::SetShowSettings(show));
        }
        EditorCommand::SetSettingsSection(section) => {
            world.write_message(session::SetSettingsSection(section));
        }
        EditorCommand::SetGraphicsBackend(backend) => {
            world.write_message(session::SetGraphicsBackend(backend));
        }
        EditorCommand::SetAudioBackend(backend) => {
            world.write_message(session::SetAudioBackend(backend));
        }
        EditorCommand::SetUiScaleMultiplier(scale) => {
            world.write_message(session::SetUiScaleMultiplier(scale));
        }
        EditorCommand::SetKeybindCapture(capture) => {
            world.write_message(session::SetKeybindCapture(capture));
        }
        EditorCommand::SetKeybind {
            action,
            slot,
            chord,
        } => {
            world.write_message(session::SetKeybind {
                action,
                slot,
                chord,
            });
        }
        EditorCommand::ClearKeybindSlot { action, slot } => {
            world.write_message(session::ClearKeybindSlot { action, slot });
        }
        EditorCommand::ResetKeybind(action) => {
            world.write_message(session::ResetKeybind(action));
        }
        EditorCommand::ResetKeybinds => {
            world.write_message(session::ResetKeybinds);
        }
        EditorCommand::CompleteImport => {
            world.write_message(session::CompleteImport);
        }
        EditorCommand::UpdateMusic(metadata) => {
            world.write_message(session::UpdateMusic(metadata));
        }
        EditorCommand::UpdateCreatorMetadata(metadata) => {
            world.write_message(session::UpdateCreatorMetadata(metadata));
        }
        EditorCommand::UpdateSkyColor(color) => {
            world.write_message(session::UpdateSkyColor(color));
        }
        EditorCommand::TriggerAudioImport => {
            world.write_message(session::TriggerAudioImport);
        }
        EditorCommand::CaptureMenuPreviewCamera => {
            world.write_message(session::CaptureMenuPreviewCamera);
        }
        EditorCommand::UseAutoMenuPreviewCamera => {
            world.write_message(session::UseAutoMenuPreviewCamera);
        }
        EditorCommand::SetShiftHeld(held) => {
            world.write_message(input_state::SetShiftHeld(held));
        }
        EditorCommand::SetCtrlHeld(held) => {
            world.write_message(input_state::SetCtrlHeld(held));
        }
        EditorCommand::SetAltHeld(held) => {
            world.write_message(input_state::SetAltHeld(held));
        }
        EditorCommand::SetPanUpHeld(held) => {
            world.write_message(input_state::SetPanUpHeld(held));
        }
        EditorCommand::SetPanDownHeld(held) => {
            world.write_message(input_state::SetPanDownHeld(held));
        }
        EditorCommand::SetPanLeftHeld(held) => {
            world.write_message(input_state::SetPanLeftHeld(held));
        }
        EditorCommand::SetPanRightHeld(held) => {
            world.write_message(input_state::SetPanRightHeld(held));
        }
        EditorCommand::MouseButton { button, pressed } => {
            world.write_message(pointer::MouseButton { button, pressed });
        }
        EditorCommand::PrimaryClick { x, y } => {
            world.write_message(pointer::PrimaryClick { x, y });
        }
        EditorCommand::PointerMoved { x, y } => {
            world.write_message(pointer::PointerMoved { x, y });
        }
        EditorCommand::UpdateCursorFromScreen { x, y } => {
            world.write_message(pointer::UpdateCursorFromScreen { x, y });
        }
        EditorCommand::CameraDrag { dx, dy } => {
            world.write_message(pointer::CameraDrag { dx, dy });
        }
        EditorCommand::TogglePlaceWindow => {
            world.write_message(misc::TogglePlaceWindow);
        }
        EditorCommand::Escape => {
            world.write_message(misc::Escape);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use eg_state::events::{navigation, session, timeline};
    use eg_state::state::editor_command::EditorCommand;

    use super::*;

    #[test]
    fn ui_plugin_drains_command_queue_into_messages() {
        let mut app = App::new();
        app.add_plugins(UiPlugin);
        enqueue_ui_commands(
            app.world_mut(),
            [
                AppCommand::TurnRight,
                AppCommand::Editor(EditorCommand::SetTimelineTime(42.0)),
            ],
        );

        app.update();

        let turn_messages = app.world().resource::<Messages<navigation::TurnRight>>();
        let mut turn_reader = turn_messages.get_cursor();
        assert_eq!(turn_reader.read(turn_messages).count(), 1);

        let timeline_messages = app
            .world()
            .resource::<Messages<timeline::SetTimelineTime>>();
        let mut timeline_reader = timeline_messages.get_cursor();
        let events = timeline_reader.read(timeline_messages).collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, 42.0);
    }

    #[test]
    fn write_ui_commands_writes_each_domain_message() {
        let mut app = App::new();
        app.add_plugins(UiPlugin);

        write_ui_commands(
            app.world_mut(),
            [
                AppCommand::Editor(EditorCommand::ToggleSettings),
                AppCommand::GameSetPracticeMode(true),
            ],
        );

        let settings_messages = app.world().resource::<Messages<session::ToggleSettings>>();
        let mut settings_reader = settings_messages.get_cursor();
        assert_eq!(settings_reader.read(settings_messages).count(), 1);

        let practice_messages = app
            .world()
            .resource::<Messages<navigation::GameSetPracticeMode>>();
        let mut practice_reader = practice_messages.get_cursor();
        let events = practice_reader.read(practice_messages).collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert!(events[0].enabled);
    }
}
