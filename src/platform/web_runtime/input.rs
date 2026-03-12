use gloo_events::{EventListener, EventListenerOptions};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, Window};

use crate::commands::InputEvent;
use crate::platform::input_mapping::egui_key_from_key_str;
use crate::platform::input_routing::{should_route_keyboard_input, should_route_pointer_input};
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
) -> Vec<EventListener> {
    let mut listeners = Vec::new();

    // Resize
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        listeners.push(EventListener::new(
            window,
            "resize",
            move |_: &web_sys::Event| {
                let window = gloo_utils::window();
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
            },
        ));
    }

    // Mouse Down
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "mousedown",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::MouseEvent>();
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

                if should_route_pointer_input(false, *ui_wants_pointer.borrow()) {
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
            },
        ));
    }

    // Mouse Up
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "mouseup",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::MouseEvent>();
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
                    input.push_pointer_button(
                        x as f32,
                        y as f32,
                        egui::PointerButton::Primary,
                        false,
                    );
                }
                let mut runtime = runtime_rc.borrow_mut();
                runtime
                    .state
                    .process_input_event(InputEvent::PointerMoved { x, y });
                runtime.state.process_input_event(InputEvent::MouseButton {
                    button: button as u32,
                    pressed: false,
                });
            },
        ));
    }

    // Mouse Move
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "mousemove",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::MouseEvent>();
                let x = event.offset_x() as f64;
                let y = event.offset_y() as f64;
                input_handler_rc
                    .borrow_mut()
                    .push_pointer_move(x as f32, y as f32);

                if !should_route_pointer_input(false, *ui_wants_pointer.borrow()) {
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
            },
        ));
    }

    // Context Menu
    {
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "contextmenu",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::MouseEvent>();
                event.prevent_default();
            },
        ));
    }

    // Wheel
    {
        let runtime_rc = runtime_rc.clone();
        let ui_wants_pointer = ui_wants_pointer.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "wheel",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::WheelEvent>();
                if !should_route_pointer_input(false, *ui_wants_pointer.borrow()) {
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
            },
        ));
    }

    // Touch Start
    {
        let input_handler_rc = input_handler_rc.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "touchstart",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::TouchEvent>();
                if event.touches().length() == 2 {
                    let t0 = event.touches().item(0).unwrap();
                    let t1 = event.touches().item(1).unwrap();
                    let dx = (t1.client_x() - t0.client_x()) as f64;
                    let dy = (t1.client_y() - t0.client_y()) as f64;
                    input_handler_rc.borrow_mut().pinch_last_distance =
                        Some((dx * dx + dy * dy).sqrt());
                    event.prevent_default();
                }
            },
        ));
    }

    // Touch Move
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let options = EventListenerOptions::enable_prevent_default();
        listeners.push(EventListener::new_with_options(
            canvas,
            "touchmove",
            options,
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::TouchEvent>();
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
            },
        ));
    }

    // Touch End/Cancel
    {
        let input_handler_rc_1 = input_handler_rc.clone();
        listeners.push(EventListener::new(
            canvas,
            "touchend",
            move |_: &web_sys::Event| {
                input_handler_rc_1.borrow_mut().pinch_last_distance = None;
            },
        ));

        let input_handler_rc_2 = input_handler_rc.clone();
        listeners.push(EventListener::new(
            canvas,
            "touchcancel",
            move |_: &web_sys::Event| {
                input_handler_rc_2.borrow_mut().pinch_last_distance = None;
            },
        ));
    }

    // Key Down
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_keyboard = ui_wants_keyboard.clone();
        listeners.push(EventListener::new(
            window,
            "keydown",
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::KeyboardEvent>();
                let key = event.key();

                // Intercept media keys to prevent default browser behavior (unexpected plays/pauses)
                match key.as_str() {
                    "MediaPlayPause" | "MediaPlay" | "MediaPause" | "MediaStop"
                    | "MediaTrackNext" | "MediaTrackPrevious" | "MediaNextTrack"
                    | "MediaPreviousTrack" => {
                        event.prevent_default();
                    }
                    _ => {}
                }

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

                if should_route_keyboard_input(false, *ui_wants_keyboard.borrow()) {
                    let mut runtime = runtime_rc.borrow_mut();
                    runtime.state.process_input_event(InputEvent::Key {
                        key: key.clone(),
                        pressed: true,
                        just_pressed: !event.repeat(),
                    });
                }
            },
        ));
    }

    // Key Up
    {
        let runtime_rc = runtime_rc.clone();
        let input_handler_rc = input_handler_rc.clone();
        let ui_wants_keyboard = ui_wants_keyboard.clone();
        listeners.push(EventListener::new(
            window,
            "keyup",
            move |event: &web_sys::Event| {
                let event = event.unchecked_ref::<web_sys::KeyboardEvent>();
                let key = event.key();

                // Intercept media keys to prevent default browser behavior (unexpected plays/pauses)
                match key.as_str() {
                    "MediaPlayPause" | "MediaPlay" | "MediaPause" | "MediaStop"
                    | "MediaTrackNext" | "MediaTrackPrevious" | "MediaNextTrack"
                    | "MediaPreviousTrack" => {
                        event.prevent_default();
                    }
                    _ => {}
                }

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

                if should_route_keyboard_input(false, *ui_wants_keyboard.borrow()) {
                    let mut runtime = runtime_rc.borrow_mut();
                    runtime.state.process_input_event(InputEvent::Key {
                        key: key.clone(),
                        pressed: false,
                        just_pressed: false,
                    });
                }
            },
        ));
    }

    listeners
}
