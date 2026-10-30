/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! History (undo / redo) events + a `HistorySnapshot` marker Message used
//! by direct-manipulation UI to mark a "before-mutation" snapshot.

use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_editor_domain::derive_tap_indicator_positions;
use eg_types::{AppPhase, EditorInputState, LevelObject};

use crate::{events::MessageReader, state::*};

/// Undo the last editor action.
#[derive(Debug, Clone, Copy, Default)]
pub struct Undo;

/// Redo the last undone editor action.
#[derive(Debug, Clone, Copy, Default)]
pub struct Redo;

/// Marker Message emitted by direct-manipulation UI (gizmo drag, marquee,
/// block drag) to request a pre-mutation history snapshot.
///
/// History stacks live in `EditorHistoryRes`. This message records an undo
/// snapshot of `ObjectBundle` data before a mutating handler runs.
#[derive(Debug, Clone, Default)]
pub struct HistorySnapshot {
    /// Free-form label for the snapshot (e.g. "gizmo_drag_release").
    pub label: String,
}

#[derive(SystemParam)]
pub struct HistoryResources<'w, 's> {
    authored_objects: Query<
        'w,
        's,
        (&'static EditorObjectOrder, &'static eg_game::ObjectData),
        With<EditorAuthoredObject>,
    >,
    existing_objects: Query<
        'w,
        's,
        (
            Entity,
            &'static eg_game::ObjectId,
            &'static eg_game::ObjectData,
            &'static EditorObjectOrder,
        ),
        With<EditorAuthoredObject>,
    >,
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
        // Single collect reused for the redo inverse AND the apply, so
        // undo/redo pays for exactly one scene collect.
        let current_objects = collect_editor_object_data(&resources.authored_objects);
        let opposite = snapshot
            .object_change
            .as_ref()
            .and_then(|change| invert_change(change, &current_objects));
        let redo_snapshot = match opposite {
            Some(change) => delta_snapshot_from_resources(&resources, change),
            None => history_snapshot_from_resources(&resources, &current_objects),
        };
        resources.history.0.redo.push(redo_snapshot);
        apply_history_snapshot_to_resources(snapshot, &mut resources, Some(current_objects));
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
        let opposite = snapshot
            .object_change
            .as_ref()
            .and_then(|change| invert_change(change, &current_objects));
        let undo_snapshot = match opposite {
            Some(change) => delta_snapshot_from_resources(&resources, change),
            None => history_snapshot_from_resources(&resources, &current_objects),
        };
        resources.history.0.undo.push(undo_snapshot);
        apply_history_snapshot_to_resources(snapshot, &mut resources, Some(current_objects));
    }
}

