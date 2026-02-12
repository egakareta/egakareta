#[cfg(not(target_arch = "wasm32"))]
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
#[cfg(not(target_arch = "wasm32"))]
use egui_winit::State as EguiWinitState;
#[cfg(not(target_arch = "wasm32"))]
use line_dash_lib::{show_editor_ui, BlockKind, State};
#[cfg(not(target_arch = "wasm32"))]
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(not(target_arch = "wasm32"))]
struct App {
    state: Option<State>,
    egui_state: Option<EguiWinitState>,
    egui_renderer: Option<EguiRenderer>,
    egui_ctx: egui::Context,
    last_cursor_pos: Option<PhysicalPosition<f64>>,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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
            let egui_renderer =
                EguiRenderer::new(state.device(), state.surface_format(), None, 1, false);

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
                state.resize(physical_size);
            }
            WindowEvent::MouseInput {
                button: winit::event::MouseButton::Left,
                state: winit::event::ElementState::Pressed,
                ..
            } => {
                if !egui_consumed {
                    state.turn_right();
                }
            }
            WindowEvent::MouseInput {
                button: winit::event::MouseButton::Right,
                state: winit::event::ElementState::Pressed,
                ..
            } => {
                if !egui_consumed {
                    state.set_editor_right_dragging(true);
                }
            }
            WindowEvent::MouseInput {
                button: winit::event::MouseButton::Right,
                state: winit::event::ElementState::Released,
                ..
            } => {
                state.set_editor_right_dragging(false);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !egui_consumed {
                    if let Some(last) = self.last_cursor_pos {
                        state.drag_editor_camera_by_pixels(
                            position.x - last.x,
                            position.y - last.y,
                        );
                    }
                    state.update_editor_cursor_from_screen(position.x, position.y);
                }
                self.last_cursor_pos = Some(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_consumed {
                    let zoom_delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                    };
                    state.adjust_editor_zoom(zoom_delta);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if egui_consumed {
                    return;
                }

                let pressed = event.state == winit::event::ElementState::Pressed;
                let just_pressed = pressed && !event.repeat;

                match &event.logical_key {
                    Key::Named(NamedKey::ArrowUp) => {
                        if state.is_editor() {
                            state.set_editor_pan_up_held(pressed);
                        } else if just_pressed {
                            state.turn_right();
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if state.is_editor() {
                            state.set_editor_pan_down_held(pressed);
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if state.is_editor() {
                            state.set_editor_pan_right_held(pressed);
                        } else if just_pressed {
                            state.next_level();
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if state.is_editor() {
                            state.set_editor_pan_left_held(pressed);
                        } else if just_pressed {
                            state.prev_level();
                        }
                    }
                    Key::Named(NamedKey::Space) => {
                        if just_pressed {
                            state.turn_right();
                        }
                    }
                    Key::Named(NamedKey::Enter) => {
                        if just_pressed {
                            state.editor_playtest();
                        }
                    }
                    Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Delete) => {
                        if just_pressed {
                            state.editor_remove_block();
                        }
                    }
                    Key::Named(NamedKey::Escape) => {
                        if just_pressed {
                            state.back_to_menu();
                        }
                    }
                    Key::Named(NamedKey::Shift) => {
                        state.set_editor_shift_held(pressed);
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("w") => {
                        if state.is_editor() {
                            state.set_editor_pan_up_held(pressed);
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                        if state.is_editor() {
                            state.set_editor_pan_down_held(pressed);
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                        if state.is_editor() {
                            state.set_editor_pan_right_held(pressed);
                        } else if just_pressed {
                            state.next_level();
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                        if state.is_editor() {
                            state.set_editor_pan_left_held(pressed);
                        } else if just_pressed {
                            state.prev_level();
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                        if just_pressed {
                            state.toggle_editor();
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("p") => {
                        if just_pressed {
                            state.editor_set_spawn_here();
                        }
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                        if just_pressed {
                            state.editor_rotate_spawn_direction();
                        }
                    }
                    Key::Character(c) if c == "+" || c == "=" => {
                        if just_pressed {
                            state.adjust_editor_zoom(1.0);
                        }
                    }
                    Key::Character(c) if c == "-" || c == "_" => {
                        if just_pressed {
                            state.adjust_editor_zoom(-1.0);
                        }
                    }
                    Key::Character(c) if c == "1" => {
                        if state.is_editor() && just_pressed {
                            state.set_editor_block_kind(BlockKind::Standard);
                        }
                    }
                    Key::Character(c) if c == "2" => {
                        if state.is_editor() && just_pressed {
                            state.set_editor_block_kind(BlockKind::Grass);
                        }
                    }
                    Key::Character(c) if c == "3" => {
                        if state.is_editor() && just_pressed {
                            state.set_editor_block_kind(BlockKind::Dirt);
                        }
                    }
                    _ => {}
                }
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
                match state.render_with_overlay(|device, queue, view, encoder| {
                    egui_renderer.update_buffers(
                        device,
                        queue,
                        encoder,
                        &paint_jobs,
                        &screen_descriptor,
                    );

                    let mut pass = encoder
                        .begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("egui_render_pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        })
                        .forget_lifetime();

                    let _ = egui_renderer.render(&mut pass, &paint_jobs, &screen_descriptor);
                }) {
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
