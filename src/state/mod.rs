mod command_dispatch;
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

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::block_repository::DEFAULT_BLOCK_ID;
use crate::game::{create_menu_scene, GameState, TimelineSimulationRuntime};
use crate::level_repository::builtin_level_names;
use crate::mesh::{build_block_vertices, build_floor_vertices, build_grid_vertices};
use crate::platform::audio::PlatformAudio;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{log_backend, PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, CameraUniform, ColorSpaceUniform, EditorMode, EditorState, LevelObject, LineUniform,
    MenuState, MusicMetadata, PhysicalSize, SpawnDirection, SpawnMetadata, TimingPoint, Vertex,
};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) enum MeshSlot {
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

/// Grouped ownership of all GPU mesh slots used for scene rendering.
/// Separates mesh lifetime management from application logic.
pub(crate) struct SceneMeshes {
    pub(crate) floor: MeshSlot,
    pub(crate) grid: MeshSlot,
    pub(crate) trail: MeshSlot,
    pub(crate) blocks: MeshSlot,
    pub(crate) editor_cursor: MeshSlot,
    pub(crate) editor_hover_outline: MeshSlot,
    pub(crate) editor_selection_outline: MeshSlot,
    pub(crate) editor_gizmo: MeshSlot,
    pub(crate) tap_indicators: MeshSlot,
    pub(crate) spawn_marker: MeshSlot,
    pub(crate) editor_preview_player: MeshSlot,
}

pub(crate) struct GpuContext {
    surface_host: SurfaceHost,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
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
}

impl GpuContext {
    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub(crate) fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub(crate) fn surface_width(&self) -> u32 {
        self.config.width
    }

    pub(crate) fn surface_height(&self) -> u32 {
        self.config.height
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn window(&self) -> &NativeWindow {
        self.surface_host.window()
    }

    pub(crate) fn current_size(&self) -> PhysicalSize<u32> {
        self.surface_host.current_size()
    }

    pub(crate) fn apply_resize(&mut self, new_size: PhysicalSize<u32>) {
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

/// Bundles all editor-related state into a single subsystem.
/// Separates editor concern from the top-level application state.
pub(crate) struct EditorSubsystem {
    pub(crate) ui: EditorState,
    pub(crate) config: EditorConfigState,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) camera: EditorCameraState,
    pub(crate) timeline: EditorTimelineState,
    pub(crate) runtime: EditorRuntimeState,
    pub(crate) perf: EditorPerfState,
    pub(crate) timing: EditorTimingState,
    pub(crate) session: EditorSessionState,
}

pub struct State {
    gpu: GpuContext,
    meshes: SceneMeshes,
    game: GameState,
    phase: AppPhase,
    menu: MenuState,
    editor: EditorSubsystem,
    frame_runtime: FrameRuntimeState,
    audio_state: AudioState,
}

type AudioImportData = (String, Vec<u8>);
type WaveformLoadData = (String, Option<(Vec<f32>, u32)>);

#[derive(Clone, Copy, Default)]
struct EditorDirtyFlags {
    sync_game_objects: bool,
    rebuild_block_mesh: bool,
    rebuild_selection_overlays: bool,
    rebuild_tap_indicators: bool,
    rebuild_preview_player: bool,
}

impl EditorDirtyFlags {
    fn from_object_sync() -> Self {
        Self {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_tap_indicators: true,
            rebuild_preview_player: true,
        }
    }

    fn merge(&mut self, other: Self) {
        self.sync_game_objects |= other.sync_game_objects;
        self.rebuild_block_mesh |= other.rebuild_block_mesh;
        self.rebuild_selection_overlays |= other.rebuild_selection_overlays;
        self.rebuild_tap_indicators |= other.rebuild_tap_indicators;
        self.rebuild_preview_player |= other.rebuild_preview_player;
    }

    fn any(self) -> bool {
        self.sync_game_objects
            || self.rebuild_block_mesh
            || self.rebuild_selection_overlays
            || self.rebuild_tap_indicators
            || self.rebuild_preview_player
    }
}

#[derive(Clone, Copy)]
enum PerfStage {
    FrameTotal = 0,
    TimelinePlayback,
    DragSelection,
    GizmoRebuild,
    DirtyProcess,
    TimelineSampleRebuild,
    TapIndicatorMeshRebuild,
    BlockMeshRebuild,
    TTapToggleTotal,
    TTapSolve,
}

const PERF_STAGE_COUNT: usize = 10;

impl PerfStage {
    const fn as_index(self) -> usize {
        self as usize
    }

    const fn name(self) -> &'static str {
        match self {
            Self::FrameTotal => "FrameTotal",
            Self::TimelinePlayback => "TimelinePlayback",
            Self::DragSelection => "DragSelection",
            Self::GizmoRebuild => "GizmoRebuild",
            Self::DirtyProcess => "DirtyProcess",
            Self::TimelineSampleRebuild => "TimelineSamples",
            Self::TapIndicatorMeshRebuild => "TapIndicatorMesh",
            Self::BlockMeshRebuild => "BlockMeshRebuild",
            Self::TTapToggleTotal => "TKeyToggle",
            Self::TTapSolve => "TKeySolve",
        }
    }
}

#[derive(Clone, Copy)]
struct PerfStat {
    last_ms: f32,
    ema_ms: f32,
    max_ms: f32,
    calls: u64,
}

impl PerfStat {
    const fn zero() -> Self {
        Self {
            last_ms: 0.0,
            ema_ms: 0.0,
            max_ms: 0.0,
            calls: 0,
        }
    }

    fn observe(&mut self, ms: f32) {
        self.last_ms = ms;
        if self.calls == 0 {
            self.ema_ms = ms;
        } else {
            self.ema_ms = self.ema_ms * 0.9 + ms * 0.1;
        }
        self.max_ms = self.max_ms.max(ms);
        self.calls += 1;
    }
}

struct EditorPerfProfiler {
    enabled: bool,
    stats: [PerfStat; PERF_STAGE_COUNT],
    frame_stage_ms: [f32; PERF_STAGE_COUNT],
    frame_spike_count: u64,
    last_spike_stage: Option<PerfStage>,
}

impl EditorPerfProfiler {
    fn new() -> Self {
        Self {
            enabled: false,
            stats: [PerfStat::zero(); PERF_STAGE_COUNT],
            frame_stage_ms: [0.0; PERF_STAGE_COUNT],
            frame_spike_count: 0,
            last_spike_stage: None,
        }
    }

    fn observe(&mut self, stage: PerfStage, ms: f32) {
        self.stats[stage.as_index()].observe(ms);
        self.frame_stage_ms[stage.as_index()] += ms;
    }

    fn begin_frame(&mut self) {
        self.frame_stage_ms = [0.0; PERF_STAGE_COUNT];
    }

    fn dominant_stage_this_frame(&self) -> Option<PerfStage> {
        let stages = [
            PerfStage::TimelinePlayback,
            PerfStage::DragSelection,
            PerfStage::GizmoRebuild,
            PerfStage::DirtyProcess,
            PerfStage::TimelineSampleRebuild,
            PerfStage::TapIndicatorMeshRebuild,
            PerfStage::BlockMeshRebuild,
            PerfStage::TTapToggleTotal,
            PerfStage::TTapSolve,
        ];

        let mut dominant: Option<(PerfStage, f32)> = None;
        for stage in stages {
            let value = self.frame_stage_ms[stage.as_index()];
            dominant = match dominant {
                None => Some((stage, value)),
                Some((_, best)) if value > best => Some((stage, value)),
                current => current,
            };
        }

        dominant.map(|(stage, _)| stage)
    }
}

struct EditorPickResult {
    cursor: [f32; 3],
    hit_block_index: Option<usize>,
}

struct EditorTimelineSampleCache {
    samples: Vec<EditorTimelineSample>,
    dirty: bool,
    rebuild_from_seconds: Option<f32>,
}

struct EditorTimelinePlaybackState {
    playing: bool,
    runtime: Option<TimelineSimulationRuntime>,
}

struct EditorTimelinePreviewState {
    position: [f32; 3],
    direction: SpawnDirection,
}

struct EditorTimelineClockState {
    time_seconds: f32,
    duration_seconds: f32,
}

struct EditorTimelineTapState {
    tap_times: Vec<f32>,
    tap_indicator_positions: Vec<[f32; 3]>,
}

pub(crate) struct EditorTimelineState {
    clock: EditorTimelineClockState,
    preview: EditorTimelinePreviewState,
    taps: EditorTimelineTapState,
    cache: EditorTimelineSampleCache,
    playback: EditorTimelinePlaybackState,
}

struct EditorGizmoState {
    rebuild_accumulator: f32,
    last_pan: [f32; 2],
    last_rotation: f32,
    last_pitch: f32,
    last_zoom: f32,
}

pub(crate) struct EditorTimingState {
    timing_points: Vec<TimingPoint>,
    playback_speed: f32,
    waveform_samples: Vec<f32>,
    waveform_sample_rate: u32,
    timing_selected_index: Option<usize>,
    waveform_zoom: f32,
    waveform_scroll: f32,
    bpm_tap_times: Vec<f64>,
    bpm_tap_result: Option<f32>,
}

pub(crate) struct EditorSessionState {
    editor_level_name: Option<String>,
    editor_music_metadata: MusicMetadata,
    editor_show_metadata: bool,
    editor_show_import: bool,
    editor_import_text: String,
    playing_level_name: Option<String>,
    playtesting_editor: bool,
}

pub(crate) struct EditorConfigState {
    selected_block_id: String,
    snap_to_grid: bool,
    snap_step: f32,
}

pub(crate) struct EditorPerfState {
    profiler: EditorPerfProfiler,
    fps_smoothed: f32,
}

pub(crate) struct EditorRuntimeState {
    dirty: EditorDirtyFlags,
    gizmo: EditorGizmoState,
    interaction: EditorInteractionState,
    history: EditorHistoryState,
}

struct EditorFrameState {
    last_frame: PlatformInstant,
    accumulator: f32,
}

struct PlayerRenderState {
    line_uniform: LineUniform,
}

struct FrameRuntimeState {
    editor: EditorFrameState,
    player_render: PlayerRenderState,
}

struct EditorAudioState {
    local_audio_cache: std::collections::HashMap<String, Vec<u8>>,
    audio_import_channel: (
        std::sync::mpsc::Sender<AudioImportData>,
        std::sync::mpsc::Receiver<AudioImportData>,
    ),
    waveform_load_channel: (
        std::sync::mpsc::Sender<WaveformLoadData>,
        std::sync::mpsc::Receiver<WaveformLoadData>,
    ),
    waveform_cache: std::collections::HashMap<String, (Vec<f32>, u32)>,
    waveform_loading_source: Option<String>,
}

struct AudioState {
    runtime: PlatformAudio,
    editor: EditorAudioState,
}

pub(crate) struct EditorCameraState {
    editor_pan: [f32; 2],
    editor_rotation: f32,
    editor_pitch: f32,
    editor_zoom: f32,
    playing_rotation: f32,
    playing_pitch: f32,
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
    cursor: [f32; 3],
    selected_block_id: String,
    spawn: SpawnMetadata,
    timeline_time_seconds: f32,
    timeline_duration_seconds: f32,
    tap_times: Vec<f32>,
    tap_indicator_positions: Vec<[f32; 3]>,
    timing_points: Vec<TimingPoint>,
}

#[derive(Clone)]
struct EditorClipboard {
    objects: Vec<LevelObject>,
    anchor: [f32; 3],
}

struct EditorInteractionState {
    gizmo_drag: Option<EditorGizmoDrag>,
    block_drag: Option<EditorBlockDrag>,
    clipboard: Option<EditorClipboard>,
}

struct EditorHistoryState {
    undo: Vec<EditorHistorySnapshot>,
    redo: Vec<EditorHistorySnapshot>,
}

#[derive(Clone, Copy)]
struct EditorTimelineSample {
    time_seconds: f32,
    position: [f32; 3],
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
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
            })
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

        let (depth_texture, depth_view) = GpuContext::create_depth_texture(&device, &config);

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
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
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
            gpu: GpuContext {
                surface_host,
                surface,
                device,
                queue,
                config,
                size,
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
            },
            meshes: SceneMeshes {
                floor: floor_mesh,
                grid: grid_mesh,
                trail: trail_mesh,
                blocks: block_mesh,
                editor_cursor: MeshSlot::Empty,
                editor_hover_outline: MeshSlot::Empty,
                editor_selection_outline: MeshSlot::Empty,
                editor_gizmo: MeshSlot::Empty,
                tap_indicators: MeshSlot::Empty,
                spawn_marker: MeshSlot::Empty,
                editor_preview_player: MeshSlot::Empty,
            },
            game,
            phase: AppPhase::Menu,
            menu,
            frame_runtime: FrameRuntimeState {
                editor: EditorFrameState {
                    last_frame: now,
                    accumulator: 0.0,
                },
                player_render: PlayerRenderState { line_uniform },
            },
            audio_state: AudioState {
                runtime: PlatformAudio::new(),
                editor: EditorAudioState {
                    local_audio_cache,
                    audio_import_channel: std::sync::mpsc::channel(),
                    waveform_load_channel: std::sync::mpsc::channel(),
                    waveform_cache: std::collections::HashMap::new(),
                    waveform_loading_source: None,
                },
            },
            editor: EditorSubsystem {
                ui: EditorState::new(),
                config: EditorConfigState {
                    selected_block_id: DEFAULT_BLOCK_ID.to_string(),
                    snap_to_grid: true,
                    snap_step: 1.0,
                },
                objects: Vec::new(),
                spawn: SpawnMetadata::default(),
                camera: EditorCameraState {
                    editor_pan: [0.0, 0.0],
                    editor_rotation: -45.0f32.to_radians(),
                    editor_pitch: 45.0f32.to_radians(),
                    editor_zoom: 1.0,
                    playing_rotation: -45.0f32.to_radians(),
                    playing_pitch: 45.0f32.to_radians(),
                },
                timeline: EditorTimelineState {
                    clock: EditorTimelineClockState {
                        time_seconds: 0.0,
                        duration_seconds: 16.0,
                    },
                    preview: EditorTimelinePreviewState {
                        position: [0.0, 0.0, 0.0],
                        direction: SpawnDirection::Forward,
                    },
                    taps: EditorTimelineTapState {
                        tap_times: Vec::new(),
                        tap_indicator_positions: Vec::new(),
                    },
                    cache: EditorTimelineSampleCache {
                        samples: Vec::new(),
                        dirty: true,
                        rebuild_from_seconds: None,
                    },
                    playback: EditorTimelinePlaybackState {
                        playing: false,
                        runtime: None,
                    },
                },
                runtime: EditorRuntimeState {
                    dirty: EditorDirtyFlags::default(),
                    gizmo: EditorGizmoState {
                        rebuild_accumulator: 0.0,
                        last_pan: [0.0, 0.0],
                        last_rotation: -45.0f32.to_radians(),
                        last_pitch: 45.0f32.to_radians(),
                        last_zoom: 1.0,
                    },
                    interaction: EditorInteractionState {
                        gizmo_drag: None,
                        block_drag: None,
                        clipboard: None,
                    },
                    history: EditorHistoryState {
                        undo: Vec::new(),
                        redo: Vec::new(),
                    },
                },
                perf: EditorPerfState {
                    profiler: EditorPerfProfiler::new(),
                    fps_smoothed: 0.0,
                },
                timing: EditorTimingState {
                    timing_points: Vec::new(),
                    playback_speed: 1.0,
                    waveform_samples: Vec::new(),
                    waveform_sample_rate: 0,
                    timing_selected_index: None,
                    waveform_zoom: 1.0,
                    waveform_scroll: 0.0,
                    bpm_tap_times: Vec::new(),
                    bpm_tap_result: None,
                },
                session: EditorSessionState {
                    editor_level_name: None,
                    editor_music_metadata: MusicMetadata {
                        source: "music.mp3".to_string(),
                        title: None,
                        author: None,
                        extra: serde_json::Map::new(),
                    },
                    editor_show_metadata: false,
                    editor_show_import: false,
                    editor_import_text: String::new(),
                    playing_level_name: None,
                    playtesting_editor: false,
                },
            },
        }
    }

    pub(crate) fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.gpu.surface_host.prepare_resize(new_size);
        self.gpu.apply_resize(new_size);
    }

    pub fn turn_right(&mut self) {
        match self.phase {
            AppPhase::Menu => {
                self.start_level(self.menu.selected_level);
            }
            AppPhase::Playing => {
                if !self.game.started {
                    self.game.started = true;
                    if self.editor.session.playtesting_editor {
                        let metadata = self.current_editor_metadata();
                        let level_name = self
                            .editor
                            .session
                            .editor_level_name
                            .clone()
                            .unwrap_or_else(|| "Untitled".to_string());
                        let start_seconds = self.editor_timeline_elapsed_seconds(
                            self.editor.timeline.clock.time_seconds,
                        );
                        self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
                    } else if let Some(level_name) = self.editor.session.playing_level_name.clone()
                    {
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
            AppPhase::Playing if self.editor.session.playtesting_editor => {
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
        self.editor.ui.right_dragging = dragging;
    }

    pub fn handle_keyboard_input(&mut self, key: &str, pressed: bool, just_pressed: bool) {
        self.process_keyboard_input(key, pressed, just_pressed);
    }

    pub fn handle_mouse_button(&mut self, button: u32, pressed: bool) {
        match button {
            0 => {
                if !pressed {
                    let had_drag = self.editor.runtime.interaction.gizmo_drag.is_some()
                        || self.editor.runtime.interaction.block_drag.is_some();
                    self.editor.runtime.interaction.gizmo_drag = None;
                    self.editor.runtime.interaction.block_drag = None;
                    if had_drag {
                        self.sync_editor_objects();
                    }
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
        self.editor.ui.pointer_screen = Some([x, y]);
        if self.phase == AppPhase::Editor {
            match self.editor.ui.mode {
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
                EditorMode::Timing => {
                    // Timing mode: clicks handled by egui waveform panel
                }
            }
            return;
        }

        self.turn_right();
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorDirtyFlags, LevelObject, SpawnDirection};
    use crate::editor_domain::derive_timeline_position;

    // ── EditorDirtyFlags contract tests ─────────────────────────────
    #[test]
    fn dirty_flags_default_is_clean() {
        let flags = EditorDirtyFlags::default();
        assert!(!flags.any());
    }

    #[test]
    fn dirty_flags_from_object_sync_sets_all() {
        let flags = EditorDirtyFlags::from_object_sync();
        assert!(flags.sync_game_objects);
        assert!(flags.rebuild_block_mesh);
        assert!(flags.rebuild_selection_overlays);
        assert!(flags.rebuild_tap_indicators);
        assert!(flags.rebuild_preview_player);
        assert!(flags.any());
    }

    #[test]
    fn dirty_flags_merge_is_union() {
        let mut a = EditorDirtyFlags {
            rebuild_block_mesh: true,
            ..EditorDirtyFlags::default()
        };
        let b = EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        };
        a.merge(b);
        assert!(a.rebuild_block_mesh);
        assert!(a.rebuild_tap_indicators);
        assert!(!a.sync_game_objects);
        assert!(a.any());
    }

    // ── Timeline position tests (pre-existing) ─────────────────────

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
