/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy UI plugin.
//!
//! Production uses `bevy_egui` `EguiPrimaryContextPass` for all UI.
//! The custom egui presentation has been removed with the old runtime.

use bevy::app::{App, Plugin};

/// Registers egui-facing resources and typed command messages.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        eg_state::events::register_command_events(app);
    }
}
