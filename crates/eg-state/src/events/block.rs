/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Block operations events: remove, duplicate, copy, paste, update.

use std::collections::HashMap;

use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_editor_domain::{
    remove_topmost_block_at_cursor, snap_cell_to_step, snap_component_to_step,
    topmost_block_index_at_cursor,
};
use eg_types::{
    AppPhase, EditorInputState, EditorMode, LevelObject, TriggerTarget, next_level_object_ids_batch,
};

use crate::{events::MessageReader, state::*};

/// Remove the block under cursor or selected blocks.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveBlock;

/// Duplicate selected blocks in place.
#[derive(Debug, Clone, Copy, Default)]
pub struct DuplicateBlock;

/// Copy selected blocks to clipboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct CopyBlock;

/// Paste clipboard blocks.
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteBlock;

/// Update the properties of the currently selected block.
#[derive(Debug, Clone)]
pub struct UpdateSelectedBlock(pub LevelObject);

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

#[derive(SystemParam)]
pub struct BlockResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
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
    input: ResMut<'w, EditorInputRes>,
    config: ResMut<'w, EditorConfigRes>,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: ResMut<'w, EditorTriggerRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
}

pub fn handle_remove_block(mut events: MessageReader<RemoveBlock>, mut resources: BlockResources) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for _ in events {
        if *resources.phase.get() != AppPhase::Editor
            || effective_editor_mode(&resources) == EditorMode::Tapping
        {
            continue;
        }
        mutated |= remove_selected_or_topmost(&mut resources, &mut objects);
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &objects,
        );
    }
}

pub fn handle_duplicate_block(
    mut events: MessageReader<DuplicateBlock>,
    mut resources: BlockResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for _ in events {
        if *resources.phase.get() == AppPhase::Editor {
            mutated |= duplicate_selected(&mut resources, &mut objects);
        }
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &objects,
        );
    }
}

pub fn handle_copy_block(mut events: MessageReader<CopyBlock>, mut resources: BlockResources) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let objects = collect_editor_object_data(&resources.authored_objects);
    for _ in events {
        if *resources.phase.get() == AppPhase::Editor {
            copy_selected(&mut resources, &objects);
        }
    }
}

pub fn handle_paste_block(mut events: MessageReader<PasteBlock>, mut resources: BlockResources) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for _ in events {
        if *resources.phase.get() == AppPhase::Editor {
            mutated |= paste_from_clipboard(&mut resources, &mut objects);
        }
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &objects,
        );
    }
}

pub fn handle_update_selected_block(
    mut events: MessageReader<UpdateSelectedBlock>,
    mut resources: BlockResources,
) {
    let events = events.read().cloned().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for event in events {
        if *resources.phase.get() == AppPhase::Editor {
            mutated |= update_selected_block(&mut resources, &mut objects, &event.0);
        }
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &objects,
        );
    }
}

fn effective_editor_mode(resources: &BlockResources<'_, '_>) -> EditorMode {
    if resources.timeline.0.playback.playing && resources.input.0.mode == EditorMode::Null {
        resources
            .runtime
            .0
            .interaction
            .last_mode
            .unwrap_or(resources.input.0.mode)
    } else {
        resources.input.0.mode
    }
}

