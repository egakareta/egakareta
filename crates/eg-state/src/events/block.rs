/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Block operations events: remove, duplicate, copy, paste, update.

use std::collections::HashMap;

use bevy_app::App;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_editor_domain::{
    remove_topmost_block_at_cursor, snap_cell_to_step, snap_component_to_step,
    topmost_block_index_at_cursor,
};
use eg_types::{
    AppPhase, EditorInputState, EditorMode, LevelObject, TriggerTarget, next_level_object_id,
};

use crate::state::{
    EditorAuthoredObject, EditorClipboard, EditorConfigRes, EditorHistoryRes,
    EditorHistorySnapshot, EditorInputRes, EditorObjectOrder, EditorRuntimeRes, EditorSpawnRes,
    EditorTimelineRes, EditorTimingRes, EditorTriggerRes, collect_editor_object_data,
    replace_editor_objects_commands,
};

/// Remove the block under cursor or selected blocks.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RemoveBlock;

/// Duplicate selected blocks in place.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct DuplicateBlock;

/// Copy selected blocks to clipboard.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct CopyBlock;

/// Paste clipboard blocks.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PasteBlock;

/// Update the properties of the currently selected block.
#[derive(Message, Debug, Clone)]
pub struct UpdateSelectedBlock(pub LevelObject);

pub fn register_events(app: &mut App) {
    app.add_message::<RemoveBlock>()
        .add_message::<DuplicateBlock>()
        .add_message::<CopyBlock>()
        .add_message::<PasteBlock>()
        .add_message::<UpdateSelectedBlock>();
}

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
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let before = objects.clone();
    for _ in events.read() {
        if *resources.phase.get() != AppPhase::Editor
            || effective_editor_mode(&resources) == EditorMode::Tapping
        {
            continue;
        }
        remove_selected_or_topmost(&mut resources, &mut objects);
    }
    if objects != before {
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
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let before = objects.clone();
    for _ in events.read() {
        if *resources.phase.get() == AppPhase::Editor {
            duplicate_selected(&mut resources, &mut objects);
        }
    }
    if objects != before {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &objects,
        );
    }
}

pub fn handle_copy_block(mut events: MessageReader<CopyBlock>, mut resources: BlockResources) {
    let objects = collect_editor_object_data(&resources.authored_objects);
    for _ in events.read() {
        if *resources.phase.get() == AppPhase::Editor {
            copy_selected(&mut resources, &objects);
        }
    }
}

pub fn handle_paste_block(mut events: MessageReader<PasteBlock>, mut resources: BlockResources) {
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let before = objects.clone();
    for _ in events.read() {
        if *resources.phase.get() == AppPhase::Editor {
            paste_from_clipboard(&mut resources, &mut objects);
        }
    }
    if objects != before {
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
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let before = objects.clone();
    for event in events.read() {
        if *resources.phase.get() == AppPhase::Editor {
            update_selected_block(&mut resources, &mut objects, &event.0);
        }
    }
    if objects != before {
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
        record_history_state(resources, objects);
        let removed_object_ids = object_ids_for_removed_indices(objects, &selected_indices);
        dereference_removed_transform_trigger_targets(objects, &removed_object_ids);
        for index in selected_indices.iter().rev() {
            objects.remove(*index);
        }
        clear_block_selection(&mut resources.input.0);
        sync_trigger_selection_from_objects(resources, objects);
        return true;
    }

    let cursor = resources.input.0.cursor;
    let Some(index) = topmost_block_index_at_cursor(objects, cursor) else {
        return false;
    };
    record_history_state(resources, objects);
    let removed_object_ids = object_ids_for_removed_indices(objects, &[index]);
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

    record_history_state(resources, objects);
    let paste_anchor = resources.input.0.cursor;
    let base_len = objects.len();
    let mut new_indices = Vec::with_capacity(clipboard.objects.len());

    let mut pasted_objects = clipboard.objects;
    regenerate_object_ids_for_clipboard(&mut pasted_objects, objects);

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
    record_history_state(resources, objects);

    let base_len = objects.len();
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

    record_history_state(resources, objects);
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

fn record_history_state(resources: &mut BlockResources<'_, '_>, objects: &[LevelObject]) {
    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    let snapshot = history_snapshot_from_resources(resources, objects);
    resources.history.0.undo.push(snapshot);
    resources.history.0.redo.clear();
}

fn history_snapshot_from_resources(
    resources: &BlockResources<'_, '_>,
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
    let mut id_map = HashMap::<String, String>::new();
    let mut allocated_objects = existing_objects.to_vec();

    for object in objects.iter_mut() {
        let old_id = object.id.clone();
        object.id = next_level_object_id(&allocated_objects);
        if !old_id.is_empty() {
            id_map.insert(old_id, object.id.clone());
        }
        allocated_objects.push(object.clone());
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