/// Snapshot carrying a delta instead of a full scene clone. The `objects`
/// vec stays empty; undo/redo applies `object_change` to the live scene.
fn delta_snapshot_from_resources(
    resources: &HistoryResources<'_, '_>,
    object_change: EditorHistoryObjectChange,
) -> EditorHistorySnapshot {
    EditorHistorySnapshot {
        objects: Vec::new(),
        object_change: Some(object_change),
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

/// Time window within which a transform trigger counts as "same time" for
/// target replacement. Mirrors the record-side constant in
/// `events::selection`.
const TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS: f32 = 0.001;

/// Derives the opposite-stack delta for `change` from the pre-apply scene
/// `current`, capturing only what applying `change` would destroy (a few
/// clones instead of a 19k scene clone). Returns `None` when the delta
/// cannot be inverted (e.g. targets missing), in which case callers fall
/// back to a full snapshot.
fn invert_change(
    change: &EditorHistoryObjectChange,
    current: &[LevelObject],
) -> Option<EditorHistoryObjectChange> {
    match change {
        EditorHistoryObjectChange::RemovePlaced { object_id, index } => {
            let found = current
                .iter()
                .position(|object| object.id == *object_id)
                .unwrap_or(*index);
            let object = current.get(found)?.clone();
            Some(EditorHistoryObjectChange::ReinsertPlaced {
                objects: vec![(found, object)],
            })
        }
        EditorHistoryObjectChange::RemoveAdded { object_ids, .. } => {
            let mut entries = Vec::with_capacity(object_ids.len());
            for id in object_ids {
                let index = current.iter().position(|object| object.id == *id)?;
                entries.push((index, current[index].clone()));
            }
            Some(EditorHistoryObjectChange::ReinsertPlaced { objects: entries })
        }
        EditorHistoryObjectChange::ReinsertRemoved { removed, .. } => {
            let object_ids = removed
                .iter()
                .map(|(_, object)| object.id.clone())
                .collect();
            Some(EditorHistoryObjectChange::RemoveAndDereference { object_ids })
        }
        EditorHistoryObjectChange::RestoreOne { index, .. } => {
            let before = current.get(*index)?.clone();
            Some(EditorHistoryObjectChange::RestoreOne {
                index: *index,
                before,
            })
        }
        EditorHistoryObjectChange::RestoreMany { before } => {
            let mut inverse = Vec::with_capacity(before.len());
            for (index, _) in before {
                inverse.push((*index, current.get(*index)?.clone()));
            }
            Some(EditorHistoryObjectChange::RestoreMany { before: inverse })
        }
        EditorHistoryObjectChange::AddTriggers { added_ids, .. } => {
            let mut entries = Vec::with_capacity(added_ids.len());
            for id in added_ids {
                let index = current.iter().position(|object| object.id == *id)?;
                entries.push((index, current[index].clone()));
            }
            Some(EditorHistoryObjectChange::ReaddTriggers { objects: entries })
        }
        EditorHistoryObjectChange::ReinsertPlaced { objects } => {
            let object_ids = objects
                .iter()
                .map(|(_, object)| object.id.clone())
                .collect::<Vec<_>>();
            let indices = objects.iter().map(|(index, _)| *index).collect();
            Some(EditorHistoryObjectChange::RemoveAdded {
                object_ids,
                indices,
            })
        }
        EditorHistoryObjectChange::RemoveAndDereference { object_ids } => {
            let mut sorted = object_ids.clone();
            sorted.sort_unstable();
            sorted.dedup();
            let mut removed = Vec::new();
            for id in &sorted {
                let index = current.iter().position(|object| object.id == *id)?;
                removed.push((index, current[index].clone()));
            }
            let repaired = triggers_referencing_ids(current, &sorted);
            Some(EditorHistoryObjectChange::ReinsertRemoved {
                removed,
                repaired_triggers: repaired,
            })
        }
        EditorHistoryObjectChange::ReaddTriggers { objects } => {
            let replaced_ids = trigger_target_ids(objects);
            let modified_before =
                same_time_triggers_referencing_ids(current, objects, &replaced_ids);
            Some(EditorHistoryObjectChange::AddTriggers {
                added_ids: objects
                    .iter()
                    .map(|(_, object)| object.id.clone())
                    .collect(),
                modified_before,
            })
        }
    }
}

/// Pre-mutation states of transform triggers whose targets intersect
/// `removed_ids`. Clones only affected triggers (usually zero).
fn triggers_referencing_ids(
    objects: &[LevelObject],
    removed_ids: &[String],
) -> Vec<(usize, LevelObject)> {
    objects
        .iter()
        .enumerate()
        .filter(|(_, object)| {
            let Some(trigger) = object.trigger.as_ref() else {
                return false;
            };
            if !trigger.transform {
                return false;
            }
            let eg_types::TriggerTarget::Objects { object_ids } = &trigger.target else {
                return false;
            };
            object_ids
                .iter()
                .any(|id| removed_ids.binary_search(id).is_ok())
        })
        .map(|(index, object)| (index, object.clone()))
        .collect()
}

/// Union of transform-trigger target IDs over readded trigger `entries`.
fn trigger_target_ids(entries: &[(usize, LevelObject)]) -> Vec<String> {
    let mut ids = Vec::new();
    for (_, object) in entries {
        if let Some(trigger) = object.trigger.as_ref().filter(|trigger| trigger.transform)
            && let eg_types::TriggerTarget::Objects { object_ids } = &trigger.target
        {
            ids.extend(object_ids.iter().cloned());
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Pre-mutation states of same-time transform triggers (excluding the
/// readded `entries` themselves) that reference `replaced_ids`.
fn same_time_triggers_referencing_ids(
    current: &[LevelObject],
    entries: &[(usize, LevelObject)],
    replaced_ids: &[String],
) -> Vec<(usize, LevelObject)> {
    if replaced_ids.is_empty() {
        return Vec::new();
    }
    let entry_indices: Vec<usize> = entries.iter().map(|(index, _)| *index).collect();
    let mut out = Vec::new();
    for (index, object) in current.iter().enumerate() {
        if entry_indices.contains(&index) {
            continue;
        }
        let Some(trigger) = object.trigger.as_ref().filter(|trigger| trigger.transform) else {
            continue;
        };
        let eg_types::TriggerTarget::Objects { object_ids } = &trigger.target else {
            continue;
        };
        if !object_ids
            .iter()
            .any(|id| replaced_ids.binary_search(id).is_ok())
        {
            continue;
        }
        let same_time = entries.iter().any(|(_, entry)| {
            entry.trigger.as_ref().is_some_and(|entry_trigger| {
                (entry_trigger.time_seconds - trigger.time_seconds).abs()
                    <= TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
            })
        });
        if same_time {
            out.push((index, object.clone()));
        }
    }
    out
}

/// Strips `removed_ids` from every transform-trigger target list.
fn strip_trigger_target_ids(objects: &mut [LevelObject], removed_ids: &[String]) {
    if removed_ids.is_empty() {
        return;
    }
    for object in objects.iter_mut() {
        let Some(trigger) = object.trigger.as_mut().filter(|trigger| trigger.transform) else {
            continue;
        };
        let eg_types::TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        object_ids.retain(|id| removed_ids.binary_search(id).is_err());
    }
}

/// Re-applies record-time target replacement for readded triggers: strips
/// each readded trigger's target IDs from same-time transform triggers.
fn strip_replaced_trigger_targets(objects: &mut [LevelObject], entries: &[(usize, LevelObject)]) {
    for (_, entry) in entries {
        let Some(entry_trigger) = entry.trigger.as_ref().filter(|trigger| trigger.transform) else {
            continue;
        };
        let eg_types::TriggerTarget::Objects { object_ids } = &entry_trigger.target else {
            continue;
        };
        if object_ids.is_empty() {
            continue;
        }
        let mut sorted = object_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        for object in objects.iter_mut() {
            if object.id == entry.id {
                continue;
            }
            let Some(trigger) = object.trigger.as_mut().filter(|trigger| trigger.transform) else {
                continue;
            };
            if (trigger.time_seconds - entry_trigger.time_seconds).abs()
                > TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
            {
                continue;
            }
            let eg_types::TriggerTarget::Objects {
                object_ids: targets,
            } = &mut trigger.target
            else {
                continue;
            };
            targets.retain(|id| sorted.binary_search(id).is_err());
        }
    }
}

fn history_snapshot_from_resources(
    resources: &HistoryResources<'_, '_>,
    objects: &[LevelObject],
) -> EditorHistorySnapshot {
    EditorHistorySnapshot {
        objects: objects.to_vec(),
        object_change: None,
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

/// Applies `snapshot` to the world, reusing `collected` (a fresh collect of
/// the live scene) when the caller already collected it, so undo/redo pays
/// for exactly one 19k collect instead of two plus a full-scene clone.
fn apply_history_snapshot_to_resources(
    snapshot: EditorHistorySnapshot,
    resources: &mut HistoryResources<'_, '_>,
    collected: Option<Vec<LevelObject>>,
) {
    // Lazily collect only for deltas that need the live scene; full
    // snapshots move their stored vec with zero collect.
    let mut collected = collected;
    let mut live = |resources: &HistoryResources<'_, '_>| {
        collected
            .take()
            .unwrap_or_else(|| collect_editor_object_data(&resources.authored_objects))
    };
    let objects = match snapshot.object_change {
        Some(EditorHistoryObjectChange::RemovePlaced { object_id, index }) => {
            let mut objects = live(resources);
            let remove_index = objects
                .iter()
                .position(|object| object.id == object_id)
                .unwrap_or(index);
            if remove_index < objects.len() {
                objects.remove(remove_index);
            }
            objects
        }
        Some(EditorHistoryObjectChange::RemoveAdded {
            object_ids,
            indices,
        }) => {
            let mut objects = live(resources);
            // Remove by ID (robust to order shifts); fall back to recorded
            // indices for IDs already missing (e.g. double-undo safety).
            let mut id_set = object_ids;
            id_set.sort_unstable();
            id_set.dedup();
            let mut remove_indices: Vec<usize> = objects
                .iter()
                .enumerate()
                .filter(|(_, object)| id_set.binary_search(&object.id).is_ok())
                .map(|(index, _)| index)
                .collect();
            // Any IDs not found by scan (already removed?) try index fallback.
            // Indices refer to pre-undo positions; clamp defensively.
            for fallback in indices {
                if fallback < objects.len()
                    && !remove_indices.contains(&fallback)
                    && !id_set.is_empty()
                {
                    // Only use fallback when the object at that index still
                    // carries one of the target IDs (avoids deleting wrong).
                    if id_set.binary_search(&objects[fallback].id).is_ok() {
                        remove_indices.push(fallback);
                    }
                }
            }
            remove_indices.sort_unstable();
            remove_indices.dedup();
            // Single O(n) retain instead of O(n*m) repeated removes.
            if remove_indices.len() == 1 {
                objects.remove(remove_indices[0]);
            } else if !remove_indices.is_empty() {
                let mut cursor = 0usize;
                let mut pos = 0usize;
                objects.retain(|_| {
                    let remove = cursor < remove_indices.len() && pos == remove_indices[cursor];
                    pos += 1;
                    if remove {
                        cursor += 1;
                        false
                    } else {
                        true
                    }
                });
            }
            objects
        }
        Some(EditorHistoryObjectChange::ReinsertRemoved {
            removed,
            repaired_triggers,
        }) => {
            let mut objects = live(resources);
            for (index, object) in repaired_triggers {
                if index < objects.len() {
                    objects[index] = object;
                }
            }
            let mut sorted = removed;
            sorted.sort_by_key(|(index, _)| *index);
            for (index, object) in sorted {
                let at = index.min(objects.len());
                objects.insert(at, object);
            }
            objects
        }
        Some(EditorHistoryObjectChange::RestoreOne { index, before }) => {
            let mut objects = live(resources);
            if index < objects.len() {
                objects[index] = before;
            }
            objects
        }
        Some(EditorHistoryObjectChange::RestoreMany { before }) => {
            let mut objects = live(resources);
            for (index, object) in before {
                if index < objects.len() {
                    objects[index] = object;
                }
            }
            objects
        }
        Some(EditorHistoryObjectChange::AddTriggers {
            added_ids,
            modified_before,
        }) => {
            let mut objects = live(resources);
            for (index, object) in modified_before {
                if index < objects.len() {
                    objects[index] = object;
                }
            }
            // Remove added triggers by ID (robust to order shifts).
            let mut id_set = added_ids;
            id_set.sort_unstable();
            id_set.dedup();
            objects.retain(|object| id_set.binary_search(&object.id).is_err());
            objects
        }
        Some(EditorHistoryObjectChange::ReinsertPlaced { objects: entries }) => {
            let mut objects = live(resources);
            let mut sorted = entries;
            sorted.sort_by_key(|(index, _)| *index);
            for (index, object) in sorted {
                let at = index.min(objects.len());
                objects.insert(at, object);
            }
            objects
        }
        Some(EditorHistoryObjectChange::RemoveAndDereference { object_ids }) => {
            let mut objects = live(resources);
            let mut id_set = object_ids;
            id_set.sort_unstable();
            id_set.dedup();
            strip_trigger_target_ids(&mut objects, &id_set);
            objects.retain(|object| id_set.binary_search(&object.id).is_err());
            objects
        }
        Some(EditorHistoryObjectChange::ReaddTriggers { objects: entries }) => {
            let mut objects = live(resources);
            strip_replaced_trigger_targets(&mut objects, &entries);
            let mut sorted = entries;
            sorted.sort_by_key(|(index, _)| *index);
            for (index, object) in sorted {
                let at = index.min(objects.len());
                objects.insert(at, object);
            }
            objects
        }
        None => snapshot.objects,
    };
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

    // The 19k trigger scan is only needed when a trigger is selected
    // (almost never in the measured edit benches).
    if let Some(selected) = snapshot.selected_trigger_index {
        let trigger_count = objects
            .iter()
            .filter(|object| object.trigger.is_some())
            .count();
        resources.trigger.0.selected_index = (selected < trigger_count).then_some(selected);
    } else {
        resources.trigger.0.selected_index = None;
    }

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

/// Drain [`HistorySnapshot`] messages. History snapshots are written by the
/// command handlers that know which mutation is about to run.
pub fn handle_history_snapshot(mut events: MessageReader<HistorySnapshot>) {
    // Drain the messages so they do not accumulate frame over frame.
    for _ in events.read() {}
}
