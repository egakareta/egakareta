mod game;
mod mesh;
mod state;
mod types;

pub use state::State;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::console;
#[cfg(target_arch = "wasm32")]
use wgpu::SurfaceError;

#[cfg(target_arch = "wasm32")]
use crate::types::PhysicalSize;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn run_game(canvas_id: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id(&canvas_id)
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap();

    let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
    let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    let state = State::new(canvas.clone()).await;
    let state_rc = Rc::new(RefCell::new(state));
    let pinch_last_distance = Rc::new(RefCell::new(None::<f64>));

    // Block selection UI listeners
    for (id, kind) in [
        ("block-standard", crate::types::BlockKind::Standard),
        ("block-grass", crate::types::BlockKind::Grass),
        ("block-dirt", crate::types::BlockKind::Dirt),
    ] {
        let state_clone = state_rc.clone();
        if let Some(el) = document.get_element_by_id(id) {
            let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                state_clone.borrow_mut().set_editor_block_kind(kind);
                event.stop_propagation();
            }) as Box<dyn FnMut(_)>);
            el.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }
    }

    {
        let state_clone = state_rc.clone();
        let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let window = web_sys::window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
            state_clone
                .borrow_mut()
                .resize(PhysicalSize::new(width, height));
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let mut state = state_clone.borrow_mut();
        match event.button() {
            0 => state.turn_right(),
            2 => {
                state.set_editor_right_dragging(true);
                event.prevent_default();
            }
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        if event.button() == 2 {
            state_clone.borrow_mut().set_editor_right_dragging(false);
            event.prevent_default();
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let mut state = state_clone.borrow_mut();
        if (event.buttons() & 2) != 0 {
            state.drag_editor_camera_by_pixels(event.movement_x() as f64, event.movement_y() as f64);
            event.prevent_default();
        } else {
            state.update_editor_cursor_from_screen(event.offset_x() as f64, event.offset_y() as f64);
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        event.prevent_default();
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
        let scale = match event.delta_mode() {
            1 => 0.2,
            2 => 1.0,
            _ => 0.01,
        };
        state_clone
            .borrow_mut()
            .adjust_editor_zoom((-event.delta_y() * scale) as f32);
        event.prevent_default();
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let pinch_last_distance_clone = pinch_last_distance.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
        if event.touches().length() == 2 {
            let t0 = event.touches().item(0).unwrap();
            let t1 = event.touches().item(1).unwrap();
            let dx = (t1.client_x() - t0.client_x()) as f64;
            let dy = (t1.client_y() - t0.client_y()) as f64;
            *pinch_last_distance_clone.borrow_mut() = Some((dx * dx + dy * dy).sqrt());
            event.prevent_default();
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("touchstart", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let pinch_last_distance_clone = pinch_last_distance.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
        if event.touches().length() == 2 {
            let t0 = event.touches().item(0).unwrap();
            let t1 = event.touches().item(1).unwrap();
            let dx = (t1.client_x() - t0.client_x()) as f64;
            let dy = (t1.client_y() - t0.client_y()) as f64;
            let distance = (dx * dx + dy * dy).sqrt();

            if let Some(previous) = *pinch_last_distance_clone.borrow() {
                let pinch_delta = ((distance - previous) * 0.04) as f32;
                state_clone.borrow_mut().adjust_editor_zoom(pinch_delta);
            }

            *pinch_last_distance_clone.borrow_mut() = Some(distance);
            event.prevent_default();
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("touchmove", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let pinch_last_distance_clone = pinch_last_distance.clone();
    let closure = Closure::wrap(Box::new(move |_event: web_sys::TouchEvent| {
        *pinch_last_distance_clone.borrow_mut() = None;
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("touchend", closure.as_ref().unchecked_ref())
        .unwrap();
    canvas
        .add_event_listener_with_callback("touchcancel", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        let mut state = state_clone.borrow_mut();
        let just_pressed = !event.repeat();
        match event.key().as_str() {
            "ArrowUp" => {
                if state.is_editor() {
                    state.set_editor_pan_up_held(true);
                } else if just_pressed {
                    state.turn_right();
                }
            }
            "ArrowDown" => {
                if state.is_editor() {
                    state.set_editor_pan_down_held(true);
                }
            }
            " " => {
                if just_pressed {
                    state.turn_right();
                }
            }
            "ArrowRight" => {
                if state.is_editor() {
                    state.set_editor_pan_right_held(true);
                } else if just_pressed {
                    state.next_level();
                }
            }
            "ArrowLeft" => {
                if state.is_editor() {
                    state.set_editor_pan_left_held(true);
                } else if just_pressed {
                    state.prev_level();
                }
            }
            "Enter" => {
                if just_pressed {
                    state.editor_playtest();
                }
            }
            "Backspace" | "Delete" => {
                if just_pressed {
                    state.editor_remove_block();
                }
            }
            "Escape" => {
                if just_pressed {
                    state.back_to_menu();
                }
            }
            "Shift" => {
                state.set_editor_shift_held(true);
            }
            "w" | "W" => {
                if state.is_editor() {
                    state.set_editor_pan_up_held(true);
                }
            }
            "s" | "S" => {
                if state.is_editor() {
                    state.set_editor_pan_down_held(true);
                }
            }
            "d" | "D" => {
                if state.is_editor() {
                    state.set_editor_pan_right_held(true);
                } else if just_pressed {
                    state.next_level();
                }
            }
            "a" | "A" => {
                if state.is_editor() {
                    state.set_editor_pan_left_held(true);
                } else if just_pressed {
                    state.prev_level();
                }
            }
            "e" | "E" => {
                if just_pressed {
                    state.toggle_editor();
                }
            }
            "p" | "P" => {
                if just_pressed {
                    state.editor_set_spawn_here();
                }
            }
            "r" | "R" => {
                if just_pressed {
                    state.editor_rotate_spawn_direction();
                }
            }
            "+" | "=" => {
                if just_pressed {
                    state.adjust_editor_zoom(1.0);
                }
            }
            "-" | "_" => {
                if just_pressed {
                    state.adjust_editor_zoom(-1.0);
                }
            }
            "1" => {
                if state.is_editor() && just_pressed {
                    state.set_editor_block_kind(crate::types::BlockKind::Standard);
                }
            }
            "2" => {
                if state.is_editor() && just_pressed {
                    state.set_editor_block_kind(crate::types::BlockKind::Grass);
                }
            }
            "3" => {
                if state.is_editor() && just_pressed {
                    state.set_editor_block_kind(crate::types::BlockKind::Dirt);
                }
            }
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        let mut state = state_clone.borrow_mut();
        match event.key().as_str() {
            "ArrowUp" | "w" | "W" => state.set_editor_pan_up_held(false),
            "ArrowDown" | "s" | "S" => state.set_editor_pan_down_held(false),
            "ArrowLeft" | "a" | "A" => state.set_editor_pan_left_held(false),
            "ArrowRight" | "d" | "D" => state.set_editor_pan_right_held(false),
            "Shift" => state.set_editor_shift_held(false),
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut state = state_rc.borrow_mut();
        state.update();
        match state.render() {
            Ok(_) => {}
            Err(SurfaceError::Lost) | Err(SurfaceError::Outdated) => {
                state.handle_surface_lost();
            }
            Err(SurfaceError::OutOfMemory) => {
                console::error_1(&"Out of memory".into());
                return;
            }
            Err(err) => console::error_1(&format!("Render error: {:?}", err).into()),
        }
        let window = web_sys::window().unwrap();
        window
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));
    web_sys::window()
        .unwrap()
        .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();
    Ok(())
}
