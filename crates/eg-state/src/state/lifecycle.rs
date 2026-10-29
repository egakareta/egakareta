/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy_ecs::world::World;
use eg_blocks::BlockTextureAtlas;
use eg_level::builtin_level_names;
use eg_mesh::{BlockInstance, Vertex, build_floor_vertices, build_grid_vertices};
#[cfg(not(target_arch = "wasm32"))]
use eg_platform::state_host::NativeWindow;
use eg_platform::state_host::SurfaceHost;
#[cfg(target_arch = "wasm32")]
use eg_platform::state_host::WasmCanvas;
use eg_render::{DEPTH_FORMAT, GpuContext, InstancedMeshSlot, MeshSlot, SceneMeshes};
use eg_types::{
    AppPhase, BasicLevelMetadata, CameraUniform, ColorSpaceUniform,
    DEFAULT_MENU_PREVIEW_CAMERA_POSITION, DEFAULT_MENU_PREVIEW_CAMERA_TARGET, GridUniform,
    LevelMetadata, LineUniform, MenuState, MusicMetadata, PhysicalSize, SettingsSection,
};
use glam::Mat4;
use wgpu::util::DeviceExt;

use super::{
    AudioImportChannel, AudioSceneRes, AuthStateRes, EditorConfigState, available_host_labels,
    canonical_backend_name, editor_objects::replace_editor_objects, runtime::EditorRuntimeState,
    set_preferred_backend_name,
};

const GRID_HALF_EXTENT: f32 = 2048.0;

fn shared_receiver<T>(
    receiver: std::sync::mpsc::Receiver<T>,
) -> std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<T>>> {
    std::sync::Arc::new(std::sync::Mutex::new(receiver))
}

fn block_texture_array_layer_count(source_layers: usize) -> u32 {
    let mut layer_count = source_layers.max(2) as u32;
    if layer_count.is_multiple_of(6) {
        layer_count += 1;
    }
    layer_count
}

fn discover_graphics_backends() -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        vec!["BrowserWebGpu".to_string(), "Gl".to_string()]
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        vec![
            "Auto".to_string(),
            "Vulkan".to_string(),
            "Metal".to_string(),
            "Dx12".to_string(),
            "Gl".to_string(),
        ]
    }
}

#[cfg(target_arch = "wasm32")]
fn normalize_web_graphics_backend_setting(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    match normalized.as_str() {
        "browserwebgpu" | "webgpu" | "gpu" => Some("BrowserWebGpu".to_string()),
        "gl" | "webgl" | "opengl" => Some("Gl".to_string()),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn wasm_graphics_backends_for_setting(value: &str) -> wgpu::Backends {
    match value {
        "BrowserWebGpu" => wgpu::Backends::BROWSER_WEBGPU,
        "Gl" => wgpu::Backends::GL,
        _ => wgpu::Backends::all(),
    }
}

fn editor_ghost_trail_primitive_state() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        // Ghost trail uses `append_prism` (untextured) winding, which is opposite
        // of the textured block/icon geometry. Keep CCW here to avoid showing the
        // floor-facing bottom face (causes z-fighting on the leading segment).
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: Some(wgpu::Face::Back),
        ..wgpu::PrimitiveState::default()
    }
}

fn editor_ghost_trail_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn editor_outline_mask_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Always,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Replace,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Always,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Replace,
            },
            read_mask: 0xff,
            write_mask: 0xff,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

fn editor_outline_occlusion_depth_mask_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn editor_outline_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::NotEqual,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::NotEqual,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            read_mask: 0xff,
            write_mask: 0x00,
        },
        bias: wgpu::DepthBiasState {
            constant: -1,
            slope_scale: 0.0,
            clamp: 0.0,
        },
    }
}

fn editor_hover_outline_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        depth_compare: Some(wgpu::CompareFunction::Always),
        bias: wgpu::DepthBiasState::default(),
        ..editor_outline_depth_stencil_state()
    }
}

fn grid_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn default_line_uniform() -> LineUniform {
    LineUniform {
        offset: [0.0, 0.0],
        rotation: 0.0,
        _pad: 0.0,
    }
}

fn default_camera_uniform() -> CameraUniform {
    CameraUniform {
        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        camera_position: [0.0, 0.0, 0.0, 0.0],
    }
}

