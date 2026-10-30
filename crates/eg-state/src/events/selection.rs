/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Selection / transform events.

use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_blocks::BlockId;
use eg_editor_domain::{
    TimelineNearSearch, derive_timeline_time_for_world_target_near_time, snap_cell_to_step,
};
use eg_types::{
    AppPhase, EasingDirection, EditorInputState, EditorMode, LevelObject,
    TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
};
use glam::Vec3;

use crate::{events::MessageReader, state::*};

const TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS: f32 = 0.001;

/// Snap selected blocks or tap to the nearest grid cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapSelectionToGrid;

/// Focus the editor camera on the selected or preview target.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusCameraTarget;

/// Add transform-object triggers for the selected blocks.
#[derive(Debug, Clone, Copy, Default)]
pub struct AddTransformTrigger;

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_snap_selection_to_grid(
    mut events: MessageReader<SnapSelectionToGrid>,
    mut resources: SelectionResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    resources.objects.objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for _ in events {
        mutated |= snap_selection_to_grid(&mut resources);
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &resources.objects.objects,
        );
    }
}

pub fn handle_focus_camera_target(
    mut events: MessageReader<FocusCameraTarget>,
    mut resources: SelectionResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    resources.objects.objects = collect_editor_object_data(&resources.authored_objects);
    for _ in events {
        focus_camera_target(&mut resources);
    }
}

pub fn handle_add_transform_trigger(
    mut events: MessageReader<AddTransformTrigger>,
    mut resources: SelectionResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    resources.objects.objects = collect_editor_object_data(&resources.authored_objects);
    let mut mutated = false;
    for _ in events {
        mutated |= add_transform_trigger(&mut resources);
    }
    if mutated {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &resources.objects.objects,
        );
    }
}

#[derive(Default)]
struct EditorObjectScratch {
    objects: Vec<LevelObject>,
}

#[derive(SystemParam)]
pub struct SelectionResources<'w, 's> {
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
    objects: Local<'s, EditorObjectScratch>,
    input: ResMut<'w, EditorInputRes>,
    config: Res<'w, EditorConfigRes>,
    spawn: Res<'w, EditorSpawnRes>,
    camera: ResMut<'w, EditorCameraRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: ResMut<'w, EditorTriggerRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
    session: ResMut<'w, SessionStateRes>,
}

fn snap_selection_to_grid(resources: &mut SelectionResources<'_, '_>) -> bool {
    if *resources.phase.get() != AppPhase::Editor {
        return false;
    }

    let selected_indices =
        selected_indices_normalized(&resources.input.0, resources.objects.objects.len());
    if !selected_indices.is_empty() {
        return snap_selected_blocks_to_grid(resources, &selected_indices);
    }

    snap_selected_tap_to_grid(resources)
}

fn focus_camera_target(resources: &mut SelectionResources<'_, '_>) -> bool {
    if *resources.phase.get() != AppPhase::Editor {
        return false;
    }

    if editor_effective_mode_for_playback(resources) == EditorMode::Tapping
        && let Some((_, _, position)) = selected_tap(resources)
    {
        set_editor_camera_focus(resources, tap_focus_point(position), None);
        return true;
    }

    if let Some((min, size)) = selected_group_bounds(resources) {
        set_editor_camera_focus(resources, block_bounds_center(min, size), None);
        return true;
    }

    if let Some((_, _, position)) = selected_tap(resources) {
        set_editor_camera_focus(resources, tap_focus_point(position), None);
        return true;
    }

    let sample = editor_preview_camera_view(resources);
    let orientation =
        camera_orientation_from_eye_target(sample.eye.to_array(), sample.target.to_array());
    set_editor_camera_focus(resources, sample.target.to_array(), orientation);
    true
}

