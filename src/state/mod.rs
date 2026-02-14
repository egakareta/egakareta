mod editor_actions;
mod editor_camera;
mod editor_interaction;
mod editor_scene;
mod editor_state;
mod history;
mod lifecycle;
mod render;
mod state_helpers;
mod update;

use glam::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::block_repository::DEFAULT_BLOCK_ID;
use crate::editor_domain::{
    add_tap_time, build_editor_playtest_transition, build_playing_transition_from_metadata,
    clear_tap_times, create_block_at_cursor, derive_timeline_elapsed_seconds,
    derive_timeline_position, editor_session_init_from_metadata, move_cursor_xy,
    playtest_return_objects, remove_tap_time, remove_topmost_block_at_cursor,
    toggle_spawn_direction,
};
use crate::game::{create_menu_scene, GameState, TimelineSimulationRuntime};
use crate::level_repository::builtin_level_names;
use crate::mesh::{
    build_block_vertices, build_editor_cursor_vertices, build_editor_gizmo_vertices,
    build_editor_hover_outline_vertices, build_editor_preview_player_vertices,
    build_editor_selection_outline_vertices, build_floor_vertices, build_grid_vertices,
    build_spawn_marker_vertices, build_tap_indicator_vertices, build_trail_vertices, GizmoPart,
};
use crate::platform::audio::PlatformAudio;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{log_backend, PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, CameraUniform, ColorSpaceUniform, Direction, EditorMode, EditorState, LevelMetadata,
    LevelObject, LineUniform, MenuState, MusicMetadata, PhysicalSize, SpawnDirection,
    SpawnMetadata, Vertex,
};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

enum MeshSlot {
    Empty,
    VertexData {
        buffer: wgpu::Buffer,
        count: u32,
    },
    Streaming {
        buffer: wgpu::Buffer,
        count: u32,
        capacity_vertices: u32,
    },
}

impl MeshSlot {
    fn from_vertices(device: &wgpu::Device, label: &'static str, vertices: &[Vertex]) -> Self {
        if vertices.is_empty() {
            return Self::Empty;
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self::VertexData {
            buffer,
            count: vertices.len() as u32,
        }
    }

    fn streaming(device: &wgpu::Device, label: &'static str, capacity_vertices: u32) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (std::mem::size_of::<Vertex>() * capacity_vertices as usize) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self::Streaming {
            buffer,
            count: 0,
            capacity_vertices,
        }
    }

    fn replace_with_vertices(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[Vertex],
    ) {
        *self = Self::from_vertices(device, label, vertices);
    }

    fn write_streaming_vertices(&mut self, queue: &wgpu::Queue, vertices: &[Vertex]) {
        match self {
            Self::Streaming {
                buffer,
                count,
                capacity_vertices,
            } => {
                let write_count = vertices.len().min(*capacity_vertices as usize);
                *count = write_count as u32;
                if write_count > 0 {
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices[..write_count]));
                }
            }
            _ => {
                *self = Self::Empty;
            }
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Empty => {}
            Self::VertexData { .. } => *self = Self::Empty,
            Self::Streaming { count, .. } => *count = 0,
        }
    }

    fn draw_data(&self) -> Option<(&wgpu::Buffer, u32)> {
        match self {
            Self::Empty => None,
            Self::VertexData { buffer, count } | Self::Streaming { buffer, count, .. }
                if *count > 0 =>
            {
                Some((buffer, *count))
            }
            _ => None,
        }
    }
}

