/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use egui_winit::State as EguiWinitState;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, Touch, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(not(target_arch = "wasm32"))]
use winit::window::Icon;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::*, JsCast};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};

use crate::commands::InputEvent;
use crate::platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
use crate::platform::input_routing::{should_route_keyboard_input, should_route_pointer_input};
use crate::platform::runtime::Runtime;
use crate::State;

struct App {
    runtime: Option<Runtime>,
    egui_state: Option<EguiWinitState>,
    window_id: Option<WindowId>,
    last_cursor_pos: Option<PhysicalPosition<f64>>,
    touch_points: HashMap<u64, PhysicalPosition<f64>>,
    pinch_last_distance: Option<f64>,
    primary_touch_id: Option<u64>,
    #[cfg(target_arch = "wasm32")]
    prebuilt_runtime: Option<Runtime>,
    #[cfg(target_arch = "wasm32")]
    web_canvas: Option<web_sys::HtmlCanvasElement>,
    #[cfg(target_arch = "wasm32")]
    web_window: Option<Window>,
}

#[derive(Clone, Copy)]
struct TouchPointEvent {
    id: u64,
    location: PhysicalPosition<f64>,
    phase: TouchPhase,
}

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    fn new_native() -> Self {
        Self {
            runtime: None,
            egui_state: None,
            window_id: None,
            last_cursor_pos: None,
            touch_points: HashMap::new(),
            pinch_last_distance: None,
            primary_touch_id: None,
            #[cfg(target_arch = "wasm32")]
            prebuilt_runtime: None,
            #[cfg(target_arch = "wasm32")]
            web_canvas: None,
            #[cfg(target_arch = "wasm32")]
            web_window: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn new_web(runtime: Runtime, canvas: web_sys::HtmlCanvasElement) -> Self {
        Self {
            runtime: None,
            egui_state: None,
            window_id: None,
            last_cursor_pos: None,
            touch_points: HashMap::new(),
            pinch_last_distance: None,
            primary_touch_id: None,
            prebuilt_runtime: Some(runtime),
            web_canvas: Some(canvas),
            web_window: None,
        }
    }

    fn handle_touch_event(
        touch_points: &mut HashMap<u64, PhysicalPosition<f64>>,
        pinch_last_distance: &mut Option<f64>,
        primary_touch_id: &mut Option<u64>,
        last_cursor_pos: &mut Option<PhysicalPosition<f64>>,
        egui_consumed: bool,
        runtime: &mut Runtime,
        touch: Touch,
    ) {
        let touch_event = TouchPointEvent {
            id: touch.id,
            location: touch.location,
            phase: touch.phase,
        };

        for event in Self::collect_touch_input_events(
            touch_points,
            pinch_last_distance,
            primary_touch_id,
            last_cursor_pos,
            egui_consumed,
            touch_event,
        ) {
            runtime.state.process_input_event(event);
        }
    }

    fn collect_touch_input_events(
        touch_points: &mut HashMap<u64, PhysicalPosition<f64>>,
        pinch_last_distance: &mut Option<f64>,
        primary_touch_id: &mut Option<u64>,
        last_cursor_pos: &mut Option<PhysicalPosition<f64>>,
        egui_consumed: bool,
        touch: TouchPointEvent,
    ) -> Vec<InputEvent> {
        let mut events = Vec::new();
        let is_routed = should_route_pointer_input(egui_consumed, false);
        let previous_count = touch_points.len();

        match touch.phase {
            TouchPhase::Started | TouchPhase::Moved => {
                touch_points.insert(touch.id, touch.location);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                touch_points.remove(&touch.id);
            }
        }

        let current_count = touch_points.len();

        // Cancel Primary Touch when a 2nd finger is added (1 -> >1 touches)
        if previous_count == 1
            && current_count > 1
            && touch.phase == TouchPhase::Started
            && primary_touch_id.is_some()
        {
            events.push(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });
            *primary_touch_id = None; // Cleared so lifting fingers doesn't trigger another release
            *last_cursor_pos = None;
        }

        if !is_routed {
            if touch_points.is_empty() {
                *pinch_last_distance = None;
                *primary_touch_id = None;
                *last_cursor_pos = None;
            }
            return events;
        }

        // Primary Touch Down
        if previous_count == 0 && current_count == 1 && touch.phase == TouchPhase::Started {
            *primary_touch_id = Some(touch.id);

            events.push(InputEvent::PointerMoved {
                x: touch.location.x,
                y: touch.location.y,
            });
            events.push(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            *last_cursor_pos = Some(touch.location);
        }

        // Primary Touch Move
        if current_count == 1
            && touch.phase == TouchPhase::Moved
            && *primary_touch_id == Some(touch.id)
        {
            events.push(InputEvent::PointerMoved {
                x: touch.location.x,
                y: touch.location.y,
            });

            if let Some(last) = *last_cursor_pos {
                events.push(InputEvent::CameraDrag {
                    dx: touch.location.x - last.x,
                    dy: touch.location.y - last.y,
                });
            }
            *last_cursor_pos = Some(touch.location);
        }

        // Primary Touch Up
        if (touch.phase == TouchPhase::Ended || touch.phase == TouchPhase::Cancelled)
            && *primary_touch_id == Some(touch.id)
        {
            events.push(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });
            *primary_touch_id = None;
            *last_cursor_pos = None;
        }

        if current_count == 2 {
            let mut touches = touch_points.values();
            if let (Some(first), Some(second)) = (touches.next(), touches.next()) {
                let dx = second.x - first.x;
                let dy = second.y - first.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if let Some(previous) = *pinch_last_distance {
                    let pinch_delta = ((distance - previous) * 0.04) as f32;
                    events.push(InputEvent::Zoom(pinch_delta));
                }

                *pinch_last_distance = Some(distance);
            }
        } else {
            *pinch_last_distance = None;
        }

        events
    }

    #[cfg(target_arch = "wasm32")]
    fn sync_web_canvas_size(runtime: &mut Runtime, window: &Window) {
        let browser_window = match web_sys::window() {
            Some(window) => window,
            None => return,
        };
        let width = browser_window
            .inner_width()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(800.0)
            .max(1.0) as u32;
        let height = browser_window
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(600.0)
            .max(1.0) as u32;

        let Some(canvas) = window.canvas() else {
            return;
        };

        if canvas.width() == width && canvas.height() == height {
            return;
        }

        let _ = canvas.set_attribute(
            "style",
            &format!("display:block;width:{width}px;height:{height}px;touch-action:none;"),
        );

        runtime
            .state
            .process_input_event(InputEvent::Resize { width, height });
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runtime.is_some() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::Arc;

            let icon = {
                let bytes = include_bytes!("../../assets/favicon.png");
                let image = image::load_from_memory(bytes).expect("Failed to load icon");
                let rgba = image.to_rgba8();
                let (width, height) = rgba.dimensions();
                Icon::from_rgba(rgba.into_raw(), width, height).ok()
            };

            let window_attributes = Window::default_attributes()
                .with_title("egakareta")
                .with_inner_size(LogicalSize::new(800.0, 600.0))
                .with_window_icon(icon);
            let window = event_loop
                .create_window(window_attributes)
                .expect("Failed to create window");

            let state = pollster::block_on(State::new_native(Arc::new(window)));
            let runtime = Runtime::new(state);
            let Some(window) = runtime.state.window() else {
                log::error!("Native runtime missing window handle after initialization");
                return;
            };

            let egui_state = EguiWinitState::new(
                runtime.pipeline.ctx().clone(),
                egui::ViewportId::ROOT,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                None,
                None,
            );

            self.window_id = Some(window.id());
            self.runtime = Some(runtime);
            self.egui_state = Some(egui_state);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let canvas = self
                .web_canvas
                .take()
                .expect("Missing prebuilt canvas for web runtime");

            let window_attributes = Window::default_attributes()
                .with_title("egakareta")
                .with_inner_size(LogicalSize::new(
                    canvas.width().max(1) as f64,
                    canvas.height().max(1) as f64,
                ))
                .with_canvas(Some(canvas))
                .with_prevent_default(true)
                .with_append(false);

            let window = event_loop
                .create_window(window_attributes)
                .expect("Failed to create web window");

            let runtime = self
                .prebuilt_runtime
                .take()
                .expect("Missing prebuilt runtime for web window");

            let egui_state = EguiWinitState::new(
                runtime.pipeline.ctx().clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                None,
                None,
            );

            self.window_id = Some(window.id());
            window.request_redraw();
            self.web_window = Some(window);
            self.runtime = Some(runtime);
            self.egui_state = Some(egui_state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let (runtime, egui_state) = match (self.runtime.as_mut(), self.egui_state.as_mut()) {
            (Some(r), Some(es)) => (r, es),
            _ => return,
        };

        if self.window_id != Some(window_id) {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let Some(window) = runtime.state.window().cloned() else {
            log::warn!("Dropping native window event because no window handle is available");
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        let egui_consumed = egui_state.on_window_event(window.as_ref(), &event).consumed;
        #[cfg(target_arch = "wasm32")]
        let egui_consumed = {
            let window = match self.web_window.as_ref() {
                Some(window) => window,
                None => return,
            };
            egui_state.on_window_event(window, &event).consumed
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                runtime.state.process_input_event(InputEvent::Resize {
                    width: physical_size.width,
                    height: physical_size.height,
                });
            }
            WindowEvent::MouseInput {
                button,
                state: element_state,
                ..
            } if should_route_pointer_input(egui_consumed, false) => {
                runtime.state.resume_audio();
                let pressed = element_state == ElementState::Pressed;
                let button_idx = mouse_button_index_from_winit(button);
                runtime.state.process_input_event(InputEvent::MouseButton {
                    button: button_idx,
                    pressed,
                });
            }
            WindowEvent::CursorMoved { position, .. } => {
                if should_route_pointer_input(egui_consumed, false) {
                    runtime.state.process_input_event(InputEvent::PointerMoved {
                        x: position.x,
                        y: position.y,
                    });

                    if let Some(last) = self.last_cursor_pos {
                        runtime.state.process_input_event(InputEvent::CameraDrag {
                            dx: position.x - last.x,
                            dy: position.y - last.y,
                        });
                    }
                }
                self.last_cursor_pos = Some(position);
            }
            WindowEvent::MouseWheel { delta, .. }
                if should_route_pointer_input(egui_consumed, false) =>
            {
                let zoom_delta = zoom_delta_from_winit(delta);
                runtime
                    .state
                    .process_input_event(InputEvent::Zoom(zoom_delta));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !should_route_keyboard_input(
                    egui_consumed,
                    runtime.pipeline.ctx().wants_keyboard_input(),
                ) {
                    return;
                }

                runtime.state.resume_audio();
                let pressed = event.state == ElementState::Pressed;
                let just_pressed = pressed && !event.repeat;

                let key_str = key_str_from_winit(&event.logical_key);
                runtime.state.process_input_event(InputEvent::Key {
                    key: key_str,
                    pressed,
                    just_pressed,
                });
            }
            WindowEvent::Touch(touch) => {
                runtime.state.resume_audio();
                Self::handle_touch_event(
                    &mut self.touch_points,
                    &mut self.pinch_last_distance,
                    &mut self.primary_touch_id,
                    &mut self.last_cursor_pos,
                    egui_consumed,
                    runtime,
                    touch,
                );
            }
            WindowEvent::RedrawRequested => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let raw_input = egui_state.take_egui_input(window.as_ref());
                    let full_output = runtime.run_frame(raw_input);
                    egui_state.handle_platform_output(window.as_ref(), full_output.platform_output);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    let window = match self.web_window.as_ref() {
                        Some(window) => window,
                        None => return,
                    };
                    Self::sync_web_canvas_size(runtime, window);
                    let raw_input = egui_state.take_egui_input(window);
                    let full_output = runtime.run_frame(raw_input);
                    egui_state.handle_platform_output(window, full_output.platform_output);
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.runtime.is_none() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(runtime) = &self.runtime {
            if let Some(window) = runtime.state.window() {
                window.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, TouchPointEvent};
    use crate::commands::InputEvent;
    use crate::test_utils::approx_eq;
    use std::collections::HashMap;
    use winit::{dpi::PhysicalPosition, event::TouchPhase};

    fn pos(x: f64, y: f64) -> PhysicalPosition<f64> {
        PhysicalPosition::new(x, y)
    }

    #[test]
    fn second_touch_cancels_primary_and_prevents_duplicate_release() {
        let mut touch_points = HashMap::new();
        let mut pinch_last_distance = None;
        let mut primary_touch_id = None;
        let mut last_cursor_pos = None;

        let start_events = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 1,
                location: pos(10.0, 20.0),
                phase: TouchPhase::Started,
            },
        );

        assert_eq!(start_events.len(), 2);
        assert!(matches!(
            start_events[0],
            InputEvent::PointerMoved { x: 10.0, y: 20.0 }
        ));
        assert!(matches!(
            start_events[1],
            InputEvent::MouseButton {
                button: 0,
                pressed: true
            }
        ));
        assert_eq!(primary_touch_id, Some(1));

        let second_touch_events = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 2,
                location: pos(40.0, 40.0),
                phase: TouchPhase::Started,
            },
        );

        assert_eq!(second_touch_events.len(), 1);
        assert!(matches!(
            second_touch_events[0],
            InputEvent::MouseButton {
                button: 0,
                pressed: false
            }
        ));
        assert_eq!(primary_touch_id, None);
        assert_eq!(last_cursor_pos, None);

        let end_primary_after_cancel_events = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 1,
                location: pos(10.0, 20.0),
                phase: TouchPhase::Ended,
            },
        );

        assert!(end_primary_after_cancel_events
            .iter()
            .all(|event| !matches!(event, InputEvent::MouseButton { .. })));
    }

    #[test]
    fn pinch_move_emits_zoom_delta() {
        let mut touch_points = HashMap::new();
        let mut pinch_last_distance = None;
        let mut primary_touch_id = None;
        let mut last_cursor_pos = None;

        let _ = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 1,
                location: pos(0.0, 0.0),
                phase: TouchPhase::Started,
            },
        );

        let _ = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 2,
                location: pos(0.0, 100.0),
                phase: TouchPhase::Started,
            },
        );

        let move_events = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            false,
            TouchPointEvent {
                id: 2,
                location: pos(0.0, 120.0),
                phase: TouchPhase::Moved,
            },
        );

        assert_eq!(move_events.len(), 1);
        let zoom_delta = match move_events[0] {
            InputEvent::Zoom(delta) => delta,
            _ => panic!("expected zoom input event"),
        };
        assert!(approx_eq(zoom_delta, 0.8, 0.0001));
    }

    #[test]
    fn egui_consumed_clears_touch_state_when_all_touches_end() {
        let mut touch_points = HashMap::new();
        touch_points.insert(7, pos(1.0, 2.0));

        let mut pinch_last_distance = Some(42.0);
        let mut primary_touch_id = Some(7);
        let mut last_cursor_pos = Some(pos(5.0, 6.0));

        let events = App::collect_touch_input_events(
            &mut touch_points,
            &mut pinch_last_distance,
            &mut primary_touch_id,
            &mut last_cursor_pos,
            true,
            TouchPointEvent {
                id: 7,
                location: pos(1.0, 2.0),
                phase: TouchPhase::Ended,
            },
        );

        assert!(events.is_empty());
        assert!(touch_points.is_empty());
        assert_eq!(pinch_last_distance, None);
        assert_eq!(primary_touch_id, None);
        assert_eq!(last_cursor_pos, None);
    }
}

