/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::iter;

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::{CurrentSurfaceTexture, TextureViewDescriptor};

use super::super::State;
use super::{MeshDrawData, MeshSlot};
use crate::types::{default_sky_color, AppPhase, EditorMode, GridUniform};

const GRID_HALF_EXTENT: f32 = 2048.0;

fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

fn color_from_rgb(rgb: [f32; 3]) -> wgpu::Color {
    wgpu::Color {
        r: rgb[0].clamp(0.0, 1.0) as f64,
        g: rgb[1].clamp(0.0, 1.0) as f64,
        b: rgb[2].clamp(0.0, 1.0) as f64,
        a: 1.0,
    }
}

fn clear_color_for_phase(phase: AppPhase, game_over: bool, sky_color: [f32; 3]) -> wgpu::Color {
    match phase {
        AppPhase::Playing if game_over => wgpu::Color {
            r: 0.15,
            g: 0.05,
            b: 0.05,
            a: 1.0,
        },
        AppPhase::Menu | AppPhase::Editor | AppPhase::Playing => color_from_rgb(sky_color),
        _ => wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.08,
            a: 1.0,
        },
    }
}

fn apply_gamma_correction_if_enabled(
    color: wgpu::Color,
    apply_gamma_correction: bool,
) -> wgpu::Color {
    if apply_gamma_correction {
        wgpu::Color {
            r: linear_to_srgb(color.r as f32) as f64,
            g: linear_to_srgb(color.g as f32) as f64,
            b: linear_to_srgb(color.b as f32) as f64,
            a: color.a,
        }
    } else {
        color
    }
}

fn should_draw_floor_and_grid(phase: AppPhase, mode: EditorMode) -> bool {
    phase != AppPhase::Menu && mode != EditorMode::Timing
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn editor_grid_darkening(phase: AppPhase, sky_color: [f32; 3]) -> f32 {
    if phase != AppPhase::Editor {
        return 0.0;
    }

    let luminance = sky_color[0].clamp(0.0, 1.0) * 0.2126
        + sky_color[1].clamp(0.0, 1.0) * 0.7152
        + sky_color[2].clamp(0.0, 1.0) * 0.0722;
    0.10 + 0.35 * smoothstep(0.35, 0.80, luminance)
}

fn should_skip_world(phase: AppPhase, mode: EditorMode) -> bool {
    phase == AppPhase::Editor && mode == EditorMode::Timing
}

fn should_draw_editor_overlays(phase: AppPhase, skip_world: bool) -> bool {
    phase == AppPhase::Editor && !skip_world
}

fn should_draw_editor_cursor(mode: EditorMode) -> bool {
    mode == EditorMode::Place || mode == EditorMode::Tapping
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceError {
    Outdated,
    Lost,
    Validation,
}

fn current_surface_output(
    texture: CurrentSurfaceTexture,
) -> Result<Option<wgpu::SurfaceTexture>, RenderSurfaceError> {
    match texture {
        CurrentSurfaceTexture::Success(output) | CurrentSurfaceTexture::Suboptimal(output) => {
            Ok(Some(output))
        }
        CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => Ok(None),
        CurrentSurfaceTexture::Outdated => Err(RenderSurfaceError::Outdated),
        CurrentSurfaceTexture::Lost => Err(RenderSurfaceError::Lost),
        CurrentSurfaceTexture::Validation => Err(RenderSurfaceError::Validation),
    }
}

fn draw_mesh(render_pass: &mut wgpu::RenderPass<'_>, draw_data: MeshDrawData<'_>) {
    match draw_data {
        MeshDrawData::Vertices { buffer, count } => {
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..count, 0..1);
        }
        MeshDrawData::Indexed {
            vertex_buffer,
            index_buffer,
            count,
        } => {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..count, 0, 0..1);
        }
    }
}