pub struct State {
    surface_host: SurfaceHost,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    floor_mesh: MeshSlot,
    grid_mesh: MeshSlot,
    trail_mesh: MeshSlot,
    block_mesh: MeshSlot,
    editor_cursor_mesh: MeshSlot,
    editor_hover_outline_mesh: MeshSlot,
    editor_selection_outline_mesh: MeshSlot,
    editor_gizmo_mesh: MeshSlot,
    tap_indicator_mesh: MeshSlot,
    spawn_marker_mesh: MeshSlot,
    editor_preview_player_mesh: MeshSlot,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    gizmo_overlay_pipeline: wgpu::RenderPipeline,
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
    editor_selected_block_id: String,
    editor_objects: Vec<LevelObject>,
    editor_spawn: SpawnMetadata,
    editor_camera_pan: [f32; 2],
    editor_camera_rotation: f32,
    editor_camera_pitch: f32,
    playing_camera_rotation: f32,
    playing_camera_pitch: f32,
    editor_zoom: f32,
    editor_timeline_time_seconds: f32,
    editor_timeline_duration_seconds: f32,
    editor_tap_times: Vec<f32>,
    editor_timeline_playing: bool,
    editor_timeline_playback_runtime: Option<TimelineSimulationRuntime>,
    editor_fps_smoothed: f32,
    editor_gizmo_rebuild_accumulator: f32,
    editor_gizmo_last_pan: [f32; 2],
    editor_gizmo_last_rotation: f32,
    editor_gizmo_last_pitch: f32,
    editor_gizmo_last_zoom: f32,
    editor_right_dragging: bool,
    editor_pan_up_held: bool,
    editor_pan_down_held: bool,
    editor_pan_left_held: bool,
    editor_pan_right_held: bool,
    editor_shift_held: bool,
    editor_ctrl_held: bool,
    editor_alt_held: bool,
    editor_mode: EditorMode,
    editor_snap_to_grid: bool,
    editor_snap_step: f32,
    editor_selected_block_index: Option<usize>,
    editor_selected_block_indices: Vec<usize>,
    editor_hovered_block_index: Option<usize>,
    editor_gizmo_drag: Option<EditorGizmoDrag>,
    editor_block_drag: Option<EditorBlockDrag>,
    editor_pointer_screen: Option<[f64; 2]>,
    editor_clipboard: Option<EditorClipboard>,
    editor_history_undo: Vec<EditorHistorySnapshot>,
    editor_history_redo: Vec<EditorHistorySnapshot>,
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
    XNeg,
    YNeg,
    ZNeg,
}

#[derive(Clone, Copy)]
enum GizmoDragKind {
    Move,
    Resize,
}

#[derive(Clone)]
struct EditorGizmoDrag {
    axis: GizmoAxis,
    kind: GizmoDragKind,
    start_mouse: [f64; 2],
    start_center_screen: [f32; 2],
    start_center_world: [f32; 3],
    start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone)]
struct EditorBlockDrag {
    start_mouse: [f64; 2],
    start_center_screen: [f32; 2],
    start_center_world: [f32; 3],
    start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone, Copy)]
struct EditorDragBlockStart {
    index: usize,
    position: [f32; 3],
    size: [f32; 3],
}

#[derive(Clone)]
struct EditorHistorySnapshot {
    objects: Vec<LevelObject>,
    selected_block_index: Option<usize>,
    selected_block_indices: Vec<usize>,
    cursor: [i32; 3],
    selected_block_id: String,
    spawn: SpawnMetadata,
    timeline_time_seconds: f32,
    timeline_duration_seconds: f32,
    tap_times: Vec<f32>,
}

