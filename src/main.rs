use line_dash_lib::State;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Line Dash")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new_native(window));

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
                    WindowEvent::KeyboardInput {
                        event: winit::event::KeyEvent {
                            logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp),
                            state: winit::event::ElementState::Pressed,
                            repeat: false,
                            ..
                        },
                        ..
                    } => {
                        state.turn_right();
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