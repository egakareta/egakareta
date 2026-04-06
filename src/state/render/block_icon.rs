/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::mesh::build_block_vertices;
use crate::types::{CameraUniform, LevelObject};

use super::super::State;

const ICON_FOV_DEGREES: f32 = 35.0;

impl State {
    /// Renders a single block to an offscreen texture for use as an editor icon.
    pub(crate) fn render_block_icon_snapshot(
        &self,
        block_id: &str,
        size: u32,
    ) -> Option<wgpu::Texture> {
        let safe_size = size.max(1);
        let block = LevelObject {
            block_id: block_id.to_string(),
            ..LevelObject::default()
        };
        let vertices = build_block_vertices(std::slice::from_ref(&block));
        if vertices.is_empty() {
            return None;
        }

        let vertex_buffer =
            self.render
                .gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Icon Vertex Buffer"),
                    contents: bytemuck::cast_slice(vertices.as_slice()),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let color_texture = self
            .render
            .gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Block Icon Color Texture"),
                size: wgpu::Extent3d {
                    width: safe_size,
                    height: safe_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = self
            .render
            .gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Block Icon Depth Texture"),
                size: wgpu::Extent3d {
                    width: safe_size,
                    height: safe_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: super::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let eye = Vec3::new(2.0, 2.0, 2.0);
        let target = Vec3::new(0.5, 0.5, 0.5);
        let up = Vec3::Y;
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(ICON_FOV_DEGREES.to_radians(), 1.0, 0.1, 100.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };
        let camera_uniform_buffer =
            self.render
                .gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Icon Camera Uniform Buffer"),
                    contents: bytemuck::bytes_of(&camera_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let camera_bind_group =
            self.render
                .gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Block Icon Camera Bind Group"),
                    layout: &self.render.gpu.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_uniform_buffer.as_entire_binding(),
                    }],
                });

        let mut encoder =
            self.render
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Block Icon Render Encoder"),
                });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Block Icon Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render.gpu.block_icon_pipeline);
            pass.set_bind_group(0, &camera_bind_group, &[]);
            pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
            pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
            pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.render
            .gpu
            .queue
            .submit(std::iter::once(encoder.finish()));
        Some(color_texture)
    }
}
