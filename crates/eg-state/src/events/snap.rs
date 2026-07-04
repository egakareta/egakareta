/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Snap / grid events.
//!
//! These handlers mutate App-world state directly (same pattern as the
//! `input_state` and `pointer` modules).
//! The handlers mutate `EditorConfigRes` directly, and the legacy
//! `State.editor.config` fields are synced from the resource at the top of
//! the per-frame `StateSchedule`.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::state::EditorConfigRes;

/// Set whether to snap to grid.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapToGrid(pub bool);

/// Set the grid snap step.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapStep(pub f32);

/// Set whether rotation uses snapping.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapRotation(pub bool);

/// Set the rotation snap step in degrees.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSnapRotationStep(pub f32);

pub fn register_events(app: &mut App) {
    app.add_message::<SetSnapToGrid>()
        .add_message::<SetSnapStep>()
        .add_message::<SetSnapRotation>()
        .add_message::<SetSnapRotationStep>();
}

// -----------------------------------------------------------------------
// Carve-out dispatch systems: handlers mutate `EditorConfigRes` directly
// so the App world is the authoritative store for the snap configuration.
// The legacy `State.editor.config` is refreshed from the resource by
// `Runtime::sync_app_editor_config_state` before any dispatch in the same
// frame.
// -----------------------------------------------------------------------

pub fn handle_set_snap_to_grid(
    mut events: MessageReader<SetSnapToGrid>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.snap_to_grid = event.0;
    }
}

pub fn handle_set_snap_step(
    mut events: MessageReader<SetSnapStep>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.snap_step = event.0;
    }
}

pub fn handle_set_snap_rotation(
    mut events: MessageReader<SetSnapRotation>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.snap_rotation = event.0;
    }
}

pub fn handle_set_snap_rotation_step(
    mut events: MessageReader<SetSnapRotationStep>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.snap_rotation_step_degrees = event.0;
    }
}
