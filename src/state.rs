#[cfg(not(target_arch = "wasm32"))]
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Cursor;
use std::iter;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use glam::{Mat4, Vec2, Vec3, Vec4};
use wgpu::{util::DeviceExt, SurfaceError, TextureViewDescriptor};
#[cfg(target_arch = "wasm32")]
use web_sys::{console, HtmlCanvasElement};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

use crate::game::GameState;
use crate::mesh::{
    build_block_vertices, build_editor_cursor_vertices, build_floor_vertices, build_grid_vertices,
    build_spawn_marker_vertices, build_trail_vertices,
};
use crate::types::{
    AppPhase, CameraUniform, Direction, EditorState, LevelMetadata, LevelObject, LineUniform,
    MenuState, PhysicalSize, SpawnDirection, SpawnMetadata, Vertex,
};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_vertex_count: u32,
    editor_cursor_vertex_buffer: Option<wgpu::Buffer>,
    editor_cursor_vertex_count: u32,
    spawn_marker_vertex_buffer: Option<wgpu::Buffer>,
    spawn_marker_vertex_count: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    line_uniform_buffer: wgpu::Buffer,
    zero_line_bind_group: wgpu::BindGroup,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    game: GameState,
    phase: AppPhase,
    menu: MenuState,
    editor: EditorState,
    editor_objects: Vec<LevelObject>,
    editor_spawn: SpawnMetadata,
    editor_camera_pan: [f32; 2],
    editor_camera_rotation: f32,
    editor_camera_pitch: f32,
    editor_zoom: f32,
    editor_right_dragging: bool,
    editor_pan_up_held: bool,
    editor_pan_down_held: bool,
    editor_pan_left_held: bool,
    editor_pan_right_held: bool,
    editor_shift_held: bool,
    playtesting_editor: bool,
    line_uniform: LineUniform,
    app_start: Instant,
    last_frame: Instant,
    accumulator: f32,
    #[cfg(target_arch = "wasm32")]
    current_audio: Option<web_sys::HtmlAudioElement>,
    #[cfg(not(target_arch = "wasm32"))]
    _audio_output_stream: Option<OutputStream>,
    #[cfg(not(target_arch = "wasm32"))]
    audio_output_handle: Option<OutputStreamHandle>,
    #[cfg(not(target_arch = "wasm32"))]
    current_audio_sink: Option<Sink>,
}

impl State {
    #[cfg(target_arch = "wasm32")]
    pub(crate) async fn new(canvas: HtmlCanvasElement) -> Self {
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
        let surface = instance
            .create_surface(&window)
            .expect("Failed to create surface");
        let surface =
            unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };

        Self::new_common(instance, CanvasOrWindow::Window(window), surface, size).await
    }

    async fn new_common(
        instance: wgpu::Instance,
        canvas_or_window: CanvasOrWindow,
        surface: wgpu::Surface<'static>,
        size: PhysicalSize<u32>,
    ) -> State {
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

        let line_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let zero_line_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

        let floor_vertices = build_floor_vertices();
        let grid_vertices = build_grid_vertices();

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
            size: (std::mem::size_of::<Vertex>() * 36 * 500) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        #[cfg(not(target_arch = "wasm32"))]
        let (audio_output_stream, audio_output_handle) = match OutputStream::try_default() {
            Ok((stream, handle)) => (Some(stream), Some(handle)),
            Err(err) => {
                log::warn!("Failed to initialize native audio output: {}", err);
                (None, None)
            }
        };

        let menu = MenuState {
            selected_level: 0,
            levels: vec!["Flowerfield".to_string(), "Golden Haze".to_string()],
        };

        let now = Instant::now();

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
            zero_line_bind_group,
            camera_uniform_buffer,
            camera_bind_group,
            game: GameState::new(),
            phase: AppPhase::Menu,
            menu,
            line_uniform,
            app_start: now,
            last_frame: now,
            accumulator: 0.0,
            #[cfg(target_arch = "wasm32")]
            current_audio: None,
            #[cfg(not(target_arch = "wasm32"))]
            _audio_output_stream: audio_output_stream,
            #[cfg(not(target_arch = "wasm32"))]
            audio_output_handle,
            #[cfg(not(target_arch = "wasm32"))]
            current_audio_sink: None,
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            trail_vertex_buffer,
            trail_vertex_count: 0,
            block_vertex_buffer: None,
            block_vertex_count: 0,
            editor_cursor_vertex_buffer: None,
            editor_cursor_vertex_count: 0,
            spawn_marker_vertex_buffer: None,
            spawn_marker_vertex_count: 0,
            editor: EditorState::new(),
            editor_objects: Vec::new(),
            editor_spawn: SpawnMetadata::default(),
            editor_camera_pan: [0.0, 0.0],
            editor_camera_rotation: -45.0f32.to_radians(),
            editor_camera_pitch: 45.0f32.to_radians(),
            editor_zoom: 1.0,
            editor_right_dragging: false,
            editor_pan_up_held: false,
            editor_pan_down_held: false,
            editor_pan_left_held: false,
            editor_pan_right_held: false,
            editor_shift_held: false,
            playtesting_editor: false,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
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
                self.start_level(self.menu.selected_level);
            }
            AppPhase::Playing => {
                if self.game.game_over {
                    self.phase = if self.playtesting_editor {
                        AppPhase::Editor
                    } else {
                        AppPhase::Menu
                    };
                    self.game.game_over = false;
                    self.game.trail_segments = vec![vec![self.game.position]];
                } else {
                    self.game.turn_right();
                }
            }
            AppPhase::Editor => {
                self.place_editor_block();
            }
            AppPhase::GameOver => {
                self.phase = AppPhase::Menu;
            }
        }
    }

    pub fn next_level(&mut self) {
        if self.phase == AppPhase::Menu {
            self.menu.selected_level = (self.menu.selected_level + 1) % self.menu.levels.len();
        } else if self.phase == AppPhase::Editor {
            self.move_editor_cursor(1, 0);
        }
    }

    pub fn prev_level(&mut self) {
        if self.phase == AppPhase::Menu {
            if self.menu.selected_level == 0 {
                self.menu.selected_level = self.menu.levels.len() - 1;
            } else {
                self.menu.selected_level -= 1;
            }
        } else if self.phase == AppPhase::Editor {
            self.move_editor_cursor(-1, 0);
        }
    }

    pub fn toggle_editor(&mut self) {
        match self.phase {
            AppPhase::Menu => self.start_editor(self.menu.selected_level),
            AppPhase::Editor => {
                self.phase = AppPhase::Menu;
                self.playtesting_editor = false;
                self.editor_right_dragging = false;
                self.clear_editor_pan_keys();
            }
            _ => {}
        }
    }

    pub fn is_editor(&self) -> bool {
        self.phase == AppPhase::Editor
    }

    pub fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor_right_dragging = dragging && self.phase == AppPhase::Editor;
    }

    fn clear_editor_pan_keys(&mut self) {
        self.editor_pan_up_held = false;
        self.editor_pan_down_held = false;
        self.editor_pan_left_held = false;
        self.editor_pan_right_held = false;
        self.editor_shift_held = false;
    }

    pub fn set_editor_pan_up_held(&mut self, held: bool) {
        self.editor_pan_up_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_down_held(&mut self, held: bool) {
        self.editor_pan_down_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_left_held(&mut self, held: bool) {
        self.editor_pan_left_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_right_held(&mut self, held: bool) {
        self.editor_pan_right_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_shift_held(&mut self, held: bool) {
        self.editor_shift_held = held && self.phase == AppPhase::Editor;
    }

    fn editor_camera_axes_xy(&self) -> (Vec2, Vec2) {
        let right = Vec2::new(self.editor_camera_rotation.cos(), self.editor_camera_rotation.sin());
        let up = Vec2::new(-self.editor_camera_rotation.sin(), self.editor_camera_rotation.cos());
        (right, up)
    }

    fn editor_camera_offset(&self) -> Vec3 {
        let zoom = self.editor_zoom.clamp(0.35, 4.0);
        let distance = 24.0 / zoom;
        let pitch = self.editor_camera_pitch.clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(self.editor_camera_rotation)
            .transform_vector3(Vec3::new(0.0, -horizontal_distance, vertical_distance))
    }

    pub fn adjust_editor_zoom(&mut self, delta: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        const ZOOM_SENSITIVITY: f32 = 0.12;
        let factor = (1.0 + delta * ZOOM_SENSITIVITY).max(0.1);
        self.editor_zoom = (self.editor_zoom * factor).clamp(0.35, 4.0);
    }

    pub fn pan_editor_camera_by_input(&mut self, screen_x: f32, screen_y: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let (camera_right_xy, camera_up_xy) = self.editor_camera_axes_xy();
        let world_delta = camera_right_xy * screen_x + camera_up_xy * screen_y;

        let max_pan = self.editor.bounds as f32;
        self.editor_camera_pan[0] = (self.editor_camera_pan[0] + world_delta.x).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] = (self.editor_camera_pan[1] + world_delta.y).clamp(-max_pan, max_pan);
    }

    fn update_editor_pan_from_keys(&mut self, frame_dt: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let mut input = Vec2::ZERO;
        if self.editor_pan_left_held {
            input.x -= 1.0;
        }
        if self.editor_pan_right_held {
            input.x += 1.0;
        }
        if self.editor_pan_up_held {
            input.y += 1.0;
        }
        if self.editor_pan_down_held {
            input.y -= 1.0;
        }

        if input.length_squared() <= f32::EPSILON {
            return;
        }

        let input = input.normalize();
        let pitch = self
            .editor_camera_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_factor = pitch.cos();
        let vertical_factor = pitch.sin();

        let mut speed_multiplier = 1.0;
        if self.editor_shift_held {
            speed_multiplier = 0.3;
        }

        const PAN_SPEED_UNITS_PER_SEC: f32 = 40.0;
        self.pan_editor_camera_by_input(
            input.x * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
            input.y * horizontal_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );

        self.adjust_editor_zoom(input.y * vertical_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier);
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return;
        }

        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let offset = self.editor_camera_offset();
        let eye = target + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let inv_view_proj = (proj * view).inverse();

        let ndc_x = (2.0 * x as f32 / self.config.width as f32) - 1.0;
        let ndc_y = 1.0 - (2.0 * y as f32 / self.config.height as f32);

        let near_clip = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let far_clip = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let mut near_world = inv_view_proj * near_clip;
        let mut far_world = inv_view_proj * far_clip;
        if near_world.w.abs() <= f32::EPSILON || far_world.w.abs() <= f32::EPSILON {
            return;
        }

        near_world /= near_world.w;
        far_world /= far_world.w;

        let ray_origin = near_world.truncate();
        let ray_dir = (far_world.truncate() - ray_origin).normalize();

        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Z;
        let mut hit_found = false;

        if ray_dir.z.abs() > f32::EPSILON {
            let t = -ray_origin.z / ray_dir.z;
            if t >= 0.0 {
                min_t = t;
                hit_found = true;
            }
        }

        for obj in &self.editor_objects {
            let min = Vec3::from_array(obj.position);
            let max = min + Vec3::from_array(obj.size);

            let inv_dir = 1.0 / ray_dir;
            let t1 = (min.x - ray_origin.x) * inv_dir.x;
            let t2 = (max.x - ray_origin.x) * inv_dir.x;
            let t3 = (min.y - ray_origin.y) * inv_dir.y;
            let t4 = (max.y - ray_origin.y) * inv_dir.y;
            let t5 = (min.z - ray_origin.z) * inv_dir.z;
            let t6 = (max.z - ray_origin.z) * inv_dir.z;

            let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
            let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

            if tmax >= 0.0 && tmin <= tmax {
                let t = if tmin < 0.0 { tmax } else { tmin };
                if t < min_t {
                    min_t = t;
                    hit_found = true;

                    let eps = 1e-5;
                    if (t - t1.min(t2)).abs() < eps {
                        best_hit_normal = if ray_dir.x > 0.0 {
                            Vec3::NEG_X
                        } else {
                            Vec3::X
                        };
                    } else if (t - t3.min(t4)).abs() < eps {
                        best_hit_normal = if ray_dir.y > 0.0 {
                            Vec3::NEG_Y
                        } else {
                            Vec3::Y
                        };
                    } else {
                        best_hit_normal = if ray_dir.z > 0.0 {
                            Vec3::NEG_Z
                        } else {
                            Vec3::Z
                        };
                    }
                }
            }
        }

        if !hit_found {
            return;
        }

        let hit = ray_origin + ray_dir * min_t;
        let target = hit + best_hit_normal * 0.01;
        let bounds = self.editor.bounds;
        let next_cursor = [
            (target.x.floor() as i32).clamp(-bounds, bounds),
            (target.y.floor() as i32).clamp(-bounds, bounds),
            (target.z.floor() as i32).max(0),
        ];

        if next_cursor != self.editor.cursor {
            self.editor.cursor = next_cursor;
            self.rebuild_editor_cursor_vertices();
        }
    }

    pub fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        if self.phase != AppPhase::Editor || !self.editor_right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;
        self.editor_camera_rotation -= dx as f32 * ROTATE_SPEED;
        self.editor_camera_pitch =
            (self.editor_camera_pitch + dy as f32 * PITCH_SPEED).clamp(10.0f32.to_radians(), 85.0f32.to_radians());
    }

    pub fn move_editor_up(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, 1);
        }
    }

    pub fn move_editor_down(&mut self) {
        if self.phase == AppPhase::Editor {
            self.move_editor_cursor(0, -1);
        }
    }

    pub fn editor_remove_block(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let cursor = self.editor.cursor;
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.editor_objects.iter().enumerate() {
            let occupies_x = cursor[0] as f32 + 0.5 >= obj.position[0]
                && cursor[0] as f32 + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_y = cursor[1] as f32 + 0.5 >= obj.position[1]
                && cursor[1] as f32 + 0.5 <= obj.position[1] + obj.size[1];
            if occupies_x && occupies_y {
                let top = obj.position[2] + obj.size[2];
                if top > top_height {
                    top_height = top;
                    top_index = Some(index);
                }
            }
        }

        if let Some(index) = top_index {
            self.editor_objects.remove(index);
        }

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub fn editor_playtest(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.stop_audio();
        self.playtesting_editor = true;
        self.game = GameState::new();
        self.game.objects = self.editor_objects.clone();
        self.apply_spawn_to_game(self.editor_spawn.position, self.editor_spawn.direction);
        self.phase = AppPhase::Playing;
        self.editor_right_dragging = false;
        self.clear_editor_pan_keys();
        self.rebuild_block_vertices();
    }

    pub fn editor_set_spawn_here(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let cursor = self.editor.cursor;
        self.editor_spawn.position = [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32];

        self.sync_editor_objects();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor_spawn.direction = match self.editor_spawn.direction {
            SpawnDirection::Forward => SpawnDirection::Right,
            SpawnDirection::Right => SpawnDirection::Forward,
        };
        self.rebuild_spawn_marker_vertices();
    }

    pub fn back_to_menu(&mut self) {
        self.stop_audio();
        self.playtesting_editor = false;
        self.editor_right_dragging = false;
        self.clear_editor_pan_keys();
        self.phase = AppPhase::Menu;
    }

    fn start_level(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();

        self.game = GameState::new();
        self.phase = AppPhase::Playing;
        self.playtesting_editor = false;
        self.clear_editor_pan_keys();

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            log::debug!("Starting level: {}", metadata.name);
            self.game.objects = metadata.objects;
            self.apply_spawn_to_game(metadata.spawn.position, metadata.spawn.direction);

            #[cfg(target_arch = "wasm32")]
            {
                let audio_url = format!("assets/levels/{}/{}", level_name, metadata.music.source);
                if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&audio_url) {
                    let _ = audio.play();
                    self.current_audio = Some(audio);
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(handle) = &self.audio_output_handle {
                    let audio_path =
                        format!("assets/levels/{}/{}", level_name, metadata.music.source);

                    match std::fs::read(&audio_path) {
                        Ok(audio_bytes) => match Decoder::new(Cursor::new(audio_bytes)) {
                            Ok(source) => match Sink::try_new(handle) {
                                Ok(sink) => {
                                    sink.append(source);
                                    sink.play();
                                    self.current_audio_sink = Some(sink);
                                }
                                Err(err) => {
                                    log::warn!(
                                        "Failed to create audio sink for '{}': {}",
                                        audio_path,
                                        err
                                    );
                                }
                            },
                            Err(err) => {
                                log::warn!(
                                    "Failed to decode level music '{}': {}",
                                    audio_path,
                                    err
                                );
                            }
                        },
                        Err(err) => {
                            log::warn!("Failed to read level music '{}': {}", audio_path, err);
                        }
                    }
                }
            }
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();
        self.stop_audio();

        self.phase = AppPhase::Editor;
        self.playtesting_editor = false;
        self.editor_right_dragging = false;
        self.clear_editor_pan_keys();
        self.editor_camera_rotation = -45.0f32.to_radians();
        self.editor_camera_pitch = 45.0f32.to_radians();
        self.editor_zoom = 1.0;
        self.game = GameState::new();
        self.trail_vertex_count = 0;

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            self.editor_objects = metadata.objects;
            self.editor_spawn = metadata.spawn;
        } else {
            self.editor_objects = Vec::new();
            self.editor_spawn = SpawnMetadata::default();
        }

        if let Some(first) = self.editor_objects.first() {
            self.editor.cursor = [
                first.position[0].round() as i32,
                first.position[1].round() as i32,
                first.position[2].round() as i32,
            ];
        } else {
            self.editor.cursor = [0, 0, 0];
        }

        self.editor_camera_pan = [self.editor.cursor[0] as f32 + 0.5, self.editor.cursor[1] as f32 + 0.5];

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    fn level_metadata_str(level_name: &str) -> &'static str {
        match level_name {
            "Flowerfield" => include_str!("../assets/levels/Flowerfield/metadata.json"),
            "Golden Haze" => include_str!("../assets/levels/Golden Haze/metadata.json"),
            _ => "{\"name\": \"Unknown\", \"music\": {\"source\": \"\"}, \"objects\": []}",
        }
    }

    fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        serde_json::from_str::<LevelMetadata>(Self::level_metadata_str(level_name)).ok()
    }

    fn stop_audio(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = self.current_audio.take() {
            let _ = audio.pause();
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sink) = self.current_audio_sink.take() {
            sink.stop();
        }
    }

    fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        let bounds = self.editor.bounds;
        self.editor.cursor[0] = (self.editor.cursor[0] + dx).clamp(-bounds, bounds);
        self.editor.cursor[1] = (self.editor.cursor[1] + dy).clamp(-bounds, bounds);
        self.rebuild_editor_cursor_vertices();
    }

    fn cell_top_height(&self, cell_x: i32, cell_y: i32) -> f32 {
        let sample_x = cell_x as f32 + 0.5;
        let sample_y = cell_y as f32 + 0.5;
        self.editor_objects
            .iter()
            .filter(|obj| {
                sample_x >= obj.position[0]
                    && sample_x <= obj.position[0] + obj.size[0]
                    && sample_y >= obj.position[1]
                    && sample_y <= obj.position[1] + obj.size[1]
            })
            .map(|obj| obj.position[2] + obj.size[2])
            .fold(0.0, f32::max)
    }

    fn place_editor_block(&mut self) {
        let cursor = self.editor.cursor;

        self.editor_objects.push(LevelObject {
            position: [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32],
            size: [1.0, 1.0, 1.0],
        });
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    fn sync_editor_objects(&mut self) {
        self.game.objects = self.editor_objects.clone();
        self.rebuild_block_vertices();
    }

    fn apply_spawn_to_game(&mut self, position: [f32; 3], direction: SpawnDirection) {
        let centered_position = [position[0].floor() + 0.5, position[1].floor() + 0.5, position[2]];
        self.game.position = centered_position;
        self.game.direction = direction.into();
        self.game.vertical_velocity = 0.0;
        self.game.is_grounded = true;
        self.game.trail_segments = vec![vec![centered_position]];
    }

    fn rebuild_editor_cursor_vertices(&mut self) {
        let vertices = build_editor_cursor_vertices(self.editor.cursor);
        self.editor_cursor_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_cursor_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Cursor Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_cursor_vertex_buffer = None;
        }
    }

    fn rebuild_spawn_marker_vertices(&mut self) {
        let vertices = build_spawn_marker_vertices(
            self.editor_spawn.position,
            matches!(self.editor_spawn.direction, SpawnDirection::Right),
        );
        self.spawn_marker_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.spawn_marker_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Spawn Marker Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.spawn_marker_vertex_buffer = None;
        }
    }

    fn rebuild_block_vertices(&mut self) {
        let vertices = build_block_vertices(&self.game.objects);

        self.block_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.block_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Block Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
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
            self.accumulator = 0.0;
            self.update_menu_camera();
            return;
        }

        if self.phase == AppPhase::Editor {
            self.accumulator = 0.0;
            self.trail_vertex_count = 0;
            self.update_editor_pan_from_keys(frame_dt);
            self.update_editor_camera();
            return;
        }

        while self.accumulator >= FIXED_DT {
            self.game.update(FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        if self.game.game_over {
            self.stop_audio();
        }

        let mut trail_vertices = Vec::new();
        for (segment_index, segment) in self.game.trail_segments.iter().enumerate() {
            let mut points = segment.clone();
            if segment_index + 1 == self.game.trail_segments.len() && self.game.is_grounded {
                points.push(self.game.position);
            }
            trail_vertices.extend(build_trail_vertices(&points, self.game.game_over));
        }

        if !self.game.is_grounded {
            let head_length = 0.22;
            let dir = match self.game.direction {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                self.game.position[0] - dir[0] * head_length,
                self.game.position[1] - dir[1] * head_length,
                self.game.position[2],
            ];
            let head_points = [head_start, self.game.position];
            trail_vertices.extend(build_trail_vertices(&head_points, self.game.game_over));
        }

        self.trail_vertex_count = trail_vertices.len() as u32;
        if !trail_vertices.is_empty() {
            let max_vertices =
                (self.trail_vertex_buffer.size() / std::mem::size_of::<Vertex>() as u64) as usize;
            let vertices_to_write = &trail_vertices[..trail_vertices.len().min(max_vertices)];
            self.queue.write_buffer(
                &self.trail_vertex_buffer,
                0,
                bytemuck::cast_slice(vertices_to_write),
            );
        }

        self.line_uniform.offset = [
            (self.game.position[0] * 100.0).round() / 100.0,
            (self.game.position[1] * 100.0).round() / 100.0,
        ];
        self.line_uniform.rotation = match self.game.direction {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        self.queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.line_uniform),
        );

        let aspect = self.config.width as f32 / self.config.height as f32;
        let pos_3d = Vec3::new(self.game.position[0], self.game.position[1], self.game.position[2]);
        let target = pos_3d;
        let offset = Mat4::from_rotation_z(-45.0f32.to_radians())
            .transform_vector3(Vec3::new(0.0, -20.0, 20.0));
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
        let time = (Instant::now() - self.app_start).as_secs_f32();

        let aspect = self.config.width as f32 / self.config.height as f32;
        let radius = 100.0;
        let eye = Vec3::new(
            radius * (time * 0.2).cos(),
            radius * (time * 0.2).sin(),
            50.0,
        );
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

    fn update_editor_camera(&mut self) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let offset = self.editor_camera_offset();
        let eye = target + offset;
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
                AppPhase::Playing if self.game.game_over => wgpu::Color {
                    r: 0.15,
                    g: 0.05,
                    b: 0.05,
                    a: 1.0,
                },
                AppPhase::Editor => wgpu::Color {
                    r: 0.04,
                    g: 0.07,
                    b: 0.09,
                    a: 1.0,
                },
                _ => wgpu::Color {
                    r: 0.05,
                    g: 0.05,
                    b: 0.08,
                    a: 1.0,
                },
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

            if self.phase == AppPhase::Playing
                || self.phase == AppPhase::GameOver
                || self.phase == AppPhase::Editor
            {
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

                if self.phase == AppPhase::Editor {
                    if let Some(buf) = &self.spawn_marker_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.spawn_marker_vertex_count, 0..1);
                    }

                    if let Some(buf) = &self.editor_cursor_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_cursor_vertex_count, 0..1);
                    }
                }
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn handle_surface_lost(&mut self) {
        let size = self.size;
        self.apply_resize(size);
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
            self.apply_resize(PhysicalSize::new(new_size.width, new_size.height));
        }
    }

    pub fn recreate_surface(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let window = self.window();
            let size = PhysicalSize::new(window.inner_size().width, window.inner_size().height);
            self.apply_resize(size);
        }
    }

    fn apply_resize(&mut self, new_size: PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
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
