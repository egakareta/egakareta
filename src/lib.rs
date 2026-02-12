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
use serde::Deserialize;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[derive(Deserialize, Clone)]
struct MusicMetadata {
    source: String,
}

#[derive(Deserialize, Clone)]
struct LevelMetadata {
    name: String,
    music: MusicMetadata,
    objects: Vec<LevelObject>,
}

#[derive(Deserialize, Clone)]
struct LevelObject {
    #[serde(default)]
    position: [f32; 2],
    #[serde(default = "default_size")]
    size: [f32; 2],
}

fn default_size() -> [f32; 2] { [1.0, 1.0] }

#[derive(PartialEq)]
enum AppPhase {
    Menu,
    Playing,
    GameOver,
}

struct MenuState {
    selected_level: usize,
    levels: Vec<String>,
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
    trail: Vec<[f32; 2]>,
    objects: Vec<LevelObject>,
    game_over: bool,
}

impl GameState {
    fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            direction: Direction::Forward,
            speed: 8.0,
            trail: vec![[0.0, 0.0]],
            objects: Vec::new(),
            game_over: false,
        }
    }

    fn turn_right(&mut self) {
        if self.game_over { return; }
        self.trail.push(self.position);
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    fn update(&mut self, dt: f32) {
        if self.game_over { return; }
        let delta = match self.direction {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };

        self.position[0] += delta[0] * self.speed * dt;
        self.position[1] += delta[1] * self.speed * dt;

        let col_size = 0.4; // Half-width of collision box (trail is 0.8)

        // Collision check
        for obj in &self.objects {
            let p_min = [self.position[0] - col_size, self.position[1] - col_size];
            let p_max = [self.position[0] + col_size, self.position[1] + col_size];
            let o_min = [obj.position[0], obj.position[1]];
            let o_max = [obj.position[0] + obj.size[0], obj.position[1] + obj.size[1]];

            if p_max[0] >= o_min[0] && p_min[0] <= o_max[0] &&
               p_max[1] >= o_min[1] && p_min[1] <= o_max[1] {
                self.game_over = true;
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineUniform {
    offset: [f32; 2],
    rotation: f32,
    _pad: f32,
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
    trail_vertex_buffer: wgpu::Buffer,
    trail_vertex_count: u32,
    player_vertex_buffer: wgpu::Buffer,
    player_vertex_count: u32,
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_vertex_count: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    line_uniform_buffer: wgpu::Buffer,
    line_bind_group: wgpu::BindGroup,
    zero_line_bind_group: wgpu::BindGroup,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    game: GameState,
    phase: AppPhase,
    menu: MenuState,
    line_uniform: LineUniform,
    last_frame: Instant,
    accumulator: f32,
    #[cfg(target_arch = "wasm32")]
    current_audio: Option<web_sys::HtmlAudioElement>,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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

        let (depth_texture, depth_view) = Self::create_depth_texture(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let line_uniform = LineUniform {
            offset: [0.0, 0.0],
            rotation: 0.0,
            _pad: 0.0,
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

        let zero_line_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Zero Line Uniform Buffer"),
            contents: bytemuck::bytes_of(&LineUniform {
                offset: [0.0, 0.0],
                rotation: 0.0,
                _pad: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let zero_line_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Zero Line Bind Group"),
            layout: &line_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: zero_line_uniform_buffer.as_entire_binding(),
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Generate 3D tiles for the floor
        let mut floor_vertices: Vec<Vertex> = Vec::new();
        let tile_color_top = [0.08, 0.08, 0.1];
        let tile_color_side = [0.05, 0.05, 0.07];
        let extent = 60;
        let tile_height = 0.1;
        let tile_margin = 0.05;

        for x in -extent..extent {
            for y in -extent..extent {
                let x_min = x as f32 + tile_margin;
                let x_max = (x + 1) as f32 - tile_margin;
                let y_min = y as f32 + tile_margin;
                let y_max = (y + 1) as f32 - tile_margin;
                let z_min = -tile_height;
                let z_max = 0.0;

                // Top face
                floor_vertices.push(Vertex { position: [x_min, y_min, z_max], color: tile_color_top });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_max], color: tile_color_top });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_top });
                floor_vertices.push(Vertex { position: [x_min, y_min, z_max], color: tile_color_top });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_top });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_max], color: tile_color_top });

                // Side faces (simplified: only Y+, Y-, X+, X-)
                // Y+
                floor_vertices.push(Vertex { position: [x_min, y_max, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_max], color: tile_color_side });
                // Y-
                floor_vertices.push(Vertex { position: [x_min, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_min, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_max], color: tile_color_side });
                // X+
                floor_vertices.push(Vertex { position: [x_max, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_max, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_max, y_min, z_max], color: tile_color_side });
                // X-
                floor_vertices.push(Vertex { position: [x_min, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_min, z_min], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_min, z_max], color: tile_color_side });
                floor_vertices.push(Vertex { position: [x_min, y_max, z_max], color: tile_color_side });
            }
        }

        // Grid lines across a large area to visualize lanes
        let mut grid_vertices: Vec<Vertex> = Vec::new();
        let extent = 60.0;
        let step = 1.0;
        let grid_color = [0.2, 0.22, 0.26];
        let line_width = 0.02;
        let grid_z = 0.01;

        let mut x = -extent;
        while x <= extent {
            // Vertical lines as quads
            let x_min = x - line_width;
            let x_max = x + line_width;
            grid_vertices.push(Vertex { position: [x_min, -extent, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [x_max, -extent, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [x_max, extent, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [x_min, -extent, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [x_max, extent, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [x_min, extent, grid_z], color: grid_color });
            x += step;
        }
        let mut y = -extent;
        while y <= extent {
            // Horizontal lines as quads
            let y_min = y - line_width;
            let y_max = y + line_width;
            grid_vertices.push(Vertex { position: [-extent, y_min, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [extent, y_min, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [extent, y_max, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [-extent, y_min, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [extent, y_max, grid_z], color: grid_color });
            grid_vertices.push(Vertex { position: [-extent, y_max, grid_z], color: grid_color });
            y += step;
        }

        let floor_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Floor Vertex Buffer"),
            contents: bytemuck::cast_slice(&floor_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let trail_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Trail Vertex Buffer"),
            size: (std::mem::size_of::<Vertex>() * 36 * 500) as u64, // 500 segments
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Generate player "head" cube
        let mut player_vertices = Vec::new();
        let head_size = 1.0;
        let head_color = [1.0, 0.9, 0.3];
        let h = head_size;
        let c = head_color;
        player_vertices.extend_from_slice(&[
            // Top
            Vertex { position: [-h, -h, 2.0*h], color: c }, Vertex { position: [h, -h, 2.0*h], color: c }, Vertex { position: [h, h, 2.0*h], color: c },
            Vertex { position: [-h, -h, 2.0*h], color: c }, Vertex { position: [h, h, 2.0*h], color: c }, Vertex { position: [-h, h, 2.0*h], color: c },
            // Bottom deleted or whatever
            // Sides ... (just enough to see it)
            Vertex { position: [-h, -h, 0.0], color: c }, Vertex { position: [h, -h, 0.0], color: c }, Vertex { position: [h, -h, 2.0*h], color: c },
            Vertex { position: [-h, -h, 0.0], color: c }, Vertex { position: [h, -h, 2.0*h], color: c }, Vertex { position: [-h, -h, 2.0*h], color: c },

            Vertex { position: [h, -h, 0.0], color: c }, Vertex { position: [h, h, 0.0], color: c }, Vertex { position: [h, h, 2.0*h], color: c },
            Vertex { position: [h, -h, 0.0], color: c }, Vertex { position: [h, h, 2.0*h], color: c }, Vertex { position: [h, -h, 2.0*h], color: c },

            Vertex { position: [h, h, 0.0], color: c }, Vertex { position: [-h, h, 0.0], color: c }, Vertex { position: [-h, h, 2.0*h], color: c },
            Vertex { position: [h, h, 0.0], color: c }, Vertex { position: [-h, h, 2.0*h], color: c }, Vertex { position: [h, h, 2.0*h], color: c },

            Vertex { position: [-h, h, 0.0], color: c }, Vertex { position: [-h, -h, 0.0], color: c }, Vertex { position: [-h, -h, 2.0*h], color: c },
            Vertex { position: [-h, h, 0.0], color: c }, Vertex { position: [-h, -h, 2.0*h], color: c }, Vertex { position: [-h, h, 2.0*h], color: c },
        ]);

        let player_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Player Vertex Buffer"),
            contents: bytemuck::cast_slice(&player_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let menu = MenuState {
            selected_level: 0,
            levels: vec!["Flowerfield".to_string(), "Golden Haze".to_string()],
        };

        Self {
            canvas_or_window,
            surface,
            device,
            queue,
            config,
            size,
            floor_vertex_buffer,
            floor_vertex_count: floor_vertices.len() as u32,
            depth_texture,
            depth_view,
            render_pipeline,
            line_uniform_buffer,
            line_bind_group,
            zero_line_bind_group,
            camera_uniform_buffer,
            camera_bind_group,
            game: GameState::new(),
            phase: AppPhase::Menu,
            menu,
            line_uniform,
            last_frame: Instant::now(),
            accumulator: 0.0,
            #[cfg(target_arch = "wasm32")]
            current_audio: None,
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            trail_vertex_buffer,
            trail_vertex_count: 0,
            player_vertex_buffer,
            player_vertex_count: player_vertices.len() as u32,
            block_vertex_buffer: None,
            block_vertex_count: 0,
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
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub fn turn_right(&mut self) {
        match self.phase {
            AppPhase::Menu => {
                // In menu, click/press enters the level
                self.start_level(self.menu.selected_level);
            }
            AppPhase::Playing => {
                if self.game.game_over {
                    self.phase = AppPhase::Menu;
                } else {
                    self.game.turn_right();
                }
            }
            AppPhase::GameOver => {
                self.phase = AppPhase::Menu;
            }
        }
    }

    pub fn next_level(&mut self) {
        if self.phase == AppPhase::Menu {
            self.menu.selected_level = (self.menu.selected_level + 1) % self.menu.levels.len();
        }
    }

    pub fn prev_level(&mut self) {
        if self.phase == AppPhase::Menu {
            if self.menu.selected_level == 0 {
                self.menu.selected_level = self.menu.levels.len() - 1;
            } else {
                self.menu.selected_level -= 1;
            }
        }
    }

    fn start_level(&mut self, index: usize) {
        let level_name = &self.menu.levels[index];
        
        self.game = GameState::new();
        self.phase = AppPhase::Playing;

        // Stop previous audio
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = self.current_audio.take() {
            let _ = audio.pause();
        }

        // Load level data from embedded metadata files
        let metadata_str = match level_name.as_str() {
            "Flowerfield" => include_str!("../assets/levels/Flowerfield/metadata.json"),
            "Golden Haze" => include_str!("../assets/levels/Golden Haze/metadata.json"),
            _ => "{\"name\": \"Unknown\", \"music\": {\"source\": \"\"}, \"objects\": []}",
        };

        if let Ok(metadata) = serde_json::from_str::<LevelMetadata>(metadata_str) {
            self.game.objects = metadata.objects;

            #[cfg(target_arch = "wasm32")]
            {
                let audio_url = format!("assets/levels/{}/{}", level_name, metadata.music.source);
                if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
                    let _ = audio.play();
                    self.current_audio = Some(audio);
                }
            }
        }

        self.rebuild_block_vertices();
    }

    fn rebuild_block_vertices(&mut self) {
        let mut vertices = Vec::new();
        let color_top = [0.4, 0.4, 0.45];
        let color_side = [0.2, 0.2, 0.25];
        let z_min = 0.0;
        let z_max = 1.0;

        for obj in &self.game.objects {
            let x_min = obj.position[0];
            let x_max = obj.position[0] + obj.size[0];
            let y_min = obj.position[1];
            let y_max = obj.position[1] + obj.size[1];

            // Top
            vertices.push(Vertex { position: [x_min, y_min, z_max], color: color_top });
            vertices.push(Vertex { position: [x_max, y_min, z_max], color: color_top });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_top });
            vertices.push(Vertex { position: [x_min, y_min, z_max], color: color_top });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_top });
            vertices.push(Vertex { position: [x_min, y_max, z_max], color: color_top });

            // Sides (simplified)
            // Y+
            vertices.push(Vertex { position: [x_min, y_max, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_side });
            vertices.push(Vertex { position: [x_min, y_max, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_side });
            vertices.push(Vertex { position: [x_min, y_max, z_max], color: color_side });
            // X+
            vertices.push(Vertex { position: [x_max, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_side });
            vertices.push(Vertex { position: [x_max, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_max, z_max], color: color_side });
            vertices.push(Vertex { position: [x_max, y_min, z_max], color: color_side });
            // X-
            vertices.push(Vertex { position: [x_min, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_min, y_max, z_max], color: color_side });
            vertices.push(Vertex { position: [x_min, y_max, z_min], color: color_side });
            vertices.push(Vertex { position: [x_min, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_min, y_min, z_max], color: color_side });
            vertices.push(Vertex { position: [x_min, y_max, z_max], color: color_side });
            // Y-
            vertices.push(Vertex { position: [x_min, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_max, y_min, z_max], color: color_side });
            vertices.push(Vertex { position: [x_max, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_min, y_min, z_min], color: color_side });
            vertices.push(Vertex { position: [x_min, y_min, z_max], color: color_side });
            vertices.push(Vertex { position: [x_max, y_min, z_max], color: color_side });
        }

        self.block_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.block_vertex_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }));
        } else {
            self.block_vertex_buffer = None;
        }
    }

    pub fn update(&mut self) {
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = Instant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.accumulator = (self.accumulator + frame_dt).min(0.25);

        if self.phase == AppPhase::Menu {
            // Animate menu 
            self.accumulator = 0.0; // No fixed update in menu for now
            self.update_menu_camera();
            return;
        }

        while self.accumulator >= FIXED_DT {
            self.game.update(FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        if self.game.game_over {
            #[cfg(target_arch = "wasm32")]
            if let Some(audio) = self.current_audio.take() {
                let _ = audio.pause();
            }
        }

        // Build trail vertices (only if not game over or for showing last state)
        let mut trail_vertices = Vec::new();
        let width = 0.8;
        let z_min = 0.3;
        let z_max = 0.8;
        let c_top = if self.game.game_over { [1.0, 0.2, 0.2] } else { [0.8, 0.25, 0.35] };
        let c_side = if self.game.game_over { [0.8, 0.1, 0.1] } else { [0.7, 0.2, 0.3] };

        let mut points = self.game.trail.clone();
        points.push(self.game.position);

        for i in 0..points.len() - 1 {
            let p1 = points[i];
            let p2 = points[i + 1];

            let dx = p2[0] - p1[0];
            let dy = p2[1] - p1[1];

            let (x_min, x_max, y_min, y_max) = if dx.abs() > dy.abs() {
                // Horizontal
                (
                    p1[0].min(p2[0]) - width / 2.0,
                    p1[0].max(p2[0]) + width / 2.0,
                    p1[1] - width / 2.0,
                    p1[1] + width / 2.0,
                )
            } else {
                // Vertical
                (
                    p1[0] - width / 2.0,
                    p1[0] + width / 2.0,
                    p1[1].min(p2[1]) - width / 2.0,
                    p1[1].max(p2[1]) + width / 2.0,
                )
            };

            // Top
            trail_vertices.push(Vertex { position: [x_min, y_min, z_max], color: c_top });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_max], color: c_top });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_top });
            trail_vertices.push(Vertex { position: [x_min, y_min, z_max], color: c_top });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_top });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_max], color: c_top });

            // Sides
            // Y+
            trail_vertices.push(Vertex { position: [x_min, y_max, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_max], color: c_side });
            // Y-
            trail_vertices.push(Vertex { position: [x_min, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_min, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_max], color: c_side });
            // X+
            trail_vertices.push(Vertex { position: [x_max, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_max, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_max, y_min, z_max], color: c_side });
            // X-
            trail_vertices.push(Vertex { position: [x_min, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_min, z_min], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_min, z_max], color: c_side });
            trail_vertices.push(Vertex { position: [x_min, y_max, z_max], color: c_side });
        }

        self.trail_vertex_count = trail_vertices.len() as u32;
        if !trail_vertices.is_empty() {
            let max_vertices = (self.trail_vertex_buffer.size() / std::mem::size_of::<Vertex>() as u64) as usize;
            let vertices_to_write = &trail_vertices[..trail_vertices.len().min(max_vertices)];
            self.queue.write_buffer(&self.trail_vertex_buffer, 0, bytemuck::cast_slice(vertices_to_write));
        }

        // Snap slightly toward grid to avoid drift
        self.line_uniform.offset = [
            (self.game.position[0] * 100.0).round() / 100.0,
            (self.game.position[1] * 100.0).round() / 100.0,
        ];
        self.line_uniform.rotation = match self.game.direction {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        self.queue
            .write_buffer(&self.line_uniform_buffer, 0, bytemuck::bytes_of(&self.line_uniform));

        let aspect = self.config.width as f32 / self.config.height as f32;
        let pos_3d = Vec3::new(self.game.position[0], self.game.position[1], 0.0);
        let target = pos_3d;
        let offset = Mat4::from_rotation_z(-45.0f32.to_radians()).transform_vector3(Vec3::new(0.0, -20.0, 20.0));
        let eye = pos_3d + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
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

    fn update_menu_camera(&mut self) {
        #[cfg(target_arch = "wasm32")]
        let time = (web_sys::window().unwrap().performance().unwrap().now() as f32) / 1000.0;
        #[cfg(not(target_arch = "wasm32"))]
        let time = Instant::now().elapsed().as_secs_f32();

        let aspect = self.config.width as f32 / self.config.height as f32;
        let radius = 100.0;
        let eye = Vec3::new(radius * (time * 0.2).cos(), radius * (time * 0.2).sin(), 50.0);
        let target = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
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
            let clear_color = match self.phase {
                AppPhase::Playing if self.game.game_over => wgpu::Color { r: 0.15, g: 0.05, b: 0.05, a: 1.0 },
                _ => wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);

            render_pass.set_vertex_buffer(0, self.floor_vertex_buffer.slice(..));
            render_pass.draw(0..self.floor_vertex_count, 0..1);

            render_pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
            render_pass.draw(0..self.grid_vertex_count, 0..1);

            if self.phase == AppPhase::Playing || self.phase == AppPhase::GameOver {
                if let Some(buf) = &self.block_vertex_buffer {
                    render_pass.set_vertex_buffer(0, buf.slice(..));
                    render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                    render_pass.draw(0..self.block_vertex_count, 0..1);
                }

                if self.trail_vertex_count > 0 {
                    render_pass.set_vertex_buffer(0, self.trail_vertex_buffer.slice(..));
                    render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                    render_pass.draw(0..self.trail_vertex_count, 0..1);
                }
            } else if self.phase == AppPhase::Menu {
                // Orbiting camera handles u_camera
                // We'll draw the floor and grid which we already do above.
            }
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

    #[cfg(not(target_arch = "wasm32"))]
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = PhysicalSize::new(new_size.width, new_size.height);
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);            let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;        }
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
            let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
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
        state_clone.borrow_mut().turn_right();
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref()).unwrap();
    closure.forget();

    let state_clone = state_rc.clone();
    let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        if event.repeat() { return; }
        match event.key().as_str() {
            "ArrowUp" | " " => state_clone.borrow_mut().turn_right(),
            "ArrowRight" => state_clone.borrow_mut().next_level(),
            "ArrowLeft" => state_clone.borrow_mut().prev_level(),
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref()).unwrap();
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
