use super::*;
use base64::Engine as _;
use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::block_repository::DEFAULT_BLOCK_ID;
use crate::editor_domain::{
    build_editor_playtest_transition, build_playing_transition_from_metadata,
    derive_tap_indicator_positions, editor_session_init_from_metadata,
};
use crate::game::{create_menu_scene, GameState};
use crate::import_export_service::{
    build_level_export, build_level_json_export, parse_level_import, parse_level_ldz_import,
};
use crate::level_repository::{builtin_level_names, load_builtin_level_metadata};
use crate::mesh::{
    build_block_obj, build_block_vertices, build_floor_vertices, build_grid_vertices,
};
use crate::platform::io::{log_platform_error, read_editor_music_bytes};
use crate::platform::services::{trigger_audio_import, trigger_level_export};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, CameraUniform, ColorSpaceUniform, LevelMetadata, LineUniform, MenuState,
    MusicMetadata, PhysicalSize, SpawnMetadata, Vertex,
};

impl State {
    #[cfg(target_arch = "wasm32")]
    pub(crate) async fn new(canvas: WasmCanvas) -> Self {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_wasm(canvas);
        Self::new_common(instance, Some(surface_host), Some(surface), size).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_native(window: NativeWindow) -> State {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_native(window);
        Self::new_common(instance, Some(surface_host), Some(surface), size).await
    }

    #[cfg(test)]
    pub(crate) async fn new_test() -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let size = PhysicalSize {
            width: 800,
            height: 600,
        };
        Self::new_common(instance, None, None, size).await
    }