fn snap_selected_blocks_to_grid(
    resources: &mut SelectionResources<'_, '_>,
    selected_indices: &[usize],
) -> bool {
    let step = resources.config.0.snap_step.max(0.05);
    let needs_snap = selected_indices.iter().any(|index| {
        resources
            .objects
            .objects
            .get(*index)
            .is_some_and(|object| object.position != snap_cell_to_step(object.position, step))
    });
    if !needs_snap {
        return false;
    }

    // Delta history: capture only snapped objects (<=100 clones).
    let before: Vec<(usize, LevelObject)> = selected_indices
        .iter()
        .filter_map(|index| {
            resources
                .objects
                .objects
                .get(*index)
                .map(|object| (*index, object.clone()))
        })
        .collect();
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::RestoreMany { before },
    );
    let mut changed = false;
    for index in selected_indices {
        if let Some(object) = resources.objects.objects.get_mut(*index) {
            let snapped = snap_cell_to_step(object.position, step);
            if object.position != snapped {
                object.position = snapped;
                changed = true;
            }
        }
    }

    if let Some(index) = resources
        .input
        .0
        .selected_block_index
        .filter(|index| selected_indices.contains(index))
        .or_else(|| selected_indices.first().copied())
        && let Some(object) = resources.objects.objects.get(index)
    {
        resources.input.0.cursor = [
            object.position[0],
            object.position[1].max(0.0),
            object.position[2],
        ];
    }

    changed
}

fn editor_preview_camera_view(resources: &SelectionResources<'_, '_>) -> CameraViewSample {
    let target = Vec3::from_array(resources.timeline.0.preview.position);
    evaluate_camera_track_for_objects(
        &resources.camera.0,
        &resources.objects.objects,
        resources.timeline.0.clock.time_seconds,
        target,
        true,
    )
}

fn set_editor_camera_focus(
    resources: &mut SelectionResources<'_, '_>,
    target: [f32; 3],
    orientation: Option<(f32, f32)>,
) {
    let camera = &mut resources.camera.0;
    camera.transition = None;
    camera.editor_pan = [target[0], target[2]];
    camera.editor_target_z = target[1];
    if let Some((rotation, pitch)) = orientation {
        camera.editor_yaw = rotation;
        camera.editor_pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
    }
}

