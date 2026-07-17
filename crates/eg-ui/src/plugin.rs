/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy UI plugin and command bridge.
//!
//! REFACTOR POINT(full-bevy): the manual command bridge
//! are replaced by `bevy_egui` systems writing Bevy input/events directly.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use eg_state::commands::AppCommand;
use eg_state::events;

/// Registers egui-facing resources and typed command messages.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        eg_state::events::register_command_events(app);
    }
}

/// Register UI resources on a bare World without adding App-level systems.
pub fn register_ui_resources_world(world: &mut World) {
    eg_state::events::register_command_events_world(world);
}

/// Immediately write UI commands to the Bevy message bus.
pub fn write_ui_commands(world: &mut World, commands: impl IntoIterator<Item = AppCommand>) {
    events::write_app_command_messages(world, commands);
}

/// Immediately write one UI command to the Bevy message bus.
pub fn write_ui_command(world: &mut World, command: AppCommand) {
    events::write_app_command_message(world, command);
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use eg_state::events::{navigation, session};
    use eg_state::state::editor_command::EditorCommand;

    use super::*;

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
