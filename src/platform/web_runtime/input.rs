use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, Window};

use crate::commands::InputEvent;
use crate::platform::input_mapping::egui_key_from_key_str;
use crate::platform::runtime::Runtime;

#[derive(Default)]
pub struct WebInputHandler {
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    width: u32,
    height: u32,
    pixels_per_point: f32,
    pinch_last_distance: Option<f64>,
}

impl WebInputHandler {
    pub fn new(width: u32, height: u32, pixels_per_point: f32) -> Self {
        Self {
            width,
            height,
            pixels_per_point: pixels_per_point.max(0.1),
            ..Default::default()
        }
    }

    pub fn set_screen(&mut self, width: u32, height: u32, pixels_per_point: f32) {
        self.width = width;
        self.height = height;
        self.pixels_per_point = pixels_per_point.max(0.1);
    }

    pub fn push_pointer_move(&mut self, x: f32, y: f32) {
        self.events
            .push(egui::Event::PointerMoved(egui::Pos2::new(x, y)));
    }

    pub fn push_pointer_button(
        &mut self,
        x: f32,
        y: f32,
        button: egui::PointerButton,
        pressed: bool,
    ) {
        self.events.push(egui::Event::PointerButton {
            pos: egui::Pos2::new(x, y),
            button,
            pressed,
            modifiers: self.modifiers,
        });
    }

    pub fn take_egui_input(&mut self) -> egui::RawInput {
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

pub fn setup_web_input_callbacks(
    window: &Window,
    canvas: &HtmlCanvasElement,
    runtime_rc: Rc<RefCell<Runtime>>,
    input_handler_rc: Rc<RefCell<WebInputHandler>>,
    ui_wants_pointer: Rc<RefCell<bool>>,
    ui_wants_keyboard: Rc<RefCell<bool>>,
) {
    // Resize
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let window = web_sys::window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
            runtime_rc
                .borrow_mut()
                .state
                .process_input_event(InputEvent::Resize { width, height });
            input_handler_rc.borrow_mut().set_screen(
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

    // Mouse Down
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f64;
            let y = event.offset_y() as f64;
            let mut input = input_handler_rc.borrow_mut();
            input.push_pointer_move(x as f32, y as f32);

            let mut runtime = runtime_rc.borrow_mut();
            let button = event.button();
            match button {
                0 => input.push_pointer_button(
                    x as f32,
                    y as f32,
                    egui::PointerButton::Primary,
                    true,
                ),
                2 => input.push_pointer_button(
                    x as f32,
                    y as f32,
                    egui::PointerButton::Secondary,
                    true,
                ),
                _ => {}
            }

            if !*ui_wants_pointer.borrow() {
                runtime
                    .state
                    .process_input_event(InputEvent::PointerMoved { x, y });
                runtime.state.process_input_event(InputEvent::MouseButton {
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
    }

    // Mouse Up
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f64;
            let y = event.offset_y() as f64;
            let mut input = input_handler_rc.borrow_mut();
            input.push_pointer_move(x as f32, y as f32);
            let button = event.button();
            if button == 2 {
                input.push_pointer_button(
                    x as f32,
                    y as f32,
                    egui::PointerButton::Secondary,
                    false,
                );
                event.prevent_default();
            } else if button == 0 {
                input.push_pointer_button(x as f32, y as f32, egui::PointerButton::Primary, false);
            }
            let mut runtime = runtime_rc.borrow_mut();
            runtime
                .state
                .process_input_event(InputEvent::PointerMoved { x, y });
            runtime.state.process_input_event(InputEvent::MouseButton {
                button: button as u32,
                pressed: false,
            });
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse Move
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f64;
            let y = event.offset_y() as f64;
            input_handler_rc
                .borrow_mut()
                .push_pointer_move(x as f32, y as f32);

            if *ui_wants_pointer.borrow() {
                return;
            }

            let mut runtime = runtime_rc.borrow_mut();
            runtime
                .state
                .process_input_event(InputEvent::PointerMoved { x, y });

            if (event.buttons() & 2) != 0 {
                runtime.state.process_input_event(InputEvent::CameraDrag {
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
    }

    // Context Menu
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            event.prevent_default();
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Wheel
    {
        let runtime_rc = runtime_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
            if *ui_wants_pointer.borrow() {
                event.prevent_default();
                return;
            }

            let scale = match event.delta_mode() {
                1 => 0.2,
                2 => 1.0,
                _ => 0.01,
            };
            runtime_rc
                .borrow_mut()
                .state
                .process_input_event(InputEvent::Zoom((-event.delta_y() * scale) as f32));
            event.prevent_default();
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Touch Start
    {
        let input_handler_rc = input_handler_rc.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
            if event.touches().length() == 2 {
                let t0 = event.touches().item(0).unwrap();
                let t1 = event.touches().item(1).unwrap();
                let dx = (t1.client_x() - t0.client_x()) as f64;
                let dy = (t1.client_y() - t0.client_y()) as f64;
                input_handler_rc.borrow_mut().pinch_last_distance =
                    Some((dx * dx + dy * dy).sqrt());
                event.prevent_default();
            }
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("touchstart", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Touch Move
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
            if event.touches().length() == 2 {
                let t0 = event.touches().item(0).unwrap();
                let t1 = event.touches().item(1).unwrap();
                let dx = (t1.client_x() - t0.client_x()) as f64;
                let dy = (t1.client_y() - t0.client_y()) as f64;
                let distance = (dx * dx + dy * dy).sqrt();

                let mut input = input_handler_rc.borrow_mut();
                if let Some(previous) = input.pinch_last_distance {
                    let pinch_delta = ((distance - previous) * 0.04) as f32;
                    runtime_rc
                        .borrow_mut()
                        .state
                        .process_input_event(InputEvent::Zoom(pinch_delta));
                }

                input.pinch_last_distance = Some(distance);
                event.prevent_default();
            }
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("touchmove", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Touch End/Cancel
    {
        let input_handler_rc = input_handler_rc.clone();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::TouchEvent| {
            input_handler_rc.borrow_mut().pinch_last_distance = None;
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("touchend", closure.as_ref().unchecked_ref())
            .unwrap();
        canvas
            .add_event_listener_with_callback("touchcancel", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Key Down
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_keyboard = ui_wants_keyboard.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            let key = event.key();
            let mut input = input_handler_rc.borrow_mut();
            if key == "Shift" {
                input.modifiers.shift = true;
            }

            if key.chars().count() == 1 {
                input.events.push(egui::Event::Text(key.clone()));
            }

            let egui_key = egui_key_from_key_str(&key);

            if let Some(k) = egui_key {
                let modifiers = input.modifiers;
                input.events.push(egui::Event::Key {
                    key: k,
                    physical_key: None,
                    pressed: true,
                    repeat: event.repeat(),
                    modifiers,
                });
            }

            if !*ui_wants_keyboard.borrow() {
                let mut runtime = runtime_rc.borrow_mut();
                runtime.state.process_input_event(InputEvent::Key {
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
    }

    // Key Up
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_keyboard = ui_wants_keyboard.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            let key = event.key();
            let mut input = input_handler_rc.borrow_mut();
            if key == "Shift" {
                input.modifiers.shift = false;
            }

            let egui_key = egui_key_from_key_str(&key);

            if let Some(k) = egui_key {
                let modifiers = input.modifiers;
                input.events.push(egui::Event::Key {
                    key: k,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers,
                });
            }

            if !*ui_wants_keyboard.borrow() {
                let mut runtime = runtime_rc.borrow_mut();
                runtime.state.process_input_event(InputEvent::Key {
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
    }
}
