/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::iter;

use eg_mesh::ChunkAabb;
use eg_render::{
    ChunkedBlockMeshSlot, RenderFrameInput, RenderSurfaceError, current_surface_output,
    draw_instanced_blocks, draw_mesh,
};
use eg_types::AppPhase;
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use glam::{Mat4, Vec3, Vec4};
use wgpu::TextureViewDescriptor;

const GRID_HALF_EXTENT: f32 = 2048.0;

#[derive(Clone, Copy)]
struct FrustumPlane {
    normal: Vec3,
    distance: f32,
}

#[derive(Clone, Copy)]
struct ViewFrustum {
    planes: [FrustumPlane; 6],
}

impl ViewFrustum {
    fn from_view_projection(view_projection: Mat4) -> Self {
        let row = |index: usize| -> Vec4 {
            Vec4::new(
                view_projection.x_axis[index],
                view_projection.y_axis[index],
                view_projection.z_axis[index],
                view_projection.w_axis[index],
            )
        };

        let row_x = row(0);
        let row_y = row(1);
        let row_z = row(2);
        let row_w = row(3);
        Self {
            planes: [
                FrustumPlane::from_vec4(row_w + row_x),
                FrustumPlane::from_vec4(row_w - row_x),
                FrustumPlane::from_vec4(row_w + row_y),
                FrustumPlane::from_vec4(row_w - row_y),
                FrustumPlane::from_vec4(row_w + row_z),
                FrustumPlane::from_vec4(row_w - row_z),
            ],
        }
    }

    fn intersects_aabb(self, aabb: ChunkAabb) -> bool {
        self.planes.iter().all(|plane| plane.intersects_aabb(aabb))
    }
}

impl FrustumPlane {
    fn from_vec4(plane: Vec4) -> Self {
        let normal = plane.truncate();
        let length = normal.length();
        if length <= f32::EPSILON {
            return Self {
                normal,
                distance: plane.w,
            };
        }
        Self {
            normal: normal / length,
            distance: plane.w / length,
        }
    }

    fn intersects_aabb(self, aabb: ChunkAabb) -> bool {
        let positive = Vec3::new(
            if self.normal.x >= 0.0 {
                aabb.max[0]
            } else {
                aabb.min[0]
            },
            if self.normal.y >= 0.0 {
                aabb.max[1]
            } else {
                aabb.min[1]
            },
            if self.normal.z >= 0.0 {
                aabb.max[2]
            } else {
                aabb.min[2]
            },
        );
        self.normal.dot(positive) + self.distance >= 0.0
    }
}

fn draw_visible_chunked_geometry(
    render_pass: &mut wgpu::RenderPass<'_>,
    slot: &ChunkedBlockMeshSlot,
    frustum: ViewFrustum,
) {
    for chunk in slot.chunks() {
        if !frustum.intersects_aabb(chunk.aabb) {
            continue;
        }
        if let Some(draw_data) = chunk.greedy.draw_data() {
            draw_mesh(render_pass, draw_data);
        }
        if let Some(draw_data) = chunk.fallback.draw_data() {
            draw_mesh(render_pass, draw_data);
        }
    }
}

fn draw_visible_chunked_instances(
    render_pass: &mut wgpu::RenderPass<'_>,
    slot: &ChunkedBlockMeshSlot,
    frustum: ViewFrustum,
) {
    for chunk in slot.chunks() {
        if frustum.intersects_aabb(chunk.aabb) {
            draw_instanced_blocks(render_pass, &chunk.instanced);
        }
    }
}

fn draw_visible_retained_instances(
    render_pass: &mut wgpu::RenderPass<'_>,
    slot: &eg_render::InstancedMeshSlot,
    frustum: ViewFrustum,
) {
    for batch in slot.draw_batches() {
        if batch
            .aabb
            .is_some_and(|aabb| !frustum.intersects_aabb(aabb))
        {
            continue;
        }
        render_pass.set_vertex_buffer(0, batch.template_buffer.slice(..));
        render_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
        render_pass.draw(0..batch.template_vertex_count, 0..batch.instance_count);
    }
}

struct RenderContext<'a> {
    gpu: &'a mut eg_render::GpuContext,
    meshes: &'a mut eg_render::SceneMeshes,
}

impl RenderContext<'_> {
    fn render_gpu(&self) -> &eg_render::GpuContext {
        self.gpu
    }

    fn render_meshes(&self) -> &eg_render::SceneMeshes {
        self.meshes
    }
}