fn editor_effective_mode_for_playback(resources: &SelectionResources<'_, '_>) -> EditorMode {
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

fn selected_tap(resources: &SelectionResources<'_, '_>) -> Option<(usize, f32, [f32; 3])> {
    let selected_index = resources.timeline.0.taps.selected_index?;
    let time_seconds = *resources.timeline.0.taps.tap_times.get(selected_index)?;
    let position = *resources
        .timeline
        .0
        .taps
        .tap_indicator_positions
        .get(selected_index)?;
    Some((selected_index, time_seconds, position))
}

fn selected_group_bounds(resources: &SelectionResources<'_, '_>) -> Option<([f32; 3], [f32; 3])> {
    let indices = selected_indices_normalized(&resources.input.0, resources.objects.objects.len());
    if indices.is_empty() {
        return selected_transform_trigger_target(resources)
            .map(|object| (object.position, object.size));
    }

    let first = *indices.first()?;
    let first_obj = resources.objects.objects.get(first)?;
    let mut min = first_obj.position;
    let mut max = [
        first_obj.position[0] + first_obj.size[0],
        first_obj.position[1] + first_obj.size[1],
        first_obj.position[2] + first_obj.size[2],
    ];

    for index in indices.into_iter().skip(1) {
        if let Some(object) = resources.objects.objects.get(index) {
            min[0] = min[0].min(object.position[0]);
            min[1] = min[1].min(object.position[1]);
            min[2] = min[2].min(object.position[2]);
            max[0] = max[0].max(object.position[0] + object.size[0]);
            max[1] = max[1].max(object.position[1] + object.size[1]);
            max[2] = max[2].max(object.position[2] + object.size[2]);
        }
    }

    Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
}

fn selected_transform_trigger_target(
    resources: &SelectionResources<'_, '_>,
) -> Option<LevelObject> {
    let selected_index = resources.trigger.0.selected_index?;
    let object = trigger_objects_sorted(&resources.objects.objects)
        .get(selected_index)?
        .clone();
    object.is_transform_trigger().then_some(object)
}

fn trigger_objects_sorted(objects: &[LevelObject]) -> Vec<LevelObject> {
    let mut triggers = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .cloned()
        .collect::<Vec<_>>();
    triggers.sort_by(|left, right| {
        f32::total_cmp(
            &left
                .trigger
                .as_ref()
                .map(|trigger| trigger.time_seconds)
                .unwrap_or(0.0),
            &right
                .trigger
                .as_ref()
                .map(|trigger| trigger.time_seconds)
                .unwrap_or(0.0),
        )
    });
    triggers
}

fn block_bounds_center(position: [f32; 3], size: [f32; 3]) -> [f32; 3] {
    [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ]
}

fn tap_focus_point(position: [f32; 3]) -> [f32; 3] {
    [position[0] + 0.5, position[1], position[2] + 0.5]
}

fn camera_orientation_from_eye_target(eye: [f32; 3], target: [f32; 3]) -> Option<(f32, f32)> {
    let offset = Vec3::from_array(eye) - Vec3::from_array(target);
    let horizontal = Vec3::new(offset.x, 0.0, offset.z).length();
    if horizontal <= f32::EPSILON && offset.y.abs() <= f32::EPSILON {
        return None;
    }

    let rotation = (-offset.x).atan2(-offset.z);
    let pitch = offset.y.atan2(horizontal);
    Some((rotation, pitch))
}

fn snap_selected_tap_to_grid(resources: &mut SelectionResources<'_, '_>) -> bool {
    let taps = &resources.timeline.0.taps;
    let Some(selected_index) = taps.selected_index else {
        return false;
    };
    if selected_index >= taps.tap_times.len()
        || selected_index >= taps.tap_indicator_positions.len()
    {
        return false;
    }

    let old_time_seconds = taps.tap_times[selected_index];
    let position = taps.tap_indicator_positions[selected_index];
    let step = resources.config.0.snap_step.max(0.05);
    let snapped_position = snap_cell_to_step(position, step);
    if (position[0] - snapped_position[0]).abs() <= f32::EPSILON
        && (position[1] - snapped_position[1]).abs() <= f32::EPSILON
        && (position[2] - snapped_position[2]).abs() <= f32::EPSILON
    {
        return false;
    }

    let mut solve_tap_times = taps.tap_times.clone();
    solve_tap_times.remove(selected_index);
    let duration_seconds = resources.timeline.0.clock.duration_seconds.max(0.0);
    let target_world = [
        snapped_position[0] + 0.5,
        snapped_position[1],
        snapped_position[2] + 0.5,
    ];
    let next_time = derive_timeline_time_for_world_target_near_time(
        resources.spawn.0.position,
        resources.spawn.0.direction,
        &solve_tap_times,
        duration_seconds,
        &resources.objects.objects,
        target_world,
        TimelineNearSearch {
            seed_time: old_time_seconds,
            window_seconds: 1.5,
        },
    )
    .clamp(0.0, duration_seconds);

    let taps = &mut resources.timeline.0.taps;
    taps.tap_times.remove(selected_index);
    taps.tap_indicator_positions.remove(selected_index);
    let next_index = taps
        .tap_times
        .partition_point(|time_seconds| *time_seconds < next_time);
    taps.tap_times.insert(next_index, next_time);
    taps.tap_indicator_positions
        .insert(next_index, snapped_position);
    taps.selected_index = Some(next_index);
    resources.timeline.0.tap_cursor_override = Some(snapped_position);
    resources.timeline.0.clock.time_seconds = next_time;
    resources.input.0.cursor = snapped_position;
    true
}

fn add_transform_trigger(resources: &mut SelectionResources<'_, '_>) -> bool {
    if *resources.phase.get() != AppPhase::Editor || resources.timeline.0.playback.playing {
        return false;
    }

    let selected_objects =
        selected_indices_normalized(&resources.input.0, resources.objects.objects.len())
            .into_iter()
            .filter_map(|index| {
                let object = resources.objects.objects.get(index)?;
                if object.trigger.is_some() {
                    return None;
                }
                let object_id = (!object.id.is_empty()).then(|| object.id.clone())?;
                Some((object_id, object.clone()))
            })
            .collect::<Vec<_>>();

    if selected_objects.is_empty() {
        return false;
    }

    let time_seconds = resources.timeline.0.clock.time_seconds.max(0.0);
    let replaced_object_ids = selected_objects
        .iter()
        .map(|(object_id, _)| object_id.clone())
        .collect::<Vec<_>>();
    // Capture pre-mutation trigger states that `remove_replaced...` will
    // touch (triggers are few; avoids a 19k full-scene history clone).
    let modified_before: Vec<(usize, LevelObject)> = resources
        .objects
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| {
            let Some(trigger) = object.trigger.as_ref() else {
                return false;
            };
            if (trigger.time_seconds - time_seconds).abs()
                > TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
                || !trigger.transform
            {
                return false;
            }
            let TriggerTarget::Objects { object_ids } = &trigger.target else {
                return false;
            };
            object_ids
                .iter()
                .any(|object_id| replaced_object_ids.contains(object_id))
        })
        .map(|(index, object)| (index, object.clone()))
        .collect();
    // Batch-allocate fresh trigger IDs in one O(n) scan (not O(n*m)).
    let fresh_ids =
        eg_types::next_level_object_ids_batch(&resources.objects.objects, selected_objects.len());
    record_history_delta(
        resources,
        crate::state::EditorHistoryObjectChange::AddTriggers {
            added_ids: fresh_ids.clone(),
            modified_before,
        },
    );
    resources.runtime.0.interaction.gizmo_drag = None;
    resources.runtime.0.interaction.block_drag = None;
    resources.session.editor_show_place_window = false;

    remove_replaced_transform_trigger_targets(
        &mut resources.objects.objects,
        time_seconds,
        &replaced_object_ids,
    );
    sync_trigger_selection_from_objects(resources);

    let mut created_trigger_indices = Vec::with_capacity(selected_objects.len());
    for ((object_id, object), fresh_id) in selected_objects.into_iter().zip(fresh_ids) {
        let object_index = resources.objects.objects.len();
        let trigger_object = LevelObject {
            id: fresh_id,
            position: object.position,
            rotation: object.rotation,
            size: object.size,
            block_id: BlockId::new(TRANSFORM_TRIGGER_BLOCK_ID),
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 1.0,
                easing: EasingDirection::EaseInOut,
                target: TriggerTarget::Objects {
                    object_ids: vec![object_id],
                },
                transform: true,
                ..TimedTrigger::default()
            }),
            ..LevelObject::default()
        };
        resources.objects.objects.push(trigger_object);
        sync_trigger_selection_from_objects(resources);
        created_trigger_indices.push(object_index);
    }

    resources.input.0.selected_block_index = created_trigger_indices.first().copied();
    resources.input.0.selected_block_indices = created_trigger_indices;
    resources.input.0.hovered_block_index = resources.input.0.selected_block_index;
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

