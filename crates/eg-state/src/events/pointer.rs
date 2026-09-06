/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Pointer / drag / cursor input messages.
//!
//! These represent editor input state, not commands. The handlers write
//! directly to mirrored App-world resources so pointer routing no longer
//! needs to stage legacy editor commands during the compatibility window.

use bevy_app::App;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_types::{AppPhase, EditorMode, EditorTapDivisionPick, LevelObject};
use glam::Vec2;

use crate::{
    events::navigation::TurnRight,
    state::{
        EditorAuthoredObject, EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes,
        EditorInteractionContext, EditorObjectOrder, EditorObjectRevision, EditorObjectsBorrow,
        EditorPointerCache, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes,
        EditorTriggerRes, RenderConfigRes, collect_editor_object_data,
        replace_editor_objects_commands, spawn_editor_object_commands,
    },
};

/// Mouse button state update for editor interaction routing.
#[derive(Message, Debug, Clone, Copy)]
pub struct MouseButton {
    pub button: u32,
    pub pressed: bool,
}

/// Primary click action at screen coordinates.
#[derive(Message, Debug, Clone, Copy)]
pub struct PrimaryClick {
    pub x: f64,
    pub y: f64,
}

/// Pointer moved to screen coordinates.
#[derive(Message, Debug, Clone, Copy)]
pub struct PointerMoved {
    pub x: f64,
    pub y: f64,
}

/// Update the editor cursor from screen coordinates without triggering a click.
#[derive(Message, Debug, Clone, Copy)]
pub struct UpdateCursorFromScreen {
    pub x: f64,
    pub y: f64,
}

/// Camera drag delta in screen pixels.
#[derive(Message, Debug, Clone, Copy)]
pub struct CameraDrag {
    pub dx: f64,
    pub dy: f64,
}

pub fn register_events(app: &mut App) {
    app.add_message::<MouseButton>()
        .add_message::<PrimaryClick>()
        .add_message::<PointerMoved>()
        .add_message::<UpdateCursorFromScreen>()
        .add_message::<CameraDrag>();
}

#[derive(SystemParam)]
pub struct PointerResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: ResMut<'w, EditorInputRes>,
    config: ResMut<'w, EditorConfigRes>,
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
    pointer_cache: ResMut<'w, EditorPointerCache>,
    camera: ResMut<'w, EditorCameraRes>,
    spawn: ResMut<'w, EditorSpawnRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    timing: ResMut<'w, EditorTimingRes>,
    triggers: ResMut<'w, EditorTriggerRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    history: ResMut<'w, EditorHistoryRes>,
    render: Res<'w, RenderConfigRes>,
    revision: Res<'w, EditorObjectRevision>,
}

fn ensure_objects(resources: &mut PointerResources<'_, '_>) {
    if resources.pointer_cache.initialized
        && resources.pointer_cache.revision == resources.revision.0
    {
        return;
    }
    resources.pointer_cache.objects = collect_editor_object_data(&resources.authored_objects);
    resources.pointer_cache.pick_spatial_index.invalidate();
    resources.pointer_cache.revision = resources.revision.0;
    resources.pointer_cache.initialized = true;
}

fn load_objects(resources: &mut PointerResources<'_, '_>) -> Vec<LevelObject> {
    ensure_objects(resources);
    resources.pointer_cache.objects.clone()
}

fn commit_objects(resources: &mut PointerResources<'_, '_>, before: Vec<LevelObject>) {
    if resources.pointer_cache.objects != before {
        replace_editor_objects_commands(
            &mut resources.commands,
            &resources.existing_objects,
            &resources.pointer_cache.objects,
        );
    }
}

fn viewport(resources: &PointerResources<'_, '_>) -> Vec2 {
    Vec2::new(
        resources.render.width.max(1) as f32,
        resources.render.height.max(1) as f32,
    )
}

