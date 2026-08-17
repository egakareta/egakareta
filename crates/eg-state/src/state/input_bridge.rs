/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Platform input resources for the Bevy App world.

use std::collections::HashMap;

use bevy::{
    ecs::prelude::*,
    input::{
        ButtonInput,
        keyboard::KeyCode,
        mouse::{
            AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButton as BevyMouseButton,
            MouseScrollUnit,
        },
        touch::{TouchInput, TouchPhase},
    },
    state::prelude::State,
    window::{PrimaryWindow, Window, WindowFocused},
};
use eg_editor_domain::{TimingDivisionDirection, timing_division_time_in_direction};
use eg_types::{AppPhase, EditorMode, KeyChord};

use crate::{
    events::{
        block, camera, editor_mode, history, misc, navigation, pointer, selection, session, snap,
        spawn, timeline,
    },
    state::{
        EditorAuthoredObject, EditorConfigRes, EditorInputRes, EditorRuntimeRes, EditorTimelineRes,
        EditorTimingRes, SessionStateRes,
    },
};

const FALLBACK_TIMELINE_SHIFT_SECONDS: f32 = 0.1;

/// Enables the direct Bevy input path for the production root.
///
/// Bevy's `InputPlugin` owns keyboard state; this marker scopes the semantic
/// input translation systems to the production app composition.
#[derive(Resource, Default)]
pub struct BevyInputBridgeEnabled;

/// Egui capture state sampled by the Bevy root before semantic input is
/// translated. The state crate intentionally does not depend on
/// `bevy_egui`; the root supplies these two booleans at the input boundary.
#[derive(Resource, Default)]
pub struct BevyUiInputCaptureRes {
    /// An egui widget owns keyboard input for this frame.
    pub keyboard: bool,
    /// An egui widget owns pointer input for this frame.
    pub pointer: bool,
}

/// Last physical pointer position routed to editor semantics by the Bevy root.
#[derive(Resource, Default)]
pub struct BevyPointerPositionRes(pub Option<[f64; 2]>);

/// App-owned touch routing state for the Bevy root input path.
#[derive(Resource, Default)]
pub struct BevyTouchRoutingRes {
    points: HashMap<u64, [f64; 2]>,
    primary_touch_id: Option<u64>,
    last_cursor_position: Option<[f64; 2]>,
    pinch_last_distance: Option<f64>,
}

/// Ordering boundary between root-owned egui capture and state-owned input
/// translation.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BevyInputBridgeSet {
    /// Egui capture is sampled after its raw-input processing.
    Capture,
    /// Raw Bevy device state is translated into semantic messages.
    Translate,
}

/// Register platform input resources on a World.
pub fn register_platform_input_resources(world: &mut World) {
    world.init_resource::<ButtonInput<KeyCode>>();
    world.init_resource::<ButtonInput<BevyMouseButton>>();
    world.init_resource::<BevyUiInputCaptureRes>();
    world.init_resource::<BevyPointerPositionRes>();
    world.init_resource::<BevyTouchRoutingRes>();
}

/// Translate Bevy keyboard edges into the existing semantic App messages.
///
/// This system runs after Bevy's input update and before the compatibility
/// queue drain. It deliberately samples rendered-frame edges in `PreUpdate`;
/// gameplay-facing command handlers then preserve them for the fixed tick.
pub fn drain_bevy_keyboard_input_system(world: &mut World) {
    if !world.contains_resource::<BevyInputBridgeEnabled>() {
        return;
    }
    if world.resource::<BevyUiInputCaptureRes>().keyboard {
        return;
    }

    let (pressed, released) = {
        let keyboard = world.resource::<ButtonInput<KeyCode>>();
        (
            keyboard.get_just_pressed().copied().collect::<Vec<_>>(),
            keyboard.get_just_released().copied().collect::<Vec<_>>(),
        )
    };

    for key in pressed {
        write_keyboard_input(world, key, true, true);
    }
    for key in released {
        write_keyboard_input(world, key, false, false);
    }
}

