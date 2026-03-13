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
    #[cfg(target_arch = "wasm32")]
    prebuilt_runtime: Option<Runtime>,
    #[cfg(target_arch = "wasm32")]
    web_canvas: Option<web_sys::HtmlCanvasElement>,
    #[cfg(target_arch = "wasm32")]
    web_window: Option<Window>,
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
            prebuilt_runtime: Some(runtime),
            web_canvas: Some(canvas),
            web_window: None,
        }
    }

    fn handle_touch_event(
        touch_points: &mut HashMap<u64, PhysicalPosition<f64>>,
        pinch_last_distance: &mut Option<f64>,
        runtime: &mut Runtime,
        touch: Touch,
    ) {
        match touch.phase {
            TouchPhase::Started | TouchPhase::Moved => {
                touch_points.insert(touch.id, touch.location);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                touch_points.remove(&touch.id);
            }
        }

        if touch_points.len() == 2 {
            let mut touches = touch_points.values();
            if let (Some(first), Some(second)) = (touches.next(), touches.next()) {
                let dx = second.x - first.x;
                let dy = second.y - first.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if let Some(previous) = *pinch_last_distance {
                    let pinch_delta = ((distance - previous) * 0.04) as f32;
                    runtime
                        .state
                        .process_input_event(InputEvent::Zoom(pinch_delta));
                }

                *pinch_last_distance = Some(distance);
            }
        } else {
            *pinch_last_distance = None;
        }
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

            let state = pollster::block_on(State::new_native(window));
            let runtime = Runtime::new(state);
            let window = runtime.state.window();

            let egui_state = EguiWinitState::new(
                runtime.pipeline.ctx().clone(),
                egui::ViewportId::ROOT,
                window,
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
        let egui_consumed = egui_state
            .on_window_event(runtime.state.window(), &event)
            .consumed;
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
                    runtime,
                    touch,
                );
            }
            WindowEvent::RedrawRequested => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let raw_input = egui_state.take_egui_input(runtime.state.window());
                    let full_output = runtime.run_frame(raw_input);
                    egui_state.handle_platform_output(
                        runtime.state.window(),
                        full_output.platform_output,
                    );
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
            runtime.state.window().request_redraw();
        }
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