/// Present the scene and egui using the App-owned render resources.
pub fn render_egui_with_resources(
    gpu: &mut eg_render::GpuContext,
    meshes: &mut eg_render::SceneMeshes,
    renderer: &mut EguiRenderer,
    paint_jobs: &[egui::ClippedPrimitive],
    screen_descriptor: &ScreenDescriptor,
    frame_input: RenderFrameInput,
    camera_view_projection: [[f32; 4]; 4],
) -> Result<(), RenderSurfaceError> {
    let mut context = RenderContext { gpu, meshes };
    context.render_egui_with_frame_input(
        renderer,
        paint_jobs,
        screen_descriptor,
        frame_input,
        camera_view_projection,
    )
}

impl RenderContext<'_> {
    fn render_egui_with_frame_input(
        &mut self,
        renderer: &mut EguiRenderer,
        paint_jobs: &[egui::ClippedPrimitive],
        screen_descriptor: &ScreenDescriptor,
        frame_input: RenderFrameInput,
        camera_view_projection: [[f32; 4]; 4],
    ) -> Result<(), RenderSurfaceError> {
        puffin::profile_scope!("RenderEgui");
        self.render_with_overlay_and_frame_input(
            frame_input,
            camera_view_projection,
            |device, queue, view, encoder| {
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
            },
        )
    }

    fn render_with_overlay_and_frame_input<F>(
        &mut self,
        frame_input: RenderFrameInput,
        camera_view_projection: [[f32; 4]; 4],
        overlay: F,
    ) -> Result<(), RenderSurfaceError>
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        puffin::profile_scope!("RenderWithOverlay");
        let surface = match &self.render_gpu().surface {
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
            self.render_gpu()
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            puffin::profile_scope!("RenderToView");
            self.render_to_view_with_frame_input(
                &view,
                &mut encoder,
                frame_input,
                camera_view_projection,
                overlay,
            );
        }

        {
            puffin::profile_scope!("GpuSubmit");
            self.render_gpu().queue.submit(iter::once(encoder.finish()));
        }
        {
            puffin::profile_scope!("SurfacePresent");
            output.present();
        }
        Ok(())
    }

    fn render_to_view_with_frame_input<F>(
        &mut self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        frame_input: RenderFrameInput,
        camera_view_projection: [[f32; 4]; 4],
        overlay: F,
    ) where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        puffin::profile_scope!("RenderWorld");
        let phase = frame_input.phase;
        let skip_world = frame_input.should_skip_world();
        let draw_editor_overlays = frame_input.should_draw_editor_overlays();
        let clear_color = frame_input.clear_color();
        let grid_uniform = frame_input.grid_uniform(GRID_HALF_EXTENT);
        let block_frustum =
            ViewFrustum::from_view_projection(Mat4::from_cols_array_2d(&camera_view_projection));
        self.render_gpu().queue.write_buffer(
            &self.render_gpu().grid_uniform_buffer,
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
                view: &self.render_gpu().depth_view,
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

        render_pass.set_pipeline(&self.render_gpu().render_pipeline);
        render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
        render_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
        render_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);

        if frame_input.should_draw_floor_and_grid() {
            puffin::profile_scope!("DrawFloorGrid");
            if let Some(draw_data) = self.render_meshes().floor.draw_data() {
                draw_mesh(&mut render_pass, draw_data);
            }

            if let Some(draw_data) = self.render_meshes().grid.draw_data() {
                render_pass.set_pipeline(&self.render_gpu().grid_pipeline);
                render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.render_gpu().grid_bind_group, &[]);
                render_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                draw_mesh(&mut render_pass, draw_data);
                render_pass.set_pipeline(&self.render_gpu().render_pipeline);
                render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                render_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                render_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
            }
        }

        if phase == AppPhase::Playing
            || phase == AppPhase::GameOver
            || phase == AppPhase::Editor
            || phase == AppPhase::Menu
        {
            if !skip_world {
                puffin::profile_scope!("DrawBlocksTrail");
                if phase == AppPhase::Editor {
                    render_pass.set_pipeline(&self.render_gpu().instanced_block_pipeline);
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_visible_retained_instances(
                        &mut render_pass,
                        &self.render_meshes().blocks_static_instanced,
                        block_frustum,
                    );
                    render_pass.set_pipeline(&self.render_gpu().render_pipeline);
                    render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    render_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                    render_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
                } else {
                    render_pass.set_pipeline(&self.render_gpu().instanced_block_pipeline);
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_visible_chunked_instances(
                        &mut render_pass,
                        &self.render_meshes().blocks_chunked,
                        block_frustum,
                    );
                    draw_instanced_blocks(&mut render_pass, &self.render_meshes().blocks_instanced);
                    render_pass.set_pipeline(&self.render_gpu().render_pipeline);
                    render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    render_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                    render_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);

                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_visible_chunked_geometry(
                        &mut render_pass,
                        &self.render_meshes().blocks_chunked,
                        block_frustum,
                    );
                }

                if let Some(draw_data) = self.render_meshes().gem_shatter_effects.draw_data() {
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render_meshes().practice_checkpoints.draw_data() {
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render_meshes().trail.draw_data() {
                    if phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render_gpu().editor_ghost_trail_pipeline);
                        render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                        render_pass.set_bind_group(
                            2,
                            &self.render_gpu().color_space_bind_group,
                            &[],
                        );
                        render_pass.set_bind_group(
                            3,
                            &self.render_gpu().block_texture_bind_group,
                            &[],
                        );
                    }

                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);

                    if phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render_gpu().render_pipeline);
                        render_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                        render_pass.set_bind_group(
                            2,
                            &self.render_gpu().color_space_bind_group,
                            &[],
                        );
                        render_pass.set_bind_group(
                            3,
                            &self.render_gpu().block_texture_bind_group,
                            &[],
                        );
                    }
                }
            }

            if draw_editor_overlays {
                puffin::profile_scope!("DrawEditorOverlayMeshes");
                if let Some(draw_data) = self.render_meshes().spawn_marker.draw_data() {
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }

                if let Some(draw_data) = self.render_meshes().tap_indicators.draw_data() {
                    render_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                    draw_mesh(&mut render_pass, draw_data);
                }
            }
        }

        drop(render_pass);

        if draw_editor_overlays {
            puffin::profile_scope!("DrawEditorOverlays");

            if !self.render_meshes().transform_trigger_markers.is_empty() {
                puffin::profile_scope!("DrawTransformTriggerMarkers");
                let mut transform_marker_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Editor Transform Trigger Marker Pass"),
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
                            view: &self.render_gpu().editor_outline_occlusion_depth_view,
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
                transform_marker_pass.set_pipeline(&self.render_gpu().render_pipeline);
                transform_marker_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                transform_marker_pass.set_bind_group(
                    1,
                    &self.render_gpu().zero_line_bind_group,
                    &[],
                );
                transform_marker_pass.set_bind_group(
                    2,
                    &self.render_gpu().color_space_bind_group,
                    &[],
                );
                transform_marker_pass.set_bind_group(
                    3,
                    &self.render_gpu().block_texture_bind_group,
                    &[],
                );
                for slot in self
                    .render_meshes()
                    .transform_trigger_markers
                    .draw_batches()
                {
                    if let Some(draw_data) = slot.draw_data() {
                        draw_mesh(&mut transform_marker_pass, draw_data);
                    }
                }
            }

            // Selection outlines use a selected-only depth prepass, so each block keeps its own
            // hollow silhouette while other selected blocks occlude outlines behind them.
            let selection_instances = self
                .render_meshes()
                .editor_selection_outline_instances
                .as_slice();

            if !selection_instances.is_empty() {
                puffin::profile_scope!("DrawSelectionOutlines");
                let selection_mask_buffer = self
                    .render_meshes()
                    .editor_selection_stencil
                    .vertex_buffer();
                let selection_outline_buffer = self
                    .render_meshes()
                    .editor_selection_outline
                    .vertex_buffer();

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
                                        view: &self
                                            .render_gpu()
                                            .editor_outline_occlusion_depth_view,
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

                        depth_pass.set_pipeline(
                            &self.render_gpu().editor_outline_occlusion_depth_pipeline,
                        );
                        depth_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                        depth_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                        depth_pass.set_bind_group(
                            2,
                            &self.render_gpu().color_space_bind_group,
                            &[],
                        );
                        depth_pass.set_bind_group(
                            3,
                            &self.render_gpu().block_texture_bind_group,
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
                                        view: &self
                                            .render_gpu()
                                            .editor_outline_occlusion_depth_view,
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

                        outline_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                        outline_pass.set_bind_group(
                            1,
                            &self.render_gpu().zero_line_bind_group,
                            &[],
                        );
                        outline_pass.set_bind_group(
                            2,
                            &self.render_gpu().color_space_bind_group,
                            &[],
                        );
                        outline_pass.set_bind_group(
                            3,
                            &self.render_gpu().block_texture_bind_group,
                            &[],
                        );

                        for (batch_index, instance) in batch.iter().enumerate() {
                            let stencil_ref = (batch_index as u32) + 1;
                            outline_pass.set_stencil_reference(stencil_ref);

                            outline_pass
                                .set_pipeline(&self.render_gpu().editor_outline_mask_pipeline);
                            outline_pass.set_vertex_buffer(0, selection_mask_buffer.slice(..));
                            outline_pass.draw(instance.mask_vertices.clone(), 0..1);

                            outline_pass.set_pipeline(&self.render_gpu().editor_outline_pipeline);
                            outline_pass.set_vertex_buffer(0, selection_outline_buffer.slice(..));
                            outline_pass.draw(instance.outline_vertices.clone(), 0..1);
                        }
                    }
                }
            }

            if let (Some(mask_data), Some(outline_data)) = (
                self.render_meshes().editor_hover_stencil.draw_data(),
                self.render_meshes().editor_hover_outline.draw_data(),
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
                        view: &self.render_gpu().editor_outline_occlusion_depth_view,
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
                hover_pass.set_pipeline(&self.render_gpu().editor_outline_mask_pipeline);
                hover_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                hover_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                hover_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                hover_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
                draw_mesh(&mut hover_pass, mask_data);

                hover_pass.set_pipeline(&self.render_gpu().editor_hover_outline_pipeline);
                hover_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                hover_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                hover_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                hover_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
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
                    view: &self.render_gpu().depth_view,
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
            gizmo_pass.set_pipeline(&self.render_gpu().render_pipeline);
            gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
            gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
            gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
            gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);

            if let Some(draw_data) = self.render_meshes().editor_gizmo.draw_data() {
                gizmo_pass.set_pipeline(&self.render_gpu().gizmo_overlay_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);

                gizmo_pass.set_pipeline(&self.render_gpu().render_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
            }

            if let Some(draw_data) = self.render_meshes().editor_hitbox_visualization.draw_data() {
                gizmo_pass.set_pipeline(&self.render_gpu().gizmo_overlay_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);

                gizmo_pass.set_pipeline(&self.render_gpu().render_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
            }

            if frame_input.should_draw_editor_cursor()
                && let Some(draw_data) = self.render_meshes().editor_cursor.draw_data()
            {
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);
            }

            if let Some(draw_data) = self.render_meshes().camera_arrow.draw_data() {
                gizmo_pass.set_pipeline(&self.render_gpu().gizmo_overlay_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
                draw_mesh(&mut gizmo_pass, draw_data);

                gizmo_pass.set_pipeline(&self.render_gpu().render_pipeline);
                gizmo_pass.set_bind_group(0, &self.render_gpu().camera_bind_group, &[]);
                gizmo_pass.set_bind_group(1, &self.render_gpu().zero_line_bind_group, &[]);
                gizmo_pass.set_bind_group(2, &self.render_gpu().color_space_bind_group, &[]);
                gizmo_pass.set_bind_group(3, &self.render_gpu().block_texture_bind_group, &[]);
            }
        }

        {
            puffin::profile_scope!("RenderOverlayClosure");
            overlay(
                &self.render_gpu().device,
                &self.render_gpu().queue,
                view,
                encoder,
            );
        }
    }
}