fn sync_trigger_selection_from_objects(resources: &mut SelectionResources<'_, '_>) {
    let trigger_count = resources
        .objects
        .objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .count();
    resources.trigger.0.selected_index = resources
        .trigger
        .0
        .selected_index
        .filter(|index| *index < trigger_count);
}

fn remove_replaced_transform_trigger_targets(
    objects: &mut [LevelObject],
    time_seconds: f32,
    object_ids_to_replace: &[String],
) {
    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        if (trigger.time_seconds - time_seconds).abs() > TRANSFORM_TRIGGER_REPLACE_EPSILON_SECONDS
            || !trigger.transform
        {
            continue;
        }
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        if !object_ids
            .iter()
            .any(|object_id| object_ids_to_replace.contains(object_id))
        {
            continue;
        }
        object_ids.retain(|object_id| !object_ids_to_replace.contains(object_id));
    }
}

fn push_history_snapshot(
    resources: &mut SelectionResources<'_, '_>,
    snapshot: EditorHistorySnapshot,
) {
    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    resources.history.0.undo.push(snapshot);
    resources.history.0.redo.clear();
}

fn record_history_delta(
    resources: &mut SelectionResources<'_, '_>,
    object_change: crate::state::EditorHistoryObjectChange,
) {
    let snapshot = history_snapshot_with_change(resources, Vec::new(), Some(object_change));
    push_history_snapshot(resources, snapshot);
}

fn history_snapshot_with_change(
    resources: &SelectionResources<'_, '_>,
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