/// Translate primary-window pointer motion and button edges into editor
/// messages. Presses respect egui capture; releases always pass through so a
/// drag begun over the scene cannot remain stuck when it ends over UI.
pub fn drain_bevy_pointer_input_system(world: &mut World) {
    if !world.contains_resource::<BevyInputBridgeEnabled>() {
        return;
    }

    let pointer_captured = world.resource::<BevyUiInputCaptureRes>().pointer;
    let position = {
        let mut windows = world.query_filtered::<&Window, With<PrimaryWindow>>();
        windows
            .iter(world)
            .next()
            .and_then(Window::physical_cursor_position)
            .map(|position| [f64::from(position.x), f64::from(position.y)])
    };

    let (pressed, released) = {
        let mouse = world.resource::<ButtonInput<BevyMouseButton>>();
        (
            mouse
                .get_just_pressed()
                .filter_map(bevy_mouse_button_index)
                .collect::<Vec<_>>(),
            mouse
                .get_just_released()
                .filter_map(bevy_mouse_button_index)
                .collect::<Vec<_>>(),
        )
    };

    if pointer_captured {
        world.resource_mut::<BevyPointerPositionRes>().0 = position;
    } else if let Some([x, y]) = position
        && world.resource::<BevyPointerPositionRes>().0 != Some([x, y])
    {
        world.resource_mut::<BevyPointerPositionRes>().0 = Some([x, y]);
        world.write_message(pointer::PointerMoved { x, y });
    }

    if !pointer_captured {
        for button in pressed {
            world.write_message(pointer::MouseButton {
                button,
                pressed: true,
            });
        }
    }
    for button in released {
        world.write_message(pointer::MouseButton {
            button,
            pressed: false,
        });
    }
}

/// Translate Bevy's per-frame accumulated mouse motion and wheel input.
/// Deltas are already accumulated for the frame and must not be scaled by
/// frame time. Egui pointer capture owns both inputs while interacting with
/// UI widgets.
pub fn drain_bevy_mouse_motion_and_wheel_system(world: &mut World) {
    if !world.contains_resource::<BevyInputBridgeEnabled>()
        || world.resource::<BevyUiInputCaptureRes>().pointer
    {
        return;
    }

    let motion = world.resource::<AccumulatedMouseMotion>().delta;
    if motion != bevy::math::Vec2::ZERO {
        world.write_message(pointer::CameraDrag {
            dx: f64::from(motion.x),
            dy: f64::from(motion.y),
        });
    }

    let scroll = *world.resource::<AccumulatedMouseScroll>();
    let zoom_delta = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y * 0.02,
    };
    if zoom_delta != 0.0 {
        world.write_message(camera::AdjustZoom(zoom_delta));
    }
}