fn remove_selected_or_topmost(
    resources: &mut BlockResources<'_, '_>,
    objects: &mut Vec<LevelObject>,
) -> bool {
    let selected_indices = selected_indices_normalized(&resources.input.0, objects.len());
    if !selected_indices.is_empty() {
        let removed_object_ids = object_ids_for_removed_indices(objects, &selected_indices);
        // Delta history: capture only removed objects (<=100 clones) plus
        // pre-mutation states of triggers whose targets will be dereferenced
        // (triggers are few, so this scan is cheap and avoids a 19k clone).
        let removed: Vec<(usize, LevelObject)> = selected_indices
            .iter()
            .filter_map(|index| objects.get(*index).map(|object| (*index, object.clone())))
            .collect();
        let repaired_triggers = triggers_referencing_ids(objects, &removed_object_ids);
        record_history_delta(
            resources,
            crate::state::EditorHistoryObjectChange::ReinsertRemoved {
                removed,
                repaired_triggers,
            },
        );
        dereference_removed_transform_trigger_targets(objects, &removed_object_ids);
        // Bulk ordered removal in a single O(n) pass. `Vec::remove` per index
        // is O(n*m) (shifts ~19k `LevelObject`s 100 times for remove-100).
        if selected_indices.len() == 1 {
            objects.remove(selected_indices[0]);
        } else {
            let mut remove_cursor = 0usize;
            let mut pos = 0usize;
            objects.retain(|_| {
                let remove = remove_cursor < selected_indices.len()
                    && pos == selected_indices[remove_cursor];
                pos += 1;
                if remove {
                    remove_cursor += 1;
                    false
                } else {
                    true
                }
            });
        }
        clear_block_selection(&mut resources.input.0);
        sync_trigger_selection_from_objects(resources, objects);
        return true;
    }

    let cursor = resources.input.0.cursor;
    let Some(index) = topmost_block_index_at_cursor(objects, cursor) else {
        return false;
    };
    let removed_object_ids = object_ids_for_removed_indices(objects, &[index]);
    let removed_one: Vec<(usize, LevelObject)> = objects
        .get(index)
        .map(|object| vec![(index, object.clone())])
        .unwrap_or_default();
    let repaired_one = triggers_referencing_ids(objects, &removed_object_ids);
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::ReinsertRemoved {
            removed: removed_one,
            repaired_triggers: repaired_one,
        },
    );
    dereference_removed_transform_trigger_targets(objects, &removed_object_ids);
    if remove_topmost_block_at_cursor(objects, cursor) {
        sync_trigger_selection_from_objects(resources, objects);
        return true;
    }
    false
}

fn copy_selected(resources: &mut BlockResources<'_, '_>, objects: &[LevelObject]) {
    let selected_indices = selected_indices_normalized(&resources.input.0, objects.len());
    if !selected_indices.is_empty() {
        let anchor_index = resources
            .input
            .0
            .selected_block_index
            .filter(|index| selected_indices.contains(index))
            .unwrap_or(selected_indices[0]);
        let anchor = objects[anchor_index].position;
        let objects = selected_indices
            .into_iter()
            .map(|index| objects[index].clone())
            .collect();
        resources.runtime.0.interaction.clipboard = Some(EditorClipboard { objects, anchor });
        return;
    }

    if let Some(index) = topmost_block_index_at_cursor(objects, resources.input.0.cursor) {
        let block = objects[index].clone();
        resources.runtime.0.interaction.clipboard = Some(EditorClipboard {
            anchor: block.position,
            objects: vec![block],
        });
    }
}

fn paste_from_clipboard(
    resources: &mut BlockResources<'_, '_>,
    objects: &mut Vec<LevelObject>,
) -> bool {
    let Some(clipboard) = resources.runtime.0.interaction.clipboard.clone() else {
        return false;
    };
    if clipboard.objects.is_empty() {
        return false;
    }

    let paste_anchor = resources.input.0.cursor;
    let base_len = objects.len();
    let mut new_indices = Vec::with_capacity(clipboard.objects.len());

    let mut pasted_objects = clipboard.objects;
    regenerate_object_ids_for_clipboard(&mut pasted_objects, objects);
    // Delta history: undo removes the pasted IDs (no 19k clone).
    let pasted_ids: Vec<String> = pasted_objects
        .iter()
        .map(|object| object.id.clone())
        .collect();
    let pasted_indices: Vec<usize> = (base_len..base_len + pasted_objects.len()).collect();
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::RemoveAdded {
            object_ids: pasted_ids,
            indices: pasted_indices,
        },
    );

    for mut block in pasted_objects {
        block.position = [
            paste_anchor[0] + (block.position[0] - clipboard.anchor[0]),
            paste_anchor[1] + (block.position[1] - clipboard.anchor[1]),
            paste_anchor[2] + (block.position[2] - clipboard.anchor[2]),
        ];
        resources.config.0.selected_block_id = block.block_id.clone();
        objects.push(block);
        new_indices.push(base_len + new_indices.len());
    }

    resources.input.0.selected_block_index = new_indices.first().copied();
    resources.input.0.selected_block_indices = new_indices;
    normalize_block_selection(&mut resources.input.0, objects.len());
    resources.input.0.hovered_block_index = resources.input.0.selected_block_index;
    sync_trigger_selection_from_objects(resources, objects);
    true
}