fn interaction_context<'a, 'w, 's>(
    resources: &'a mut PointerResources<'w, 's>,
) -> EditorInteractionContext<'a> {
    let EditorPointerCache {
        objects,
        pick_spatial_index,
        ..
    } = &mut *resources.pointer_cache;
    EditorInteractionContext {
        input: &mut resources.input.0,
        config: &mut resources.config.0,
        objects: EditorObjectsBorrow(objects),
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
        pick_spatial_index,
    }
}

fn handle_tapping_click(
    editor: &mut EditorInteractionContext<'_>,
    x: f64,
    y: f64,
    viewport: Vec2,
) -> bool {
    const ADD_EPSILON_SECONDS: f32 = 0.001;

    let Some(pick) = editor.pick_from_screen(x, y, viewport) else {
        return false;
    };

    if let Some(tap_index) = pick.hit_tap_index {
        let Some(time_seconds) = editor.timeline.taps.tap_times.get(tap_index).copied() else {
            return false;
        };
        editor.set_selected_tap_index(Some(tap_index));
        editor.runtime.interaction.hovered_tap_index = Some(tap_index);
        editor.set_timeline_time_seconds(time_seconds);
        editor.input.cursor = pick.cursor;
        return true;
    }

    if let Some(pending_click) = editor.runtime.interaction.pending_tap_click {
        let same_screen = (pending_click.screen[0] - x).abs()
            <= crate::state::TAP_CLICK_SCREEN_EPSILON_PIXELS
            && (pending_click.screen[1] - y).abs() <= crate::state::TAP_CLICK_SCREEN_EPSILON_PIXELS;
        let same_time = (editor.timeline.clock.time_seconds - pending_click.pick.time_seconds)
            .abs()
            <= ADD_EPSILON_SECONDS;
        if same_screen && same_time {
            editor.runtime.interaction.pending_tap_click = None;
            editor.record_history_state();
            editor.set_timeline_time_seconds(pending_click.pick.time_seconds);
            editor.add_tap(pending_click.pick.indicator_position);
            editor.input.cursor = pending_click.pick.indicator_position;
            return true;
        }
    }

    let target_pick = editor
        .runtime
        .interaction
        .hovered_tap_division
        .or(pick.hit_tap_division)
        .unwrap_or_else(|| EditorTapDivisionPick {
            time_seconds: editor.tap_time_for_indicator_cell(pick.cursor),
            indicator_position: pick.cursor,
        });
    let current_time = editor.timeline.clock.time_seconds;
    if (current_time - target_pick.time_seconds).abs() > ADD_EPSILON_SECONDS {
        editor.set_selected_tap_index(None);
        editor.runtime.interaction.pending_tap_click = Some(crate::state::EditorPendingTapClick {
            screen: [x, y],
            pick: target_pick,
        });
        editor.set_timeline_time_seconds(target_pick.time_seconds);
        editor.input.cursor = target_pick.indicator_position;
        return true;
    }

    editor.runtime.interaction.pending_tap_click = None;
    editor.record_history_state();
    editor.add_tap(target_pick.indicator_position);
    editor.input.cursor = target_pick.indicator_position;
    true
}

