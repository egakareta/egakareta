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
    let closure = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
        state_clone.borrow_mut().turn_right();
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        if event.repeat() {
            return;
        }
        match event.key().as_str() {
            "ArrowUp" | " " => state_clone.borrow_mut().turn_right(),
            "ArrowRight" => state_clone.borrow_mut().next_level(),
            "ArrowLeft" => state_clone.borrow_mut().prev_level(),
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
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
