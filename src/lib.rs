use std::iter;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(target_arch = "wasm32")]
use std::{rc::Rc, cell::RefCell};

use wgpu::{util::DeviceExt, SurfaceError, TextureViewDescriptor};
use glam::{Mat4, Vec3};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::{console, HtmlCanvasElement};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

#[derive(Clone, Copy)]
struct PhysicalSize<T> {
    width: T,
    height: T,
}

impl<T> PhysicalSize<T> {
    fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Right,
}

struct GameState {
    position: [f32; 2],
    direction: Direction,
    speed: f32,
}

impl GameState {
    fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            direction: Direction::Forward,
            speed: 0.6,
        }
    }

    fn turn_right(&mut self) {
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    fn update(&mut self, dt: f32) {
        let delta = match self.direction {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };

        self.position[0] += delta[0] * self.speed * dt;
        self.position[1] += delta[1] * self.speed * dt;
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineUniform {
    offset: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
enum CanvasOrWindow {
    Canvas(HtmlCanvasElement),
}

#[cfg(not(target_arch = "wasm32"))]
enum CanvasOrWindow {
    Window(Window),
}

pub struct State {
    canvas_or_window: CanvasOrWindow,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    floor_vertex_buffer: wgpu::Buffer,
    floor_vertex_count: u32,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_count: u32,
    render_pipeline: wgpu::RenderPipeline,
    line_uniform_buffer: wgpu::Buffer,
    line_bind_group: wgpu::BindGroup,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    game: GameState,
    line_uniform: LineUniform,
    last_frame: Instant,
    accumulator: f32,
}

impl State {
    #[cfg(target_arch = "wasm32")]
    async fn new(canvas: HtmlCanvasElement) -> Self {
        let size = PhysicalSize::new(canvas.width(), canvas.height());

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .expect("Failed to create surface");

        Self::new_common(instance, CanvasOrWindow::Canvas(canvas), surface, size).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_native(window: Window) -> State {
        let size = PhysicalSize::new(window.inner_size().width, window.inner_size().height);

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(&window).expect("Failed to create surface");
        let surface = unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };

        Self::new_common(instance, CanvasOrWindow::Window(window), surface, size).await
    }

    async fn new_common(instance: wgpu::Instance, canvas_or_window: CanvasOrWindow, surface: wgpu::Surface<'static>, size: PhysicalSize<u32>) -> State {

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let adapter_info = adapter.get_info();
        #[cfg(target_arch = "wasm32")]
        console::log_1(&format!("Using graphics API backend: {:?}", adapter_info.backend).into());
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("Using graphics API backend: {:?}", adapter_info.backend);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let line_uniform = LineUniform {
            offset: [0.0, 0.0],
            _pad: [0.0, 0.0],
        };

        let line_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Uniform Buffer"),
            contents: bytemuck::bytes_of(&line_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_uniform = CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        };

        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let line_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Line Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let line_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Line Bind Group"),
            layout: &line_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: line_uniform_buffer.as_entire_binding(),
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &line_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let floor_vertices: [Vertex; 6] = [
            Vertex {
                position: [-12.0, -12.0, 0.0],
                color: [0.08, 0.08, 0.1],
            },
            Vertex {
                position: [12.0, -12.0, 0.0],
                color: [0.1, 0.1, 0.12],
            },
            Vertex {
                position: [12.0, 12.0, 0.0],
                color: [0.12, 0.12, 0.14],
            },
            Vertex {
                position: [-12.0, -12.0, 0.0],
                color: [0.08, 0.08, 0.1],
            },
            Vertex {
                position: [12.0, 12.0, 0.0],
                color: [0.12, 0.12, 0.14],
            },
            Vertex {
                position: [-12.0, 12.0, 0.0],
                color: [0.1, 0.1, 0.12],
            },
        ];

        let line_vertices: [Vertex; 6] = [
            Vertex {
                position: [-0.05, -0.15, 0.05],
                color: [0.85, 0.2, 0.35],
            },
            Vertex {
                position: [0.05, -0.15, 0.05],
                color: [0.9, 0.25, 0.4],
            },
            Vertex {
                position: [0.05, 0.15, 0.05],
                color: [0.95, 0.3, 0.45],
            },
            Vertex {
                position: [-0.05, -0.15, 0.05],
                color: [0.85, 0.2, 0.35],
            },
            Vertex {
                position: [0.05, 0.15, 0.05],
                color: [0.95, 0.3, 0.45],
            },
            Vertex {
                position: [-0.05, 0.15, 0.05],
                color: [0.9, 0.25, 0.4],
            },
        ];

        // Grid lines across a 24x24 area to visualize lanes
        let mut grid_vertices: Vec<Vertex> = Vec::new();
        let extent = 12.0;
        let step = 1.0;
        let grid_color = [0.2, 0.22, 0.26];
        let mut x = -extent;
        while x <= extent {
            grid_vertices.push(Vertex {
                position: [x, -extent, 0.02],
                color: grid_color,
            });
            grid_vertices.push(Vertex {
                position: [x, extent, 0.02],
                color: grid_color,
            });
            x += step;
        }
        let mut y = -extent;
        while y <= extent {
            grid_vertices.push(Vertex {
                position: [-extent, y, 0.02],
                color: grid_color,
            });
            grid_vertices.push(Vertex {
                position: [extent, y, 0.02],
                color: grid_color,
            });
            y += step;
        }

        let floor_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Floor Vertex Buffer"),
            contents: bytemuck::cast_slice(&floor_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let line_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Vertex Buffer"),
            contents: bytemuck::cast_slice(&line_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            canvas_or_window,
            surface,
            device,
            queue,
            config,
            size,
            floor_vertex_buffer,
            floor_vertex_count: floor_vertices.len() as u32,
            line_vertex_buffer,
            line_vertex_count: line_vertices.len() as u32,
            render_pipeline,
            line_uniform_buffer,
            line_bind_group,
            camera_uniform_buffer,
            camera_bind_group,
            game: GameState::new(),
            line_uniform,
            last_frame: Instant::now(),
            accumulator: 0.0,
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        match &self.canvas_or_window {
            CanvasOrWindow::Canvas(canvas) => {
                canvas.set_width(new_size.width);
                canvas.set_height(new_size.height);
            }
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn update(&mut self) {
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = Instant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.accumulator = (self.accumulator + frame_dt).min(0.25);

        while self.accumulator >= FIXED_DT {
            self.game.update(FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        // Snap slightly toward grid to avoid drift
        self.line_uniform.offset = [
            (self.game.position[0] * 100.0).round() / 100.0,
            (self.game.position[1] * 100.0).round() / 100.0,
        ];

        self.queue
            .write_buffer(&self.line_uniform_buffer, 0, bytemuck::bytes_of(&self.line_uniform));

        let aspect = self.config.width as f32 / self.config.height as f32;
        let eye = Vec3::new(4.0, -10.0, 10.0);
        let target = Vec3::new(0.0, 2.5, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 100.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.update();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.line_bind_group, &[]);

            render_pass.set_vertex_buffer(0, self.floor_vertex_buffer.slice(..));
            render_pass.draw(0..self.floor_vertex_count, 0..1);

            render_pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
            render_pass.draw(0..self.grid_vertex_count, 0..1);

            render_pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
            render_pass.draw(0..self.line_vertex_count, 0..1);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> &Window {
        match &self.canvas_or_window {
            CanvasOrWindow::Window(w) => w,
        }
    }

    pub fn turn_right(&mut self) {
        self.game.turn_right();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = PhysicalSize::new(new_size.width, new_size.height);
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn recreate_surface(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let window = self.window();
            let size = PhysicalSize::new(window.inner_size().width, window.inner_size().height);
            self.size = size;
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

#[cfg(target_arch = "wasm32")]
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

    // Set initial size to window size
    let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
    let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    let state = State::new(canvas.clone()).await;

    // add event listener
    let state_rc = Rc::new(RefCell::new(state));

    // Handle window resize
    {
        let state_clone = state_rc.clone();
        let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let window = web_sys::window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
            state_clone
                .borrow_mut()
                .resize(PhysicalSize::new(width, height));
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
        state_clone.borrow_mut().game.turn_right();
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
    closure.forget();

    // start render loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut state = state_rc.borrow_mut();
        match state.render() {
            Ok(_) => {}
            Err(SurfaceError::Lost) => {
                let size = state.size;
                state.resize(size);
            }
            Err(SurfaceError::Outdated) => {
                let size = state.size;
                state.resize(size);
            }
            Err(SurfaceError::OutOfMemory) => {
                console::error_1(&"Out of memory".into());
                return;
            }
            Err(err) => console::error_1(&format!("Render error: {:?}", err).into()),
        }
        let window = web_sys::window().unwrap();
        window.request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
    }) as Box<dyn FnMut()>));
    web_sys::window().unwrap().request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
    Ok(())
}
