use egui_wgpu::ScreenDescriptor;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;
use wgpu::SurfaceError;

use crate::commands::InputEvent;
use crate::platform::input_mapping::egui_key_from_key_str;
use crate::{load_menu_wordmark_texture, show_editor_ui, show_menu_wordmark_ui, State};

#[derive(Default)]
struct WebUiInput {
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    width: u32,
    height: u32,
    pixels_per_point: f32,
}

impl WebUiInput {
    fn set_screen(&mut self, width: u32, height: u32, pixels_per_point: f32) {
        self.width = width;
        self.height = height;
        self.pixels_per_point = pixels_per_point.max(0.1);
    }

    fn push_pointer_move(&mut self, x: f32, y: f32) {
        self.events
            .push(egui::Event::PointerMoved(egui::Pos2::new(x, y)));
    }

    fn push_pointer_button(&mut self, x: f32, y: f32, button: egui::PointerButton, pressed: bool) {
        self.events.push(egui::Event::PointerButton {
            pos: egui::Pos2::new(x, y),
            button,
            pressed,
            modifiers: self.modifiers,
        });
    }

    fn take(&mut self) -> egui::RawInput {
        let size_points = egui::Vec2::new(
            self.width as f32 / self.pixels_per_point,
            self.height as f32 / self.pixels_per_point,
        );
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(self.pixels_per_point),
                ..Default::default()
            },
        );
        egui::RawInput {
            viewport_id: egui::ViewportId::ROOT,
            viewports,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size_points)),
            modifiers: self.modifiers,
            events: std::mem::take(&mut self.events),
            ..Default::default()
        }
    }
}

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
    let ui_ctx = egui::Context::default();
    let mut initial_ui_input = WebUiInput::default();
    initial_ui_input.set_screen(width, height, window.device_pixel_ratio() as f32);
    let ui_input_rc = Rc::new(RefCell::new(initial_ui_input));
    let ui_wants_pointer = Rc::new(RefCell::new(false));
    let ui_wants_keyboard = Rc::new(RefCell::new(false));
    let ui_renderer = state_rc.borrow().create_egui_renderer();
    let ui_renderer_rc = Rc::new(RefCell::new(ui_renderer));
    let menu_wordmark = Rc::new(load_menu_wordmark_texture(&ui_ctx));

    {
        let state_clone = state_rc.clone();
        let ui_input_clone = ui_input_rc.clone();
        let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let window = web_sys::window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
            state_clone
                .borrow_mut()
                .process_input_event(InputEvent::Resize { width, height });
            ui_input_clone.borrow_mut().set_screen(
                width,
                height,
                window.device_pixel_ratio() as f32,
            );
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    let state_clone = state_rc.clone();
    let ui_input_clone = ui_input_rc.clone();
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let x = event.offset_x() as f64;
        let y = event.offset_y() as f64;
        let mut ui_input = ui_input_clone.borrow_mut();
        ui_input.push_pointer_move(x as f32, y as f32);

        let mut state = state_clone.borrow_mut();
        let button = event.button();
        match button {
            0 => {
                ui_input.push_pointer_button(x as f32, y as f32, egui::PointerButton::Primary, true)
            }
            2 => ui_input.push_pointer_button(
                x as f32,
                y as f32,
                egui::PointerButton::Secondary,
                true,
            ),
            _ => {}
        }

        if !*ui_wants_pointer_clone.borrow() {
            state.process_input_event(InputEvent::PointerMoved { x, y });
            state.process_input_event(InputEvent::MouseButton {
                button: button as u32,
                pressed: true,
            });
        }

        if button == 2 {
            event.prevent_default();
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let ui_input_clone = ui_input_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let x = event.offset_x() as f64;
        let y = event.offset_y() as f64;
        let mut ui_input = ui_input_clone.borrow_mut();
        ui_input.push_pointer_move(x as f32, y as f32);
        let button = event.button();
        if button == 2 {
            ui_input.push_pointer_button(x as f32, y as f32, egui::PointerButton::Secondary, false);
            event.prevent_default();
        } else if button == 0 {
            ui_input.push_pointer_button(x as f32, y as f32, egui::PointerButton::Primary, false);
        }
        let mut state = state_clone.borrow_mut();
        state.process_input_event(InputEvent::PointerMoved { x, y });
        state.process_input_event(InputEvent::MouseButton {
            button: button as u32,
            pressed: false,
        });
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let ui_input_clone = ui_input_rc.clone();
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let x = event.offset_x() as f64;
        let y = event.offset_y() as f64;
        ui_input_clone
            .borrow_mut()
            .push_pointer_move(x as f32, y as f32);

        if *ui_wants_pointer_clone.borrow() {
            return;
        }

        let mut state = state_clone.borrow_mut();
        state.process_input_event(InputEvent::PointerMoved { x, y });

        if (event.buttons() & 2) != 0 {
            state.process_input_event(InputEvent::CameraDrag {
                dx: event.movement_x() as f64,
                dy: event.movement_y() as f64,
            });
            event.prevent_default();
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
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
        if *ui_wants_pointer_clone.borrow() {
            event.prevent_default();
            return;
        }

        let scale = match event.delta_mode() {
            1 => 0.2,
            2 => 1.0,
            _ => 0.01,
        };
        state_clone
            .borrow_mut()
            .process_input_event(InputEvent::Zoom((-event.delta_y() * scale) as f32));
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
                state_clone
                    .borrow_mut()
                    .process_input_event(InputEvent::Zoom(pinch_delta));
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
    let ui_input_clone = ui_input_rc.clone();
    let ui_wants_keyboard_clone = ui_wants_keyboard.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        let mut ui_input = ui_input_clone.borrow_mut();
        if key == "Shift" {
            ui_input.modifiers.shift = true;
        }

        if key.chars().count() == 1 {
            ui_input.events.push(egui::Event::Text(key.clone()));
        }

        let egui_key = egui_key_from_key_str(&key);

        if let Some(k) = egui_key {
            let modifiers = ui_input.modifiers;
            ui_input.events.push(egui::Event::Key {
                key: k,
                physical_key: None,
                pressed: true,
                repeat: event.repeat(),
                modifiers,
            });
        }

        if !*ui_wants_keyboard_clone.borrow() {
            let mut state = state_clone.borrow_mut();
            state.process_input_event(InputEvent::Key {
                key: key.clone(),
                pressed: true,
                just_pressed: !event.repeat(),
            });
        }
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let ui_input_clone = ui_input_rc.clone();
    let ui_wants_keyboard_clone = ui_wants_keyboard.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        let mut ui_input = ui_input_clone.borrow_mut();
        if key == "Shift" {
            ui_input.modifiers.shift = false;
        }

        let egui_key = egui_key_from_key_str(&key);

        if let Some(k) = egui_key {
            let modifiers = ui_input.modifiers;
            ui_input.events.push(egui::Event::Key {
                key: k,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            });
        }

        if !*ui_wants_keyboard_clone.borrow() {
            let mut state = state_clone.borrow_mut();
            state.process_input_event(InputEvent::Key {
                key: key.clone(),
                pressed: false,
                just_pressed: false,
            });
        }
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let state_clone = state_rc.clone();
    let ui_ctx_clone = ui_ctx.clone();
    let ui_input_clone = ui_input_rc.clone();
    let ui_renderer_clone = ui_renderer_rc.clone();
    let menu_wordmark_clone = menu_wordmark.clone();
    let ui_wants_pointer_clone = ui_wants_pointer.clone();
    let ui_wants_keyboard_clone = ui_wants_keyboard.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut state = state_clone.borrow_mut();

        let raw_input = ui_input_clone.borrow_mut().take();
        let full_output = ui_ctx_clone.run(raw_input, |ctx| {
            show_editor_ui(ctx, &mut state);
            if let Some(wordmark) = menu_wordmark_clone.as_ref() {
                show_menu_wordmark_ui(ctx, &state, wordmark);
            }
        });

        *ui_wants_pointer_clone.borrow_mut() = ui_ctx_clone.wants_pointer_input();
        *ui_wants_keyboard_clone.borrow_mut() = ui_ctx_clone.wants_keyboard_input();

        let paint_jobs = ui_ctx_clone.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [state.surface_width(), state.surface_height()],
            pixels_per_point: full_output.pixels_per_point,
        };

        {
            let mut renderer = ui_renderer_clone.borrow_mut();
            for (id, image_delta) in &full_output.textures_delta.set {
                renderer.update_texture(state.device(), state.queue(), *id, image_delta);
            }
        }

        state.update();
        match state.render_egui(
            &mut ui_renderer_clone.borrow_mut(),
            &paint_jobs,
            &screen_descriptor,
        ) {
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

        {
            let mut renderer = ui_renderer_clone.borrow_mut();
            for id in &full_output.textures_delta.free {
                renderer.free_texture(id);
            }
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