fn default_color_space_uniform(apply_gamma_correction: bool) -> ColorSpaceUniform {
    ColorSpaceUniform {
        apply_gamma_correction: if apply_gamma_correction { 1.0 } else { 0.0 },
        time_seconds: 0.0,
        viewport_size: [1.0, 1.0],
    }
}

fn default_grid_uniform() -> GridUniform {
    GridUniform {
        center: [0.0, 0.0],
        half_extent: GRID_HALF_EXTENT,
        darkening: 0.0,
    }
}

/// Transient result of the shared GPU/platform initializer.
struct AppInitialization {
    auth: AuthStateRes,
    auth_receiver: crate::state::async_bridge::SharedAuthReceiver,
    editor_input: eg_types::EditorInputState,
    editor_config: EditorConfigState,
    editor_objects: Vec<eg_types::LevelObject>,
    editor_spawn: eg_types::SpawnMetadata,
    editor_camera: super::EditorCameraState,
    editor_triggers: super::EditorTriggerState,
    editor_timeline: super::EditorTimelineState,
    editor_runtime: EditorRuntimeState,
    editor_timing: super::EditorTimingState,
    editor_history: super::EditorHistoryState,
    audio_runtime: AudioSceneRes,
    audio_import_channel: AudioImportChannel,
    session: super::SessionStateRes,
    level_import_channel: (
        std::sync::mpsc::Sender<Vec<u8>>,
        crate::state::async_bridge::SharedLevelImportReceiver,
    ),
    menu: MenuState,
    phase: AppPhase,
    render_resources: super::RenderBootstrapResources,
}

/// One-time production bootstrap that installs authoritative platform and
/// application resources into the Bevy App world.
pub struct AppBootstrap(AppInitialization);

