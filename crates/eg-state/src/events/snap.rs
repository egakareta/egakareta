/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Snap / grid events.
//!
//! These handlers own snap normalization, settings persistence, and selected
//! object resnapping directly in App resources.

use bevy::{
    app::App,
    ecs::{prelude::*, system::SystemParam},
    state::prelude::State,
};
use eg_editor_domain::{snap_cell_to_step, snap_component_to_step};
use eg_types::AppPhase;

use crate::{
    events::session::persist_app_settings,
    state::{
        EditorAuthoredObject, EditorConfigRes, EditorHistoryRes, EditorHistorySnapshot,
        EditorInputRes, EditorObjectOrder, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes,
        EditorTimingRes, EditorTriggerRes, SessionStateRes,
    },
};

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

#[derive(SystemParam)]
pub struct SnapResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    config: ResMut<'w, EditorConfigRes>,
    input: ResMut<'w, EditorInputRes>,
    objects: Query<
        'w,
        's,
        (
            &'static EditorObjectOrder,
            &'static mut eg_game::ObjectData,
            &'static mut eg_game::Position,
            &'static mut eg_game::Rotation,
            &'static mut eg_game::Size,
        ),
        With<EditorAuthoredObject>,
    >,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: Res<'w, EditorTriggerRes>,
    session: ResMut<'w, SessionStateRes>,
}

pub fn handle_set_snap_to_grid(
    mut events: MessageReader<SetSnapToGrid>,
    mut resources: SnapResources,
) {
    for event in events.read() {
        if resources.config.0.snap_to_grid == event.0 {
            continue;
        }
        resources.config.0.snap_to_grid = event.0;
        resources.session.app_settings.editor_snap_to_grid = event.0;
        persist_app_settings(&resources.session.app_settings);
        resnap_selected_object(&mut resources, SelectedSnap::PositionAndSize);
    }
}

pub fn handle_set_snap_step(mut events: MessageReader<SetSnapStep>, mut resources: SnapResources) {
    for event in events.read() {
        let step = event.0.max(0.05);
        if resources.config.0.snap_step == step {
            continue;
        }
        resources.config.0.snap_step = step;
        resources.session.app_settings.editor_snap_step = step;
        persist_app_settings(&resources.session.app_settings);
        if resources.config.0.snap_to_grid {
            resnap_selected_object(&mut resources, SelectedSnap::PositionAndSize);
        }
    }
}

pub fn handle_set_snap_rotation(
    mut events: MessageReader<SetSnapRotation>,
    mut resources: SnapResources,
) {
    for event in events.read() {
        if resources.config.0.snap_rotation == event.0 {
            continue;
        }
        resources.config.0.snap_rotation = event.0;
        resources.session.app_settings.editor_rotation_snap = event.0;
        persist_app_settings(&resources.session.app_settings);
        resnap_selected_object(&mut resources, SelectedSnap::Rotation);
    }
}

pub fn handle_set_snap_rotation_step(
    mut events: MessageReader<SetSnapRotationStep>,
    mut resources: SnapResources,
) {
    for event in events.read() {
        let step = event.0.max(1.0);
        if resources.config.0.snap_rotation_step_degrees == step {
            continue;
        }
        resources.config.0.snap_rotation_step_degrees = step;
        resources.session.app_settings.editor_rotation_snap_step = step;
        persist_app_settings(&resources.session.app_settings);
        if resources.config.0.snap_rotation {
            resnap_selected_object(&mut resources, SelectedSnap::Rotation);
        }
    }
}

#[derive(Clone, Copy)]
enum SelectedSnap {
    PositionAndSize,
    Rotation,
}

fn resnap_selected_object(resources: &mut SnapResources<'_, '_>, selected_snap: SelectedSnap) {
    let mut authored_objects = resources
        .objects
        .iter_mut()
        .map(|(order, data, ..)| (order.0, data.0.clone()))
        .collect::<Vec<_>>();
    authored_objects.sort_by_key(|(order, _)| *order);
    let authored_objects = authored_objects
        .into_iter()
        .map(|(_, object)| object)
        .collect::<Vec<_>>();
    if *resources.phase.get() != AppPhase::Editor {
        return;
    }
    let Some(index) = resources
        .input
        .0
        .selected_block_index
        .filter(|index| *index < authored_objects.len())
    else {
        return;
    };

    let before = authored_objects[index].clone();
    let mut selected = before.clone();
    match selected_snap {
        SelectedSnap::PositionAndSize => {
            selected.position = snap_cell_to_step(selected.position, resources.config.0.snap_step);
            selected.size = snap_cell_to_step(selected.size, resources.config.0.snap_step)
                .map(|component| component.max(resources.config.0.snap_step));
        }
        SelectedSnap::Rotation => {
            selected.rotation = selected.rotation.map(|component| {
                snap_component_to_step(component, resources.config.0.snap_rotation_step_degrees)
            });
        }
    }

    if selected == before {
        return;
    }

    record_history_state(resources, &authored_objects);
    let Some((_, mut data, mut position, mut rotation, mut size)) = resources
        .objects
        .iter_mut()
        .find(|(order, ..)| order.0 == index)
    else {
        return;
    };
    data.0 = selected.clone();
    position.0 = selected.position;
    rotation.0 = selected.rotation;
    size.0 = selected.size;
    resources.input.0.cursor = selected.position;
    resources
        .runtime
        .0
        .dirty
        .merge(crate::state::EditorDirtyFlags::drag_object_sync());
}

fn record_history_state(resources: &mut SnapResources<'_, '_>, objects: &[eg_types::LevelObject]) {
    if resources.runtime.0.interaction.gizmo_drag.is_some()
        || resources.runtime.0.interaction.block_drag.is_some()
    {
        return;
    }

    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    resources.history.0.undo.push(EditorHistorySnapshot {
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
    });
    resources.history.0.redo.clear();
}
