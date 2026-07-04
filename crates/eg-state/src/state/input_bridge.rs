/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Platform input resources for the Bevy App world.

use bevy_ecs::prelude::*;

use crate::commands::{AppCommand, InputEvent};
use crate::state::editor_command::EditorCommand;
use crate::state::{
    AppPhaseRes, EditorConfigRes, EditorInputRes, EditorObjectsRes, EditorRuntimeRes,
    EditorTimelineRes, EditorTimingRes, SessionStateRes,
};
use eg_editor_domain::{TimingDivisionDirection, timing_division_time_in_direction};
use eg_types::{AppPhase, EditorMode, KeyChord, normalize_binding_key};

const FALLBACK_TIMELINE_SHIFT_SECONDS: f32 = 0.1;

/// Platform input events captured for the current App world.
#[derive(Resource, Default, Debug)]
pub struct PlatformInputQueue {
    events: Vec<InputEvent>,
}

impl PlatformInputQueue {
    /// Push an input event into the queue.
    pub fn push(&mut self, event: InputEvent) {
        self.events.push(event);
    }

    /// Returns true when no input events are queued.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Number of queued input events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Read the queued input events.
    pub fn events(&self) -> &[InputEvent] {
        &self.events
    }

    /// Drain queued input events.
    pub fn drain(&mut self) -> impl Iterator<Item = InputEvent> + '_ {
        self.events.drain(..)
    }
}

/// Register platform input resources on a World.
pub fn register_platform_input_resources(world: &mut World) {
    world.init_resource::<PlatformInputQueue>();
}

/// Queue one platform input event into a World.
pub fn enqueue_platform_input(world: &mut World, event: InputEvent) {
    world.resource_mut::<PlatformInputQueue>().push(event);
}

/// Drain platform input into App-world resources and semantic command messages.
pub fn drain_platform_input_system(world: &mut World) {
    let events = {
        let mut queue = world.resource_mut::<PlatformInputQueue>();
        if queue.is_empty() {
            return;
        }
        queue.drain().collect::<Vec<_>>()
    };

    for event in events {
        write_input_event(world, event);
    }
}

fn write_input_event(world: &mut World, event: InputEvent) {
    match event {
        InputEvent::Key {
            key,
            pressed,
            just_pressed,
        } => write_keyboard_input(world, &key, pressed, just_pressed),
        InputEvent::MouseButton { button, pressed } => {
            crate::events::write_editor_command_message(
                world,
                EditorCommand::MouseButton { button, pressed },
            );
        }
        InputEvent::PrimaryClick { x, y } => {
            crate::events::write_editor_command_message(
                world,
                EditorCommand::PrimaryClick { x, y },
            );
        }
        InputEvent::PointerMoved { x, y } => {
            crate::events::write_editor_command_message(
                world,
                EditorCommand::PointerMoved { x, y },
            );
        }
        InputEvent::CameraDrag { dx, dy } => {
            crate::events::write_editor_command_message(
                world,
                EditorCommand::CameraDrag { dx, dy },
            );
        }
        InputEvent::Zoom(delta) => {
            crate::events::write_editor_command_message(world, EditorCommand::AdjustZoom(delta));
        }
        InputEvent::Resize { width, height } => {
            crate::events::write_app_command_message(
                world,
                AppCommand::ResizeSurface { width, height },
            );
        }
    }
}

fn write_keyboard_input(world: &mut World, key: &str, pressed: bool, just_pressed: bool) {
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

        if key == "Escape" {
            crate::events::write_editor_command_message(
                world,
                EditorCommand::SetKeybindCapture(None),
            );
            return;
        }

        let input = world.resource::<EditorInputRes>();
        let chord = KeyChord::new(
            normalize_binding_key(key),
            input.0.ctrl_held,
            input.0.shift_held,
            input.0.alt_held,
        );
        crate::events::write_editor_command_message(
            world,
            EditorCommand::SetKeybind {
                action,
                slot,
                chord,
            },
        );
        crate::events::write_editor_command_message(world, EditorCommand::SetKeybindCapture(None));
        return;
    }

    if key == "Escape" && world.resource::<SessionStateRes>().editor_show_settings {
        crate::events::write_editor_command_message(world, EditorCommand::SetShowSettings(false));
        return;
    }

    for command in keybind_commands(world, key, just_pressed, pressed) {
        crate::events::write_app_command_message(world, command);
    }
}

