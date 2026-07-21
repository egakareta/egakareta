/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! History (undo / redo) events + a `HistorySnapshot` marker Message used
//! by direct-manipulation UI to mark a "before-mutation" snapshot.

use bevy::{
    app::App,
    ecs::{prelude::*, system::SystemParam},
    state::prelude::State,
};
use eg_editor_domain::derive_tap_indicator_positions;
use eg_types::{AppPhase, EditorInputState, LevelObject};

use crate::state::{
    EditorAuthoredObject, EditorConfigRes, EditorHistoryRes, EditorHistorySnapshot,
    EditorObjectOrder, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes,
    EditorTriggerRes, collect_editor_object_data, replace_editor_objects_commands,
};

/// Undo the last editor action.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Undo;

/// Redo the last undone editor action.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Redo;

/// Marker Message emitted by direct-manipulation UI (gizmo drag, marquee,
/// block drag) to request a pre-mutation history snapshot.
///
/// Undo/redo history stacks now live in `EditorHistoryRes`, but
/// pre-mutation snapshot recording still belongs to the legacy mutation paths.
/// In a later phase this Message drives a `PreCommandHistorySet` system that
/// snapshots `ObjectBundle` data before any mutating handler runs.
#[derive(Message, Debug, Clone, Default)]
pub struct HistorySnapshot {
    /// Free-form label for the snapshot (e.g. "gizmo_drag_release").
    pub label: String,
}

pub fn register_events(app: &mut App) {
    app.add_message::<Undo>()
        .add_message::<Redo>()
        .add_message::<HistorySnapshot>();
}

#[derive(SystemParam)]
pub struct HistoryResources<'w, 's> {
    authored_objects: Query<
        'w,
        's,
        (&'static EditorObjectOrder, &'static eg_game::ObjectData),
        With<EditorAuthoredObject>,
    >,
    existing_objects: Query<'w, 's, Entity, With<EditorAuthoredObject>>,
    commands: Commands<'w, 's>,
    input: ResMut<'w, crate::state::EditorInputRes>,
    config: ResMut<'w, EditorConfigRes>,
    spawn: ResMut<'w, EditorSpawnRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    timing: ResMut<'w, EditorTimingRes>,
    trigger: ResMut<'w, EditorTriggerRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
}

pub fn handle_undo(
    mut events: MessageReader<Undo>,
    phase: Res<State<AppPhase>>,
    mut resources: HistoryResources,
) {
    for _ in events.read() {
        if *phase.get() != AppPhase::Editor {
            continue;
        }
        let Some(snapshot) = resources.history.0.undo.pop() else {
            continue;
        };
        let current_objects = collect_editor_object_data(&resources.authored_objects);
        let current = history_snapshot_from_resources(&resources, &current_objects);
        resources.history.0.redo.push(current);
        apply_history_snapshot_to_resources(snapshot, &mut resources);
    }
}

pub fn handle_redo(
    mut events: MessageReader<Redo>,
    phase: Res<State<AppPhase>>,
    mut resources: HistoryResources,
) {
    for _ in events.read() {
        if *phase.get() != AppPhase::Editor {
            continue;
        }
        let Some(snapshot) = resources.history.0.redo.pop() else {
            continue;
        };
        let current_objects = collect_editor_object_data(&resources.authored_objects);
        let current = history_snapshot_from_resources(&resources, &current_objects);
        resources.history.0.undo.push(current);
        apply_history_snapshot_to_resources(snapshot, &mut resources);
    }
}

fn history_snapshot_from_resources(
    resources: &HistoryResources<'_, '_>,
    objects: &[LevelObject],
) -> EditorHistorySnapshot {
    EditorHistorySnapshot {
        objects: objects.to_vec(),
        selected_block_index: resources.input.0.selected_block_index,
        selected_block_indices: resources.input.0.selected_block_indices.clone(),
        cursor: resources.input.0.cursor,
        selected_block_id: resources.config.0.selected_block_id.clone(),
        selected_block_rotation_degrees: resources.config.0.selected_block_rotation_degrees,
        recent_block_ids: resources.config.0.recent_block_ids.clone(),
        spawn: resources.spawn.0.clone(),
        timeline_time_seconds: resources.timeline.0.clock.time_seconds,
        timeline_duration_seconds: resources.timeline.0.clock.duration_seconds,
        tap_times: resources.timeline.0.taps.tap_times.clone(),
        tap_indicator_positions: resources.timeline.0.taps.tap_indicator_positions.clone(),
        selected_tap_index: resources.timeline.0.taps.selected_index,
        timing_points: resources.timing.0.timing_points.clone(),
        selected_trigger_index: resources.trigger.0.selected_index,
    }
}

