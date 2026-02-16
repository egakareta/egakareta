use super::render::{GpuContext, MeshSlot, RenderSubsystem, SceneMeshes, DEPTH_FORMAT};
use super::runtime::{
    EditorDirtyFlags, EditorFrameState, EditorGizmoState, EditorRuntimeState, FrameRuntimeState,
    PlayerRenderState, SplashRuntimeState,
};
use super::{
    AudioState, AudioSubsystem, EditorCameraState, EditorConfigState, EditorHistoryState,
    EditorInteractionState, EditorPerfState, EditorSubsystem, EditorTimelineState,
    EditorTimingState, GameplaySubsystem, MenuSubsystem, SessionSubsystem, State,
};
use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::block_repository::DEFAULT_BLOCK_ID;
use crate::game::{create_menu_scene, GameState};
use crate::level_repository::builtin_level_names;
use crate::mesh::{build_block_vertices, build_floor_vertices, build_grid_vertices};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, CameraUniform, ColorSpaceUniform, EditorState, LineUniform, MenuState, MusicMetadata,
    PhysicalSize, SpawnMetadata, Vertex,
};

impl State {
    #[cfg(target_arch = "wasm32")]
    pub(crate) async fn new(canvas: WasmCanvas) -> Self {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_wasm(canvas);
        Self::new_common(instance, Some(surface_host), Some(surface), size)
            .await
            .expect("Failed to initialize state: No compatible GPU adapter found")
    }

    /// Creates a new `State` instance for native platforms.
    ///
    /// This initializes the GPU surface, adapter, and device using the provided
    /// `NativeWindow` and configures the default application state.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_native(window: NativeWindow) -> State {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_native(window);
        Self::new_common(instance, Some(surface_host), Some(surface), size)
            .await
            .expect("Failed to initialize state: No compatible GPU adapter found")
    }

    #[cfg(test)]
    pub(crate) async fn new_test() -> Option<State> {
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
    ) -> Option<State> {
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .ok()
        {
            Some(a) => Some(a),
            None => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: surface.as_ref(),
                    force_fallback_adapter: true,
                })
                .await
                .ok(),
        }?;

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
            .ok()?;

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
        game.rebuild_behavior_cache();

        let local_audio_cache = crate::platform::io::load_all_local_audio().await;

        let block_vertices = build_block_vertices(&game.objects);
        let block_mesh = MeshSlot::from_vertices(&device, "Block Vertex Buffer", &block_vertices);

        let now = PlatformInstant::now();

        Some(Self {
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
                    blocks_static: MeshSlot::Empty,
                    blocks_selected: MeshSlot::Empty,
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
            phase: AppPhase::Splash,
            menu: MenuSubsystem { state: menu },
            frame_runtime: FrameRuntimeState {
                editor: EditorFrameState {
                    last_frame: now,
                    accumulator: 0.0,
                },
                player_render: PlayerRenderState { line_uniform },
                splash: SplashRuntimeState {
                    start_time: now,
                    progress: 0.0,
                },
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
                playtest_audio_start_seconds: None,
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
                    editor_target_z: 0.0,
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
                    drag_heavy_rebuild_accumulator: 0.0,
                    interaction: EditorInteractionState::new(),
                    history: EditorHistoryState {
                        undo: Vec::new(),
                        redo: Vec::new(),
                    },
                },
                perf: EditorPerfState::new(),
                timing: EditorTimingState::new(),
                selected_mask_cache: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::AppCommand;
    use crate::game::TimelineSimulationRuntime;
    use crate::types::SpawnDirection;

    use super::State;
    use crate::types::AppPhase;

    #[test]
    fn test_lifecycle_transitions() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };

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
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };

            state.start_editor(0);
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.playback.runtime = Some(TimelineSimulationRuntime::new(
                [0.0, 0.0, 0.0],
                SpawnDirection::Forward,
                &[],
                &[],
            ));

            let level_name = state.menu.state.levels[0].clone();
            state.dispatch(AppCommand::EditorLoadLevel(level_name));

            assert!(
                !state.editor.timeline.playback.playing,
                "Loading a level in editor must stop timeline playback"
            );
            assert!(
                state.editor.timeline.playback.runtime.is_none(),
                "Loading a level in editor must clear playback runtime"
            );
            assert!(
                !state.audio.state.runtime.is_playing(),
                "Loading a level in editor must stop runtime audio"
            );
        });
    }

    #[test]
    fn test_phase_transition_clipboard_clearing() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };

            // 1. Setup: Enter editor and copy a block
            state.dispatch(AppCommand::ToggleEditor);
            state.dispatch(AppCommand::TurnRight); // Place block
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.dispatch(AppCommand::EditorCopyBlock);
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            // 2. Transition to playing: Should clear clipboard
            state.start_level(0);
            assert_eq!(state.phase, AppPhase::Playing);
            assert!(
                state.editor.runtime.interaction.clipboard.is_none(),
                "Clipboard should be cleared when entering game phase"
            );
        });
    }

    #[test]
    fn test_editor_load_level_resets_history_and_clipboard() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };

            state.dispatch(AppCommand::ToggleEditor);
            state.dispatch(AppCommand::TurnRight); // Place block -> adds to undo history
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.dispatch(AppCommand::EditorCopyBlock);
            assert!(!state.editor.runtime.history.undo.is_empty());
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            let level_name = state.menu.state.levels[0].clone();
            state.dispatch(AppCommand::EditorLoadLevel(level_name));

            assert!(
                state.editor.runtime.history.undo.is_empty(),
                "Loading a level in editor must clear undo history"
            );
            assert!(
                state.editor.runtime.history.redo.is_empty(),
                "Loading a level in editor must clear redo history"
            );
            assert!(
                state.editor.runtime.interaction.clipboard.is_none(),
                "Loading a level in editor must clear clipboard"
            );
        });
    }
}