#[cfg(test)]
mod tests {

    use eg_mesh::ChunkAabb;
    use eg_render::{
        apply_gamma_correction_if_enabled, clear_color_for_phase, editor_grid_darkening,
        linear_to_srgb, should_draw_editor_cursor, should_draw_editor_overlays,
        should_draw_floor_and_grid, should_skip_world,
    };
    use eg_types::{AppPhase, EditorMode, LevelMetadata};
    use glam::Mat4;
    use wgpu::Color;

    use super::{RenderSurfaceError, ViewFrustum, current_surface_output};

    fn assert_color_approx(actual: Color, expected: Color) {
        approx::assert_abs_diff_eq!(actual.r as f32, expected.r as f32, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(actual.g as f32, expected.g as f32, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(actual.b as f32, expected.b as f32, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(actual.a as f32, expected.a as f32, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn frustum_intersects_aabb_inside_clip_space() {
        let frustum = ViewFrustum::from_view_projection(Mat4::IDENTITY);
        let aabb = ChunkAabb::from_min_max([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]);

        assert!(frustum.intersects_aabb(aabb));
    }

    #[tokio::test]
    async fn frustum_rejects_aabb_outside_clip_space() {
        let frustum = ViewFrustum::from_view_projection(Mat4::IDENTITY);
        let aabb = ChunkAabb::from_min_max([2.0, -0.5, -0.5], [3.0, 0.5, 0.5]);

        assert!(!frustum.intersects_aabb(aabb));
    }

    #[tokio::test]
    async fn linear_to_srgb_uses_linear_segment_at_or_below_threshold() {
        let below = 0.002;
        let at_threshold = 0.0031308;

        approx::assert_abs_diff_eq!(linear_to_srgb(below), below * 12.92, epsilon = 1e-7);
        approx::assert_abs_diff_eq!(
            linear_to_srgb(at_threshold),
            at_threshold * 12.92,
            epsilon = 1e-7
        );
    }

    #[tokio::test]
    async fn linear_to_srgb_uses_gamma_segment_above_threshold() {
        let mid_gray_linear = 0.18;
        let expected = 0.461_356_1;

        approx::assert_abs_diff_eq!(linear_to_srgb(mid_gray_linear), expected, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn clear_color_for_phase_matches_expected_palette() {
        assert_color_approx(
            clear_color_for_phase(AppPhase::Playing, true, LevelMetadata::default().sky_color),
            Color {
                r: 0.15,
                g: 0.05,
                b: 0.05,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Editor, false, LevelMetadata::default().sky_color),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Playing, false, LevelMetadata::default().sky_color),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(AppPhase::Menu, false, LevelMetadata::default().sky_color),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            },
        );

        assert_color_approx(
            clear_color_for_phase(
                AppPhase::GameOver,
                false,
                LevelMetadata::default().sky_color,
            ),
            Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            },
        );
    }

    #[tokio::test]
    async fn clear_color_for_phase_uses_custom_sky_for_menu_editor_and_playing() {
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

    #[tokio::test]
    async fn apply_gamma_correction_if_enabled_only_changes_rgb() {
        let linear = Color {
            r: 0.18,
            g: 0.04,
            b: 0.01,
            a: 0.5,
        };

        let unchanged = apply_gamma_correction_if_enabled(linear, false);
        assert_eq!(unchanged, linear);

        let corrected = apply_gamma_correction_if_enabled(linear, true);
        approx::assert_abs_diff_eq!(
            corrected.r as f32,
            linear_to_srgb(linear.r as f32),
            epsilon = 1e-7
        );
        approx::assert_abs_diff_eq!(
            corrected.g as f32,
            linear_to_srgb(linear.g as f32),
            epsilon = 1e-7
        );
        approx::assert_abs_diff_eq!(
            corrected.b as f32,
            linear_to_srgb(linear.b as f32),
            epsilon = 1e-7
        );
        approx::assert_abs_diff_eq!(corrected.a as f32, linear.a as f32, epsilon = 1e-7);
    }

    #[tokio::test]
    async fn render_gate_helpers_match_phase_and_mode_rules() {
        assert!(!should_draw_floor_and_grid(
            AppPhase::Menu,
            EditorMode::Place
        ));
        assert!(!should_draw_floor_and_grid(
            AppPhase::Editor,
            EditorMode::Timing
        ));
        assert!(!should_draw_floor_and_grid(
            AppPhase::Playing,
            EditorMode::Place
        ));
        assert!(!should_draw_floor_and_grid(
            AppPhase::GameOver,
            EditorMode::Place
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
        assert!(!should_draw_editor_cursor(EditorMode::Timing));
    }

    #[tokio::test]
    async fn editor_grid_darkening_scales_with_bright_editor_skies() {
        approx::assert_abs_diff_eq!(
            editor_grid_darkening(AppPhase::Editor, [0.0, 0.0, 0.0]),
            0.10,
            epsilon = 1e-6,
        );
        approx::assert_abs_diff_eq!(
            editor_grid_darkening(AppPhase::Editor, [1.0, 1.0, 1.0]),
            0.45,
            epsilon = 1e-6,
        );
        assert!(editor_grid_darkening(AppPhase::Editor, [0.55, 0.75, 0.84]) > 0.40);
        approx::assert_abs_diff_eq!(
            editor_grid_darkening(AppPhase::Playing, [1.0, 1.0, 1.0]),
            0.0,
            epsilon = 1e-6,
        );
    }

    #[tokio::test]
    async fn current_surface_statuses_match_render_policy() {
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
}