fn apply_modifier_key(world: &mut World, key: &str, pressed: bool) -> bool {
    let mut input = world.resource_mut::<EditorInputRes>();
    match key {
        "Shift" | "ShiftLeft" | "ShiftRight" => {
            input.0.shift_held = pressed;
            true
        }
        "Control" | "ControlLeft" | "ControlRight" => {
            input.0.ctrl_held = pressed;
            true
        }
        "Alt" | "AltLeft" | "AltRight" => {
            input.0.alt_held = pressed;
            true
        }
        _ => false,
    }
}

fn apply_pan_keybinds(world: &mut World, key: &str, pressed: bool) {
    if !is_editor(world) {
        return;
    }

    let normalized_key = normalize_binding_key(key);
    let input = world.resource::<EditorInputRes>();
    let ctrl = input.0.ctrl_held;
    let alt = input.0.alt_held;
    let actions = world
        .resource::<SessionStateRes>()
        .app_settings
        .keybinds
        .iter()
        .filter_map(|binding| {
            let chord = binding.chord.normalized();
            let key_match = chord.key == normalized_key;
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

fn keybind_commands(
    world: &World,
    key: &str,
    just_pressed: bool,
    pressed: bool,
) -> Vec<AppCommand> {
    if !pressed {
        return Vec::new();
    }

    let normalized_key = normalize_binding_key(key);
    let input = world.resource::<EditorInputRes>();
    let ctrl = input.0.ctrl_held;
    let shift = input.0.shift_held;
    let alt = input.0.alt_held;

    let mut commands = Vec::new();
    for binding in &world.resource::<SessionStateRes>().app_settings.keybinds {
        let chord = binding.chord.normalized();
        if chord.key == normalized_key
            && chord.ctrl == ctrl
            && chord.shift == shift
            && chord.alt == alt
            && let Some(command) = command_for_keybind_action(world, &binding.action, just_pressed)
        {
            commands.push(command);
        }
    }

    commands
}

fn command_for_keybind_action(
    world: &World,
    action: &str,
    just_pressed: bool,
) -> Option<AppCommand> {
    match action {
        "toggle_settings" => {
            if just_pressed && (!is_playing(world) || is_game_paused(world)) {
                Some(AppCommand::Editor(EditorCommand::ToggleSettings))
            } else {
                None
            }
        }
        "toggle_timeline_playback" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback))
            } else {
                None
            }
        }
        "playtest" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::Playtest))
            } else {
                None
            }
        }
        "remove_block" => {
            if is_editor(world) && just_pressed {
                if editor_effective_mode(world) == EditorMode::Tapping {
                    selected_tap(world).map(|(_, time_seconds, _)| {
                        AppCommand::Editor(EditorCommand::RemoveTapAt(time_seconds))
                    })
                } else {
                    Some(AppCommand::Editor(EditorCommand::RemoveBlock))
                }
            } else {
                None
            }
        }
        "copy" => just_pressed.then_some(AppCommand::Editor(EditorCommand::CopyBlock)),
        "paste" => just_pressed.then_some(AppCommand::Editor(EditorCommand::PasteBlock)),
        "duplicate" => just_pressed.then_some(AppCommand::Editor(EditorCommand::DuplicateBlock)),
        "undo" => just_pressed.then_some(AppCommand::Editor(EditorCommand::Undo)),
        "redo" => just_pressed.then_some(AppCommand::Editor(EditorCommand::Redo)),
        "snap_selection_to_grid" => {
            if is_editor(world)
                && just_pressed
                && (has_block_selection(world) || selected_tap(world).is_some())
            {
                Some(AppCommand::Editor(EditorCommand::SnapSelectionToGrid))
            } else {
                None
            }
        }
        "pick_selected_block" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::PickSelectedBlock))
            } else {
                None
            }
        }
        "select_recent_block_1" => select_recent_block_command(world, just_pressed, 0),
        "select_recent_block_2" => select_recent_block_command(world, just_pressed, 1),
        "select_recent_block_3" => select_recent_block_command(world, just_pressed, 2),
        "select_recent_block_4" => select_recent_block_command(world, just_pressed, 3),
        "focus_camera_target" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::FocusCameraTarget))
            } else {
                None
            }
        }
        "add_transform_trigger" => {
            if is_editor(world) && just_pressed && has_block_selection(world) {
                Some(AppCommand::Editor(EditorCommand::AddTransformTrigger))
            } else {
                None
            }
        }
        "timeline_forward" => is_editor(world)
            .then(|| timeline_shift_to_division_command(world, TimingDivisionDirection::Forward)),
        "timeline_backward" => is_editor(world)
            .then(|| timeline_shift_to_division_command(world, TimingDivisionDirection::Backward)),
        "escape" => just_pressed.then_some(AppCommand::Editor(EditorCommand::Escape)),
        "zoom_in" => just_pressed.then_some(AppCommand::Editor(EditorCommand::AdjustZoom(1.0))),
        "zoom_out" => just_pressed.then_some(AppCommand::Editor(EditorCommand::AdjustZoom(-1.0))),
        "toggle_editor" => {
            if !is_editor(world) && just_pressed {
                Some(AppCommand::ToggleEditor)
            } else {
                None
            }
        }
        "toggle_place_window" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::TogglePlaceWindow))
            } else {
                None
            }
        }
        "game_turn" => {
            if !is_editor(world) && just_pressed {
                Some(AppCommand::TurnRight)
            } else {
                None
            }
        }
        "practice_checkpoint" => {
            if is_practice_mode_enabled(world) && just_pressed {
                Some(AppCommand::GameSetPracticeCheckpoint)
            } else {
                None
            }
        }
        "practice_remove_checkpoint" => {
            if is_practice_mode_enabled(world) && just_pressed {
                Some(AppCommand::GameRemovePracticeCheckpoint)
            } else {
                None
            }
        }
        "menu_prev_level" => {
            if !is_editor(world) && just_pressed {
                Some(AppCommand::PrevLevel)
            } else {
                None
            }
        }
        "menu_next_level" => {
            if !is_editor(world) && just_pressed {
                Some(AppCommand::NextLevel)
            } else {
                None
            }
        }
        "spawn_set" => just_pressed.then_some(AppCommand::Editor(EditorCommand::SetSpawnHere)),
        "spawn_rotate" => {
            if just_pressed {
                if editor_effective_mode(world) == EditorMode::Place {
                    Some(AppCommand::Editor(EditorCommand::RotatePlacementPreview))
                } else {
                    Some(AppCommand::Editor(EditorCommand::RotateSpawnDirection))
                }
            } else {
                None
            }
        }
        "add_camera_trigger" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::AddCameraTrigger))
            } else {
                None
            }
        }
        "add_camera_follow_trigger" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::AddCameraFollowTrigger))
            } else {
                None
            }
        }
        "toggle_hitbox_visualization" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::ToggleHitboxVisualization))
            } else {
                None
            }
        }
        "toggle_perf_overlay" => {
            just_pressed.then_some(AppCommand::Editor(EditorCommand::TogglePerfOverlay))
        }
        "mode_select" => mode_command(world, just_pressed, EditorMode::Select),
        "mode_move" => mode_command(world, just_pressed, EditorMode::Move),
        "mode_scale" => mode_command(world, just_pressed, EditorMode::Scale),
        "mode_rotate" => mode_command(world, just_pressed, EditorMode::Rotate),
        "tab_compose" => mode_command(world, just_pressed, EditorMode::Place),
        "tab_timing" => mode_command(world, just_pressed, EditorMode::Timing),
        "tab_tapping" => mode_command(world, just_pressed, EditorMode::Tapping),
        "toggle_grid_snap" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::SetSnapToGrid(
                    !world.resource::<EditorConfigRes>().0.snap_to_grid,
                )))
            } else {
                None
            }
        }
        "toggle_rotation_snap" => {
            if is_editor(world) && just_pressed {
                Some(AppCommand::Editor(EditorCommand::SetSnapRotation(
                    !world.resource::<EditorConfigRes>().0.snap_rotation,
                )))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn select_recent_block_command(
    world: &World,
    just_pressed: bool,
    index: usize,
) -> Option<AppCommand> {
    if is_editor(world) && just_pressed {
        Some(AppCommand::Editor(EditorCommand::SelectRecentBlock(index)))
    } else {
        None
    }
}

fn mode_command(world: &World, just_pressed: bool, mode: EditorMode) -> Option<AppCommand> {
    if is_editor(world) && just_pressed {
        Some(AppCommand::Editor(EditorCommand::SetMode(mode)))
    } else {
        None
    }
}

fn timeline_shift_to_division_command(
    world: &World,
    direction: TimingDivisionDirection,
) -> AppCommand {
    let timeline = world.resource::<EditorTimelineRes>();
    let current_time = timeline.0.clock.time_seconds;
    if let Some(target_time) = timing_division_time_in_direction(
        current_time,
        &world.resource::<EditorTimingRes>().0.timing_points,
        timeline.0.clock.duration_seconds,
        direction,
    ) {
        return AppCommand::Editor(EditorCommand::ShiftTimeline(target_time - current_time));
    }

    let fallback_delta = match direction {
        TimingDivisionDirection::Forward => FALLBACK_TIMELINE_SHIFT_SECONDS,
        TimingDivisionDirection::Backward => -FALLBACK_TIMELINE_SHIFT_SECONDS,
    };
    AppCommand::Editor(EditorCommand::ShiftTimeline(fallback_delta))
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
    let object_count = world.resource::<EditorObjectsRes>().objects.len();
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
    world.resource::<AppPhaseRes>().current == AppPhase::Editor
}

fn is_playing(world: &World) -> bool {
    world.resource::<AppPhaseRes>().current == AppPhase::Playing
}

fn is_game_paused(world: &World) -> bool {
    world.resource::<SessionStateRes>().game_paused
}

fn is_practice_mode_enabled(world: &World) -> bool {
    world.resource::<SessionStateRes>().practice_mode_enabled
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "Shift" | "Control" | "ControlLeft" | "ControlRight" | "Alt" | "AltLeft" | "AltRight"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{auth, pointer};
    use crate::state::plugins::register_state_resources;
    use bevy_ecs::message::Messages;

    #[test]
    fn enqueue_platform_input_buffers_event() {
        let mut world = World::new();
        register_platform_input_resources(&mut world);

        enqueue_platform_input(
            &mut world,
            InputEvent::Resize {
                width: 640,
                height: 360,
            },
        );

        let queue = world.resource::<PlatformInputQueue>();
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.events()[0],
            InputEvent::Resize {
                width: 640,
                height: 360
            }
        ));
    }

    #[test]
    fn platform_input_queue_drain_clears_events() {
        let mut queue = PlatformInputQueue::default();
        queue.push(InputEvent::Zoom(1.5));

        let drained = queue.drain().collect::<Vec<_>>();

        assert_eq!(drained.len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn drain_platform_input_writes_resize_message() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);

        enqueue_platform_input(
            &mut world,
            InputEvent::Resize {
                width: 800,
                height: 600,
            },
        );

        drain_platform_input_system(&mut world);

        let messages = world.resource::<Messages<auth::ResizeSurface>>();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn drain_platform_input_writes_pointer_message() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);

        enqueue_platform_input(&mut world, InputEvent::PointerMoved { x: 10.0, y: 20.0 });

        drain_platform_input_system(&mut world);

        let messages = world.resource::<Messages<pointer::PointerMoved>>();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn drain_platform_input_updates_modifier_before_shortcut_mapping() {
        let mut world = World::new();
        register_state_resources(&mut world);
        crate::events::register_command_events_world(&mut world);
        world.resource_mut::<AppPhaseRes>().current = AppPhase::Editor;

        enqueue_platform_input(
            &mut world,
            InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            },
        );
        enqueue_platform_input(
            &mut world,
            InputEvent::Key {
                key: "c".to_string(),
                pressed: true,
                just_pressed: true,
            },
        );

        drain_platform_input_system(&mut world);

        assert!(world.resource::<EditorInputRes>().0.ctrl_held);
        let messages = world.resource::<Messages<crate::events::block::CopyBlock>>();
        assert_eq!(messages.len(), 1);
    }
}