fn log_envs() {
    log::info!("API_URL: {}", env!("API_URL"));
    log::info!("PUBLISHABLE_KEY: {}", env!("PUBLISHABLE_KEY"));
}

/// Runs the native application for desktop platforms.
/// Initializes the event loop, creates the window, and starts the game loop.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native_app() {
    env_logger::init();
    log_envs();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new_native();
    event_loop.run_app(&mut app).unwrap();
}

/// Runs the application in the web environment using the winit web event loop.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn run_game() -> Result<(), JsValue> {
    // Set up logging to browser console
    console_log::init_with_level(log::Level::Debug).expect("failed to init logger");
    console_error_panic_hook::set_once();
    log_envs();

    let browser_window = web_sys::window().ok_or_else(|| JsValue::from_str("Missing window"))?;
    let document = browser_window
        .document()
        .ok_or_else(|| JsValue::from_str("Missing document"))?;

    let canvas = match document.get_element_by_id("gameCanvas") {
        Some(existing) => existing.dyn_into::<web_sys::HtmlCanvasElement>()?,
        None => {
            let created = document
                .create_element("canvas")?
                .dyn_into::<web_sys::HtmlCanvasElement>()?;
            created.set_id("gameCanvas");
            let body = document
                .body()
                .ok_or_else(|| JsValue::from_str("Missing document body"))?;
            body.append_child(&created)?;
            created
        }
    };

    let width = browser_window
        .inner_width()?
        .as_f64()
        .unwrap_or(800.0)
        .max(1.0) as u32;
    let height = browser_window
        .inner_height()?
        .as_f64()
        .unwrap_or(600.0)
        .max(1.0) as u32;

    canvas.set_width(width);
    canvas.set_height(height);
    canvas.set_attribute(
        "style",
        "display:block;width:100vw;height:100vh;touch-action:none;",
    )?;

    let state = State::new(canvas.clone()).await;
    let runtime = Runtime::new(state);

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let app = App::new_web(runtime, canvas);
    event_loop.spawn_app(app);

    Ok(())
}
