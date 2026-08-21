/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Spawn / placement preview events.
//!
//! All 3 messages in this module are **carve-outs** (same pattern as
//! `events::snap`, `events::editor_mode::handle_set_block_id`, and
//! `events::session`): the per-message handlers in this file mutate
//! App-world resources directly instead of staging a legacy
//! `EditorCommand`.
//!
//! - `SetSpawnHere` / `RotateSpawnDirection` mutate the authoritative
//!   `EditorSpawnRes.0` spawn position and direction directly.
//!
//! - `RotatePlacementPreview` mutates the authoritative
//!   `EditorConfigRes.0.selected_block_rotation_degrees[1]` placement yaw.

use bevy::{app::App, ecs::prelude::*};
use eg_editor_domain::toggle_spawn_direction;

use crate::state::{EditorConfigRes, EditorInputRes, EditorSpawnRes};

/// Set spawn position to the current editor cursor.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct SetSpawnHere;

/// Rotate the spawn direction (cycles Forward / Right).
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RotateSpawnDirection;

/// Rotate the block placement preview yaw by 90 degrees.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RotatePlacementPreview;

pub fn register_events(app: &mut App) {
    app.add_message::<SetSpawnHere>()
        .add_message::<RotateSpawnDirection>()
        .add_message::<RotatePlacementPreview>();
}

// -----------------------------------------------------------------------
// Dispatch systems (all carve-outs: write directly to App-world resources)
// -----------------------------------------------------------------------

/// Set the spawn position to the current editor cursor (read from
/// `EditorInputRes.0.cursor`). Mirrors the `SetSpawnHere` legacy
/// dispatch path (`force_editor_cursor_from_pointer` +
/// `editor_set_spawn_here`).
pub fn handle_set_spawn_here(
    mut events: MessageReader<SetSpawnHere>,
    mut spawn: ResMut<EditorSpawnRes>,
    input: Res<EditorInputRes>,
) {
    for _ in events.read() {
        spawn.0.position = input.0.cursor;
    }
}

/// Cycle the spawn direction Forward → Right → Forward using the shared
/// `toggle_spawn_direction` domain operation.
pub fn handle_rotate_spawn_direction(
    mut events: MessageReader<RotateSpawnDirection>,
    mut spawn: ResMut<EditorSpawnRes>,
) {
    for _ in events.read() {
        spawn.0.direction = toggle_spawn_direction(spawn.0.direction);
    }
}

/// Increment the block-placement preview yaw (`[1]` index) by 90
/// degrees, wrapping into `0..360`.
pub fn handle_rotate_placement_preview(
    mut events: MessageReader<RotatePlacementPreview>,
    mut config: ResMut<EditorConfigRes>,
) {
    for _ in events.read() {
        config.0.selected_block_rotation_degrees[1] =
            (config.0.selected_block_rotation_degrees[1] + 90.0).rem_euclid(360.0);
    }
}
