//! Web runtime for egakareta.
//!
//! This module provides the WebAssembly entry point for running the game in web browsers.
//! It handles canvas setup, input event binding, and the animation loop using `requestAnimationFrame`.

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::platform::runtime::Runtime;
use crate::State;

use gloo_render::{request_animation_frame, AnimationFrame};
use gloo_utils::window;

mod input;
use input::{setup_web_input_callbacks, WebInputHandler};

fn request_frame(f: Rc<RefCell<Option<AnimationFrame>>>, mut cb: impl FnMut(f64) + 'static) {
    let f_clone = f.clone();
    *f.borrow_mut() = Some(request_animation_frame(move |time| {
        cb(time);
        request_frame(f_clone, cb);
    }));
}

/// Runs the game in the web environment.
/// Initializes the WASM runtime, sets up the canvas, input handlers, and starts the animation loop.
///
/// # Arguments
/// * `canvas_id` - The ID of the HTML canvas element to render to.
#[wasm_bindgen]
pub async fn run_game(canvas_id: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = window();
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
    let runtime = Runtime::new(state);
    let runtime_rc = Rc::new(RefCell::new(runtime));

    let input_handler = WebInputHandler::new(width, height, window.device_pixel_ratio() as f32);
    let input_handler_rc = Rc::new(RefCell::new(input_handler));

    let ui_wants_pointer = Rc::new(RefCell::new(false));
    let ui_wants_keyboard = Rc::new(RefCell::new(false));

    let listeners = setup_web_input_callbacks(
        &window,
        &canvas,
        runtime_rc.clone(),
        input_handler_rc.clone(),
        ui_wants_pointer.clone(),
        ui_wants_keyboard.clone(),
    );

    let runtime_clone = runtime_rc.clone();
    let input_handler_clone = input_handler_rc.clone();
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let ui_wants_keyboard_clone = ui_wants_keyboard.clone();

    let frame_handle = Rc::new(RefCell::new(None));
    request_frame(frame_handle, move |_time| {
        let _keep_listeners = &listeners;
        let mut runtime = runtime_clone.borrow_mut();

        let raw_input = input_handler_clone.borrow_mut().take_egui_input();
        let _full_output = runtime.run_frame(raw_input);

        *ui_wants_pointer_clone.borrow_mut() = runtime.pipeline.ctx().wants_pointer_input();
        *ui_wants_keyboard_clone.borrow_mut() = runtime.pipeline.ctx().wants_keyboard_input();
    });

    Ok(())
}
