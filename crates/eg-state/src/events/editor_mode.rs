/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Editor mode / block-id / pick events.

use bevy_app::App;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_blocks::BlockId;
use eg_types::{AppPhase, EditorInputState, EditorMode};
use glam::Vec2;

use crate::state::{
    EditorAuthoredObject, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
    EditorInteractionContext, EditorObjectOrder, EditorObjectsBorrow, EditorPickSpatialIndex,
    EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
    RenderUniformsRes, collect_editor_object_data,
};

/// Switch the editor mode.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetEditorMode(pub EditorMode);

/// Set the current block ID for placement.
#[derive(Message, Debug, Clone)]
pub struct SetBlockId(pub BlockId);

/// Select the Nth recent placeable block and switch to Place mode.
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectRecentBlock(pub usize);

/// Pick the selected block and enter place mode with that block type.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PickSelectedBlock;

/// Pick the block under the pointer and enter place mode with that block type.
#[derive(Message, Debug, Clone, Copy)]
pub struct PickBlockAt {
    pub x: f64,
    pub y: f64,
}

pub fn register_events(app: &mut App) {
    app.add_message::<SetEditorMode>()
        .add_message::<SetBlockId>()
        .add_message::<SelectRecentBlock>()
        .add_message::<PickSelectedBlock>()
        .add_message::<PickBlockAt>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

// `SetEditorMode` writes the authoritative editor mode directly. App-world
// interaction, UI, and render extraction consume `EditorInputRes`.

pub fn handle_set_editor_mode(
    mut events: MessageReader<SetEditorMode>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.mode = event.0;
    }
}

// Block-id selection is fully App-world owned. Real changes also update the
// most-recently-used block list consumed by `SelectRecentBlock`.
pub fn handle_set_block_id(
    mut events: MessageReader<SetBlockId>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        set_selected_block_id(&mut config, event.0.clone());
    }
}

// `SelectRecentBlock` reads the `recent_block_ids` list from
// `EditorConfigRes`, picks the indexed (placeable) entry, and writes the
// result directly to `EditorConfigRes.0.selected_block_id` +
// `EditorInputRes.0.mode`. Only blocks with `placeable == true` are indexed.
// Selecting a different entry promotes it to the front of the App-owned
// recent-block list.
pub fn handle_select_recent_block(
    mut events: MessageReader<SelectRecentBlock>,
    mut config: ResMut<EditorConfigRes>,
    mut input: ResMut<crate::state::EditorInputRes>,
) {
    for event in events.read() {
        let placeable_ids: Vec<eg_blocks::BlockId> = config
            .0
            .recent_block_ids
            .iter()
            .filter(|id| eg_blocks::BlockDefinition::get(id).placeable)
            .cloned()
            .collect();
        if let Some(id) = placeable_ids.get(event.0) {
            set_selected_block_id(&mut config, id.clone());
            input.0.mode = eg_types::EditorMode::Place;
        }
    }
}

// `PickSelectedBlock` reads App-owned selection, rejects non-placeable block
// ids, and updates the selected id, recent list, and effective Place mode.
pub fn handle_pick_selected_block(
    mut events: MessageReader<PickSelectedBlock>,
    phase: Res<State<AppPhase>>,
    authored_objects: Query<(&EditorObjectOrder, &eg_game::ObjectData), With<EditorAuthoredObject>>,
    mut config: ResMut<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut runtime: ResMut<EditorRuntimeRes>,
    timeline: Res<EditorTimelineRes>,
) {
    for _ in events.read() {
        pick_selected_block_for_place(
            &phase,
            &collect_editor_object_data(&authored_objects),
            &mut config,
            &mut input,
            &mut runtime,
            &timeline,
        );
    }
}

pub fn handle_pick_block_at(
    mut events: MessageReader<PickBlockAt>,
    mut resources: PickBlockAtResources,
) {
    for event in events.read() {
        if input_pointer_over_ui_input(&resources.input.0, event.x, event.y) {
            continue;
        }
        pick_block_at_for_place(event.x, event.y, &mut resources);
    }
}