/// Route Bevy touch messages into the existing semantic pointer commands.
#[allow(clippy::too_many_arguments)]
pub fn drain_bevy_touch_input_system(
    enabled: Option<Res<BevyInputBridgeEnabled>>,
    capture: Res<BevyUiInputCaptureRes>,
    mut route: ResMut<BevyTouchRoutingRes>,
    touch_events: Option<MessageReader<TouchInput>>,
    mut pointer_moved: MessageWriter<pointer::PointerMoved>,
    mut mouse_button: MessageWriter<pointer::MouseButton>,
    mut camera_drag: MessageWriter<pointer::CameraDrag>,
    mut adjust_zoom: MessageWriter<camera::AdjustZoom>,
) {
    let Some(mut touch_events) = touch_events else {
        return;
    };
    if enabled.is_none() {
        return;
    }

    for touch in touch_events.read() {
        let position = [f64::from(touch.position.x), f64::from(touch.position.y)];
        let previous_count = route.points.len();
        match touch.phase {
            TouchPhase::Started | TouchPhase::Moved => {
                route.points.insert(touch.id, position);
            }
            TouchPhase::Ended | TouchPhase::Canceled => {
                route.points.remove(&touch.id);
            }
        }
        let current_count = route.points.len();

        if previous_count == 1
            && current_count > 1
            && touch.phase == TouchPhase::Started
            && route.primary_touch_id.is_some()
        {
            mouse_button.write(pointer::MouseButton {
                button: 0,
                pressed: false,
            });
            route.primary_touch_id = None;
            route.last_cursor_position = None;
        }

        if capture.pointer {
            if route.points.is_empty() {
                route.pinch_last_distance = None;
                route.primary_touch_id = None;
                route.last_cursor_position = None;
            }
            continue;
        }

        if previous_count == 0 && current_count == 1 && touch.phase == TouchPhase::Started {
            route.primary_touch_id = Some(touch.id);
            pointer_moved.write(pointer::PointerMoved {
                x: position[0],
                y: position[1],
            });
            mouse_button.write(pointer::MouseButton {
                button: 0,
                pressed: true,
            });
            route.last_cursor_position = Some(position);
        }
        if current_count == 1
            && touch.phase == TouchPhase::Moved
            && route.primary_touch_id == Some(touch.id)
        {
            pointer_moved.write(pointer::PointerMoved {
                x: position[0],
                y: position[1],
            });
            if let Some(last) = route.last_cursor_position {
                camera_drag.write(pointer::CameraDrag {
                    dx: position[0] - last[0],
                    dy: position[1] - last[1],
                });
            }
            route.last_cursor_position = Some(position);
        }
        if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Canceled)
            && route.primary_touch_id == Some(touch.id)
        {
            mouse_button.write(pointer::MouseButton {
                button: 0,
                pressed: false,
            });
            route.primary_touch_id = None;
            route.last_cursor_position = None;
        }

        if current_count == 2 {
            let mut points = route.points.values();
            if let (Some(first), Some(second)) = (points.next(), points.next()) {
                let distance = (second[0] - first[0]).hypot(second[1] - first[1]);
                if let Some(previous) = route.pinch_last_distance {
                    adjust_zoom.write(camera::AdjustZoom(((distance - previous) * 0.04) as f32));
                }
                route.pinch_last_distance = Some(distance);
            }
        } else if current_count < 2 {
            route.pinch_last_distance = None;
        }
    }
}

/// Clear held editor input when Bevy reports that its window lost focus.
pub fn clear_held_input_on_bevy_focus_loss_system(
    enabled: Option<Res<BevyInputBridgeEnabled>>,
    focused_events: Option<MessageReader<WindowFocused>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<BevyMouseButton>>,
    mut input: ResMut<EditorInputRes>,
) {
    let Some(mut focused_events) = focused_events else {
        return;
    };
    if enabled.is_none() || !focused_events.read().any(|event| !event.focused) {
        return;
    }

    clear_held_input_resources(&mut keyboard, &mut mouse, &mut input);
}

fn clear_held_input_resources(
    keyboard: &mut ButtonInput<KeyCode>,
    mouse: &mut ButtonInput<BevyMouseButton>,
    input: &mut EditorInputRes,
) {
    *keyboard = ButtonInput::default();
    *mouse = ButtonInput::default();

    input.0.left_mouse_down = false;
    input.0.right_dragging = false;
    input.0.pan_up_held = false;
    input.0.pan_down_held = false;
    input.0.pan_left_held = false;
    input.0.pan_right_held = false;
    input.0.shift_held = false;
    input.0.ctrl_held = false;
    input.0.alt_held = false;
}

fn bevy_mouse_button_index(button: &BevyMouseButton) -> Option<u32> {
    match button {
        BevyMouseButton::Left => Some(0),
        BevyMouseButton::Middle => Some(1),
        BevyMouseButton::Right => Some(2),
        BevyMouseButton::Back => Some(3),
        BevyMouseButton::Forward => Some(4),
        BevyMouseButton::Other(index) => Some(u32::from(*index)),
    }
}

