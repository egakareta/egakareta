use super::*;

impl State {
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
            if (self.editor_gizmo_drag.is_some() || self.editor_block_drag.is_some())
                && self.editor_pointer_screen.is_some()
            {
                let pointer = self.editor_pointer_screen.unwrap();
                self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
            }
            self.rebuild_editor_gizmo_vertices();
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

    pub(super) fn update_menu_camera(&mut self) {
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

    pub(super) fn update_editor_camera(&mut self) {
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

                    if let Some(buf) = &self.editor_hover_outline_vertex_buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_hover_outline_vertex_count, 0..1);
                    }

                    if let Some(buf) = &self.editor_gizmo_vertex_buffer {
                        render_pass.set_pipeline(&self.gizmo_overlay_pipeline);
                        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.color_space_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..self.editor_gizmo_vertex_count, 0..1);

                        render_pass.set_pipeline(&self.render_pipeline);
                        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.color_space_bind_group, &[]);
                    }

                    if self.editor_mode == EditorMode::Place {
                        if let Some(buf) = &self.editor_cursor_vertex_buffer {
                            render_pass.set_vertex_buffer(0, buf.slice(..));
                            render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                            render_pass.draw(0..self.editor_cursor_vertex_count, 0..1);
                        }
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

    pub(super) fn apply_resize(&mut self, new_size: PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub(super) fn create_depth_texture(
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