fn duplicate_selected(
    resources: &mut BlockResources<'_, '_>,
    objects: &mut Vec<LevelObject>,
) -> bool {
    let selected_indices = selected_indices_normalized(&resources.input.0, objects.len());
    if selected_indices.is_empty() {
        return false;
    }

    let anchor_index = resources
        .input
        .0
        .selected_block_index
        .filter(|index| selected_indices.contains(index))
        .unwrap_or(selected_indices[0]);
    let anchor = objects[anchor_index].position;
    let mut duplicates = selected_indices
        .iter()
        .map(|index| objects[*index].clone())
        .collect::<Vec<_>>();
    regenerate_object_ids_for_clipboard(&mut duplicates, objects);

    resources.runtime.0.interaction.clipboard = Some(EditorClipboard {
        objects: duplicates.clone(),
        anchor,
    });
    let base_len = objects.len();
    // Delta history: undo removes the duplicated IDs (no 19k clone).
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::RemoveAdded {
            object_ids: duplicates.iter().map(|object| object.id.clone()).collect(),
            indices: (base_len..base_len + duplicates.len()).collect(),
        },
    );
    let mut new_indices = Vec::with_capacity(duplicates.len());
    for duplicated in duplicates {
        resources.config.0.selected_block_id = duplicated.block_id.clone();
        objects.push(duplicated);
        new_indices.push(base_len + new_indices.len());
    }

    resources.input.0.selected_block_index = new_indices.first().copied();
    resources.input.0.selected_block_indices = new_indices;
    normalize_block_selection(&mut resources.input.0, objects.len());
    resources.input.0.hovered_block_index = resources.input.0.selected_block_index;
    sync_trigger_selection_from_objects(resources, objects);
    true
}

fn update_selected_block(
    resources: &mut BlockResources<'_, '_>,
    objects: &mut [LevelObject],
    updated: &LevelObject,
) -> bool {
    normalize_block_selection(&mut resources.input.0, objects.len());
    let Some(index) = resources.input.0.selected_block_index else {
        return false;
    };
    if index >= objects.len() {
        return false;
    }

    // Delta history: capture the single pre-mutation object (1 clone).
    let before = objects[index].clone();
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::RestoreOne { index, before },
    );
    let snap_step = resources.config.0.snap_step.max(0.05);
    let position = if resources.config.0.snap_to_grid && !resources.input.0.ctrl_held {
        snap_cell_to_step(updated.position, snap_step)
    } else {
        [
            updated.position[0],
            updated.position[1].max(0.0),
            updated.position[2],
        ]
    };
    let snapped_size = if resources.config.0.snap_to_grid && !resources.input.0.ctrl_held {
        updated
            .size
            .map(|component| snap_component_to_step(component, snap_step))
    } else {
        updated.size
    };
    let min_size = if resources.config.0.snap_to_grid && !resources.input.0.ctrl_held {
        snap_step
    } else {
        0.25
    };
    let rotation = if resources.config.0.snap_rotation && !resources.input.0.ctrl_held {
        let step = resources.config.0.snap_rotation_step_degrees.max(1.0);
        updated
            .rotation
            .map(|component| (component / step).round() * step)
    } else {
        updated.rotation
    };

    let object = &mut objects[index];
    object.position = position;
    object.size = [
        snapped_size[0].max(min_size),
        snapped_size[1].max(min_size),
        snapped_size[2].max(min_size),
    ];
    object.block_id = updated.block_id.clone();
    object.rotation = rotation;
    object.color_tint = updated
        .color_tint
        .map(|component| component.clamp(0.0, 1.0));
    object.trigger = updated.trigger.clone();

    resources.input.0.cursor = [position[0], position[1].max(0.0), position[2]];
    sync_trigger_selection_from_objects(resources, objects);
    true
}