fn write_keyboard_input(world: &mut World, key: KeyCode, pressed: bool, just_pressed: bool) {
    if apply_modifier_key(world, key, pressed) {
        return;
    }

    apply_pan_keybinds(world, key, pressed);

    if !pressed {
        return;
    }

    if let Some((action, slot)) = world
        .resource::<SessionStateRes>()
        .editor_keybind_capture_action
        .clone()
    {
        if !just_pressed || is_modifier_key(key) {
            return;
        }

        if key == KeyCode::Escape {
            world.write_message(session::SetKeybindCapture(None));
            return;
        }

        let input = world.resource::<EditorInputRes>();
        let chord = KeyChord::new(key, input.0.ctrl_held, input.0.shift_held, input.0.alt_held);
        world.write_message(session::SetKeybind {
            action,
            slot,
            chord,
        });
        world.write_message(session::SetKeybindCapture(None));
        return;
    }

    if key == KeyCode::Escape && world.resource::<SessionStateRes>().editor_show_settings {
        world.write_message(session::SetShowSettings(false));
        return;
    }

    dispatch_direct_keybind_actions(world, key, just_pressed, pressed);
}

/// Route shortcut actions that already have small, self-contained typed
/// messages without rebuilding an intermediate command enum.
fn dispatch_direct_keybind_actions(
    world: &mut World,
    key: KeyCode,
    just_pressed: bool,
    pressed: bool,
) {
    if !pressed {
        return;
    }
    let input = world.resource::<EditorInputRes>();
    let actions = world
        .resource::<SessionStateRes>()
        .app_settings
        .keybinds
        .iter()
        .filter(|binding| {
            let chord = &binding.chord;
            chord.key == key
                && chord.ctrl == input.0.ctrl_held
                && chord.shift == input.0.shift_held
                && chord.alt == input.0.alt_held
        })
        .map(|binding| binding.action.clone())
        .collect::<Vec<_>>();

    for action in actions {
        match action.as_str() {
            "toggle_settings" if just_pressed && (!is_playing(world) || is_game_paused(world)) => {
                world.write_message(session::ToggleSettings);
            }
            "toggle_editor" if just_pressed && !is_editor(world) => {
                world.write_message(navigation::ToggleEditor);
            }
            "game_turn" if just_pressed && !is_editor(world) => {
                world.write_message(navigation::TurnRight);
            }
            "practice_checkpoint" if just_pressed && is_practice_mode_enabled(world) => {
                world.write_message(navigation::GameSetPracticeCheckpoint);
            }
            "practice_remove_checkpoint" if just_pressed && is_practice_mode_enabled(world) => {
                world.write_message(navigation::GameRemovePracticeCheckpoint);
            }
            "menu_prev_level" if just_pressed && !is_editor(world) => {
                world.write_message(navigation::PrevLevel);
            }
            "menu_next_level" if just_pressed && !is_editor(world) => {
                world.write_message(navigation::NextLevel);
            }
            "toggle_perf_overlay" if just_pressed => {
                world.write_message(misc::TogglePerfOverlay);
            }
            "toggle_timeline_playback" if just_pressed && is_editor(world) => {
                world.write_message(timeline::ToggleTimelinePlayback);
            }
            "playtest" if just_pressed && is_editor(world) => {
                world.write_message(timeline::Playtest);
            }
            "copy" if just_pressed => {
                world.write_message(block::CopyBlock);
            }
            "paste" if just_pressed => {
                world.write_message(block::PasteBlock);
            }
            "duplicate" if just_pressed => {
                world.write_message(block::DuplicateBlock);
            }
            "undo" if just_pressed => {
                world.write_message(history::Undo);
            }
            "redo" if just_pressed => {
                world.write_message(history::Redo);
            }
            "pick_selected_block" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::PickSelectedBlock);
            }
            "focus_camera_target" if just_pressed && is_editor(world) => {
                world.write_message(selection::FocusCameraTarget);
            }
            "spawn_set" if just_pressed => {
                world.write_message(spawn::SetSpawnHere);
            }
            "add_camera_trigger" if just_pressed && is_editor(world) => {
                world.write_message(camera::AddCameraTrigger);
            }
            "add_transform_trigger"
                if just_pressed && is_editor(world) && has_block_selection(world) =>
            {
                world.write_message(selection::AddTransformTrigger);
            }
            "remove_block" if just_pressed && is_editor(world) => {
                if editor_effective_mode(world) == EditorMode::Tapping {
                    if let Some((_, time_seconds, _)) = selected_tap(world) {
                        world.write_message(timeline::RemoveTapAt(time_seconds));
                    }
                } else {
                    world.write_message(block::RemoveBlock);
                }
            }
            "select_recent_block_1" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SelectRecentBlock(0));
            }
            "select_recent_block_2" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SelectRecentBlock(1));
            }
            "select_recent_block_3" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SelectRecentBlock(2));
            }
            "select_recent_block_4" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SelectRecentBlock(3));
            }
            "spawn_rotate" if just_pressed => {
                if editor_effective_mode(world) == EditorMode::Place {
                    world.write_message(spawn::RotatePlacementPreview);
                } else {
                    world.write_message(spawn::RotateSpawnDirection);
                }
            }
            "add_camera_follow_trigger" if just_pressed && is_editor(world) => {
                world.write_message(camera::AddCameraFollowTrigger);
            }
            "toggle_hitbox_visualization" if just_pressed && is_editor(world) => {
                world.write_message(misc::ToggleHitboxVisualization);
            }
            "timeline_forward" if is_editor(world) => {
                world.write_message(timeline::ShiftTimeline(timeline_shift_to_division_delta(
                    world,
                    TimingDivisionDirection::Forward,
                )));
            }
            "timeline_backward" if is_editor(world) => {
                world.write_message(timeline::ShiftTimeline(timeline_shift_to_division_delta(
                    world,
                    TimingDivisionDirection::Backward,
                )));
            }
            "escape" if just_pressed => {
                world.write_message(misc::Escape);
            }
            "zoom_in" if just_pressed => {
                world.write_message(camera::AdjustZoom(1.0));
            }
            "zoom_out" if just_pressed => {
                world.write_message(camera::AdjustZoom(-1.0));
            }
            "toggle_place_window" if just_pressed && is_editor(world) => {
                world.write_message(misc::TogglePlaceWindow);
            }
            "mode_select" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Select));
            }
            "mode_move" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Move));
            }
            "mode_scale" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Scale));
            }
            "mode_rotate" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Rotate));
            }
            "tab_compose" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Place));
            }
            "tab_timing" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Timing));
            }
            "tab_tapping" if just_pressed && is_editor(world) => {
                world.write_message(editor_mode::SetEditorMode(EditorMode::Tapping));
            }
            "toggle_grid_snap" if just_pressed && is_editor(world) => {
                let enabled = !world.resource::<EditorConfigRes>().0.snap_to_grid;
                world.write_message(snap::SetSnapToGrid(enabled));
            }
            "toggle_rotation_snap" if just_pressed && is_editor(world) => {
                let enabled = !world.resource::<EditorConfigRes>().0.snap_rotation;
                world.write_message(snap::SetSnapRotation(enabled));
            }
            _ => {}
        }
    }
}

