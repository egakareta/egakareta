use std::iter;

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::{SurfaceError, TextureViewDescriptor};

use super::super::State;
use crate::types::{AppPhase, EditorMode};

fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

impl State {
    /// Renders the `egui` user interface over the current frame.
    ///
    /// This should be called after the main scene rendering to ensure the UI
    /// appears on top. It handles updating `egui` buffers and executing the
    /// render pass.
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
                        depth_slice: None,
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

    /// Creates a new `EguiRenderer` configured for the current GPU device and surface format.
    pub fn create_egui_renderer(&self) -> EguiRenderer {
        EguiRenderer::new(
            &self.render.gpu.device,
            self.render.gpu.config.format,
            egui_wgpu::RendererOptions::default(),
        )
    }

    /// Performs a full render of the current application state.
    ///
    /// This method clears the surface and draws the active scene (Menu, Editor, or Gameplay).
    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.render_with_overlay(|_, _, _, _| {})
    }

    /// Renders the current scene with an additional custom overlay pass.
    ///
    /// The `overlay` closure is provided with the GPU device, queue, current texture view,
    /// and a command encoder to perform additional drawing operations.
    pub fn render_with_overlay<F>(&mut self, overlay: F) -> Result<(), SurfaceError>
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        let surface = match &self.render.gpu.surface {
            Some(s) => s,
            None => return Ok(()),
        };
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder =
            self.render
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let clear_color = match self.phase {
                AppPhase::Splash => {
                    let p = self.frame_runtime.splash.progress;
                    // Stay black for the first 40% then fade in
                    let bg_p = ((p - 0.4) / 0.6).max(0.0);
                    let eased_bg_p = bg_p * bg_p; // Quadratic ease-in

                    let target_color = wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.08,
                        a: 1.0,
                    };
                    wgpu::Color {
                        r: target_color.r * eased_bg_p as f64,
                        g: target_color.g * eased_bg_p as f64,
                        b: target_color.b * eased_bg_p as f64,
                        a: 1.0,
                    }
                }
                AppPhase::Playing if self.gameplay.state.game_over => wgpu::Color {
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

            let clear_color = if self.render.gpu.apply_gamma_correction {
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
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.render.gpu.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render.gpu.render_pipeline);
            render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
            render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);

            if self.phase != AppPhase::Menu
                && self.phase != AppPhase::Splash
                && self.editor.ui.mode != EditorMode::Timing
            {
                if let Some((buffer, count)) = self.render.meshes.floor.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.grid.draw_data() {
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
                    self.phase == AppPhase::Editor && self.editor.ui.mode == EditorMode::Timing;

                if !skip_world {
                    if self.phase == AppPhase::Editor {
                        if let Some((buffer, count)) = self.render.meshes.blocks_static.draw_data()
                        {
                            render_pass.set_vertex_buffer(0, buffer.slice(..));
                            render_pass.set_bind_group(
                                1,
                                &self.render.gpu.zero_line_bind_group,
                                &[],
                            );
                            render_pass.draw(0..count, 0..1);
                        }

                        if let Some((buffer, count)) =
                            self.render.meshes.blocks_selected.draw_data()
                        {
                            render_pass.set_vertex_buffer(0, buffer.slice(..));
                            render_pass.set_bind_group(
                                1,
                                &self.render.gpu.zero_line_bind_group,
                                &[],
                            );
                            render_pass.draw(0..count, 0..1);
                        }
                    } else if let Some((buffer, count)) = self.render.meshes.blocks.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.render.meshes.trail.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }
                }

                if self.phase == AppPhase::Editor && !skip_world {
                    if let Some((buffer, count)) = self.render.meshes.spawn_marker.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) =
                        self.render.meshes.camera_trigger_markers.draw_data()
                    {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.render.meshes.tap_indicators.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) =
                        self.render.meshes.editor_preview_player.draw_data()
                    {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) =
                        self.render.meshes.editor_selection_outline.draw_data()
                    {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) =
                        self.render.meshes.editor_hover_outline.draw_data()
                    {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.render.meshes.editor_gizmo.draw_data() {
                        render_pass.set_pipeline(&self.render.gpu.gizmo_overlay_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);

                        render_pass.set_pipeline(&self.render.gpu.render_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                    }

                    if self.editor.ui.mode == EditorMode::Place {
                        if let Some((buffer, count)) = self.render.meshes.editor_cursor.draw_data()
                        {
                            render_pass.set_vertex_buffer(0, buffer.slice(..));
                            render_pass.set_bind_group(
                                1,
                                &self.render.gpu.zero_line_bind_group,
                                &[],
                            );
                            render_pass.draw(0..count, 0..1);
                        }
                    }
                }
            }
        }

        overlay(
            &self.render.gpu.device,
            &self.render.gpu.queue,
            &view,
            &mut encoder,
        );

        self.render.gpu.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    /// Recreates the window surface following a resize or other configuration change.
    pub fn recreate_surface(&mut self) {
        let size = self.render.gpu.current_size();
        self.resize_surface(size);
    }
}