fn selected_indices_normalized(input: &EditorInputState, object_count: usize) -> Vec<usize> {
    let mut indices = input
        .selected_block_indices
        .iter()
        .copied()
        .filter(|index| *index < object_count)
        .collect::<Vec<_>>();
    if let Some(index) = input
        .selected_block_index
        .filter(|index| *index < object_count)
    {
        indices.push(index);
    }
    indices.sort_unstable();
    indices.dedup();
    indices
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

fn clear_block_selection(input: &mut EditorInputState) {
    input.selected_block_index = None;
    input.selected_block_indices.clear();
    input.hovered_block_index = None;
}

fn sync_trigger_selection_from_objects(
    resources: &mut BlockResources<'_, '_>,
    objects: &[LevelObject],
) {
    // Fast path: most edits have no trigger selected, so skip the full
    // 19k scan entirely.
    if resources.trigger.0.selected_index.is_none() {
        return;
    }
    let trigger_count = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .count();
    resources.trigger.0.selected_index = resources
        .trigger
        .0
        .selected_index
        .filter(|index| *index < trigger_count);
}

fn push_history_snapshot(resources: &mut BlockResources<'_, '_>, snapshot: EditorHistorySnapshot) {
    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    resources.history.0.undo.push(snapshot);
    resources.history.0.redo.clear();
}

/// Pre-mutation states of transform triggers whose targets intersect
/// `removed_ids`. Clones only affected triggers (usually zero), not the scene.
fn triggers_referencing_ids(
    objects: &[LevelObject],
    removed_ids: &[String],
) -> Vec<(usize, LevelObject)> {
    if removed_ids.is_empty() {
        return Vec::new();
    }
    let mut sorted_removed = removed_ids.to_vec();
    sorted_removed.sort_unstable();
    sorted_removed.dedup();
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
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                return false;
            };
            object_ids
                .iter()
                .any(|id| sorted_removed.binary_search(id).is_ok())
        })
        .map(|(index, object)| (index, object.clone()))
        .collect()
}

/// Push a delta history entry without cloning the full 19k scene.
/// `objects` is stored empty; undo applies `object_change` to the live scene.
fn record_history_delta(
    resources: &mut BlockResources<'_, '_>,
    object_change: crate::state::EditorHistoryObjectChange,
) {
    let snapshot = history_snapshot_with_change(resources, Vec::new(), Some(object_change));
    push_history_snapshot(resources, snapshot);
}

fn history_snapshot_with_change(
    resources: &BlockResources<'_, '_>,
    objects: Vec<LevelObject>,
    object_change: Option<crate::state::EditorHistoryObjectChange>,
) -> EditorHistorySnapshot {
    EditorHistorySnapshot {
        objects,
        object_change,
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

fn object_ids_for_removed_indices(
    objects: &[LevelObject],
    removed_indices: &[usize],
) -> Vec<String> {
    let mut removed_indices = removed_indices
        .iter()
        .copied()
        .filter(|index| *index < objects.len())
        .collect::<Vec<_>>();
    removed_indices.sort_unstable();
    removed_indices.dedup();

    objects
        .iter()
        .enumerate()
        .filter(|(index, object)| {
            removed_indices.binary_search(index).is_ok()
                && object.trigger.is_none()
                && !object.id.is_empty()
        })
        .map(|(_, object)| object.id.clone())
        .collect()
}

fn dereference_removed_transform_trigger_targets(
    objects: &mut [LevelObject],
    removed_object_ids: &[String],
) {
    if removed_object_ids.is_empty() {
        return;
    }

    let mut removed_object_ids = removed_object_ids.to_vec();
    removed_object_ids.sort_unstable();
    removed_object_ids.dedup();

    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        if !trigger.transform {
            continue;
        }
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };

        object_ids.retain(|object_id| removed_object_ids.binary_search(object_id).is_err());
        object_ids.sort_unstable();
        object_ids.dedup();
    }
}

fn regenerate_object_ids_for_clipboard(
    objects: &mut [LevelObject],
    existing_objects: &[LevelObject],
) {
    // Batch-allocate all fresh IDs in one O(n+m) scan. Per-object
    // `next_level_object_id` rescans the scene each time (O(n*m)).
    let fresh_ids = next_level_object_ids_batch(existing_objects, objects.len());
    let mut id_map = HashMap::<String, String>::with_capacity(objects.len());
    for (object, fresh_id) in objects.iter_mut().zip(fresh_ids) {
        let old_id = std::mem::replace(&mut object.id, fresh_id);
        if !old_id.is_empty() {
            id_map.insert(old_id, object.id.clone());
        }
    }

    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        for object_id in object_ids {
            if let Some(new_id) = id_map.get(object_id) {
                *object_id = new_id.clone();
            }
        }
    }
}
