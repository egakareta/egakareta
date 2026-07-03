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
use eg_state::events;

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
    eg_state::events::register_command_events_world(world);
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
    events::write_app_command_messages(world, commands);
}

/// Immediately write one UI command to the Bevy message bus.
pub fn write_ui_command(world: &mut World, command: AppCommand) {
    events::write_app_command_message(world, command);
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
