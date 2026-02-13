use super::*;

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

    pub(super) async fn new_common(
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
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            trail_vertex_buffer,
            trail_vertex_count: 0,
            block_vertex_buffer: Some(block_vertex_buffer),
            block_vertex_count,
            editor_cursor_vertex_buffer: None,
            editor_cursor_vertex_count: 0,
            editor_hover_outline_vertex_buffer: None,
            editor_hover_outline_vertex_count: 0,
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
            editor_ctrl_held: false,
            editor_mode: EditorMode::Place,
            editor_snap_to_grid: true,
            editor_snap_step: 1.0,
            editor_selected_block_index: None,
            editor_selected_block_indices: Vec::new(),
            editor_hovered_block_index: None,
            editor_gizmo_drag: None,
            editor_block_drag: None,
            editor_pointer_screen: None,
            editor_clipboard_block: None,
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

        if key == "Control" || key == "ControlLeft" || key == "ControlRight" {
            self.set_editor_ctrl_held(pressed);
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

    pub fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor_right_dragging {
            return false;
        }

        self.editor_pointer_screen = Some([x, y]);

        let Some(drag) = self.editor_gizmo_drag.clone() else {
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
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
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

        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let axis_screen_dir = axis_screen_delta.normalize();
        let projected_pixels = effective_mouse_delta.dot(axis_screen_dir);
        let pixels_per_world_unit = axis_screen_delta.length();
        if pixels_per_world_unit <= f32::EPSILON {
            return true;
        }
        let world_delta = projected_pixels / pixels_per_world_unit;

        match drag.kind {
            GizmoDragKind::Move => {
                let snap_enabled = self.editor_snap_to_grid;
                let snap_step = self.editor_snap_step.max(0.05);
                let mut first_cursor: Option<[f32; 3]> = None;
                for block in &drag.start_blocks {
                    if let Some(obj) = self.editor_objects.get_mut(block.index) {
                        let mut next = block.position;
                        match drag.axis {
                            GizmoAxis::X => next[0] += world_delta,
                            GizmoAxis::Y => next[1] += world_delta,
                            GizmoAxis::Z => next[2] += world_delta,
                        }
                        if snap_enabled {
                            next[0] = (next[0] / snap_step).round() * snap_step;
                            next[1] = (next[1] / snap_step).round() * snap_step;
                            next[2] = (next[2].max(0.0) / snap_step).round() * snap_step;
                        } else {
                            next[2] = next[2].max(0.0);
                        }
                        obj.position = next;
                        if first_cursor.is_none() {
                            first_cursor = Some(next);
                        }
                    }
                }
                if let Some(next_position) = first_cursor {
                    let bounds = self.editor.bounds;
                    self.editor.cursor = [
                        (next_position[0].floor() as i32).clamp(-bounds, bounds),
                        (next_position[1].floor() as i32).clamp(-bounds, bounds),
                        (next_position[2].floor() as i32).max(0),
                    ];
                }
                self.sync_editor_objects();
                self.rebuild_editor_cursor_vertices();
            }
            GizmoDragKind::Resize => {
                let snap_enabled = self.editor_snap_to_grid;
                let snap_step = self.editor_snap_step.max(0.05);
                let min_size = if snap_enabled { snap_step } else { 0.25 };
                for block in &drag.start_blocks {
                    if let Some(obj) = self.editor_objects.get_mut(block.index) {
                        let mut next = block.size;
                        match drag.axis {
                            GizmoAxis::X => next[0] += world_delta,
                            GizmoAxis::Y => next[1] += world_delta,
                            GizmoAxis::Z => next[2] += world_delta,
                        }
                        if snap_enabled {
                            next[0] = (next[0] / snap_step).round() * snap_step;
                            next[1] = (next[1] / snap_step).round() * snap_step;
                            next[2] = (next[2] / snap_step).round() * snap_step;
                        }
                        obj.size = [
                            next[0].max(min_size),
                            next[1].max(min_size),
                            next[2].max(min_size),
                        ];
                    }
                }
                self.sync_editor_objects();
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

        self.editor_pointer_screen = Some([x, y]);

        let Some(drag) = self.editor_block_drag.clone() else {
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
        let center = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );

        let Some(origin_screen) = self.world_to_screen(center) else {
            return true;
        };

        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let right_world = Vec3::new(camera_right_xy.x, camera_right_xy.y, 0.0);
        let up_world = Vec3::new(camera_up_xy.x, camera_up_xy.y, 0.0);

        let Some(right_screen) = self.world_to_screen(center + right_world) else {
            return true;
        };
        let Some(up_screen) = self.world_to_screen(center + up_world) else {
            return true;
        };

        let right_screen_delta = right_screen - origin_screen;
        let up_screen_delta = up_screen - origin_screen;

        let det =
            right_screen_delta.x * up_screen_delta.y - right_screen_delta.y * up_screen_delta.x;
        if det.abs() <= f32::EPSILON {
            return true;
        }

        let delta_x = effective_mouse_delta.x;
        let delta_y = effective_mouse_delta.y;
        let right_units = (delta_x * up_screen_delta.y - delta_y * up_screen_delta.x) / det;
        let up_units = (delta_y * right_screen_delta.x - delta_x * right_screen_delta.y) / det;

        let move_x = right_world.x * right_units + up_world.x * up_units;
        let move_y = right_world.y * right_units + up_world.y * up_units;
        let snap_enabled = self.editor_snap_to_grid;
        let snap_step = self.editor_snap_step.max(0.05);
        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.editor_objects.get_mut(block.index) {
                let mut next = block.position;
                next[0] += move_x;
                next[1] += move_y;
                if snap_enabled {
                    next[0] = (next[0] / snap_step).round() * snap_step;
                    next[1] = (next[1] / snap_step).round() * snap_step;
                    next[2] = (next[2].max(0.0) / snap_step).round() * snap_step;
                } else {
                    next[2] = next[2].max(0.0);
                }
                obj.position = next;
                if first_cursor.is_none() {
                    first_cursor = Some(next);
                }
            }
        }
        if let Some(next_position) = first_cursor {
            let bounds = self.editor.bounds;
            self.editor.cursor = [
                (next_position[0].floor() as i32).clamp(-bounds, bounds),
                (next_position[1].floor() as i32).clamp(-bounds, bounds),
                (next_position[2].floor() as i32).max(0),
            ];
        }
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
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
}
