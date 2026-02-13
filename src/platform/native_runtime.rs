use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use egui_winit::State as EguiWinitState;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
use crate::types::PhysicalSize;
use crate::{show_editor_ui, State};

struct App {
    state: Option<State>,
    egui_state: Option<EguiWinitState>,
    egui_renderer: Option<EguiRenderer>,
    egui_ctx: egui::Context,
    last_cursor_pos: Option<PhysicalPosition<f64>>,
}

impl App {
    fn new() -> Self {
        Self {
            state: None,
            egui_state: None,
            egui_renderer: None,
            egui_ctx: egui::Context::default(),
            last_cursor_pos: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Line Dash")
                .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));
            let window = event_loop
                .create_window(window_attributes)
                .expect("Failed to create window");

            let state = pollster::block_on(State::new_native(window));
            let egui_state = EguiWinitState::new(
                self.egui_ctx.clone(),
                egui::ViewportId::ROOT,
                state.window(),
                Some(state.window().scale_factor() as f32),
                None,
                None,
            );
            let egui_renderer = state.create_egui_renderer();

            self.state = Some(state);
            self.egui_state = Some(egui_state);
            self.egui_renderer = Some(egui_renderer);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let (state, egui_state, egui_renderer) = match (
            self.state.as_mut(),
            self.egui_state.as_mut(),
            self.egui_renderer.as_mut(),
        ) {
            (Some(s), Some(es), Some(er)) => (s, es, er),
            _ => return,
        };

        let egui_consumed = egui_state.on_window_event(state.window(), &event).consumed;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                state.resize_surface(PhysicalSize::new(physical_size.width, physical_size.height));
            }
            WindowEvent::MouseInput {
                button,
                state: element_state,
                ..
            } => {
                if !egui_consumed {
                    let pressed = element_state == winit::event::ElementState::Pressed;
                    let button_idx = mouse_button_index_from_winit(button);
                    state.handle_mouse_button(button_idx, pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !egui_consumed {
                    if let Some(last) = self.last_cursor_pos {
                        state
                            .drag_editor_camera_by_pixels(position.x - last.x, position.y - last.y);
                    }
                    state.update_editor_cursor_from_screen(position.x, position.y);
                }
                self.last_cursor_pos = Some(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_consumed {
                    let zoom_delta = zoom_delta_from_winit(delta);
                    state.adjust_editor_zoom(zoom_delta);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if egui_consumed || self.egui_ctx.wants_keyboard_input() {
                    return;
                }

                let pressed = event.state == winit::event::ElementState::Pressed;
                let just_pressed = pressed && !event.repeat;

                let key_str = key_str_from_winit(&event.logical_key);
                state.handle_keyboard_input(&key_str, pressed, just_pressed);
            }
            WindowEvent::RedrawRequested => {
                let raw_input = egui_state.take_egui_input(state.window());
                let full_output = self.egui_ctx.run(raw_input, |ctx| {
                    show_editor_ui(ctx, state);
                });

                egui_state.handle_platform_output(state.window(), full_output.platform_output);

                let paint_jobs = self
                    .egui_ctx
                    .tessellate(full_output.shapes, full_output.pixels_per_point);
                let window_size = state.window().inner_size();
                let screen_descriptor = ScreenDescriptor {
                    size_in_pixels: [window_size.width, window_size.height],
                    pixels_per_point: full_output.pixels_per_point,
                };

                for (id, image_delta) in &full_output.textures_delta.set {
                    egui_renderer.update_texture(state.device(), state.queue(), *id, image_delta);
                }

                state.update();
                match state.render_egui(egui_renderer, &paint_jobs, &screen_descriptor) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.recreate_surface(),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(err) => eprintln!("{:?}", err),
                }

                for id in &full_output.textures_delta.free {
                    egui_renderer.free_texture(id);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window().request_redraw();
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
