use std::iter;

use glam::{Mat4, Vec2, Vec3, Vec4};
use wgpu::{util::DeviceExt, SurfaceError, TextureViewDescriptor};

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};

use crate::editor_domain::{
    add_tap_step, build_editor_playtest_transition, build_playing_transition_from_metadata,
    clear_tap_steps, create_block_at_cursor, derive_timeline_position,
    editor_session_init_from_metadata, move_cursor_xy, playtest_return_objects, remove_tap_step,
    remove_topmost_block_at_cursor, toggle_spawn_direction,
};
use crate::game::{create_menu_scene, GameState};
use crate::level_repository::{
    build_ldz_archive, builtin_level_names, load_builtin_level_metadata, parse_level_metadata_json,
    read_metadata_from_ldz, serialize_level_metadata_pretty,
};
use crate::mesh::{
    build_block_vertices, build_editor_cursor_vertices, build_editor_gizmo_vertices,
    build_editor_selection_outline_vertices, build_floor_vertices, build_grid_vertices,
    build_spawn_marker_vertices, build_trail_vertices,
};
use crate::platform::audio::PlatformAudio;
use crate::platform::io::{log_platform_error, read_editor_music_bytes, save_level_export};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{log_backend, PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, BlockKind, CameraUniform, ColorSpaceUniform, Direction, EditorMode, EditorState,
    LevelMetadata, LevelObject, LineUniform, MenuState, MusicMetadata, PhysicalSize,
    SpawnDirection, SpawnMetadata, Vertex,
};

use base64::Engine as _;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct State {
    surface_host: SurfaceHost,
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
    editor_selection_outline_vertex_buffer: Option<wgpu::Buffer>,
    editor_selection_outline_vertex_count: u32,
    editor_gizmo_vertex_buffer: Option<wgpu::Buffer>,
    editor_gizmo_vertex_count: u32,
    spawn_marker_vertex_buffer: Option<wgpu::Buffer>,
    spawn_marker_vertex_count: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    line_uniform_buffer: wgpu::Buffer,
    zero_line_bind_group: wgpu::BindGroup,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    color_space_bind_group: wgpu::BindGroup,
    apply_gamma_correction: bool,
    game: GameState,
    phase: AppPhase,
    menu: MenuState,
    editor: EditorState,
    editor_selected_kind: BlockKind,
    editor_objects: Vec<LevelObject>,
    editor_spawn: SpawnMetadata,
    editor_camera_pan: [f32; 2],
    editor_camera_rotation: f32,
    editor_camera_pitch: f32,
    playing_camera_rotation: f32,
    playing_camera_pitch: f32,
    editor_zoom: f32,
    editor_timeline_step: u32,
    editor_timeline_length: u32,
    editor_tap_steps: Vec<u32>,
    editor_right_dragging: bool,
    editor_pan_up_held: bool,
    editor_pan_down_held: bool,
    editor_pan_left_held: bool,
    editor_pan_right_held: bool,
    editor_shift_held: bool,
    editor_mode: EditorMode,
    editor_snap_to_grid: bool,
    editor_selected_block_index: Option<usize>,
    editor_gizmo_drag: Option<EditorGizmoDrag>,
    editor_block_drag: Option<EditorBlockDrag>,
    editor_level_name: Option<String>,
    editor_music_metadata: MusicMetadata,
    editor_show_metadata: bool,
    playing_level_name: Option<String>,
    editor_show_import: bool,
    editor_import_text: String,
    playtesting_editor: bool,
    line_uniform: LineUniform,
    last_frame: PlatformInstant,
    accumulator: f32,
    audio: PlatformAudio,
    local_audio_cache: std::collections::HashMap<String, Vec<u8>>,
    audio_import_channel: (
        std::sync::mpsc::Sender<AudioImportData>,
        std::sync::mpsc::Receiver<AudioImportData>,
    ),
}

type AudioImportData = (String, Vec<u8>);

struct EditorPickResult {
    cursor: [i32; 3],
    hit_block_index: Option<usize>,
}

#[derive(Clone, Copy)]
enum GizmoAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy)]
enum GizmoDragKind {
    Move,
    Resize,
}

#[derive(Clone, Copy)]
struct EditorGizmoDrag {
    axis: GizmoAxis,
    kind: GizmoDragKind,
    start_mouse: [f64; 2],
    start_position: [f32; 3],
    start_size: [f32; 3],
}

#[derive(Clone, Copy)]
struct EditorBlockDrag {
    start_mouse: [f64; 2],
    start_position: [f32; 3],
}

