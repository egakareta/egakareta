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
//! App-world resources directly.
//!
//! - `SetSpawnHere` / `RotateSpawnDirection` mutate the authoritative
//!   `EditorSpawnRes.0` spawn position and direction directly.
//!
//! - `RotatePlacementPreview` mutates the authoritative
//!   `EditorConfigRes.0.selected_block_rotation_degrees[1]` placement yaw.

use bevy::{
    app::App,
    ecs::{prelude::*, system::SystemParam},
};
use eg_editor_domain::toggle_spawn_direction;

use crate::state::{
    EditorAuthoredObject, EditorConfigRes, EditorHistoryRes, EditorHistorySnapshot, EditorInputRes,
    EditorObjectOrder, EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
    collect_editor_object_data,
};

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
    mut resources: SpawnActionResources,
) {
    for _ in events.read() {
        resources.record_history();
        resources.spawn.0.position = resources.input.0.cursor;
    }
}

/// Cycle the spawn direction Forward → Right → Forward.
pub fn handle_rotate_spawn_direction(
    mut events: MessageReader<RotateSpawnDirection>,
    mut resources: SpawnActionResources,
) {
    for _ in events.read() {
        resources.record_history();
        resources.spawn.0.direction = toggle_spawn_direction(resources.spawn.0.direction);
    }
}

#[derive(SystemParam)]
pub struct SpawnActionResources<'w, 's> {
    input: Res<'w, EditorInputRes>,
    config: Res<'w, EditorConfigRes>,
    spawn: ResMut<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: Res<'w, EditorTriggerRes>,
    authored_objects: Query<
        'w,
        's,
        (&'static EditorObjectOrder, &'static eg_game::ObjectData),
        With<EditorAuthoredObject>,
    >,
    history: ResMut<'w, EditorHistoryRes>,
}

impl SpawnActionResources<'_, '_> {
    fn record_history(&mut self) {
        crate::state::record_history_snapshot(
            &mut self.history.0,
            EditorHistorySnapshot {
                objects: collect_editor_object_data(&self.authored_objects),
                selected_block_index: self.input.0.selected_block_index,
                selected_block_indices: self.input.0.selected_block_indices.clone(),
                cursor: self.input.0.cursor,
                selected_block_id: self.config.0.selected_block_id.clone(),
                selected_block_rotation_degrees: self.config.0.selected_block_rotation_degrees,
                recent_block_ids: self.config.0.recent_block_ids.clone(),
                spawn: self.spawn.0.clone(),
                timeline_time_seconds: self.timeline.0.clock.time_seconds,
                timeline_duration_seconds: self.timeline.0.clock.duration_seconds,
                tap_times: self.timeline.0.taps.tap_times.clone(),
                tap_indicator_positions: self.timeline.0.taps.tap_indicator_positions.clone(),
                selected_tap_index: self.timeline.0.taps.selected_index,
                timing_points: self.timing.0.timing_points.clone(),
                selected_trigger_index: self.trigger.0.selected_index,
            },
        );
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