#[derive(Clone)]
struct EditorClipboard {
    objects: Vec<LevelObject>,
    anchor: [f32; 3],
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
            device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));

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

        let gizmo_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gizmo Overlay Pipeline"),
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
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let floor_vertices = build_floor_vertices();
        let grid_vertices = build_grid_vertices();

        let floor_mesh = MeshSlot::from_vertices(&device, "Floor Vertex Buffer", &floor_vertices);

        let grid_mesh = MeshSlot::from_vertices(&device, "Grid Vertex Buffer", &grid_vertices);

        let trail_mesh = MeshSlot::streaming(&device, "Trail Vertex Buffer", 36 * 20000);

        let menu = MenuState {
            selected_level: 0,
            levels: builtin_level_names(),
        };

        let mut game = GameState::new();
        game.objects = create_menu_scene();

        let local_audio_cache = crate::platform::io::load_all_local_audio().await;

        let block_vertices = build_block_vertices(&game.objects);
        let block_mesh = MeshSlot::from_vertices(&device, "Block Vertex Buffer", &block_vertices);

        let now = PlatformInstant::now();

        Self {
            surface_host,
            surface,
            device,
            queue,
            config,
            size,
            floor_mesh,
            depth_texture,
            depth_view,
            render_pipeline,
            gizmo_overlay_pipeline,
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
            grid_mesh,
            trail_mesh,
            block_mesh,
            editor_cursor_mesh: MeshSlot::Empty,
            editor_hover_outline_mesh: MeshSlot::Empty,
            editor_selection_outline_mesh: MeshSlot::Empty,
            editor_gizmo_mesh: MeshSlot::Empty,
            tap_indicator_mesh: MeshSlot::Empty,
            spawn_marker_mesh: MeshSlot::Empty,
            editor_preview_player_mesh: MeshSlot::Empty,
            editor: EditorState::new(),
            editor_selected_block_id: DEFAULT_BLOCK_ID.to_string(),
            editor_objects: Vec::new(),
            editor_spawn: SpawnMetadata::default(),
            editor_camera_pan: [0.0, 0.0],
            editor_camera_rotation: -45.0f32.to_radians(),
            editor_camera_pitch: 45.0f32.to_radians(),
            playing_camera_rotation: -45.0f32.to_radians(),
            playing_camera_pitch: 45.0f32.to_radians(),
            editor_zoom: 1.0,
            editor_timeline_time_seconds: 0.0,
            editor_timeline_duration_seconds: 16.0,
            editor_tap_times: Vec::new(),
            editor_timeline_playing: false,
            editor_timeline_playback_runtime: None,
            editor_fps_smoothed: 0.0,
            editor_gizmo_rebuild_accumulator: 0.0,
            editor_gizmo_last_pan: [0.0, 0.0],
            editor_gizmo_last_rotation: -45.0f32.to_radians(),
            editor_gizmo_last_pitch: 45.0f32.to_radians(),
            editor_gizmo_last_zoom: 1.0,
            editor_right_dragging: false,
            editor_pan_up_held: false,
            editor_pan_down_held: false,
            editor_pan_left_held: false,
            editor_pan_right_held: false,
            editor_shift_held: false,
            editor_ctrl_held: false,
            editor_alt_held: false,
            editor_mode: EditorMode::Place,
            editor_snap_to_grid: true,
            editor_snap_step: 1.0,
            editor_selected_block_index: None,
            editor_selected_block_indices: Vec::new(),
            editor_hovered_block_index: None,
            editor_gizmo_drag: None,
            editor_block_drag: None,
            editor_pointer_screen: None,
            editor_clipboard: None,
            editor_history_undo: Vec::new(),
            editor_history_redo: Vec::new(),
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
                        let start_seconds =
                            self.editor_timeline_elapsed_seconds(self.editor_timeline_time_seconds);
                        self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
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

    pub fn is_menu(&self) -> bool {
        self.phase == AppPhase::Menu
    }

    pub fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor_right_dragging = dragging;
    }

    pub fn handle_keyboard_input(&mut self, key: &str, pressed: bool, just_pressed: bool) {
        if key == "Shift" {
            self.set_editor_shift_held(pressed);
            return;
        }

        if key == "Control" || key == "ControlLeft" || key == "ControlRight" {
            self.set_editor_ctrl_held(pressed);
            return;
        }

        if key == "Alt" || key == "AltLeft" || key == "AltRight" {
            self.set_editor_alt_held(pressed);
            return;
        }

        if !pressed {
            match key {
                "w" | "W" => self.set_editor_pan_up_held(false),
                "s" | "S" => self.set_editor_pan_down_held(false),
                "a" | "A" => self.set_editor_pan_left_held(false),
                "d" | "D" => self.set_editor_pan_right_held(false),
                _ => {}
            }
            return;
        }

        match key {
            "ArrowUp" => {
                if self.is_editor() {
                    if !self.editor_nudge_selected_blocks(0, 1) {
                        self.editor_shift_timeline_time(0.1);
                    }
                } else if just_pressed {
                    self.turn_right();
                }
            }
            "ArrowDown" => {
                if self.is_editor() && !self.editor_nudge_selected_blocks(0, -1) {
                    self.editor_shift_timeline_time(-0.1);
                }
            }
            "ArrowRight" => {
                if self.is_editor() {
                    if !self.editor_nudge_selected_blocks(1, 0) {
                        self.editor_shift_timeline_time(0.1);
                    }
                } else if just_pressed {
                    self.next_level();
                }
            }
            "ArrowLeft" => {
                if self.is_editor() {
                    if !self.editor_nudge_selected_blocks(-1, 0) {
                        self.editor_shift_timeline_time(-0.1);
                    }
                } else if just_pressed {
                    self.prev_level();
                }
            }
            "w" | "W" => {
                if self.is_editor() {
                    self.set_editor_pan_up_held(true);
                } else if just_pressed {
                    self.turn_right();
                }
            }
            "s" | "S" => {
                if self.is_editor() {
                    self.set_editor_pan_down_held(true);
                }
            }
            " " | "Space" => {
                if just_pressed {
                    if self.is_editor() {
                        self.toggle_editor_timeline_playback();
                    } else {
                        self.turn_right();
                    }
                }
            }
            "d" | "D" => {
                if self.is_editor() {
                    if self.editor_ctrl_held && just_pressed {
                        self.editor_duplicate_selected_block_in_place();
                    } else {
                        self.set_editor_pan_right_held(true);
                    }
                } else if just_pressed {
                    self.next_level();
                }
            }
            "a" | "A" => {
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
            "q" | "Q" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_mode(EditorMode::Select);
                }
            }
            "e" | "E" => {
                if just_pressed {
                    if self.is_editor() {
                        self.set_editor_mode(EditorMode::Place);
                    } else {
                        self.toggle_editor();
                    }
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
                    self.set_editor_block_id("core/standard".to_string());
                }
            }
            "2" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_id("core/grass".to_string());
                }
            }
            "3" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_id("core/dirt".to_string());
                }
            }
            "4" => {
                if self.is_editor() && just_pressed {
                    self.set_editor_block_id("core/void".to_string());
                }
            }
            "o" | "O" => {
                if self.is_editor()
                    && self.editor_ctrl_held
                    && self.editor_shift_held
                    && self.editor_alt_held
                    && just_pressed
                {
                    self.trigger_selected_block_obj_export();
                }
            }
            "c" | "C" => {
                if self.is_editor() && self.editor_ctrl_held && just_pressed {
                    self.editor_copy_block();
                }
            }
            "v" | "V" => {
                if self.is_editor() && self.editor_ctrl_held && just_pressed {
                    self.editor_paste_block();
                }
            }
            "z" | "Z" => {
                if self.is_editor() && self.editor_ctrl_held && just_pressed {
                    self.editor_undo();
                }
            }
            "y" | "Y" => {
                if self.is_editor() && self.editor_ctrl_held && just_pressed {
                    self.editor_redo();
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
        self.editor_pointer_screen = Some([x, y]);
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
}

#[cfg(test)]
mod tests {
    use super::{LevelObject, SpawnDirection};
    use crate::editor_domain::derive_timeline_position;

    #[test]
    fn derives_position_without_taps() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &[],
            3.0 * step_time,
            &[],
        );
        assert!((position[0] - 0.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn derives_position_with_taps() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [2.0 * step_time, 4.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &taps,
            5.0 * step_time,
            &[],
        );
        assert!((position[0] - 2.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn tap_at_zero_changes_direction() {
        let taps = [0.0];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 0.0, &[]);
        assert!((position[0] - 0.5).abs() < 0.1);
        assert!((position[1] - 0.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Right));
    }

    #[test]
    fn ignores_taps_after_step() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [5.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [1.0, 1.0, 0.0],
            SpawnDirection::Forward,
            &taps,
            2.0 * step_time,
            &[],
        );
        assert!((position[0] - 1.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn supports_offset_spawn_with_tap() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [2.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [2.0, 2.0, 0.0],
            SpawnDirection::Right,
            &taps,
            3.0 * step_time,
            &[],
        );
        assert!((position[0] - 4.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn falls_from_elevated_platform() {
        let objects = [LevelObject {
            position: [0.0, 0.0, 2.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        }];
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 3.0],
            SpawnDirection::Forward,
            &[],
            1.0 / crate::game::BASE_PLAYER_SPEED,
            &objects,
        );
        assert!(position[2] <= 3.0);
        assert!(matches!(direction, SpawnDirection::Forward));
    }
}