fn apply_modifier_key(world: &mut World, key: KeyCode, pressed: bool) -> bool {
    let mut input = world.resource_mut::<EditorInputRes>();
    match key {
        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
            input.0.shift_held = pressed;
            true
        }
        KeyCode::ControlLeft | KeyCode::ControlRight => {
            input.0.ctrl_held = pressed;
            true
        }
        KeyCode::AltLeft | KeyCode::AltRight => {
            input.0.alt_held = pressed;
            true
        }
        _ => false,
    }
}

fn apply_pan_keybinds(world: &mut World, key: KeyCode, pressed: bool) {
    if !is_editor(world) {
        return;
    }

    let input = world.resource::<EditorInputRes>();
    let ctrl = input.0.ctrl_held;
    let alt = input.0.alt_held;
    let actions = world
        .resource::<SessionStateRes>()
        .app_settings
        .keybinds
        .iter()
        .filter_map(|binding| {
            let chord = &binding.chord;
            let key_match = chord.key == key;
            let modifiers_match = chord.ctrl == ctrl && chord.alt == alt;
            if key_match && (!pressed || modifiers_match) {
                Some(binding.action.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut input = world.resource_mut::<EditorInputRes>();
    for action in actions {
        match action.as_str() {
            "pan_up" => input.0.pan_up_held = pressed,
            "pan_down" => input.0.pan_down_held = pressed,
            "pan_left" => input.0.pan_left_held = pressed,
            "pan_right" => input.0.pan_right_held = pressed,
            _ => {}
        }
    }
}

fn timeline_shift_to_division_delta(world: &World, direction: TimingDivisionDirection) -> f32 {
    let timeline = world.resource::<EditorTimelineRes>();
    let current_time = timeline.0.clock.time_seconds;
    if let Some(target_time) = timing_division_time_in_direction(
        current_time,
        &world.resource::<EditorTimingRes>().0.timing_points,
        timeline.0.clock.duration_seconds,
        direction,
    ) {
        return target_time - current_time;
    }

    match direction {
        TimingDivisionDirection::Forward => FALLBACK_TIMELINE_SHIFT_SECONDS,
        TimingDivisionDirection::Backward => -FALLBACK_TIMELINE_SHIFT_SECONDS,
    }
}

fn editor_effective_mode(world: &World) -> EditorMode {
    let input = world.resource::<EditorInputRes>();
    let timeline = world.resource::<EditorTimelineRes>();
    if timeline.0.playback.playing && input.0.mode == EditorMode::Null {
        world
            .resource::<EditorRuntimeRes>()
            .0
            .interaction
            .last_mode
            .unwrap_or(input.0.mode)
    } else {
        input.0.mode
    }
}

fn selected_tap(world: &World) -> Option<(usize, f32, [f32; 3])> {
    let taps = &world.resource::<EditorTimelineRes>().0.taps;
    let index = taps.selected_index?;
    let time_seconds = *taps.tap_times.get(index)?;
    let position = *taps.tap_indicator_positions.get(index)?;
    Some((index, time_seconds, position))
}

fn has_block_selection(world: &World) -> bool {
    let input = world.resource::<EditorInputRes>();
    let object_count = world
        .iter_entities()
        .filter(|entity| entity.contains::<EditorAuthoredObject>())
        .count();
    input
        .0
        .selected_block_indices
        .iter()
        .any(|index| *index < object_count)
        || input
            .0
            .selected_block_index
            .is_some_and(|index| index < object_count)
}

fn is_editor(world: &World) -> bool {
    *world.resource::<State<AppPhase>>().get() == AppPhase::Editor
}

fn is_playing(world: &World) -> bool {
    *world.resource::<State<AppPhase>>().get() == AppPhase::Playing
}

fn is_game_paused(world: &World) -> bool {
    world.resource::<SessionStateRes>().game_paused
}

fn is_practice_mode_enabled(world: &World) -> bool {
    world.resource::<SessionStateRes>().practice_mode_enabled
}

fn is_modifier_key(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
    )
}

#[cfg(test)]
mod tests {
    use bevy::ecs::message::Messages;

    use super::*;
    use crate::{
        events::{camera, navigation, pointer},
        state::plugins::register_state_resources,
    };

    #[test]
    fn direct_bevy_keyboard_path_preserves_edges_and_updates_modifiers() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.insert_resource(State::new(AppPhase::Editor));
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);

        drain_bevy_keyboard_input_system(&mut world);

        assert!(world.resource::<EditorInputRes>().0.ctrl_held);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::ControlLeft),
            "the direct path must not clear Bevy's rendered-frame edge"
        );
    }

    #[test]
    fn direct_bevy_keyboard_path_writes_one_navigation_message() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.insert_resource(State::new(AppPhase::Menu));
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        drain_bevy_keyboard_input_system(&mut world);

        assert_eq!(world.resource::<Messages<navigation::TurnRight>>().len(), 1);
    }

    #[test]
    fn direct_bevy_keyboard_path_respects_egui_capture() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.insert_resource(State::new(AppPhase::Editor));
        world.resource_mut::<BevyUiInputCaptureRes>().keyboard = true;
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);

        drain_bevy_keyboard_input_system(&mut world);

        assert!(
            !world.resource::<EditorInputRes>().0.ctrl_held,
            "captured keyboard input must not activate editor modifiers"
        );
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::ControlLeft)
        );
    }

    #[test]
    fn direct_bevy_pointer_path_respects_capture_but_routes_releases() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.resource_mut::<BevyUiInputCaptureRes>().pointer = true;
        world
            .resource_mut::<ButtonInput<BevyMouseButton>>()
            .press(BevyMouseButton::Left);

        drain_bevy_pointer_input_system(&mut world);
        assert!(
            world
                .resource::<Messages<pointer::MouseButton>>()
                .is_empty()
        );

        world
            .resource_mut::<ButtonInput<BevyMouseButton>>()
            .release(BevyMouseButton::Left);
        drain_bevy_pointer_input_system(&mut world);

        let messages = world.resource::<Messages<pointer::MouseButton>>();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn direct_bevy_motion_and_wheel_preserve_custom_zoom_scaling() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.insert_resource(AccumulatedMouseMotion {
            delta: bevy::math::Vec2::new(5.0, -3.0),
        });
        world.insert_resource(AccumulatedMouseScroll {
            unit: MouseScrollUnit::Pixel,
            delta: bevy::math::Vec2::new(0.0, 50.0),
        });

        drain_bevy_mouse_motion_and_wheel_system(&mut world);

        assert_eq!(world.resource::<Messages<pointer::CameraDrag>>().len(), 1);
        assert_eq!(world.resource::<Messages<camera::AdjustZoom>>().len(), 1);
    }

    #[test]
    fn direct_bevy_touch_routes_single_finger_drag_and_cancel() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.init_resource::<Messages<TouchInput>>();

        for (phase, x, y) in [
            (TouchPhase::Started, 10.0, 20.0),
            (TouchPhase::Moved, 15.0, 25.0),
            (TouchPhase::Canceled, 15.0, 25.0),
        ] {
            world.write_message(TouchInput {
                phase,
                position: bevy::math::Vec2::new(x, y),
                window: Entity::PLACEHOLDER,
                force: None,
                id: 7,
            });
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(drain_bevy_touch_input_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<Messages<pointer::PointerMoved>>().len(), 2);
        assert_eq!(world.resource::<Messages<pointer::CameraDrag>>().len(), 1);
        assert_eq!(world.resource::<Messages<pointer::MouseButton>>().len(), 2);
    }

    #[test]
    fn direct_bevy_touch_uses_two_finger_distance_for_pinch_zoom() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.init_resource::<Messages<TouchInput>>();

        for (phase, id, x) in [
            (TouchPhase::Started, 1, 0.0),
            (TouchPhase::Started, 2, 10.0),
            (TouchPhase::Moved, 2, 20.0),
        ] {
            world.write_message(TouchInput {
                phase,
                position: bevy::math::Vec2::new(x, 0.0),
                window: Entity::PLACEHOLDER,
                force: None,
                id,
            });
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(drain_bevy_touch_input_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<Messages<camera::AdjustZoom>>().len(), 1);
    }

    #[test]
    fn direct_bevy_focus_loss_clears_held_input() {
        let mut world = World::new();
        register_state_resources(&mut world);
        world.insert_resource(BevyInputBridgeEnabled);
        world.init_resource::<Messages<WindowFocused>>();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyA);
        world.resource_mut::<EditorInputRes>().0.shift_held = true;
        world.write_message(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused: false,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(clear_held_input_on_bevy_focus_loss_system);
        schedule.run(&mut world);

        assert!(
            !world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        assert!(!world.resource::<EditorInputRes>().0.shift_held);
    }
}
