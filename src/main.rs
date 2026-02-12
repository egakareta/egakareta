#[cfg(not(target_arch = "wasm32"))]
use line_dash_lib::State;
#[cfg(not(target_arch = "wasm32"))]
use winit::{
    dpi::PhysicalPosition,
    event::{Event, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Line Dash")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new_native(window));
    let mut last_cursor_pos: Option<PhysicalPosition<f64>> = None;

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::MouseInput { button: winit::event::MouseButton::Left, state: winit::event::ElementState::Pressed, .. } => {
                        state.turn_right();
                    }
                    WindowEvent::MouseInput { button: winit::event::MouseButton::Right, state: winit::event::ElementState::Pressed, .. } => {
                        state.set_editor_right_dragging(true);
                    }
                    WindowEvent::MouseInput { button: winit::event::MouseButton::Right, state: winit::event::ElementState::Released, .. } => {
                        state.set_editor_right_dragging(false);
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        if let Some(last) = last_cursor_pos {
                            state.drag_editor_camera_by_pixels(position.x - last.x, position.y - last.y);
                        }
                        state.update_editor_cursor_from_screen(position.x, position.y);
                        last_cursor_pos = Some(*position);
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let zoom_delta = match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                        };
                        state.adjust_editor_zoom(zoom_delta);
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        use winit::event::ElementState;
                        use winit::keyboard::{Key, NamedKey};

                        let pressed = event.state == ElementState::Pressed;
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
                            _ => {}
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        state.update();
                        match state.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => state.recreate_surface(),
                            Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                            Err(e) => eprintln!("{:?}", e),
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                state.window().request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {}