use egui_winit::State as EguiWinitState;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Icon, Window, WindowId},
};

use crate::commands::InputEvent;
use crate::platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
use crate::platform::runtime::Runtime;
use crate::State;

struct App {
    runtime: Option<Runtime>,
    egui_state: Option<EguiWinitState>,
    last_cursor_pos: Option<PhysicalPosition<f64>>,
}

impl App {
    fn new() -> Self {
        Self {
            runtime: None,
            egui_state: None,
            last_cursor_pos: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runtime.is_none() {
            let icon = {
                let bytes = include_bytes!("../../assets/favicon.png");
                let image = image::load_from_memory(bytes).expect("Failed to load icon");
                let rgba = image.to_rgba8();
                let (width, height) = rgba.dimensions();
                Icon::from_rgba(rgba.into_raw(), width, height).ok()
            };

            let window_attributes = Window::default_attributes()
                .with_title("Line Dash")
                .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
                .with_window_icon(icon);
            let window = event_loop
                .create_window(window_attributes)
                .expect("Failed to create window");

            let state = pollster::block_on(State::new_native(window));
            let runtime = Runtime::new(state);

            let egui_state = EguiWinitState::new(
                runtime.pipeline.ctx().clone(),
                egui::ViewportId::ROOT,
                runtime.state.window(),
                Some(runtime.state.window().scale_factor() as f32),
                None,
                None,
            );

            self.runtime = Some(runtime);
            self.egui_state = Some(egui_state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let (runtime, egui_state) = match (self.runtime.as_mut(), self.egui_state.as_mut()) {
            (Some(r), Some(es)) => (r, es),
            _ => return,
        };

        let egui_consumed = egui_state
            .on_window_event(runtime.state.window(), &event)
            .consumed;

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
            } => {
                if !egui_consumed {
                    let pressed = element_state == winit::event::ElementState::Pressed;
                    let button_idx = mouse_button_index_from_winit(button);
                    runtime.state.process_input_event(InputEvent::MouseButton {
                        button: button_idx,
                        pressed,
                    });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !egui_consumed {
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
            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_consumed {
                    let zoom_delta = zoom_delta_from_winit(delta);
                    runtime
                        .state
                        .process_input_event(InputEvent::Zoom(zoom_delta));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if egui_consumed || runtime.pipeline.ctx().wants_keyboard_input() {
                    return;
                }

                let pressed = event.state == winit::event::ElementState::Pressed;
                let just_pressed = pressed && !event.repeat;

                let key_str = key_str_from_winit(&event.logical_key);
                runtime.state.process_input_event(InputEvent::Key {
                    key: key_str,
                    pressed,
                    just_pressed,
                });
            }
            WindowEvent::RedrawRequested => {
                let raw_input = egui_state.take_egui_input(runtime.state.window());
                let full_output = runtime.run_frame(raw_input);

                egui_state
                    .handle_platform_output(runtime.state.window(), full_output.platform_output);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(runtime) = &self.runtime {
            runtime.state.window().request_redraw();
        }
    }
}

pub fn run_native_app() {
    env_logger::init();
    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