impl AppBootstrap {
    #[cfg(target_arch = "wasm32")]
    pub async fn new(
        canvas: WasmCanvas,
        graphics_backend_override: Option<String>,
        audio_backend_override: Option<String>,
    ) -> Self {
        let graphics_backend_override = graphics_backend_override.and_then(|backend| {
            let normalized = normalize_web_graphics_backend_setting(&backend);
            if normalized.is_none() {
                log::warn!("Ignoring unsupported graphics backend query override: {backend}");
            }
            normalized
        });
        let wasm_backends = graphics_backend_override
            .as_deref()
            .map(wasm_graphics_backends_for_setting)
            .unwrap_or_else(wgpu::Backends::all);
        let (surface_host, instance, surface, size) =
            SurfaceHost::create_for_wasm(canvas, wasm_backends);
        Self(
            AppInitialization::new_common(
                instance,
                Some(surface_host),
                Some(surface),
                size,
                graphics_backend_override,
                audio_backend_override,
            )
            .await
            .expect("Failed to initialize state: No compatible GPU adapter found"),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_native(window: NativeWindow) -> Self {
        let (surface_host, instance, surface, size) = SurfaceHost::create_for_native(window);
        Self(
            AppInitialization::new_common(
                instance,
                Some(surface_host),
                Some(surface),
                size,
                None,
                None,
            )
            .await
            .expect("Failed to initialize state: No compatible GPU adapter found"),
        )
    }

    pub fn install(self, world: &mut World) {
        let AppInitialization {
            auth,
            auth_receiver,

            editor_input,
            editor_config,
            editor_objects,
            editor_spawn,
            editor_camera,
            editor_triggers,
            editor_timeline,
            editor_runtime,
            editor_timing,
            editor_history,
            audio_runtime,
            audio_import_channel,
            session,
            level_import_channel,
            menu,
            phase,
            render_resources,
        } = self.0;

        super::async_bridge::install_async_channel_resources_arc(
            world,
            std::sync::Arc::clone(&auth_receiver),
            std::sync::Arc::clone(&audio_import_channel.1),
            std::sync::Arc::clone(&level_import_channel.1),
        );
        world.insert_resource(super::LevelImportSenderRes::new(
            level_import_channel.0.clone(),
        ));
        world.insert_resource(super::AudioImportSenderRes::new(
            audio_import_channel.0.clone(),
        ));
        #[cfg(not(target_arch = "wasm32"))]
        world.insert_resource(audio_runtime);
        #[cfg(target_arch = "wasm32")]
        world.insert_non_send(audio_runtime);
        let (render_gpu, render_meshes, render_surface_host) = render_resources.into_parts();
        world.insert_resource(super::RenderConfigRes::from_gpu(&render_gpu));
        world.insert_non_send(super::RenderUniformUploadRes::from_gpu(&render_gpu));
        world.insert_non_send(super::RenderGpuRes(render_gpu));
        world.insert_non_send(super::RenderMeshesRes(render_meshes));
        world.insert_non_send(super::RenderSurfaceHostRes(render_surface_host));

        super::gameplay_world::seed_app_gameplay_world(world);
        world.insert_resource(super::MenuPreviewObjectsRes {
            objects: Vec::new(),
        });
        world.insert_resource(bevy_state::prelude::State::new(phase));
        world.init_resource::<bevy_state::prelude::NextState<eg_types::AppPhase>>();
        world.insert_resource(super::MenuStateRes::new(menu));
        world.insert_resource(auth);

        world.insert_resource(super::EditorConfigRes(editor_config));
        world.insert_resource(super::EditorInputRes(editor_input));
        replace_editor_objects(world, editor_objects);
        world.insert_resource(super::EditorTimingRes(editor_timing));
        world.insert_resource(super::EditorTimelineRes(editor_timeline));
        world.insert_resource(super::EditorRuntimeRes(editor_runtime));
        world.insert_resource(super::EditorHistoryRes(editor_history));
        world.insert_resource(super::EditorSpawnRes(editor_spawn));
        world.insert_resource(super::EditorCameraRes(editor_camera));
        world.insert_resource(super::EditorTriggerRes(editor_triggers));

        let mut session = session;
        session.practice_mode_enabled = phase == AppPhase::Playing
            && !session.playtesting_editor
            && session.practice_mode_enabled;
        session.audio_stop_pending = false;
        world.insert_resource(session);
        world.insert_resource(super::FrameTimeRes {
            last_frame: web_time::Instant::now(),
            delta_seconds: 0.0,
            global_time_seconds: 0.0,
            fps_smoothed: 0.0,
        });
    }
}

fn build_state_render_resources(
    gpu: GpuContext,
    surface_host: Option<SurfaceHost>,
    meshes: SceneMeshes,
) -> super::RenderBootstrapResources {
    super::RenderBootstrapResources {
        gpu,
        meshes,
        surface_host,
    }
}

impl AppInitialization {
    async fn new_common(
        instance: wgpu::Instance,
        surface_host: Option<SurfaceHost>,
        surface: Option<wgpu::Surface<'static>>,
        size: PhysicalSize<u32>,
        graphics_backend_override: Option<String>,
        audio_backend_override: Option<String>,
    ) -> Option<AppInitialization> {
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
                ..Default::default()
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
                    ..Default::default()
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
            color_space: wgpu::SurfaceColorSpace::Auto,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (editor_outline_occlusion_depth_texture, editor_outline_occlusion_depth_view) =
            GpuContext::create_depth_texture(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(eg_render::SHADER_WGSL.into()),
        });
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Grid Shader"),
            source: wgpu::ShaderSource::Wgsl(eg_render::GRID_WGSL.into()),
        });

        let line_uniform = default_line_uniform();

