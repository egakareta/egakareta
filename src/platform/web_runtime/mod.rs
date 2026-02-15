use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::platform::runtime::Runtime;
use crate::State;

mod input;
use input::{setup_web_input_callbacks, WebInputHandler};

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
    let runtime = Runtime::new(state);
    let runtime_rc = Rc::new(RefCell::new(runtime));

    let input_handler = WebInputHandler::new(width, height, window.device_pixel_ratio() as f32);
    let input_handler_rc = Rc::new(RefCell::new(input_handler));

    let ui_wants_pointer = Rc::new(RefCell::new(false));
    let ui_wants_keyboard = Rc::new(RefCell::new(false));

    setup_web_input_callbacks(
        &window,
        &canvas,
        runtime_rc.clone(),
        input_handler_rc.clone(),
        ui_wants_pointer.clone(),
        ui_wants_keyboard.clone(),
    );

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let runtime_clone = runtime_rc.clone();
    let input_handler_clone = input_handler_rc.clone();
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let ui_wants_keyboard_clone = ui_wants_keyboard.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut runtime = runtime_clone.borrow_mut();

        let raw_input = input_handler_clone.borrow_mut().take_egui_input();
        let _full_output = runtime.run_frame(raw_input);

        *ui_wants_pointer_clone.borrow_mut() = runtime.pipeline.ctx().wants_pointer_input();
        *ui_wants_keyboard_clone.borrow_mut() = runtime.pipeline.ctx().wants_keyboard_input();

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