pub fn handle_mouse_button(
    mut events: MessageReader<MouseButton>,
    mut resources: PointerResources,
    mut turn_right: MessageWriter<TurnRight>,
    mut primary_click: MessageWriter<PrimaryClick>,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let before = events
        .iter()
        .any(|event| !event.pressed && *resources.phase.get() == AppPhase::Editor)
        .then(|| load_objects(&mut resources));
    for event in events {
        match event.button {
            0 => {
                resources.input.0.left_mouse_down = event.pressed;
                if event.pressed {
                    if *resources.phase.get() == AppPhase::Editor {
                        if let Some(pointer) = resources.input.0.pointer_screen {
                            primary_click.write(PrimaryClick {
                                x: pointer[0],
                                y: pointer[1],
                            });
                        } else {
                            turn_right.write(TurnRight);
                        }
                    } else if *resources.phase.get() != AppPhase::Menu {
                        turn_right.write(TurnRight);
                    }
                }
                if !event.pressed {
                    let phase = *resources.phase.get();
                    let pointer = resources.input.0.pointer_screen;
                    let viewport = viewport(&resources);
                    if phase == AppPhase::Editor
                        && let Some(pointer) = pointer
                    {
                        let mut editor = interaction_context(&mut resources);
                        editor.finish_marquee_selection(pointer[0], pointer[1], viewport, phase);
                        let had_drag = editor.runtime.interaction.gizmo_drag.is_some()
                            || editor.runtime.interaction.block_drag.is_some();
                        editor.clear_interaction_drags();
                        if had_drag {
                            editor.sync_objects_after_drag_release();
                        }
                    } else {
                        resources.input.0.marquee_start_screen = None;
                        resources.input.0.marquee_current_screen = None;
                    }
                }
            }
            2 => {
                resources.input.0.right_dragging = event.pressed;
            }
            _ => {}
        }
    }
    if let Some(before) = before {
        commit_objects(&mut resources, before);
    }
}

pub fn handle_primary_click(
    mut events: MessageReader<PrimaryClick>,
    mut resources: PointerResources,
    mut turn_right: MessageWriter<TurnRight>,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let mut placed_objects = Vec::new();
    for event in events {
        resources.input.0.pointer_screen = Some([event.x, event.y]);
        resources.input.0.left_mouse_down = true;
        if *resources.phase.get() != AppPhase::Editor {
            if *resources.phase.get() != AppPhase::Menu {
                turn_right.write(TurnRight);
            }
            continue;
        }
        ensure_objects(&mut resources);

        let viewport = viewport(&resources);
        if resources
            .input
            .0
            .ui_input_blocking_rects
            .iter()
            .any(|rect| {
                let pixels_per_point = resources.input.0.ui_input_pixels_per_point.max(1.0) as f64;
                let point_x = event.x / pixels_per_point;
                let point_y = event.y / pixels_per_point;
                point_x >= rect[0] as f64
                    && point_x <= rect[2] as f64
                    && point_y >= rect[1] as f64
                    && point_y <= rect[3] as f64
            })
        {
            continue;
        }

        let phase = *resources.phase.get();
        let mut editor = interaction_context(&mut resources);
        match editor.input.mode {
            EditorMode::Place => {
                editor.update_cursor_from_screen_ext(event.x, event.y, viewport, phase);
                let place_mode = if editor.input.shift_held {
                    eg_types::EditorPlaceMode::Stamp
                } else {
                    eg_types::EditorPlaceMode::SelectPlaced
                };
                editor.add_block_at_cursor(place_mode);
                let placed_index = editor.objects.len() - 1;
                placed_objects.push((placed_index, editor.objects[placed_index].clone()));
                if place_mode == eg_types::EditorPlaceMode::SelectPlaced {
                    editor.input.mode = EditorMode::Scale;
                }
            }
            EditorMode::Tapping => {
                handle_tapping_click(&mut editor, event.x, event.y, viewport);
            }
            EditorMode::Timing | EditorMode::Null => {}
            EditorMode::Select | EditorMode::Move | EditorMode::Scale | EditorMode::Rotate => {
                if editor.begin_gizmo_drag_ext(event.x, event.y, viewport, phase) {
                    continue;
                }
                if !editor.input.shift_held
                    && editor.begin_selected_block_drag_ext(event.x, event.y, viewport, phase)
                {
                    continue;
                }

                let hit_selectable = editor
                    .pick_from_screen(event.x, event.y, viewport)
                    .is_some_and(|pick| {
                        pick.hit_block_index.is_some() || pick.hit_trigger_index.is_some()
                    });
                if !editor.input.shift_held && hit_selectable {
                    editor.select_block_from_screen(event.x, event.y, viewport, phase);
                } else {
                    editor.begin_marquee_selection(event.x, event.y, phase);
                }
            }
        }
    }
    for (order, object) in placed_objects {
        spawn_editor_object_commands(&mut resources.commands, order, object);
    }
}