    pub(crate) async fn new_common(
        instance: wgpu::Instance,
        surface_host: Option<SurfaceHost>,
        surface: Option<wgpu::Surface<'static>>,
        size: PhysicalSize<u32>,
    ) -> State {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        let surface_capabilities = surface
            .as_ref()
            .map(|s| s.get_capabilities(&adapter))
            .unwrap_or_default();
        let surface_format = surface_capabilities
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_capabilities.formats.first().copied().unwrap_or(
                if cfg!(target_arch = "wasm32") {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Bgra8UnormSrgb
                },
            ));

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_capabilities
                .present_modes
                .first()
                .copied()
                .unwrap_or(wgpu::PresentMode::Fifo),
            alpha_mode: surface_capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        if let Some(ref s) = surface {
            s.configure(&device, &config);
        }

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
        });

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
            render: RenderSubsystem {
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
            },
            gameplay: GameplaySubsystem { state: game },
            phase: AppPhase::Menu,
            menu: MenuSubsystem { state: menu },
            frame_runtime: FrameRuntimeState {
                editor: EditorFrameState {
                    last_frame: now,
                    accumulator: 0.0,
                },
                player_render: PlayerRenderState { line_uniform },
            },
            audio: AudioSubsystem {
                state: AudioState::new(local_audio_cache),
            },
            session: SessionSubsystem {
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
                timeline: EditorTimelineState::new(),
                runtime: EditorRuntimeState {
                    dirty: EditorDirtyFlags::from_object_sync(),
                    gizmo: EditorGizmoState {
                        rebuild_accumulator: 0.0,
                        last_pan: [0.0, 0.0],
                        last_rotation: 0.0,
                        last_pitch: 0.0,
                        last_zoom: 1.0,
                    },
                    interaction: EditorInteractionState::new(),
                    history: EditorHistoryState {
                        undo: Vec::new(),
                        redo: Vec::new(),
                    },
                },
                perf: EditorPerfState::new(),
                timing: EditorTimingState::new(),
            },
        }
    }

    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();

        self.gameplay.state = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.gameplay.state.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn restart_level(&mut self) {
        self.stop_audio();
        self.gameplay.state = GameState::new();

        if self.session.playtesting_editor {
            let transition = build_editor_playtest_transition(
                &self.editor.objects,
                self.session.editor_level_name.as_deref(),
                self.editor.spawn.clone(),
                &self.editor.timeline.taps.tap_times,
                self.editor.timeline.clock.time_seconds,
            );
            self.gameplay.state.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        } else if let Some(level_name) = self.session.playing_level_name.clone() {
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let transition = build_playing_transition_from_metadata(metadata);
                self.gameplay.state.objects = transition.objects;
                self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
            }
        }

        self.gameplay.state.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    pub(super) fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let init = editor_session_init_from_metadata(self.load_level_metadata(&level_name));
        self.editor.objects = init.objects;
        self.editor.spawn = init.spawn;
        self.session.editor_music_metadata = init.music;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor.timing.timing_selected_index = None;
        self.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.timeline.taps.tap_times,
            &self.editor.objects,
        );
        self.editor.timeline.clock.time_seconds = init.timeline_time_seconds;
        self.editor.timeline.clock.duration_seconds = init.timeline_duration_seconds;
        self.editor.ui.cursor = init.cursor;
        self.editor.camera.editor_pan = init.camera_pan;

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        load_builtin_level_metadata(level_name)
    }

    pub(super) fn stop_audio(&mut self) {
        self.audio.state.runtime.stop();
    }

    pub(super) fn start_audio(&mut self, level_name: &str, metadata: &LevelMetadata) {
        self.start_audio_at_seconds(level_name, metadata, 0.0);
    }

    pub(super) fn start_audio_at_seconds(
        &mut self,
        level_name: &str,
        metadata: &LevelMetadata,
        start_seconds: f32,
    ) {
        if let Some(bytes) = self
            .audio
            .state
            .editor
            .local_audio_cache
            .get(&metadata.music.source)
        {
            self.audio.state.runtime.start_with_bytes_at(
                &metadata.music.source,
                bytes,
                start_seconds,
            );
        } else {
            self.audio
                .state
                .runtime
                .start_at(level_name, &metadata.music.source, start_seconds);
        }
    }

    pub fn trigger_audio_import(&self) {
        trigger_audio_import(self.audio.state.editor.audio_import_channel.0.clone());
    }

    pub(super) fn update_audio_imports(&mut self) {
        while let Ok((filename, bytes)) = self.audio.state.editor.audio_import_channel.1.try_recv()
        {
            self.session.editor_music_metadata.source = filename.clone();
            self.audio
                .state
                .editor
                .local_audio_cache
                .insert(filename, bytes);
            self.audio
                .state
                .editor
                .waveform_cache
                .remove(&self.session.editor_music_metadata.source);
            self.audio.state.editor.waveform_loading_source = None;
            self.load_waveform_for_current_audio();
        }
    }

    pub(super) fn update_waveform_loading(&mut self) {
        while let Ok((source, decoded)) = self.audio.state.editor.waveform_load_channel.1.try_recv()
        {
            if let Some((samples, sample_rate)) = decoded {
                self.audio
                    .state
                    .editor
                    .waveform_cache
                    .insert(source.clone(), (samples.clone(), sample_rate));

                if source != self.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples = samples;
                self.editor.timing.waveform_sample_rate = sample_rate;
            } else {
                if source != self.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples.clear();
                self.editor.timing.waveform_sample_rate = 0;
            }

            if self.audio.state.editor.waveform_loading_source.as_deref() == Some(source.as_str()) {
                self.audio.state.editor.waveform_loading_source = None;
            }
        }
    }

    pub fn export_level_ldz(&self) -> Result<Vec<u8>, String> {
        let metadata = self.current_editor_metadata();
        let audio_bytes = self
            .audio
            .state
            .editor
            .local_audio_cache
            .get(&metadata.music.source)
            .cloned()
            .or_else(|| {
                read_editor_music_bytes(
                    self.session.editor_level_name.as_deref(),
                    &metadata.music.source,
                )
            });

        build_level_export(&metadata, audio_bytes)
    }

    pub fn import_level_ldz(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = parse_level_ldz_import(data)?;
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    pub fn export_level(&self) -> String {
        build_level_json_export(&self.current_editor_metadata())
    }

    pub fn import_level(&mut self, json: &str) -> Result<(), String> {
        let metadata = parse_level_import(json)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(
            self.session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            self.session.editor_music_metadata.clone(),
            self.editor.spawn.clone(),
            self.editor.timeline.taps.tap_times.clone(),
            self.editor.timing.timing_points.clone(),
            self.editor.timeline.clock.time_seconds,
            self.editor.timeline.clock.duration_seconds,
            self.editor.objects.clone(),
        )
    }

    fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
        let level_name = metadata.name.clone();
        let init = editor_session_init_from_metadata(Some(metadata));

        self.editor.objects = init.objects;
        self.editor.ui.selected_block_index = None;
        self.editor.ui.selected_block_indices.clear();
        self.editor.ui.hovered_block_index = None;
        self.editor.spawn = init.spawn;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor
            .timing
            .timing_points
            .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
        self.editor.timing.timing_selected_index = None;
        self.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.timeline.taps.tap_times,
            &self.editor.objects,
        );
        self.editor.timeline.clock.time_seconds = init.timeline_time_seconds;
        self.editor.timeline.clock.duration_seconds = init.timeline_duration_seconds;
        self.session.editor_level_name = Some(level_name);
        self.session.editor_music_metadata = init.music;
        self.editor.ui.cursor = init.cursor;
        self.editor.camera.editor_pan = init.camera_pan;

        self.editor.runtime.history.undo.clear();
        self.editor.runtime.history.redo.clear();

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    pub fn load_builtin_level_into_editor(&mut self, name: &str) {
        if let Some(metadata) = self.load_level_metadata(name) {
            let _ = self.import_level(&serde_json::to_string(&metadata).unwrap());
            self.session.editor_level_name = Some(name.to_string());
        }
    }

    pub fn editor_level_name(&self) -> Option<String> {
        self.session.editor_level_name.clone()
    }

    pub fn set_editor_level_name(&mut self, name: String) {
        self.session.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.session.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.session.editor_music_metadata = metadata;
    }

    pub fn editor_show_import(&self) -> bool {
        self.session.editor_show_import
    }

    pub fn set_editor_show_import(&mut self, show: bool) {
        self.session.editor_show_import = show;
    }

    pub fn editor_import_text(&self) -> &str {
        &self.session.editor_import_text
    }

    pub fn set_editor_import_text(&mut self, text: String) {
        self.session.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.session.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.session.editor_show_metadata = show;
    }

    pub fn available_levels(&self) -> &[String] {
        &self.menu.state.levels
    }

    pub fn trigger_level_export(&self) {
        match self.export_level_ldz() {
            Ok(data) => {
                let filename = format!(
                    "{}.ldz",
                    self.editor_level_name()
                        .unwrap_or_else(|| "level".to_string())
                );

                trigger_level_export(&filename, &data);
            }
            Err(e) => {
                log_platform_error(&format!("Export failed: {}", e));
            }
        }
    }

    pub fn trigger_selected_block_obj_export(&self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(block) = self.editor_selected_block() else {
            log_platform_error("OBJ export failed: no selected block");
            return;
        };

        let sanitized_id = block
            .block_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();

        let object_name = if sanitized_id.is_empty() {
            "block".to_string()
        } else {
            sanitized_id
        };

        let filename = format!("{}_selected.obj", object_name);
        let obj = build_block_obj(&block, &object_name);

        trigger_level_export(&filename, obj.as_bytes());
    }

    pub fn complete_import(&mut self) {
        let text = self.session.editor_import_text.clone();
        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            if let Err(e) = self.import_level_ldz(&data) {
                log_platform_error(&format!("LDZ Import failed: {}", e));
            } else {
                self.session.editor_show_import = false;
                self.session.editor_import_text.clear();
                return;
            }
        }

        let text = self.session.editor_import_text.clone();
        if let Err(e) = self.import_level(&text) {
            log_platform_error(&format!("JSON Import failed: {}", e));
        } else {
            self.session.editor_show_import = false;
            self.session.editor_import_text.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::types::AppPhase;

    #[test]
    fn test_lifecycle_transitions() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            // Start level 0 (should be Flowerfield)
            state.start_level(0);
            assert_eq!(state.phase, AppPhase::Playing);
            assert_eq!(
                state.session.playing_level_name,
                Some("Flowerfield".to_string())
            );

            // Start editor for level 1 (Golden Haze)
            state.start_editor(1);
            assert_eq!(state.phase, AppPhase::Editor);
            assert_eq!(
                state.session.editor_level_name,
                Some("Golden Haze".to_string())
            );

            // Back to menu
            state.back_to_menu();
            assert_eq!(state.phase, AppPhase::Menu);
        });
    }

    #[test]
    fn test_lifecycle_audio_side_effects() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            // Mock that audio is playing (not actually possible without a real backend but we can check the call)
            // For now we just check if it resets the phase correctly
            state.start_level(0);
            assert_eq!(state.phase, AppPhase::Playing);

            state.stop_audio(); // Should not crash
        });
    }
}