fn apply_history_snapshot_to_resources(
    snapshot: EditorHistorySnapshot,
    resources: &mut HistoryResources<'_, '_>,
) {
    let objects = snapshot.objects;
    replace_editor_objects_commands(
        &mut resources.commands,
        &resources.existing_objects,
        &objects,
    );

    resources.input.0.selected_block_index = snapshot.selected_block_index;
    resources.input.0.selected_block_indices = snapshot.selected_block_indices;
    normalize_block_selection(&mut resources.input.0, objects.len());
    resources.input.0.hovered_block_index = resources.input.0.selected_block_index;
    resources.input.0.cursor = snapshot.cursor;

    resources.config.0.selected_block_id = snapshot.selected_block_id;
    resources.config.0.selected_block_rotation_degrees = snapshot.selected_block_rotation_degrees;
    resources.config.0.recent_block_ids = snapshot.recent_block_ids;
    resources.spawn.0 = snapshot.spawn;

    resources.timeline.0.clock.time_seconds = snapshot.timeline_time_seconds.max(0.0);
    resources.timeline.0.clock.duration_seconds = snapshot.timeline_duration_seconds.max(0.1);
    resources.timeline.0.taps.tap_times = snapshot.tap_times;
    resources.timeline.0.taps.tap_indicator_positions = snapshot.tap_indicator_positions;
    resources.timeline.0.taps.selected_index = snapshot.selected_tap_index;
    normalize_taps(&mut resources.timeline.0, &resources.spawn.0, &objects);

    resources.timing.0.timing_points = snapshot.timing_points;
    resources.timing.0.mark_timing_points_changed();

    let trigger_count = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .count();
    resources.trigger.0.selected_index = snapshot
        .selected_trigger_index
        .filter(|index| *index < trigger_count);

    resources.runtime.0.interaction.gizmo_drag = None;
    resources.runtime.0.interaction.block_drag = None;
}

fn normalize_block_selection(input: &mut EditorInputState, object_count: usize) {
    input
        .selected_block_indices
        .retain(|index| *index < object_count);
    if let Some(primary) = input.selected_block_index
        && primary < object_count
        && !input.selected_block_indices.contains(&primary)
    {
        input.selected_block_indices.push(primary);
    }
    input.selected_block_indices.sort_unstable();
    input.selected_block_indices.dedup();
    input.selected_block_index = input
        .selected_block_index
        .filter(|index| input.selected_block_indices.binary_search(index).is_ok())
        .or_else(|| input.selected_block_indices.first().copied());
    if input
        .hovered_block_index
        .is_some_and(|index| index >= object_count)
    {
        input.hovered_block_index = None;
    }
}

fn normalize_taps(
    timeline: &mut crate::state::EditorTimelineState,
    spawn: &eg_types::SpawnMetadata,
    objects: &[LevelObject],
) {
    timeline
        .taps
        .tap_times
        .retain(|tap| tap.is_finite() && *tap >= 0.0);
    timeline.taps.tap_times.sort_by(f32::total_cmp);
    timeline
        .taps
        .tap_times
        .retain(|tap| *tap <= timeline.clock.duration_seconds);

    if timeline.taps.tap_indicator_positions.len() != timeline.taps.tap_times.len() {
        timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            spawn.position,
            spawn.direction,
            &timeline.taps.tap_times,
            objects,
        );
    }
    timeline.taps.selected_index = timeline
        .taps
        .selected_index
        .filter(|index| *index < timeline.taps.tap_times.len());
    timeline.clock.time_seconds = timeline
        .clock
        .time_seconds
        .min(timeline.clock.duration_seconds);
}

/// No-op handler for [`HistorySnapshot`]. Reserves the system slot in the
/// `CommandHandlerSet` so a follow-up phase can wire a real pre-mutation
/// snapshot system without re-orchestrating the schedule.
pub fn handle_history_snapshot(mut events: MessageReader<HistorySnapshot>) {
    // Drain the events so they don't accumulate frame-over-frame. No
    // command is generated — snapshots are written by a dedicated
    // pre-mutation system in a later phase.
    for _ in events.read() {}
}