fn grid_center_for_phase(state: &State) -> [f32; 2] {
    match state.phase {
        AppPhase::Editor if state.editor_is_playing() => [
            state.editor.timeline.preview.position[0],
            state.editor.timeline.preview.position[2],
        ],
        AppPhase::Editor => state.editor.camera.editor_pan,
        AppPhase::Playing | AppPhase::GameOver => [
            state.gameplay.state.position[0],
            state.gameplay.state.position[2],
        ],
        _ => [0.0, 0.0],
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
    ) -> Result<(), RenderSurfaceError> {
        puffin::profile_scope!("RenderEgui");
        self.render_with_overlay(|device, queue, view, encoder| {
            {
                puffin::profile_scope!("EguiUpdateBuffers");
                renderer.update_buffers(device, queue, encoder, paint_jobs, screen_descriptor);
            }

            {
                puffin::profile_scope!("EguiRenderPass");
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
                        multiview_mask: None,
                    })
                    .forget_lifetime();

                renderer.render(&mut pass, paint_jobs, screen_descriptor);
            }
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
    pub fn render(&mut self) -> Result<(), RenderSurfaceError> {
        puffin::profile_scope!("RenderFrame");
        self.render_with_overlay(|_, _, _, _| {})
    }

    /// Renders the current scene with an additional custom overlay pass.
    ///
    /// The `overlay` closure is provided with the GPU device, queue, current texture view,
    /// and a command encoder to perform additional drawing operations.
    pub fn render_with_overlay<F>(&mut self, overlay: F) -> Result<(), RenderSurfaceError>
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        puffin::profile_scope!("RenderWithOverlay");
        let surface = match &self.render.gpu.surface {
            Some(s) => s,
            None => return Ok(()),
        };
        let Some(output) = ({
            puffin::profile_scope!("SurfaceAcquire");
            current_surface_output(surface.get_current_texture())?
        }) else {
            return Ok(());
        };
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
            puffin::profile_scope!("RenderToView");
            self.render_to_view(&view, &mut encoder, overlay);
        }

        {
            puffin::profile_scope!("GpuSubmit");
            self.render.gpu.queue.submit(iter::once(encoder.finish()));
        }
        {
            puffin::profile_scope!("SurfacePresent");
            output.present();
        }
        Ok(())
    }

    pub(crate) fn render_to_view<F>(
        &mut self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        overlay: F,
    ) where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        puffin::profile_scope!("RenderWorld");
        let editor_mode = self.editor.ui.mode;
        let skip_world = should_skip_world(self.phase, editor_mode);
        let draw_editor_overlays = should_draw_editor_overlays(self.phase, skip_world);
        let sky_color = match self.phase {
            AppPhase::Menu => self.menu.state.preview_sky_color,
            AppPhase::Editor => self.session.editor_sky_color,
            AppPhase::Playing => self.session.playing_sky_color,
            _ => default_sky_color(),
        };
        let clear_color = apply_gamma_correction_if_enabled(
            clear_color_for_phase(self.phase, self.gameplay.state.game_over, sky_color),
            self.render.gpu.apply_gamma_correction,
        );

        let grid_uniform = GridUniform {
            center: grid_center_for_phase(self),
            half_extent: GRID_HALF_EXTENT,
            darkening: editor_grid_darkening(self.phase, sky_color),
        };
        self.render.gpu.queue.write_buffer(
            &self.render.gpu.grid_uniform_buffer,
            0,
            bytemuck::bytes_of(&grid_uniform),
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.render.gpu.render_pipeline);
        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
        render_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);

        if should_draw_floor_and_grid(self.phase, editor_mode) {
            puffin::profile_scope!("DrawFloorGrid");
            if let Some(draw_data) = self.render.meshes.floor.draw_data() {
                draw_mesh(&mut render_pass, draw_data);
            }

            if let Some(draw_data) = self.render.meshes.grid.draw_data() {
                render_pass.set_pipeline(&self.render.gpu.grid_pipeline);
                render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.render.gpu.grid_bind_group, &[]);
                render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                draw_mesh(&mut render_pass, draw_data);
                render_pass.set_pipeline(&self.render.gpu.render_pipeline);
                render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                render_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
            }
        }

        if self.phase == AppPhase::Playing
            || self.phase == AppPhase::GameOver
            || self.phase == AppPhase::Editor
            || self.phase == AppPhase::Menu
        {
            if !skip_world {
                puffin::profile_scope!("DrawBlocksTrail");
                if self.phase == AppPhase::Editor {
                    if let Some(draw_data) = self.render.meshes.blocks_static.draw_data() {
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        draw_mesh(&mut render_pass, draw_data);
                    }

                    if let Some(draw_data) = self.render.meshes.blocks_selected.draw_data() {
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        draw_mesh(&mut render_pass, draw_data);
                    }
                } else if let Some(draw_data) = self.render.meshes.blocks.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render.meshes.gem_shatter_effects.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render.meshes.practice_checkpoints.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render.meshes.trail.draw_data() {
                    if self.phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render.gpu.editor_ghost_trail_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        render_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );
                    }

                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);

                    if self.phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render.gpu.render_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        render_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );
                    }
                }
            }

            if draw_editor_overlays {
                puffin::profile_scope!("DrawEditorOverlayMeshes");
                if let Some(draw_data) = self.render.meshes.spawn_marker.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render.meshes.camera_trigger_markers.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render.meshes.tap_indicators.draw_data() {
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }
            }
        }

        drop(render_pass);

        if draw_editor_overlays {
            puffin::profile_scope!("DrawEditorOverlays");
            // Selection outlines use a selected-only depth prepass, so each block keeps its own
            // hollow silhouette while other selected blocks occlude outlines behind them.
            let selection_instances = self
                .render
                .meshes
                .editor_selection_outline_instances
                .as_slice();

            if !selection_instances.is_empty() {
                puffin::profile_scope!("DrawSelectionOutlines");
                let selection_mask_buffer = match &self.render.meshes.editor_selection_stencil {
                    MeshSlot::VertexData { buffer, .. } => Some(buffer),
                    _ => None,
                };
                let selection_outline_buffer = match &self.render.meshes.editor_selection_outline {
                    MeshSlot::VertexData { buffer, .. } => Some(buffer),
                    _ => None,
                };

                if let (Some(selection_mask_buffer), Some(selection_outline_buffer)) =
                    (selection_mask_buffer, selection_outline_buffer)
                {
                    {
                        let mut depth_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Editor Selection Outline Depth Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: &self.render.gpu.editor_outline_occlusion_depth_view,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                multiview_mask: None,
                            });

                        depth_pass
                            .set_pipeline(&self.render.gpu.editor_outline_occlusion_depth_pipeline);
                        depth_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        depth_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        depth_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        depth_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );
                        depth_pass.set_vertex_buffer(0, selection_mask_buffer.slice(..));
                        for instance in selection_instances {
                            depth_pass.draw(instance.mask_vertices.clone(), 0..1);
                        }
                    }

                    for batch in selection_instances.chunks(255) {
                        let mut outline_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Editor Selection Outline Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: &self.render.gpu.editor_outline_occlusion_depth_view,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Load,
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                multiview_mask: None,
                            });

                        outline_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        outline_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        outline_pass.set_bind_group(
                            2,
                            &self.render.gpu.color_space_bind_group,
                            &[],
                        );
                        outline_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );

                        for (batch_index, instance) in batch.iter().enumerate() {
                            let stencil_ref = (batch_index as u32) + 1;
                            outline_pass.set_stencil_reference(stencil_ref);

                            outline_pass
                                .set_pipeline(&self.render.gpu.editor_outline_mask_pipeline);
                            outline_pass.set_vertex_buffer(0, selection_mask_buffer.slice(..));
                            outline_pass.draw(instance.mask_vertices.clone(), 0..1);

                            outline_pass.set_pipeline(&self.render.gpu.editor_outline_pipeline);
                            outline_pass.set_vertex_buffer(0, selection_outline_buffer.slice(..));
                            outline_pass.draw(instance.outline_vertices.clone(), 0..1);
                        }
                    }
                }
            }

            if let (Some(mask_data), Some(outline_data)) = (
                self.render.meshes.editor_hover_stencil.draw_data(),
                self.render.meshes.editor_hover_outline.draw_data(),
            ) {
                puffin::profile_scope!("DrawHoverOutline");
                let mut hover_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Editor Hover Outline Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.render.gpu.editor_outline_occlusion_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });

                hover_pass.set_stencil_reference(1);
                hover_pass.set_pipeline(&self.render.gpu.editor_outline_mask_pipeline);
                hover_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                hover_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                hover_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                hover_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                draw_mesh(&mut hover_pass, mask_data);

                hover_pass.set_pipeline(&self.render.gpu.editor_hover_outline_pipeline);
                hover_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                hover_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                hover_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                hover_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                draw_mesh(&mut hover_pass, outline_data);
            }

            puffin::profile_scope!("DrawGizmoCursor");
            let mut gizmo_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Editor Gizmo/Cursor Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.render.gpu.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            gizmo_pass.set_stencil_reference(0);
            gizmo_pass.set_pipeline(&self.render.gpu.render_pipeline);
            gizmo_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
            gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
            gizmo_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
            gizmo_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);

            if let Some(draw_data) = self.render.meshes.editor_gizmo.draw_data() {
                gizmo_pass.set_pipeline(&self.render.gpu.gizmo_overlay_pipeline);
                gizmo_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);

                gizmo_pass.set_pipeline(&self.render.gpu.render_pipeline);
                gizmo_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
            }

            if let Some(draw_data) = self.render.meshes.editor_hitbox_visualization.draw_data() {
                gizmo_pass.set_pipeline(&self.render.gpu.gizmo_overlay_pipeline);
                gizmo_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);

                gizmo_pass.set_pipeline(&self.render.gpu.render_pipeline);
                gizmo_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
            }

            if should_draw_editor_cursor(editor_mode) {
                if let Some(draw_data) = self.render.meshes.editor_cursor.draw_data() {
                    gizmo_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    draw_mesh(&mut gizmo_pass, draw_data);
                }
            }
        }

        {
            puffin::profile_scope!("RenderOverlayClosure");
            overlay(
                &self.render.gpu.device,
                &self.render.gpu.queue,
                view,
                encoder,
            );
        }
    }

    /// Recreates the window surface following a resize or other configuration change.
    pub fn recreate_surface(&mut self) {
        let size = self.render.gpu.current_size();
        self.resize_surface(size);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::mpsc;

    use egui_wgpu::ScreenDescriptor;
    use glam::Mat4;
    use wgpu::Color;

    use super::super::{EditorOutlineInstance, MeshSlot};
    use super::{
        apply_gamma_correction_if_enabled, clear_color_for_phase, current_surface_output,
        editor_grid_darkening, linear_to_srgb, should_draw_editor_cursor,
        should_draw_editor_overlays, should_draw_floor_and_grid, should_skip_world,
        RenderSurfaceError,
    };
    use crate::state::State;
    use crate::types::{AppPhase, CameraUniform, EditorMode, PhysicalSize, Vertex};

    fn approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {a} to be within {eps} of {b}"
        );
    }

    fn assert_color_approx(actual: Color, expected: Color) {
        approx_eq(actual.r as f32, expected.r as f32, 1e-6);
        approx_eq(actual.g as f32, expected.g as f32, 1e-6);
        approx_eq(actual.b as f32, expected.b as f32, 1e-6);
        approx_eq(actual.a as f32, expected.a as f32, 1e-6);
    }

    fn fullscreen_triangle(z: f32, color: [f32; 4]) -> Vec<Vertex> {
        vec![
            Vertex::untextured([-1.0, -1.0, z], color),
            Vertex::untextured([3.0, -1.0, z], color),
            Vertex::untextured([-1.0, 3.0, z], color),
        ]
    }

    fn corner_triangle(z: f32, color: [f32; 4]) -> Vec<Vertex> {
        vec![
            Vertex::untextured([-0.95, -0.95, z], color),
            Vertex::untextured([-0.85, -0.95, z], color),
            Vertex::untextured([-0.95, -0.85, z], color),
        ]
    }

    fn rgba_from_surface_format(format: wgpu::TextureFormat, pixel: [u8; 4]) -> [u8; 4] {
        match format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                [pixel[2], pixel[1], pixel[0], pixel[3]]
            }
            _ => pixel,
        }
    }

    fn render_center_pixel(state: &mut State) -> [u8; 4] {
        let size = 8;
        state.resize_surface(PhysicalSize {
            width: size,
            height: size,
        });
        let camera = CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        };
        state.queue().write_buffer(
            &state.render.gpu.camera_uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera]),
        );

        let texture = state.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Outline Occlusion Test Render Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: state.render.gpu.config.format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let readback = state.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Outline Occlusion Test Readback Buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = state
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Outline Occlusion Test Encoder"),
            });

        state.render_to_view(&view, &mut encoder, |_, _, _, _| {});
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: size / 2,
                    y: size / 2,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        state.queue().submit(std::iter::once(encoder.finish()));

        let buffer_slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).expect("readback result should send");
        });
        state
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device polling should succeed");
        receiver
            .recv()
            .expect("readback result should be received")
            .expect("readback mapping should succeed");
        let data = buffer_slice.get_mapped_range();
        let pixel = [data[0], data[1], data[2], data[3]];
        drop(data);
        readback.unmap();
        rgba_from_surface_format(state.render.gpu.config.format, pixel)
    }

    fn prepare_outline_occlusion_test_state(state: &mut State) {
        state.phase = AppPhase::Editor;
        state.editor.ui.mode = EditorMode::Select;
        state.render.meshes.floor = MeshSlot::Empty;
        state.render.meshes.grid = MeshSlot::Empty;
        state.render.meshes.trail = MeshSlot::Empty;
        state.render.meshes.blocks = MeshSlot::Empty;
        state.render.meshes.blocks_static = MeshSlot::Empty;
        state.render.meshes.blocks_selected = MeshSlot::Empty;
        state.render.meshes.editor_cursor = MeshSlot::Empty;
        state.render.meshes.editor_hover_stencil = MeshSlot::Empty;
        state.render.meshes.editor_hover_outline = MeshSlot::Empty;
        state.render.meshes.editor_selection_stencil = MeshSlot::Empty;
        state.render.meshes.editor_selection_outline = MeshSlot::Empty;
        state
            .render
            .meshes
            .editor_selection_outline_instances
            .clear();
        state.render.meshes.editor_gizmo = MeshSlot::Empty;
        state.render.meshes.tap_indicators = MeshSlot::Empty;
        state.render.meshes.spawn_marker = MeshSlot::Empty;
        state.render.meshes.camera_trigger_markers = MeshSlot::Empty;
        state.render.meshes.editor_preview_player = MeshSlot::Empty;
    }

    #[test]
    fn linear_to_srgb_uses_linear_segment_at_or_below_threshold() {
        let below = 0.002;
        let at_threshold = 0.0031308;

        approx_eq(linear_to_srgb(below), below * 12.92, 1e-7);
        approx_eq(linear_to_srgb(at_threshold), at_threshold * 12.92, 1e-7);
    }

    #[test]
    fn linear_to_srgb_uses_gamma_segment_above_threshold() {
        let mid_gray_linear = 0.18;
        let expected = 0.461_356_1;

        approx_eq(linear_to_srgb(mid_gray_linear), expected, 1e-6);
    }

    #[test]
    fn clear_color_for_phase_matches_expected_palette() {
        assert_color_approx(
            clear_color_for_phase(AppPhase::Playing, true, crate::types::default_sky_color()),
            Color {
                r: 0.15,
                g: 0.05,
                b: 0.05,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Editor, false, crate::types::default_sky_color()),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Playing, false, crate::types::default_sky_color()),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Menu, false, crate::types::default_sky_color()),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::GameOver, false, crate::types::default_sky_color()),
            Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            },
        );
    }

    #[test]
    fn clear_color_for_phase_uses_custom_sky_for_menu_editor_and_playing() {
        let sky_color = [0.2, 0.4, 0.8];

        assert_color_approx(
            clear_color_for_phase(AppPhase::Menu, false, sky_color),
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
        );
        assert_color_approx(
            clear_color_for_phase(AppPhase::Editor, false, sky_color),
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
        );
        assert_color_approx(
            clear_color_for_phase(AppPhase::Playing, false, sky_color),
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
        );
    }

    #[test]
    fn apply_gamma_correction_if_enabled_only_changes_rgb() {
        let linear = Color {
            r: 0.18,
            g: 0.04,
            b: 0.01,
            a: 0.5,
        };

        let unchanged = apply_gamma_correction_if_enabled(linear, false);
        assert_eq!(unchanged, linear);

        let corrected = apply_gamma_correction_if_enabled(linear, true);
        approx_eq(corrected.r as f32, linear_to_srgb(linear.r as f32), 1e-7);
        approx_eq(corrected.g as f32, linear_to_srgb(linear.g as f32), 1e-7);
        approx_eq(corrected.b as f32, linear_to_srgb(linear.b as f32), 1e-7);
        approx_eq(corrected.a as f32, linear.a as f32, 1e-7);
    }

    #[test]
    fn render_gate_helpers_match_phase_and_mode_rules() {
        assert!(!should_draw_floor_and_grid(
            AppPhase::Menu,
            EditorMode::Place
        ));
        assert!(!should_draw_floor_and_grid(
            AppPhase::Editor,
            EditorMode::Timing
        ));
        assert!(should_draw_floor_and_grid(
            AppPhase::Playing,
            EditorMode::Place
        ));
        assert!(should_draw_floor_and_grid(
            AppPhase::GameOver,
            EditorMode::Trigger
        ));

        assert!(should_skip_world(AppPhase::Editor, EditorMode::Timing));
        assert!(!should_skip_world(AppPhase::Editor, EditorMode::Place));
        assert!(!should_skip_world(AppPhase::Playing, EditorMode::Timing));

        assert!(should_draw_editor_overlays(AppPhase::Editor, false));
        assert!(!should_draw_editor_overlays(AppPhase::Editor, true));
        assert!(!should_draw_editor_overlays(AppPhase::Menu, false));

        assert!(should_draw_editor_cursor(EditorMode::Place));
        assert!(should_draw_editor_cursor(EditorMode::Tapping));
        assert!(!should_draw_editor_cursor(EditorMode::Select));
        assert!(!should_draw_editor_cursor(EditorMode::Trigger));
        assert!(!should_draw_editor_cursor(EditorMode::Timing));
    }

    #[test]
    fn editor_grid_darkening_scales_with_bright_editor_skies() {
        approx_eq(
            editor_grid_darkening(AppPhase::Editor, [0.0, 0.0, 0.0]),
            0.10,
            1e-6,
        );
        approx_eq(
            editor_grid_darkening(AppPhase::Editor, [1.0, 1.0, 1.0]),
            0.45,
            1e-6,
        );
        assert!(editor_grid_darkening(AppPhase::Editor, [0.55, 0.75, 0.84]) > 0.40);
        approx_eq(
            editor_grid_darkening(AppPhase::Playing, [1.0, 1.0, 1.0]),
            0.0,
            1e-6,
        );
    }

    #[test]
    fn current_surface_statuses_match_render_policy() {
        assert!(matches!(
            current_surface_output(wgpu::CurrentSurfaceTexture::Timeout),
            Ok(None)
        ));
        assert!(matches!(
            current_surface_output(wgpu::CurrentSurfaceTexture::Occluded),
            Ok(None)
        ));
        assert!(matches!(
            current_surface_output(wgpu::CurrentSurfaceTexture::Outdated),
            Err(RenderSurfaceError::Outdated)
        ));
        assert!(matches!(
            current_surface_output(wgpu::CurrentSurfaceTexture::Lost),
            Err(RenderSurfaceError::Lost)
        ));
        assert!(matches!(
            current_surface_output(wgpu::CurrentSurfaceTexture::Validation),
            Err(RenderSurfaceError::Validation)
        ));
    }

    #[test]
    fn render_paths_are_safe_when_surface_is_absent() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            assert!(state.render().is_ok());

            let overlay_called = Cell::new(false);
            let render_result = state.render_with_overlay(|_, _, _, _| {
                overlay_called.set(true);
            });

            assert!(render_result.is_ok());
            assert!(
                !overlay_called.get(),
                "overlay should not run when there is no surface"
            );

            let before = (state.surface_width(), state.surface_height());
            state.recreate_surface();
            let after = (state.surface_width(), state.surface_height());
            assert_eq!(before, after);
        });
    }

    #[test]
    fn render_egui_returns_ok_when_surface_is_absent() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let mut renderer = state.create_egui_renderer();
            let paint_jobs: Vec<egui::ClippedPrimitive> = Vec::new();
            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [state.surface_width(), state.surface_height()],
                pixels_per_point: 1.0,
            };

            let result = state.render_egui(&mut renderer, &paint_jobs, &screen_descriptor);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn selection_outline_ignores_unselected_scene_depth() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            prepare_outline_occlusion_test_state(&mut state);

            let red_front = fullscreen_triangle(0.1, [1.0, 0.0, 0.0, 1.0]);
            let cyan_outline = fullscreen_triangle(0.2, [0.0, 1.0, 1.0, 1.0]);
            let corner_mask = corner_triangle(0.2, [1.0, 1.0, 1.0, 1.0]);

            state.render.meshes.blocks_static =
                MeshSlot::from_vertices(state.device(), "Unselected Front Block", &red_front);
            state.render.meshes.editor_selection_stencil =
                MeshSlot::from_vertices(state.device(), "Selection Mask", &corner_mask);
            state.render.meshes.editor_selection_outline =
                MeshSlot::from_vertices(state.device(), "Selection Outline", &cyan_outline);
            state.render.meshes.editor_selection_outline_instances = vec![EditorOutlineInstance {
                mask_vertices: 0..corner_mask.len() as u32,
                outline_vertices: 0..cyan_outline.len() as u32,
            }];

            let pixel = render_center_pixel(&mut state);
            assert!(
                pixel[0] < 80 && pixel[1] > 180 && pixel[2] > 180,
                "selection outline should draw over unselected scene depth; got rgba={pixel:?}"
            );
        });
    }

    #[test]
    fn hover_outline_ignores_unselected_scene_depth() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            prepare_outline_occlusion_test_state(&mut state);

            let red_front = fullscreen_triangle(0.1, [1.0, 0.0, 0.0, 1.0]);
            let cyan_outline = fullscreen_triangle(0.2, [0.0, 1.0, 1.0, 1.0]);
            let corner_mask = corner_triangle(0.2, [1.0, 1.0, 1.0, 1.0]);

            state.render.meshes.blocks_static =
                MeshSlot::from_vertices(state.device(), "Unselected Front Block", &red_front);
            state.render.meshes.editor_hover_stencil =
                MeshSlot::from_vertices(state.device(), "Hover Mask", &corner_mask);
            state.render.meshes.editor_hover_outline =
                MeshSlot::from_vertices(state.device(), "Hover Outline", &cyan_outline);

            let pixel = render_center_pixel(&mut state);
            assert!(
                pixel[0] < 80 && pixel[1] > 180 && pixel[2] > 180,
                "hover outline should draw over unselected scene depth; got rgba={pixel:?}"
            );
        });
    }

    #[test]
    fn hover_outline_draws_over_selected_blocks() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            prepare_outline_occlusion_test_state(&mut state);

            let red_selected = fullscreen_triangle(0.1, [1.0, 0.0, 0.0, 1.0]);
            let cyan_outline = fullscreen_triangle(0.2, [0.0, 1.0, 1.0, 1.0]);
            let selected_mask = fullscreen_triangle(0.1, [1.0, 1.0, 1.0, 1.0]);
            let hover_mask = corner_triangle(0.2, [1.0, 1.0, 1.0, 1.0]);

            state.render.meshes.blocks_selected =
                MeshSlot::from_vertices(state.device(), "Selected Front Block", &red_selected);
            state.render.meshes.editor_selection_stencil =
                MeshSlot::from_vertices(state.device(), "Selected Occlusion Mask", &selected_mask);
            state.render.meshes.editor_hover_stencil =
                MeshSlot::from_vertices(state.device(), "Hover Mask", &hover_mask);
            state.render.meshes.editor_hover_outline =
                MeshSlot::from_vertices(state.device(), "Hover Outline", &cyan_outline);

            let pixel = render_center_pixel(&mut state);
            assert!(
                pixel[0] < 80 && pixel[1] > 180 && pixel[2] > 180,
                "hover outline should draw over selected block depth; got rgba={pixel:?}"
            );
        });
    }

    #[test]
    fn test_render_to_view_exercises_full_pipeline() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let size = 64;
            state.resize_surface(crate::types::PhysicalSize {
                width: size,
                height: size,
            });

            let texture = state.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("Test Render Texture"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: state.render.gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                state
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Test Encoder"),
                    });

            // 1. Test Editor Phase + Place Mode
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Place;

            // Populate some meshes
            state.editor.objects.push(crate::types::LevelObject {
                block_id: "core/stone".to_string(),
                ..Default::default()
            });
            state.editor.ui.selected_block_indices = vec![0];
            state.sync_editor_objects();
            state.rebuild_editor_cursor_vertices();
            state.rebuild_spawn_marker_vertices();

            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 2. Test Playing Phase + GameOver
            state.phase = AppPhase::Playing;
            state.gameplay.state.game_over = true;
            state.editor.timeline.playback.playing = true; // For trail
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 3. Test Timing Mode (skip world)
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 4. Test with gamma correction enabled
            state.render.gpu.apply_gamma_correction = true;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            state.queue().submit(std::iter::once(encoder.finish()));
        });
    }

    #[test]
    fn render_to_view_draws_populated_mesh_slots_across_phases() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let size = 96;
            state.resize_surface(crate::types::PhysicalSize {
                width: size,
                height: size,
            });

            let texture = state.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("Branch Coverage Render Texture"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: state.render.gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                state
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Branch Coverage Encoder"),
                    });

            let tri = vec![
                Vertex::untextured([0.0, 0.0, 0.0], [1.0, 0.2, 0.2, 1.0]),
                Vertex::untextured([0.2, 0.0, 0.0], [0.2, 1.0, 0.2, 1.0]),
                Vertex::untextured([0.0, 0.2, 0.0], [0.2, 0.2, 1.0, 1.0]),
            ];

            state.render.meshes.floor = MeshSlot::from_vertices(state.device(), "floor", &tri);
            state.render.meshes.grid = MeshSlot::from_vertices(state.device(), "grid", &tri);
            state.render.meshes.trail = MeshSlot::from_vertices(state.device(), "trail", &tri);
            state.render.meshes.blocks = MeshSlot::from_vertices(state.device(), "blocks", &tri);
            state.render.meshes.blocks_static =
                MeshSlot::from_vertices(state.device(), "blocks_static", &tri);
            state.render.meshes.blocks_selected =
                MeshSlot::from_vertices(state.device(), "blocks_selected", &tri);
            state.render.meshes.editor_cursor =
                MeshSlot::from_vertices(state.device(), "cursor", &tri);
            state.render.meshes.editor_hover_stencil =
                MeshSlot::from_vertices(state.device(), "hover_stencil", &tri);
            state.render.meshes.editor_hover_outline =
                MeshSlot::from_vertices(state.device(), "hover", &tri);
            state.render.meshes.editor_selection_stencil =
                MeshSlot::from_vertices(state.device(), "selection_stencil", &tri);
            state.render.meshes.editor_selection_outline =
                MeshSlot::from_vertices(state.device(), "selection", &tri);
            state.render.meshes.editor_gizmo =
                MeshSlot::from_vertices(state.device(), "gizmo", &tri);
            state.render.meshes.tap_indicators =
                MeshSlot::from_vertices(state.device(), "taps", &tri);
            state.render.meshes.spawn_marker =
                MeshSlot::from_vertices(state.device(), "spawn", &tri);
            state.render.meshes.camera_trigger_markers =
                MeshSlot::from_vertices(state.device(), "camera_markers", &tri);

            let overlay_calls = Cell::new(0_u32);

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Place;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Playing;
            state.gameplay.state.game_over = false;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::GameOver;
            state.gameplay.state.game_over = true;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Menu;
            state.editor.ui.mode = EditorMode::Select;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            assert_eq!(overlay_calls.get(), 5);
            state.queue().submit(std::iter::once(encoder.finish()));
        });
    }
}