pub fn handle_pointer_moved(
    mut events: MessageReader<PointerMoved>,
    mut resources: PointerResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    let before = if *resources.phase.get() == AppPhase::Editor {
        ensure_objects(&mut resources);
        (resources.input.0.left_mouse_down && !resources.input.0.right_dragging)
            .then(|| resources.pointer_cache.objects.clone())
    } else {
        None
    };
    for event in events {
        resources.input.0.pointer_screen = Some([event.x, event.y]);
        if *resources.phase.get() != AppPhase::Editor {
            continue;
        }

        let phase = *resources.phase.get();
        let viewport = viewport(&resources);
        let mut editor = interaction_context(&mut resources);
        let mut handled = false;
        if editor.input.left_mouse_down && !editor.input.right_dragging {
            handled = editor.drag_gizmo_from_screen(event.x, event.y, viewport, phase)
                || editor.drag_selection_from_screen(event.x, event.y, viewport, phase);
            if !handled && editor.update_marquee_selection(event.x, event.y, phase) {
                if editor.marquee_has_dragged_far_enough() {
                    editor.apply_marquee_selection(viewport, editor.input.shift_held);
                }
                handled = true;
            }
        }
        if !handled {
            if editor.input.mode == EditorMode::Tapping {
                let pick = editor.pick_from_screen(event.x, event.y, viewport);
                editor.runtime.interaction.hovered_tap_index =
                    pick.and_then(|pick| pick.hit_tap_index);
                editor.runtime.interaction.hovered_tap_division =
                    pick.and_then(|pick| pick.hit_tap_division);
            } else {
                editor.runtime.interaction.hovered_gizmo =
                    editor.pick_gizmo_handle(event.x, event.y, viewport);
                editor.update_cursor_from_screen_ext(event.x, event.y, viewport, phase);
            }
        }
    }
    if let Some(before) = before {
        commit_objects(&mut resources, before);
    }
}

pub fn handle_update_cursor_from_screen(
    mut events: MessageReader<UpdateCursorFromScreen>,
    mut resources: PointerResources,
) {
    let events = events.read().copied().collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    if *resources.phase.get() == AppPhase::Editor {
        ensure_objects(&mut resources);
    }
    for event in events {
        resources.input.0.pointer_screen = Some([event.x, event.y]);
        if *resources.phase.get() != AppPhase::Editor
            || resources.input.0.right_dragging
            || resources.input.0.mode == EditorMode::Null
        {
            continue;
        }
        let phase = *resources.phase.get();
        let viewport = viewport(&resources);
        let mut editor = interaction_context(&mut resources);
        editor.update_cursor_from_screen_ext(event.x, event.y, viewport, phase);
    }
}

pub fn handle_camera_drag(
    mut events: MessageReader<CameraDrag>,
    input: Res<EditorInputRes>,
    phase: Res<State<AppPhase>>,
    mut camera: ResMut<EditorCameraRes>,
) {
    if !input.0.right_dragging {
        events.clear();
        return;
    }

    const ROTATE_SPEED: f32 = 0.004;
    const PITCH_SPEED: f32 = 0.006;

    for event in events.read() {
        match *phase.get() {
            eg_types::AppPhase::Editor => {
                camera.0.editor_yaw -= event.dx as f32 * ROTATE_SPEED;
                camera.0.editor_pitch = (camera.0.editor_pitch + event.dy as f32 * PITCH_SPEED)
                    .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
            }
            eg_types::AppPhase::Playing => {
                camera.0.playing_rotation -= event.dx as f32 * ROTATE_SPEED;
                camera.0.playing_pitch = (camera.0.playing_pitch + event.dy as f32 * PITCH_SPEED)
                    .clamp(0.1f32.to_radians(), 89.9f32.to_radians());
            }
            _ => {}
        }
    }
}
