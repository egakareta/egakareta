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
use bevy_ecs::prelude::*;

use crate::state::{AppPhaseRes, EditorCameraRes, EditorInputRes};

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

pub fn handle_mouse_button(
    mut events: MessageReader<MouseButton>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        match event.button {
            0 => {
                input.0.left_mouse_down = event.pressed;
                if !event.pressed {
                    input.0.marquee_start_screen = None;
                    input.0.marquee_current_screen = None;
                }
            }
            2 => {
                input.0.right_dragging = event.pressed;
            }
            _ => {}
        }
    }
}

pub fn handle_primary_click(
    mut events: MessageReader<PrimaryClick>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pointer_screen = Some([event.x, event.y]);
        input.0.left_mouse_down = true;
    }
}

pub fn handle_pointer_moved(
    mut events: MessageReader<PointerMoved>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pointer_screen = Some([event.x, event.y]);
        if input.0.left_mouse_down {
            input.0.marquee_current_screen = Some([event.x, event.y]);
        }
    }
}

pub fn handle_update_cursor_from_screen(
    mut events: MessageReader<UpdateCursorFromScreen>,
    mut input: ResMut<EditorInputRes>,
) {
    for event in events.read() {
        input.0.pointer_screen = Some([event.x, event.y]);
    }
}

pub fn handle_camera_drag(
    mut events: MessageReader<CameraDrag>,
    input: Res<EditorInputRes>,
    phase: Res<AppPhaseRes>,
    mut camera: ResMut<EditorCameraRes>,
) {
    if !input.0.right_dragging {
        events.clear();
        return;
    }

    const ROTATE_SPEED: f32 = 0.004;
    const PITCH_SPEED: f32 = 0.006;

    for event in events.read() {
        match phase.current {
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
