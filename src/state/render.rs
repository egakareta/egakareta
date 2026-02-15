use std::iter;

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::{SurfaceError, TextureViewDescriptor};

use super::*;

impl State {
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

            if self.phase != AppPhase::Menu && self.editor_mode != EditorMode::Timing {
                if let Some((buffer, count)) = self.meshes.floor.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.meshes.grid.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..count, 0..1);
                }
            }

            if self.phase == AppPhase::Playing
                || self.phase == AppPhase::GameOver
                || self.phase == AppPhase::Editor
                || self.phase == AppPhase::Menu
            {
                let skip_world =
                    self.phase == AppPhase::Editor && self.editor_mode == EditorMode::Timing;

                if !skip_world {
                    if let Some((buffer, count)) = self.meshes.blocks.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.trail.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }
                }

                if self.phase == AppPhase::Editor && !skip_world {
                    if let Some((buffer, count)) = self.meshes.spawn_marker.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.tap_indicators.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.editor_preview_player.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.editor_selection_outline.draw_data()
                    {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.editor_hover_outline.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.meshes.editor_gizmo.draw_data() {
                        render_pass.set_pipeline(&self.gizmo_overlay_pipeline);
                        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.color_space_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);

                        render_pass.set_pipeline(&self.render_pipeline);
                        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.color_space_bind_group, &[]);
                    }

                    if self.editor_mode == EditorMode::Place {
                        if let Some((buffer, count)) = self.meshes.editor_cursor.draw_data() {
                            render_pass.set_vertex_buffer(0, buffer.slice(..));
                            render_pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
                            render_pass.draw(0..count, 0..1);
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

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}