        let line_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Uniform Buffer"),
            contents: bytemuck::bytes_of(&line_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_uniform = default_camera_uniform();

        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let should_apply_gamma_correction =
            !surface_format.is_srgb() && adapter_info.backend == wgpu::Backend::BrowserWebGpu;

        let color_space_uniform = default_color_space_uniform(should_apply_gamma_correction);

        let color_space_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Color Space Uniform Buffer"),
                contents: bytemuck::bytes_of(&color_space_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let grid_uniform = default_grid_uniform();

        let grid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Uniform Buffer"),
            contents: bytemuck::bytes_of(&grid_uniform),
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let grid_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Grid Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let block_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Block Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let texture_atlas = BlockTextureAtlas::get();
        let block_texture_layer_count = block_texture_array_layer_count(texture_atlas.layers.len());
        let block_texture_array = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Block Texture Array"),
            size: wgpu::Extent3d {
                width: texture_atlas.width,
                height: texture_atlas.height,
                depth_or_array_layers: block_texture_layer_count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (layer_index, layer) in texture_atlas.layers.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &block_texture_array,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer_index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                layer.rgba.as_slice(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * texture_atlas.width),
                    rows_per_image: Some(texture_atlas.height),
                },
                wgpu::Extent3d {
                    width: texture_atlas.width,
                    height: texture_atlas.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let block_texture_array_view =
            block_texture_array.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Block Texture Array View"),
                format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(block_texture_layer_count),
            });

        let block_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Block Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let block_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Block Texture Bind Group"),
            layout: &block_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&block_texture_array_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&block_texture_sampler),
                },
            ],
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

        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grid Bind Group"),
            layout: &grid_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: grid_uniform_buffer.as_entire_binding(),
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
                Some(&camera_bind_group_layout),
                Some(&line_bind_group_layout),
                Some(&color_space_bind_group_layout),
                Some(&block_texture_bind_group_layout),
            ],
            // wgpu 29 replaces push constants in this descriptor with immediate shader bytes.
            // We don't use immediates in our shaders, so this remains disabled.
            immediate_size: 0,
        });

        let grid_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Grid Pipeline Layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&grid_bind_group_layout),
                Some(&color_space_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::desc())],
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instanced_block_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Instanced Block Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_instanced"),
                    buffers: &[Some(Vertex::desc()), Some(BlockInstance::desc())],
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
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Pipeline"),
            layout: Some(&grid_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &grid_shader,
                entry_point: Some("vs_grid"),
                buffers: &[Some(Vertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_shader,
                entry_point: Some("fs_grid"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(grid_depth_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let block_icon_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Block Icon Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let editor_ghost_trail_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Editor Ghost Trail Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
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
                primitive: editor_ghost_trail_primitive_state(),
                depth_stencil: Some(editor_ghost_trail_depth_stencil_state()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let gizmo_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gizmo Overlay Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
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
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let editor_outline_occlusion_depth_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Editor Outline Occlusion Depth Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(editor_outline_occlusion_depth_mask_depth_stencil_state()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let editor_outline_mask_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Editor Outline Mask Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(editor_outline_mask_depth_stencil_state()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let editor_hover_outline_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Editor Hover Outline Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
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
                depth_stencil: Some(editor_hover_outline_depth_stencil_state()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let editor_outline_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Editor Outline Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
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
                depth_stencil: Some(editor_outline_depth_stencil_state()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
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
            preview_level_index: None,
            preview_sky_color: LevelMetadata::default().sky_color,
            preview_camera_position: DEFAULT_MENU_PREVIEW_CAMERA_POSITION,
            preview_camera_target: DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
        };

        let mut app_settings = if cfg!(test) {
            eg_types::AppSettings::default()
        } else {
            match eg_platform::io::load_app_settings_from_storage() {
                Ok(settings) => settings,
                Err(error) => {
                    log::error!("Failed to load app settings: {error}");
                    eg_types::AppSettings::default()
                }
            }
        };
        let app_settings_migrated = app_settings.migrate_defaults();
        if app_settings_migrated
            && !cfg!(test)
            && let Err(error) = eg_platform::io::save_app_settings_to_storage(&app_settings)
        {
            log::error!("Failed to persist migrated app settings: {error}");
        }

        let graphics_backend_locked = graphics_backend_override.is_some();
        if let Some(graphics_backend) = graphics_backend_override {
            app_settings.graphics_backend = graphics_backend;
        }

        let mut audio_backend_locked = false;
        if let Some(audio_backend) = audio_backend_override {
            if let Some(audio_backend) = canonical_backend_name(&audio_backend) {
                audio_backend_locked = true;
                app_settings.audio_backend = audio_backend;
            } else {
                log::warn!("Ignoring unsupported audio backend query override: {audio_backend}");
            }
        }

        let auth_session = if cfg!(test) {
            None
        } else {
            match eg_platform::storage::load_auth_session() {
                Ok(session) => session,
                Err(error) => {
                    log::error!("Failed to load auth session: {error}");
                    None
                }
            }
        };

        let available_graphics_backends = discover_graphics_backends();
        let available_audio_backends = available_host_labels();
        let audio_runtime = AudioSceneRes::default();
        let _ = set_preferred_backend_name(&audio_runtime.0, &app_settings.audio_backend);
        let (audio_import_sender, audio_import_receiver) =
            std::sync::mpsc::channel::<(String, Vec<u8>)>();
        let audio_import_channel = (audio_import_sender, shared_receiver(audio_import_receiver));
        let (auth_sender, auth_receiver) = std::sync::mpsc::channel();
        let auth_receiver = std::sync::Arc::new(std::sync::Mutex::new(auth_receiver));
        let (level_import_sender, level_import_receiver) = std::sync::mpsc::channel();
        let level_import_channel = (
            level_import_sender,
            std::sync::Arc::new(std::sync::Mutex::new(level_import_receiver)),
        );

        Some(AppInitialization {
            render_resources: build_state_render_resources(
                GpuContext {
                    surface,
                    adapter_info,
                    device,
                    queue,
                    config,
                    size,
                    depth_texture,
                    depth_view,
                    editor_outline_occlusion_depth_texture,
                    editor_outline_occlusion_depth_view,
                    render_pipeline,
                    instanced_block_pipeline,
                    grid_pipeline,
                    block_icon_pipeline,
                    editor_ghost_trail_pipeline,
                    gizmo_overlay_pipeline,
                    editor_outline_occlusion_depth_pipeline,
                    editor_outline_mask_pipeline,
                    editor_outline_pipeline,
                    editor_hover_outline_pipeline,
                    line_bind_group_layout,
                    line_uniform_buffer,
                    zero_line_bind_group,
                    camera_uniform_buffer,
                    camera_bind_group_layout,
                    camera_bind_group,
                    color_space_bind_group_layout,
                    color_space_uniform_buffer,
                    color_space_bind_group,
                    grid_bind_group_layout,
                    grid_uniform_buffer,
                    grid_bind_group,
                    block_texture_bind_group,
                    apply_gamma_correction: should_apply_gamma_correction,
                },
                surface_host,
                SceneMeshes {
                    floor: floor_mesh,
                    grid: grid_mesh,
                    trail: trail_mesh,
                    gem_shatter_effects: MeshSlot::Empty,
                    practice_checkpoints: MeshSlot::Empty,
                    blocks_chunked: eg_render::ChunkedBlockMeshSlot::default(),
                    blocks_instanced: InstancedMeshSlot::default(),
                    blocks_static_instanced: InstancedMeshSlot::default(),
                    editor_cursor: MeshSlot::Empty,
                    editor_hover_stencil: MeshSlot::Empty,
                    editor_hover_outline: MeshSlot::Empty,
                    editor_selection_stencil: MeshSlot::Empty,
                    editor_selection_outline: MeshSlot::Empty,
                    editor_selection_outline_instances: Vec::new(),
                    editor_gizmo: MeshSlot::Empty,
                    tap_indicators: MeshSlot::Empty,
                    transform_trigger_markers: eg_render::RetainedMeshSlot::default(),
                    spawn_marker: MeshSlot::Empty,
                    editor_preview_player: MeshSlot::Empty,
                    editor_hitbox_visualization: MeshSlot::Empty,
                    camera_arrow: MeshSlot::Empty,
                    editor_block_mesh_generation: u64::MAX,
                    transform_trigger_marker_generation: u64::MAX,
                },
            ),
            auth: AuthStateRes {
                session: auth_session,
                pending: false,
                message: None,
                refresh_started: false,
                external_navigation_enabled: true,
                sender: auth_sender,
            },
            auth_receiver,

            phase: AppPhase::Menu,
            menu,
            audio_runtime,
            audio_import_channel,
            session: super::SessionStateRes {
                editor_level_name: None,
                editor_music_metadata: MusicMetadata {
                    title: None,
                    author: None,
                },
                editor_creator_metadata: BasicLevelMetadata::default(),
                editor_sky_color: LevelMetadata::default().sky_color,
                editor_menu_preview_camera: None,
                editor_show_metadata: false,
                editor_show_place_window: false,
                editor_show_settings: false,
                editor_settings_section: SettingsSection::Backends,
                editor_keybind_capture_action: None,
                settings_restart_required: false,
                graphics_backend_locked,
                audio_backend_locked,
                available_graphics_backends,
                available_audio_backends,
                app_settings: app_settings.clone(),
                playing_level_name: None,
                editor_custom_audio_bytes: None,
                playtesting_editor: false,
                game_paused: false,
                playtest_audio_start_seconds: None,
                playing_sky_color: LevelMetadata::default().sky_color,
                playing_trigger_base_objects: None,
                practice_mode_enabled: false,
                practice_checkpoints: Vec::new(),
                playtest_editor_objects: None,
                audio_stop_pending: false,
            },
            level_import_channel,
            editor_input: eg_types::EditorInputState::default(),
            editor_config: EditorConfigState {
                snap_to_grid: app_settings.editor_snap_to_grid,
                snap_step: app_settings.editor_snap_step.max(0.05),
                snap_rotation: app_settings.editor_rotation_snap,
                snap_rotation_step_degrees: app_settings.editor_rotation_snap_step.max(1.0),
                ..EditorConfigState::default()
            },
            editor_objects: Vec::new(),
            editor_spawn: eg_types::SpawnMetadata::default(),
            editor_camera: super::EditorCameraState::default(),
            editor_triggers: super::EditorTriggerState::default(),
            editor_timeline: super::EditorTimelineState::default(),
            editor_runtime: EditorRuntimeState::default(),
            editor_timing: super::EditorTimingState::default(),
            editor_history: super::EditorHistoryState::default(),
        })
    }
}

#[cfg(test)]
mod tests {

    use super::{
        block_texture_array_layer_count, editor_ghost_trail_depth_stencil_state,
        editor_ghost_trail_primitive_state, editor_hover_outline_depth_stencil_state,
        editor_outline_depth_stencil_state, editor_outline_mask_depth_stencil_state,
        editor_outline_occlusion_depth_mask_depth_stencil_state,
    };

    #[test]
    fn block_texture_array_layer_count_avoids_gl_cube_heuristics() {
        assert_eq!(block_texture_array_layer_count(0), 2);
        assert_eq!(block_texture_array_layer_count(1), 2);
        assert_eq!(block_texture_array_layer_count(5), 5);
        assert_eq!(block_texture_array_layer_count(6), 7);
        assert_eq!(block_texture_array_layer_count(11), 11);
        assert_eq!(block_texture_array_layer_count(12), 13);
    }

    #[test]
    fn editor_ghost_trail_pipeline_state_prevents_floor_z_fighting() {
        let primitive = editor_ghost_trail_primitive_state();
        assert_eq!(
            primitive.front_face,
            wgpu::FrontFace::Ccw,
            "ghost trail must keep CCW front-face culling so floor-facing bottoms stay culled"
        );
        assert_eq!(
            primitive.cull_mode,
            Some(wgpu::Face::Back),
            "ghost trail should cull back faces"
        );

        let depth = editor_ghost_trail_depth_stencil_state();
        assert_eq!(
            depth.depth_compare,
            Some(wgpu::CompareFunction::Less),
            "ghost trail depth compare should remain Less"
        );
        assert!(
            !depth.depth_write_enabled.unwrap_or_default(),
            "ghost trail should not write to depth to preserve translucent blending order"
        );
    }

    #[test]
    fn editor_outline_pipeline_state_respects_selected_outline_depth() {
        let occlusion_depth = editor_outline_occlusion_depth_mask_depth_stencil_state();
        assert_eq!(
            occlusion_depth.depth_compare,
            Some(wgpu::CompareFunction::Less)
        );
        assert!(occlusion_depth.depth_write_enabled.unwrap_or_default());

        let mask = editor_outline_mask_depth_stencil_state();
        assert_eq!(mask.depth_compare, Some(wgpu::CompareFunction::Always));
        assert!(!mask.depth_write_enabled.unwrap_or_default());

        let outline = editor_outline_depth_stencil_state();
        assert_eq!(
            outline.depth_compare,
            Some(wgpu::CompareFunction::LessEqual)
        );
        assert!(!outline.depth_write_enabled.unwrap_or_default());
        assert_eq!(outline.bias.constant, -1);

        let hover_outline = editor_hover_outline_depth_stencil_state();
        assert_eq!(
            hover_outline.depth_compare,
            Some(wgpu::CompareFunction::Always)
        );
        assert!(!hover_outline.depth_write_enabled.unwrap_or_default());
        assert_eq!(
            hover_outline.stencil.front.compare,
            wgpu::CompareFunction::NotEqual
        );
    }
}