#[derive(SystemParam)]
pub struct PickBlockAtResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    authored_objects: Query<
        'w,
        's,
        (&'static EditorObjectOrder, &'static eg_game::ObjectData),
        With<EditorAuthoredObject>,
    >,
    camera: ResMut<'w, EditorCameraRes>,
    triggers: ResMut<'w, EditorTriggerRes>,
    timing: ResMut<'w, EditorTimingRes>,
    uniforms: Res<'w, RenderUniformsRes>,
    config: ResMut<'w, EditorConfigRes>,
    input: ResMut<'w, EditorInputRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    spawn: ResMut<'w, EditorSpawnRes>,
    pick_spatial_index: Local<'s, EditorPickSpatialIndex>,
}

fn pick_block_at_for_place(x: f64, y: f64, resources: &mut PickBlockAtResources<'_, '_>) -> bool {
    if *resources.phase.get() != AppPhase::Editor {
        return false;
    }

    let viewport_size = Vec2::from_array(resources.uniforms.color_space.viewport_size);
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    let Some(block_id) = ({
        let mut editor = EditorInteractionContext {
            input: &mut resources.input.0,
            config: &mut resources.config.0,
            objects: EditorObjectsBorrow(&mut objects),
            spawn: &mut resources.spawn.0,
            camera: &mut resources.camera.0,
            triggers: &mut resources.triggers.0,
            timeline: &mut resources.timeline.0,
            runtime: &mut resources.runtime.0,
            timing: &mut resources.timing.0,
            history: &mut resources.history.0,
            selected_mask_cache: None,
            block_static_vertex_cache: Default::default(),
            block_static_vertex_cache_complete_len: None,
            pick_spatial_index: &mut resources.pick_spatial_index,
        };
        editor
            .pick_from_screen(x, y, viewport_size)
            .and_then(|pick| pick.hit_block_index)
            .and_then(|index| editor.objects.get(index))
            .map(|object| object.block_id.clone())
    }) else {
        return false;
    };

    if !eg_blocks::BlockDefinition::get_by_id(&block_id).placeable {
        return false;
    }

    set_effective_place_mode(
        &mut resources.input,
        &mut resources.runtime,
        &resources.timeline,
    );
    set_selected_block_id(&mut resources.config, block_id);
    true
}

fn input_pointer_over_ui_input(input: &EditorInputState, x: f64, y: f64) -> bool {
    let pixels_per_point = input.ui_input_pixels_per_point as f64;
    let point_x = x / pixels_per_point;
    let point_y = y / pixels_per_point;

    input.ui_input_blocking_rects.iter().any(|rect| {
        point_x >= rect[0] as f64
            && point_x <= rect[2] as f64
            && point_y >= rect[1] as f64
            && point_y <= rect[3] as f64
    })
}

fn pick_selected_block_for_place(
    phase: &State<AppPhase>,
    objects: &[eg_types::LevelObject],
    config: &mut EditorConfigRes,
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &EditorTimelineRes,
) -> bool {
    if *phase.get() != AppPhase::Editor {
        return false;
    }

    let selected_indices = selected_indices_normalized(&input.0, objects.len());
    let block_index = input
        .0
        .selected_block_index
        .filter(|index| selected_indices.contains(index))
        .or_else(|| selected_indices.first().copied());

    let Some(block_id) = block_index
        .and_then(|index| objects.get(index))
        .map(|object| object.block_id.clone())
    else {
        return false;
    };

    if !eg_blocks::BlockDefinition::get_by_id(&block_id).placeable {
        return false;
    }

    set_effective_place_mode(input, runtime, timeline);
    set_selected_block_id(config, block_id);
    true
}

fn set_selected_block_id(config: &mut EditorConfigRes, block_id: BlockId) -> bool {
    if config.0.selected_block_id == block_id {
        return false;
    }
    config.0.selected_block_id = block_id.clone();
    config.0.remember_recent_block_id(block_id);
    true
}

fn set_effective_place_mode(
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &EditorTimelineRes,
) {
    if timeline.0.playback.playing {
        runtime.0.interaction.last_mode = Some(EditorMode::Place);
        input.0.mode = EditorMode::Null;
    } else {
        input.0.mode = EditorMode::Place;
    }
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
