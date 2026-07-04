/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Editor mode / block-id / pick events.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use eg_blocks::BlockId;
use eg_types::{AppPhase, EditorInputState, EditorMode};
use glam::Vec2;

use crate::state::{
    AppPhaseRes, EditorCameraRes, EditorConfigRes, EditorInputRes, EditorObjectsRes,
    EditorRuntimeRes, EditorSubsystem, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
    RenderUniformsRes,
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

// `SetEditorMode` mutates App-world state directly (same pattern as
// `events::snap`, `events::bpm`, and `events::misc`): the handler mutates
// `EditorInputRes.0.mode` directly, and the legacy
// `State.editor.input.mode` is synced from the resource by
// `Runtime::sync_app_editor_input_state` before any dispatch in the
// same frame.
//
// The legacy `set_editor_mode` has heavy side effects (rebuilds
// gizmo/hover/selection outline vertices, marks
// `EditorDirtyFlags::tap_timeline_changed()`). The change guard in
// the runtime ensures those side effects fire only on real
// user-driven changes. The legacy dispatch path
// (`EditorCommand::SetMode`) goes through `dispatch_editor` which
// also clears `editor.timeline.taps.selected_index` on
// Tapping→other transitions and calls
// `refresh_editor_tapping_preview_on_mode_entry` on
// other→Tapping transitions. The sync helper invokes
// `set_editor_mode` (the same setter used by
// `set_editor_playback_effective_mode` after a mode change), so
// the side effects fire through the same path.
pub fn handle_set_editor_mode(
    mut events: MessageReader<SetEditorMode>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.mode = event.0;
    }
}

// `SetBlockId` mutates `EditorConfigRes.0.selected_block_id` directly,
// and the legacy `State.editor.config.selected_block_id` is synced from
// the resource by `Runtime::sync_app_editor_config_state` before any
// dispatch in the same frame.
pub fn handle_set_block_id(
    mut events: MessageReader<SetBlockId>,
    mut config: ResMut<EditorConfigRes>,
) {
    for event in events.read() {
        config.0.selected_block_id = event.0.clone();
    }
}

// `SelectRecentBlock` is a carve-out: the handler reads the
// `recent_block_ids` list from `EditorConfigRes`, picks the indexed
// (placeable) entry, and writes the result directly to
// `EditorConfigRes.0.selected_block_id` + `EditorInputRes.0.mode`.
// The legacy `select_recent_block` is a no-op outside the editor
// phase and also gates on placeable; the App-world handler mirrors
// the placeable filter via `eg_blocks::BlockDefinition::get` so
// the carve-out matches the legacy behavior. The change-guarded
// syncs in `Runtime` (config + input) propagate the values to the
// legacy `State.editor.config.selected_block_id` and
// `State.editor.input.mode` fields; their setters handle the
// heavy side effects (recent-blocks list, vertex rebuilds,
// dirty flags) once per real user-driven change.
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
            config.0.selected_block_id = id.clone();
            input.0.mode = eg_types::EditorMode::Place;
        }
    }
}

// `PickSelectedBlock` is a carve-out: the handler reads the current
// block selection from `EditorInputRes` / `EditorObjectsRes`, rejects
// non-placeable block ids, then writes the picked id and effective
// Place mode directly to App-world resources. Runtime's config/input/
// timeline syncs propagate those changes to legacy State once per
// frame while `PickBlockAt` remains on the legacy screen-picking path.
pub fn handle_pick_selected_block(
    mut events: MessageReader<PickSelectedBlock>,
    phase: Res<AppPhaseRes>,
    objects: Res<EditorObjectsRes>,
    mut config: ResMut<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut runtime: ResMut<EditorRuntimeRes>,
    timeline: Res<EditorTimelineRes>,
) {
    for _ in events.read() {
        pick_selected_block_for_place(
            &phase,
            &objects,
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
pub struct PickBlockAtResources<'w> {
    phase: Res<'w, AppPhaseRes>,
    objects: Res<'w, EditorObjectsRes>,
    camera: Res<'w, EditorCameraRes>,
    triggers: Res<'w, EditorTriggerRes>,
    timing: Res<'w, EditorTimingRes>,
    uniforms: Res<'w, RenderUniformsRes>,
    config: ResMut<'w, EditorConfigRes>,
    input: ResMut<'w, EditorInputRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    timeline: Res<'w, EditorTimelineRes>,
}

fn pick_block_at_for_place(x: f64, y: f64, resources: &mut PickBlockAtResources<'_>) -> bool {
    if resources.phase.current != AppPhase::Editor {
        return false;
    }

    let viewport_size = Vec2::from_array(resources.uniforms.color_space.viewport_size);
    let mut editor = EditorSubsystem {
        input: resources.input.0.clone(),
        config: resources.config.0.clone(),
        objects: resources.objects.objects.clone(),
        camera: resources.camera.0.clone(),
        triggers: resources.triggers.0.clone(),
        timing: resources.timing.0.clone(),
        ..Default::default()
    };
    let Some(block_id) = editor
        .pick_from_screen(x, y, viewport_size)
        .and_then(|pick| pick.hit_block_index)
        .and_then(|index| resources.objects.objects.get(index))
        .map(|object| object.block_id.clone())
    else {
        return false;
    };

    if !eg_blocks::BlockDefinition::get(&block_id).placeable {
        return false;
    }

    set_effective_place_mode(
        &mut resources.input,
        &mut resources.runtime,
        &resources.timeline,
    );
    resources.config.0.selected_block_id = block_id;
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
    phase: &AppPhaseRes,
    objects: &EditorObjectsRes,
    config: &mut EditorConfigRes,
    input: &mut EditorInputRes,
    runtime: &mut EditorRuntimeRes,
    timeline: &EditorTimelineRes,
) -> bool {
    if phase.current != AppPhase::Editor {
        return false;
    }

    let selected_indices = selected_indices_normalized(&input.0, objects.objects.len());
    let block_index = input
        .0
        .selected_block_index
        .filter(|index| selected_indices.contains(index))
        .or_else(|| selected_indices.first().copied());

    let Some(block_id) = block_index
        .and_then(|index| objects.objects.get(index))
        .map(|object| object.block_id.clone())
    else {
        return false;
    };

    if !eg_blocks::BlockDefinition::get(&block_id).placeable {
        return false;
    }

    set_effective_place_mode(input, runtime, timeline);
    config.0.selected_block_id = block_id;
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