impl State {
    #[cfg(target_arch = "wasm32")]
    pub(crate) async fn new(canvas: WasmCanvas) -> Self {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_wasm(canvas);
        Self::new_common(instance, surface_host, surface, size).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_native(window: NativeWindow) -> State {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_native(window);
        Self::new_common(instance, surface_host, surface, size).await
    }

    async fn new_common(
        instance: wgpu::Instance,
        surface_host: SurfaceHost,
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
        log_backend(&adapter_info);

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

        let shader: wgpu::ShaderModule =
            device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

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

        let should_apply_gamma_correction =
            !surface_format.is_srgb() && adapter_info.backend == wgpu::Backend::BrowserWebGpu;

        let color_space_uniform = ColorSpaceUniform {
            apply_gamma_correction: if should_apply_gamma_correction {
                1.0
            } else {
                0.0
            },
            _pad: [0.0; 3],
        };

        let color_space_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Color Space Uniform Buffer"),
                contents: bytemuck::bytes_of(&color_space_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
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

        let color_space_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Color Space Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
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

        let color_space_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Color Space Bind Group"),
            layout: &color_space_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: color_space_uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &line_bind_group_layout,
                &color_space_bind_group_layout,
            ],
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let menu = MenuState {
            selected_level: 0,
            levels: builtin_level_names(),
        };

        let mut game = GameState::new();
        game.objects = create_menu_scene();

        let local_audio_cache = crate::platform::io::load_all_local_audio().await;

        let block_vertices = build_block_vertices(&game.objects);
        let block_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Vertex Buffer"),
            contents: bytemuck::cast_slice(&block_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let block_vertex_count = block_vertices.len() as u32;

        let now = PlatformInstant::now();

        Self {
            surface_host,
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
            color_space_bind_group,
            apply_gamma_correction: should_apply_gamma_correction,
            game,
            phase: AppPhase::Menu,
            menu,
            line_uniform,
            last_frame: now,
            accumulator: 0.0,
            audio: PlatformAudio::new(),
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            trail_vertex_buffer,
            trail_vertex_count: 0,
            block_vertex_buffer: Some(block_vertex_buffer),
            block_vertex_count,
            editor_cursor_vertex_buffer: None,
            editor_cursor_vertex_count: 0,
            editor_selection_outline_vertex_buffer: None,
            editor_selection_outline_vertex_count: 0,
            editor_gizmo_vertex_buffer: None,
            editor_gizmo_vertex_count: 0,
            spawn_marker_vertex_buffer: None,
            spawn_marker_vertex_count: 0,
            editor: EditorState::new(),
            editor_selected_kind: BlockKind::Standard,
            editor_objects: Vec::new(),
            editor_spawn: SpawnMetadata::default(),
            editor_camera_pan: [0.0, 0.0],
            editor_camera_rotation: -45.0f32.to_radians(),
            editor_camera_pitch: 45.0f32.to_radians(),
            playing_camera_rotation: -45.0f32.to_radians(),
            playing_camera_pitch: 45.0f32.to_radians(),
            editor_zoom: 1.0,
            editor_timeline_step: 0,
            editor_timeline_length: 64,
            editor_tap_steps: Vec::new(),
            editor_right_dragging: false,
            editor_pan_up_held: false,
            editor_pan_down_held: false,
            editor_pan_left_held: false,
            editor_pan_right_held: false,
            editor_shift_held: false,
            editor_mode: EditorMode::Place,
            editor_snap_to_grid: true,
            editor_selected_block_index: None,
            editor_gizmo_drag: None,
            editor_block_drag: None,
            editor_level_name: None,
            editor_music_metadata: MusicMetadata {
                source: "music.mp3".to_string(),
                title: None,
                author: None,
                extra: serde_json::Map::new(),
            },
            editor_show_metadata: false,
            playing_level_name: None,
            editor_show_import: false,
            editor_import_text: String::new(),
            playtesting_editor: false,
            local_audio_cache,
            audio_import_channel: std::sync::mpsc::channel(),
        }
    }

    pub(crate) fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.surface_host.prepare_resize(new_size);
        self.apply_resize(new_size);
    }

    pub fn turn_right(&mut self) {
        match self.phase {
            AppPhase::Menu => {
                self.start_level(self.menu.selected_level);
            }
            AppPhase::Playing => {
                if !self.game.started {
                    self.game.started = true;
                    if self.playtesting_editor {
                        let metadata = self.current_editor_metadata();
                        let level_name = self
                            .editor_level_name
                            .clone()
                            .unwrap_or_else(|| "Untitled".to_string());
                        self.start_audio(&level_name, &metadata);
                    } else if let Some(level_name) = self.playing_level_name.clone() {
                        if let Some(metadata) = self.load_level_metadata(&level_name) {
                            self.start_audio(&level_name, &metadata);
                        }
                    }
                } else if self.game.game_over {
                    self.restart_level();
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
            AppPhase::Editor => self.back_to_menu(),
            AppPhase::Playing if self.playtesting_editor => {
                self.phase = AppPhase::Editor;
                self.stop_audio();
                self.sync_editor_objects();
            }
            _ => {}
        }
    }

    pub fn is_editor(&self) -> bool {
        self.phase == AppPhase::Editor
    }

    pub fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor_right_dragging = dragging;
    }

    pub fn handle_keyboard_input(&mut self, key: &str, pressed: bool, just_pressed: bool) {
        if key == "Shift" {
            self.set_editor_shift_held(pressed);
            return;
        }

        if !pressed {
            match key {
                "ArrowUp" | "w" | "W" => self.set_editor_pan_up_held(false),
                "ArrowDown" | "s" | "S" => self.set_editor_pan_down_held(false),
                "ArrowLeft" | "a" | "A" => self.set_editor_pan_left_held(false),
                "ArrowRight" | "d" | "D" => self.set_editor_pan_right_held(false),
                _ => {}
            }
            return;
        }

        match key {
            "ArrowUp" | "w" | "W" => {
                if self.is_editor() {
                    self.set_editor_pan_up_held(true);
                } else if just_pressed {
                    self.turn_right();
                }
            }
            "ArrowDown" | "s" | "S" => {
                if self.is_editor() {
                    self.set_editor_pan_down_held(true);
                }
            }
            " " | "Space" => {
                if just_pressed {
                    self.turn_right();
                }
            }
            "ArrowRight" | "d" | "D" => {
                if self.is_editor() {
                    self.set_editor_pan_right_held(true);
                } else if just_pressed {
                    self.next_level();
                }
            }
            "ArrowLeft" | "a" | "A" => {
                if self.is_editor() {
                    self.set_editor_pan_left_held(true);
                } else if just_pressed {
                    self.prev_level();
                }
            }
            "Enter" => {
                if just_pressed {
                    self.editor_playtest();
                }
            }
            "Backspace" | "Delete" => {
                if just_pressed {
                    self.editor_remove_block();
                }
            }
            "Escape" => {
                if just_pressed {
                    self.back_to_menu();
                }
            }
            "e" | "E" => {
                if just_pressed {
                    self.toggle_editor();
                }
            }
            "p" | "P" => {
                if just_pressed {
                    self.editor_set_spawn_here();
                }
            }
            "r" | "R" => {
                if just_pressed {
                    self.editor_rotate_spawn_direction();
                }
            }
            "+" | "=" => {
                if just_pressed {
                    self.adjust_editor_zoom(1.0);
                }
            }
            "-" | "_" => {
                if just_pressed {
                    self.adjust_editor_zoom(-1.0);
                }
            }
            "1" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_kind(BlockKind::Standard);
                }
            }
            "2" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_kind(BlockKind::Grass);
                }
            }
            "3" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_kind(BlockKind::Dirt);
                }
            }
            "4" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_kind(BlockKind::Void);
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse_button(&mut self, button: u32, pressed: bool) {
        match button {
            0 => {
                if !pressed {
                    self.editor_gizmo_drag = None;
                    self.editor_block_drag = None;
                } else {
                    self.turn_right();
                }
            }
            2 => {
                self.set_editor_right_dragging(pressed);
            }
            _ => {}
        }
    }

    pub fn handle_primary_click(&mut self, x: f64, y: f64) {
        if self.phase == AppPhase::Editor {
            match self.editor_mode {
                EditorMode::Place => {
                    self.update_editor_cursor_from_screen(x, y);
                    self.place_editor_block();
                }
                EditorMode::Select => {
                    if self.begin_editor_gizmo_drag(x, y) {
                        return;
                    }
                    if self.begin_editor_selected_block_drag(x, y) {
                        return;
                    }
                    self.select_editor_block_from_screen(x, y);
                }
            }
            return;
        }

        self.turn_right();
    }

    pub fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return false;
        }

        let Some(drag) = self.editor_gizmo_drag else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let center = Vec3::new(
            drag.start_position[0] + drag.start_size[0] * 0.5,
            drag.start_position[1] + drag.start_size[1] * 0.5,
            drag.start_position[2] + drag.start_size[2] * 0.5,
        );
        let axis_dir = match drag.axis {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        };

        let Some(origin_screen) = self.world_to_screen(center) else {
            self.editor_gizmo_drag = Some(drag);
            return true;
        };
        let Some(axis_screen) = self.world_to_screen(center + axis_dir) else {
            self.editor_gizmo_drag = Some(drag);
            return true;
        };

        let axis_screen_delta = axis_screen - origin_screen;
        if axis_screen_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let axis_screen_dir = axis_screen_delta.normalize();
        let projected_pixels = mouse_delta.dot(axis_screen_dir);
        let world_delta = projected_pixels * (0.012 / self.editor_zoom.max(0.25));

        match drag.kind {
            GizmoDragKind::Move => {
                let mut position = drag.start_position;
                match drag.axis {
                    GizmoAxis::X => position[0] += world_delta,
                    GizmoAxis::Y => position[1] += world_delta,
                    GizmoAxis::Z => position[2] += world_delta,
                }
                self.set_editor_selected_block_position(position);
            }
            GizmoDragKind::Resize => {
                let mut size = drag.start_size;
                match drag.axis {
                    GizmoAxis::X => size[0] += world_delta,
                    GizmoAxis::Y => size[1] += world_delta,
                    GizmoAxis::Z => size[2] += world_delta,
                }
                self.set_editor_selected_block_size(size);
            }
        }
        true
    }

    pub fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.drag_editor_gizmo_from_screen(x, y) {
            return true;
        }

        if self.phase != AppPhase::Editor
            || self.editor_right_dragging
            || self.editor_mode != EditorMode::Select
        {
            return false;
        }

        let Some(drag) = self.editor_block_drag else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let (camera_right_xy, camera_up_xy) = self.editor_camera_axes_xy();
        let world_delta_xy = camera_right_xy * mouse_delta.x + camera_up_xy * -mouse_delta.y;
        let world_scale = 0.012 / self.editor_zoom.max(0.25);

        let mut position = drag.start_position;
        position[0] += world_delta_xy.x * world_scale;
        position[1] += world_delta_xy.y * world_scale;
        self.set_editor_selected_block_position(position);
        true
    }

    pub fn render_egui(
        &mut self,
        renderer: &mut EguiRenderer,
        paint_jobs: &[egui::ClippedPrimitive],
        screen_descriptor: &ScreenDescriptor,
    ) -> Result<(), SurfaceError> {
        self.render_with_overlay(|device, queue, view, encoder| {
            renderer.update_buffers(device, queue, encoder, paint_jobs, screen_descriptor);

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

            renderer.render(&mut pass, paint_jobs, screen_descriptor);
        })
    }

    pub fn create_egui_renderer(&self) -> EguiRenderer {
        EguiRenderer::new(&self.device, self.config.format, None, 1, false)
    }

    fn clear_editor_pan_keys(&mut self) {
        self.editor_pan_up_held = false;
        self.editor_pan_down_held = false;
        self.editor_pan_left_held = false;
        self.editor_pan_right_held = false;
        self.editor_shift_held = false;
    }

    fn reset_playing_camera_defaults(&mut self) {
        self.playing_camera_rotation = -45.0f32.to_radians();
        self.playing_camera_pitch = 45.0f32.to_radians();
    }

    fn enter_playing_phase(&mut self, level_name: Option<String>, playtesting_editor: bool) {
        self.phase = AppPhase::Playing;
        self.playtesting_editor = playtesting_editor;
        self.playing_level_name = level_name;
        self.reset_playing_camera_defaults();
        self.clear_editor_pan_keys();
    }

    fn enter_editor_phase(&mut self, level_name: String) {
        self.phase = AppPhase::Editor;
        self.editor_level_name = Some(level_name);
        self.playtesting_editor = false;
        self.editor_right_dragging = false;
        self.editor_mode = EditorMode::Place;
        self.editor_selected_block_index = None;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        self.clear_editor_pan_keys();
        self.editor_camera_rotation = -45.0f32.to_radians();
        self.editor_camera_pitch = 45.0f32.to_radians();
        self.editor_zoom = 1.0;
        self.game = GameState::new();
        self.trail_vertex_count = 0;
    }

    fn enter_menu_phase(&mut self) {
        self.playtesting_editor = false;
        self.editor_level_name = None;
        self.editor_selected_block_index = None;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        self.playing_level_name = None;
        self.editor_right_dragging = false;
        self.clear_editor_pan_keys();
        self.phase = AppPhase::Menu;
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

    pub fn set_editor_block_kind(&mut self, kind: BlockKind) {
        self.editor_selected_kind = kind;
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor_mode = mode;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;
        if mode == EditorMode::Place {
            self.editor_selected_block_index = None;
        }
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor_mode
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor_snap_to_grid
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor_snap_to_grid = snap;
        if self.editor_selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.editor_selected_block_index
            .and_then(|index| self.editor_objects.get(index).cloned())
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let bounds = self.editor.bounds;
            let next_position = if self.editor_snap_to_grid {
                [
                    position[0].round(),
                    position[1].round(),
                    position[2].max(0.0).round(),
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.editor_objects[index].position = next_position;
            self.editor.cursor = [
                (next_position[0].floor() as i32).clamp(-bounds, bounds),
                (next_position[1].floor() as i32).clamp(-bounds, bounds),
                (next_position[2].floor() as i32).max(0),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_size(&mut self, size: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            let snapped_size = if self.editor_snap_to_grid {
                [size[0].round(), size[1].round(), size[2].round()]
            } else {
                size
            };
            let min_size = if self.editor_snap_to_grid { 1.0 } else { 0.25 };
            self.editor_objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_kind(&mut self, kind: BlockKind) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        {
            self.editor_objects[index].kind = kind;
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub fn editor_selected_block_kind(&self) -> BlockKind {
        self.editor_selected_kind
    }

    pub fn editor_timeline_step(&self) -> u32 {
        self.editor_timeline_step
    }

    pub fn editor_timeline_length(&self) -> u32 {
        self.editor_timeline_length
    }

    pub fn editor_tap_steps(&self) -> &[u32] {
        &self.editor_tap_steps
    }

    pub fn set_editor_timeline_step(&mut self, step: u32) {
        let max_step = self.editor_timeline_length.saturating_sub(1);
        self.editor_timeline_step = step.min(max_step);
        self.refresh_editor_timeline_position();
    }

    pub fn set_editor_timeline_length(&mut self, length: u32) {
        let length = length.max(1);
        let max_step = length.saturating_sub(1);
        self.editor_timeline_length = length;
        self.editor_timeline_step = self.editor_timeline_step.min(max_step);
        self.editor_tap_steps.retain(|step| *step < length);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_add_tap(&mut self) {
        add_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_remove_tap(&mut self) {
        remove_tap_step(&mut self.editor_tap_steps, self.editor_timeline_step);
        self.refresh_editor_timeline_position();
    }

    pub fn editor_clear_taps(&mut self) {
        clear_tap_steps(&mut self.editor_tap_steps);
        self.refresh_editor_timeline_position();
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        self.editor_timeline_position(self.editor_timeline_step)
    }

    fn editor_camera_axes_xy(&self) -> (Vec2, Vec2) {
        let right = Vec2::new(
            self.editor_camera_rotation.cos(),
            self.editor_camera_rotation.sin(),
        );
        let up = Vec2::new(
            -self.editor_camera_rotation.sin(),
            self.editor_camera_rotation.cos(),
        );
        (right, up)
    }

    fn editor_camera_offset(&self) -> Vec3 {
        let zoom = self.editor_zoom.clamp(0.35, 4.0);
        let distance = 24.0 / zoom;
        let pitch = self
            .editor_camera_pitch
            .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(self.editor_camera_rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
    }

    fn playing_camera_offset(&self) -> Vec3 {
        let distance = 28.28;
        let rotation = if self.game.game_over || !self.game.started {
            self.playing_camera_rotation
        } else {
            -45.0f32.to_radians()
        };
        let pitch = if self.game.game_over || !self.game.started {
            self.playing_camera_pitch
        } else {
            45.0f32.to_radians()
        };

        let horizontal_distance = distance * pitch.cos();
        let vertical_distance = distance * pitch.sin();
        Mat4::from_rotation_z(rotation).transform_vector3(Vec3::new(
            0.0,
            -horizontal_distance,
            vertical_distance,
        ))
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
        self.editor_camera_pan[0] =
            (self.editor_camera_pan[0] + world_delta.x).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] =
            (self.editor_camera_pan[1] + world_delta.y).clamp(-max_pan, max_pan);
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

        self.adjust_editor_zoom(
            input.y * vertical_factor * PAN_SPEED_UNITS_PER_SEC * frame_dt * speed_multiplier,
        );
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return;
        }

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            return;
        };

        if pick.cursor != self.editor.cursor {
            self.editor.cursor = pick.cursor;
            self.rebuild_editor_cursor_vertices();
        }
    }

    fn begin_editor_gizmo_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            return false;
        }

        let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            return false;
        };

        let Some((kind, axis)) = self.pick_editor_gizmo_handle(x, y) else {
            return false;
        };

        let obj = self.editor_objects[index].clone();

        self.editor_gizmo_drag = Some(EditorGizmoDrag {
            axis,
            kind,
            start_mouse: [x, y],
            start_position: obj.position,
            start_size: obj.size,
        });
        true
    }

    fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            return false;
        }

        let Some(selected_index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            return false;
        };

        let obj = self.editor_objects[selected_index].clone();

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            return false;
        };

        if pick.hit_block_index == Some(selected_index) {
            self.editor_block_drag = Some(EditorBlockDrag {
                start_mouse: [x, y],
                start_position: obj.position,
            });
            return true;
        }

        false
    }

    fn editor_view_proj(&self) -> Option<Mat4> {
        if self.config.width == 0 || self.config.height == 0 {
            return None;
        }

        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let eye = target + self.editor_camera_offset();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        Some(proj * view)
    }

    fn world_to_screen(&self, world: Vec3) -> Option<Vec2> {
        let view_proj = self.editor_view_proj()?;
        let clip = view_proj * world.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }

        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            return None;
        }

        let screen_x = (ndc.x + 1.0) * 0.5 * self.config.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.config.height as f32;
        Some(Vec2::new(screen_x, screen_y))
    }

    fn pick_editor_gizmo_handle(&self, x: f64, y: f64) -> Option<(GizmoDragKind, GizmoAxis)> {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            return None;
        }

        let index = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())?;
        let obj = &self.editor_objects[index];

        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let arm_length = obj.size[0].max(obj.size[1]).max(obj.size[2]).max(1.0) * 0.9;
        let pointer = Vec2::new(x as f32, y as f32);

        let candidates = [
            (
                GizmoDragKind::Move,
                GizmoAxis::X,
                center + Vec3::new(arm_length, 0.0, 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::Y,
                center + Vec3::new(0.0, arm_length, 0.0),
            ),
            (
                GizmoDragKind::Move,
                GizmoAxis::Z,
                center + Vec3::new(0.0, 0.0, arm_length),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::X,
                Vec3::new(obj.position[0] + obj.size[0] + 0.36, center.y, center.z),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::Y,
                Vec3::new(center.x, obj.position[1] + obj.size[1] + 0.36, center.z),
            ),
            (
                GizmoDragKind::Resize,
                GizmoAxis::Z,
                Vec3::new(center.x, center.y, obj.position[2] + obj.size[2] + 0.36),
            ),
        ];

        let mut best: Option<(GizmoDragKind, GizmoAxis, f32)> = None;
        for (kind, axis, world) in candidates {
            if let Some(screen) = self.world_to_screen(world) {
                let dist = screen.distance(pointer);
                if dist <= 22.0 {
                    match best {
                        Some((.., best_dist)) if dist >= best_dist => {}
                        _ => best = Some((kind, axis, dist)),
                    }
                }
            }
        }

        best.map(|(kind, axis, _)| (kind, axis))
    }

    fn editor_pick_from_screen(&self, x: f64, y: f64) -> Option<EditorPickResult> {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return None;
        }

        if self.config.width == 0 || self.config.height == 0 {
            return None;
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
            return None;
        }

        near_world /= near_world.w;
        far_world /= far_world.w;

        let ray_origin = near_world.truncate();
        let ray_dir = (far_world.truncate() - ray_origin).normalize();

        let mut min_t = f32::INFINITY;
        let mut best_hit_normal = Vec3::Z;
        let mut hit_found = false;
        let mut hit_block_index: Option<usize> = None;

        if ray_dir.z.abs() > f32::EPSILON {
            let t = -ray_origin.z / ray_dir.z;
            if t >= 0.0 {
                min_t = t;
                hit_found = true;
            }
        }

        for (index, obj) in self.editor_objects.iter().enumerate() {
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
                    hit_block_index = Some(index);

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
            return None;
        }

        let hit = ray_origin + ray_dir * min_t;
        let target = hit + best_hit_normal * 0.01;
        let bounds = self.editor.bounds;
        let next_cursor = [
            (target.x.floor() as i32).clamp(-bounds, bounds),
            (target.y.floor() as i32).clamp(-bounds, bounds),
            (target.z.floor() as i32).max(0),
        ];

        Some(EditorPickResult {
            cursor: next_cursor,
            hit_block_index,
        })
    }

    fn select_editor_block_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(pick) = self.editor_pick_from_screen(x, y) else {
            self.editor_selected_block_index = None;
            self.editor_gizmo_drag = None;
            self.editor_block_drag = None;
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
            return;
        };

        self.editor_selected_block_index = pick.hit_block_index;
        self.editor_gizmo_drag = None;
        self.editor_block_drag = None;

        if let Some(index) = pick.hit_block_index {
            if let Some(obj) = self.editor_objects.get(index) {
                self.editor.cursor = [
                    obj.position[0].floor() as i32,
                    obj.position[1].floor() as i32,
                    obj.position[2].floor() as i32,
                ];
                self.rebuild_editor_cursor_vertices();
            }
        } else if pick.cursor != self.editor.cursor {
            self.editor.cursor = pick.cursor;
            self.rebuild_editor_cursor_vertices();
        }

        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        if !self.editor_right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;

        if self.phase == AppPhase::Editor {
            self.editor_camera_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor_camera_pitch = (self.editor_camera_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        } else if self.phase == AppPhase::Playing && (self.game.game_over || !self.game.started) {
            self.playing_camera_rotation -= dx as f32 * ROTATE_SPEED;
            self.playing_camera_pitch = (self.playing_camera_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        }
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

        if let Some(index) = self.editor_selected_block_index {
            if index < self.editor_objects.len() {
                self.editor_objects.remove(index);
            }
            self.editor_selected_block_index = None;
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            return;
        }

        remove_topmost_block_at_cursor(&mut self.editor_objects, self.editor.cursor);

        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub fn editor_playtest(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.stop_audio();

        let transition = build_editor_playtest_transition(
            &self.editor_objects,
            self.editor_level_name.as_deref(),
            self.editor_spawn.clone(),
            &self.editor_tap_steps,
            self.editor_timeline_step,
        );

        self.enter_playing_phase(transition.playing_level_name, true);
        self.game = GameState::new();
        self.game.objects = transition.objects;
        self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        self.playing_camera_rotation = transition.camera_rotation;
        self.playing_camera_pitch = transition.camera_pitch;
        self.editor_right_dragging = false;
        self.rebuild_block_vertices();
    }

    pub fn editor_set_spawn_here(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let cursor = self.editor.cursor;
        self.editor_spawn.position = [cursor[0] as f32, cursor[1] as f32, cursor[2] as f32];

        self.sync_editor_objects();
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn editor_rotate_spawn_direction(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.editor_spawn.direction = toggle_spawn_direction(self.editor_spawn.direction);
        self.refresh_editor_timeline_position();
        self.rebuild_spawn_marker_vertices();
    }

    pub fn back_to_menu(&mut self) {
        self.stop_audio();
        if let Some(objects) =
            playtest_return_objects(self.playtesting_editor, &self.editor_objects)
        {
            self.playtesting_editor = false;
            self.phase = AppPhase::Editor;
            self.game = GameState::new();
            self.game.objects = objects;
            self.rebuild_block_vertices();
            return;
        }

        self.enter_menu_phase();

        self.game = GameState::new();
        self.game.objects = create_menu_scene();
        self.rebuild_block_vertices();
        self.trail_vertex_count = 0;
    }

    fn start_level(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();

        self.game = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    fn restart_level(&mut self) {
        self.stop_audio();
        self.game = GameState::new();

        if self.playtesting_editor {
            let transition = build_editor_playtest_transition(
                &self.editor_objects,
                self.editor_level_name.as_deref(),
                self.editor_spawn.clone(),
                &self.editor_tap_steps,
                self.editor_timeline_step,
            );
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        } else if let Some(level_name) = self.playing_level_name.clone() {
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let transition = build_playing_transition_from_metadata(metadata);
                self.game.objects = transition.objects;
                self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
            }
        }

        self.game.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let init = editor_session_init_from_metadata(self.load_level_metadata(&level_name));
        self.editor_objects = init.objects;
        self.editor_spawn = init.spawn;
        self.editor_music_metadata = init.music;
        self.editor_tap_steps = init.tap_steps;
        self.editor_timeline_step = init.timeline_step;
        self.editor.cursor = init.cursor;
        self.editor_camera_pan = init.camera_pan;

        self.sync_editor_objects();
        // Refresh cursor/camera to match the current timeline step.
        self.set_editor_timeline_step(self.editor_timeline_step);
        self.rebuild_spawn_marker_vertices();
    }

    fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        load_builtin_level_metadata(level_name)
    }

    fn stop_audio(&mut self) {
        self.audio.stop();
    }

    fn start_audio(&mut self, level_name: &str, metadata: &LevelMetadata) {
        if let Some(bytes) = self.local_audio_cache.get(&metadata.music.source) {
            self.audio.start_with_bytes(&metadata.music.source, bytes);
        } else {
            self.audio.start(level_name, &metadata.music.source);
        }
    }

    pub fn trigger_audio_import(&self) {
        let sender = self.audio_import_channel.0.clone();
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                    let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                    let _ = sender.send((filename, bytes));
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                pollster::block_on(async {
                    if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                        let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                        let _ = sender.send((filename, bytes));
                    }
                });
            });
        }
    }

    pub fn update_audio_imports(&mut self) {
        while let Ok((filename, bytes)) = self.audio_import_channel.1.try_recv() {
            self.editor_music_metadata.source = filename.clone();
            self.local_audio_cache.insert(filename, bytes);
        }
    }

    pub fn export_level_ldz(&self) -> Result<Vec<u8>, String> {
        let metadata = self.current_editor_metadata();
        let audio_bytes = self
            .local_audio_cache
            .get(&metadata.music.source)
            .cloned()
            .or_else(|| {
                read_editor_music_bytes(self.editor_level_name.as_deref(), &metadata.music.source)
            });
        let audio_file = audio_bytes
            .as_ref()
            .map(|bytes| (metadata.music.source.as_str(), bytes.as_slice()));

        build_ldz_archive(&metadata, audio_file)
    }

    pub fn import_level_ldz(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = read_metadata_from_ldz(data)?;
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    pub fn export_level(&self) -> String {
        serialize_level_metadata_pretty(&self.current_editor_metadata()).unwrap_or_default()
    }

    pub fn import_level(&mut self, json: &str) -> Result<(), String> {
        let metadata = parse_level_metadata_json(json)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(
            self.editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            self.editor_music_metadata.clone(),
            self.editor_spawn.clone(),
            self.editor_tap_steps.clone(),
            self.editor_timeline_step,
            self.editor_objects.clone(),
        )
    }

    fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
        self.editor_objects = metadata.objects;
        self.editor_selected_block_index = None;
        self.editor_spawn = metadata.spawn;
        self.editor_tap_steps = metadata.taps;
        self.editor_tap_steps.sort_unstable();
        self.editor_timeline_step = metadata.timeline_step;
        self.editor_level_name = Some(metadata.name);
        self.editor_music_metadata = metadata.music;

        if let Some(first) = self.editor_objects.first() {
            self.editor.cursor = [
                first.position[0].round() as i32,
                first.position[1].round() as i32,
                first.position[2].round() as i32,
            ];
        } else {
            self.editor.cursor = [0, 0, 0];
        }

        self.editor_camera_pan = [
            self.editor.cursor[0] as f32 + 0.5,
            self.editor.cursor[1] as f32 + 0.5,
        ];

        self.sync_editor_objects();
        self.set_editor_timeline_step(self.editor_timeline_step);
        self.rebuild_spawn_marker_vertices();
    }

    pub fn load_builtin_level_into_editor(&mut self, name: &str) {
        if let Some(metadata) = self.load_level_metadata(name) {
            let _ = self.import_level(&serde_json::to_string(&metadata).unwrap());
            self.editor_level_name = Some(name.to_string());
        }
    }

    pub fn editor_level_name(&self) -> Option<String> {
        self.editor_level_name.clone()
    }

    pub fn set_editor_level_name(&mut self, name: String) {
        self.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.editor_music_metadata = metadata;
    }

    pub fn editor_show_import(&self) -> bool {
        self.editor_show_import
    }

    pub fn set_editor_show_import(&mut self, show: bool) {
        self.editor_show_import = show;
    }

    pub fn editor_import_text(&self) -> &str {
        &self.editor_import_text
    }

    pub fn set_editor_import_text(&mut self, text: String) {
        self.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.editor_show_metadata = show;
    }

    pub fn available_levels(&self) -> &[String] {
        &self.menu.levels
    }

    pub fn trigger_level_export(&self) {
        match self.export_level_ldz() {
            Ok(data) => {
                let filename = format!(
                    "{}.ldz",
                    self.editor_level_name()
                        .unwrap_or_else(|| "level".to_string())
                );

                if let Err(error) = save_level_export(&filename, &data) {
                    log_platform_error(&format!("Export failed: {}", error));
                }
            }
            Err(e) => {
                log_platform_error(&format!("Export failed: {}", e));
            }
        }
    }

    pub fn complete_import(&mut self) {
        let text = self.editor_import_text.clone();
        // Try LDZ first (base64)
        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            if let Err(e) = self.import_level_ldz(&data) {
                log_platform_error(&format!("LDZ Import failed: {}", e));
            } else {
                self.editor_show_import = false;
                self.editor_import_text.clear();
                return;
            }
        }

        // Fallback to raw JSON
        let text = self.editor_import_text.clone();
        if let Err(e) = self.import_level(&text) {
            log_platform_error(&format!("JSON Import failed: {}", e));
        } else {
            self.editor_show_import = false;
            self.editor_import_text.clear();
        }
    }

    fn move_editor_cursor(&mut self, dx: i32, dy: i32) {
        move_cursor_xy(&mut self.editor.cursor, dx, dy, self.editor.bounds);
        self.rebuild_editor_cursor_vertices();
    }

    fn place_editor_block(&mut self) {
        self.editor_objects.push(create_block_at_cursor(
            self.editor.cursor,
            self.editor_selected_kind,
        ));
        self.editor_selected_block_index = None;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    fn sync_editor_objects(&mut self) {
        if let Some(index) = self.editor_selected_block_index {
            if index >= self.editor_objects.len() {
                self.editor_selected_block_index = None;
            }
        }
        self.game.objects = self.editor_objects.clone();
        self.rebuild_block_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    fn apply_spawn_to_game(&mut self, position: [f32; 3], direction: SpawnDirection) {
        let centered_position = [
            position[0].floor() + 0.5,
            position[1].floor() + 0.5,
            position[2],
        ];
        self.game.position = centered_position;
        self.game.direction = direction.into();
        self.game.vertical_velocity = 0.0;
        self.game.is_grounded = true;
        self.game.trail_segments = vec![vec![centered_position]];
    }

    fn editor_timeline_position(&self, step: u32) -> ([f32; 3], SpawnDirection) {
        derive_timeline_position(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_tap_steps,
            step,
            &self.editor_objects,
        )
    }

    fn refresh_editor_timeline_position(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let (position, ..) = self.editor_timeline_position(self.editor_timeline_step);
        let bounds = self.editor.bounds;
        self.editor.cursor = [
            position[0].round() as i32,
            position[1].round() as i32,
            position[2].round() as i32,
        ];
        self.editor.cursor[0] = self.editor.cursor[0].clamp(-bounds, bounds);
        self.editor.cursor[1] = self.editor.cursor[1].clamp(-bounds, bounds);
        self.editor.cursor[2] = self.editor.cursor[2].max(0);

        let max_pan = bounds as f32;
        self.editor_camera_pan[0] = (position[0] + 0.5).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] = (position[1] + 0.5).clamp(-max_pan, max_pan);

        self.rebuild_editor_cursor_vertices();
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

    fn rebuild_editor_gizmo_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_gizmo_vertex_count = 0;
            self.editor_gizmo_vertex_buffer = None;
            return;
        }

        let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            self.editor_gizmo_vertex_count = 0;
            self.editor_gizmo_vertex_buffer = None;
            return;
        };

        let obj = &self.editor_objects[index];
        let vertices = build_editor_gizmo_vertices(obj.position, obj.size);
        self.editor_gizmo_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_gizmo_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Gizmo Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_gizmo_vertex_buffer = None;
        }
    }

    fn rebuild_editor_selection_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_selection_outline_vertex_count = 0;
            self.editor_selection_outline_vertex_buffer = None;
            return;
        }

        let Some(index) = self
            .editor_selected_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            self.editor_selection_outline_vertex_count = 0;
            self.editor_selection_outline_vertex_buffer = None;
            return;
        };

        let obj = &self.editor_objects[index];
        let vertices = build_editor_selection_outline_vertices(obj.position, obj.size);
        self.editor_selection_outline_vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.editor_selection_outline_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Editor Selection Outline Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        } else {
            self.editor_selection_outline_vertex_buffer = None;
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
        self.update_audio_imports();
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = PlatformInstant::now();
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
        let pos_3d = Vec3::new(
            self.game.position[0],
            self.game.position[1],
            self.game.position[2],
        );
        let target = pos_3d;
        let offset = self.playing_camera_offset();
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
        let aspect = self.config.width as f32 / self.config.height as f32;
        let radius = 25.0;
        let angle = -25.0f32.to_radians();
        let eye = Vec3::new(radius * angle.cos(), radius * angle.sin(), 15.0);
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
        self.render_with_overlay(|_, _, _, _| {})
    }

    pub fn render_with_overlay<F>(&mut self, overlay: F) -> Result<(), SurfaceError>
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
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

            let clear_color = if self.apply_gamma_correction {
                wgpu::Color {
                    r: linear_to_srgb(clear_color.r as f32) as f64,
                    g: linear_to_srgb(clear_color.g as f32) as f64,
                    b: linear_to_srgb(clear_color.b as f32) as f64,
                    a: clear_color.a,
                }
            } else {
                clear_color
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
            render_pass.set_bind_group(2, &self.color_space_bind_group, &[]);

            if self.phase != AppPhase::Menu {
                render_pass.set_vertex_buffer(0, self.floor_vertex_buffer.slice(..));
                render_pass.draw(0..self.floor_vertex_count, 0..1);

                render_pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                render_pass.draw(0..self.grid_vertex_count, 0..1);
            }

            if self.phase == AppPhase::Playing
                || self.phase == AppPhase::GameOver
                || self.phase == AppPhase::Editor
                || self.phase == AppPhase::Menu
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

                    if let Some(buf) = &self.editor_selection_outline_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_selection_outline_vertex_count, 0..1);
                    }

                    if let Some(buf) = &self.editor_gizmo_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_gizmo_vertex_count, 0..1);
                    }

                    if let Some(buf) = &self.editor_cursor_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_cursor_vertex_count, 0..1);
                    }
                }
            }
        }

        overlay(&self.device, &self.queue, &view, &mut encoder);

        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn surface_width(&self) -> u32 {
        self.config.width
    }

    pub fn surface_height(&self) -> u32 {
        self.config.height
    }

    pub fn handle_surface_lost(&mut self) {
        let size = self.size;
        self.apply_resize(size);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> &NativeWindow {
        self.surface_host.window()
    }

    pub fn recreate_surface(&mut self) {
        let size = self.surface_host.current_size();
        self.resize_surface(size);
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

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::{LevelObject, SpawnDirection};
    use crate::editor_domain::derive_timeline_position;

    #[test]
    fn derives_position_without_taps() {
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], 3, &[]);
        assert_eq!(position, [0.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn derives_position_with_taps() {
        let taps = [2, 4];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 5, &[]);
        assert_eq!(position, [2.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn tap_at_zero_changes_direction() {
        let taps = [0];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 0, &[]);
        assert_eq!(position, [0.0, 0.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Right));
    }

    #[test]
    fn ignores_taps_after_step() {
        let taps = [5];
        let (position, direction) =
            derive_timeline_position([1.0, 1.0, 0.0], SpawnDirection::Forward, &taps, 2, &[]);
        assert_eq!(position, [1.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn supports_offset_spawn_with_tap() {
        let taps = [2];
        let (position, direction) =
            derive_timeline_position([2.0, 2.0, 0.0], SpawnDirection::Right, &taps, 3, &[]);
        assert_eq!(position, [4.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn falls_from_elevated_platform() {
        let objects = [LevelObject {
            position: [0.0, 0.0, 2.0],
            size: [1.0, 1.0, 1.0],
            kind: crate::types::BlockKind::Standard,
        }];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 3.0], SpawnDirection::Forward, &[], 1, &objects);
        assert_eq!(position, [0.0, 1.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }
}
