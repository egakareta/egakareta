/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{EulerRot, Mat3, Mat4, Vec3};
use wgpu::util::DeviceExt;

use eg_block_geometry::visual_cuboids;
use eg_blocks::{resolve_block_definition, resolve_block_texture_layers};

use eg_mesh::shapes::{append_prism_with_layers, PrismFaceColors, PrismTextureLayers};
use eg_mesh::{
    append_camera_trigger_visual_vertices, append_transform_trigger_visual_vertices,
    build_block_geometry, CameraTriggerVisualStyle, MeshGeometry, TransformTriggerVisualStyle,
};
use eg_types::{
    BlockIconCamera, BlockRenderProfile, CameraUniform, ColorSpaceUniform, LevelObject,
    CAMERA_TRIGGER_BLOCK_ID, TRANSFORM_TRIGGER_BLOCK_ID,
};

use crate::{GpuContext, DEPTH_FORMAT};

const ICON_DIMETRIC_YAW_DEGREES: f32 = 45.0;
const ICON_DIMETRIC_PITCH_DEGREES: f32 = 30.0;
const ICON_CAMERA_DISTANCE: f32 = 4.0;
const ICON_ORTHO_HALF_EXTENT: f32 = 0.9;
const ICON_PERSPECTIVE_FOV_DEGREES: f32 = 35.0;
const ICON_ORTHO_NEAR_PLANE: f32 = 0.1;
const ICON_ORTHO_FAR_PLANE: f32 = 16.0;

struct IconCamera {
    eye: Vec3,
    target: Vec3,
    up: Vec3,
    projection: Mat4,
}

fn uses_dimetric_icon_projection(block_id: &str) -> bool {
    resolve_block_definition(block_id)
        .render
        .icon_dimetric_projection
}

fn configured_icon_camera(block_id: &str) -> BlockIconCamera {
    resolve_block_definition(block_id)
        .render
        .icon_camera
        .clone()
}

fn default_icon_camera(dimetric: bool) -> IconCamera {
    let target = Vec3::new(0.5, 0.5, 0.5);
    let up = Vec3::Y;
    if dimetric {
        let yaw = ICON_DIMETRIC_YAW_DEGREES.to_radians();
        let pitch = ICON_DIMETRIC_PITCH_DEGREES.to_radians();
        let camera_direction = Vec3::new(
            pitch.cos() * yaw.cos(),
            pitch.sin(),
            pitch.cos() * yaw.sin(),
        )
        .normalize();
        return IconCamera {
            eye: target + (camera_direction * ICON_CAMERA_DISTANCE),
            target,
            up,
            projection: orthographic_projection(ICON_ORTHO_HALF_EXTENT),
        };
    }

    IconCamera {
        eye: Vec3::new(2.0, 2.0, 2.0),
        target,
        up,
        projection: Mat4::perspective_rh_gl(
            ICON_PERSPECTIVE_FOV_DEGREES.to_radians(),
            1.0,
            0.1,
            100.0,
        ),
    }
}

fn resolve_icon_camera(block_id: &str, dimetric: bool) -> IconCamera {
    let mut camera = default_icon_camera(dimetric);
    let config = configured_icon_camera(block_id);

    if let Some(position) = finite_vec3(config.position) {
        camera.eye = position;
    }

    if let Some(rotation) = finite_vec3(config.rotation) {
        let rotation = Mat3::from_euler(
            EulerRot::XYZ,
            rotation.x.to_radians(),
            rotation.y.to_radians(),
            rotation.z.to_radians(),
        );
        let forward = (rotation * Vec3::NEG_Z).normalize();
        camera.target = camera.eye + forward;
        camera.up = (rotation * Vec3::Y).normalize();
    }

    if let Some(half_extent) = config
        .orthographic_half_extent
        .filter(|half_extent| half_extent.is_finite() && *half_extent > 0.0)
    {
        camera.projection = orthographic_projection(half_extent);
    }

    camera
}

fn finite_vec3(value: Option<[f32; 3]>) -> Option<Vec3> {
    value
        .filter(|components| components.iter().all(|component| component.is_finite()))
        .map(Vec3::from)
}

fn orthographic_projection(half_extent: f32) -> Mat4 {
    Mat4::orthographic_rh(
        -half_extent,
        half_extent,
        -half_extent,
        half_extent,
        ICON_ORTHO_NEAR_PLANE,
        ICON_ORTHO_FAR_PLANE,
    )
}

fn build_block_icon_geometry(block_id: &str, dimetric: bool) -> MeshGeometry {
    const LIQUID_PROFILE_TAG: f32 = 1.0;

    if !dimetric {
        let block = LevelObject {
            block_id: block_id.to_string(),
            ..LevelObject::default()
        };
        return build_block_geometry(std::slice::from_ref(&block));
    }

    let block = resolve_block_definition(block_id);
    if block.id == CAMERA_TRIGGER_BLOCK_ID {
        return build_camera_trigger_icon_geometry();
    }

    if block.id == TRANSFORM_TRIGGER_BLOCK_ID {
        return build_transform_trigger_icon_geometry();
    }

    let layers = resolve_block_texture_layers(block_id);
    let colors = PrismFaceColors::new_with_outline(
        block.render.color_top,
        block.render.color_side,
        block.render.color_bottom,
        block.render.color_outline,
    );

    let icon_object = LevelObject {
        block_id: block_id.to_string(),
        ..LevelObject::default()
    };
    let cuboids = visual_cuboids(&icon_object);
    let mut vertices = Vec::with_capacity(cuboids.len().max(1) * 36);
    if cuboids.is_empty() {
        append_prism_with_layers(
            &mut vertices,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            colors,
            PrismTextureLayers::new(layers.top, layers.side, layers.bottom),
        );
    } else {
        for cuboid in cuboids {
            let cuboid_colors = cuboid
                .color_tint
                .map(|color_tint| colors.tinted(color_tint))
                .unwrap_or(colors);
            append_prism_with_layers(
                &mut vertices,
                cuboid.min,
                cuboid.max,
                cuboid_colors,
                PrismTextureLayers::new(layers.top, layers.side, layers.bottom),
            );
        }
    }
    if matches!(block.render.profile, BlockRenderProfile::Liquid) {
        for vertex in &mut vertices {
            vertex.set_render_profile(LIQUID_PROFILE_TAG);
        }
    }

    MeshGeometry::from_vertices(vertices)
}

fn build_camera_trigger_icon_geometry() -> MeshGeometry {
    let mut vertices = Vec::new();
    append_camera_trigger_visual_vertices(
        &mut vertices,
        [0.5, 0.5, 0.5],
        [0.0, 0.0, 0.0],
        &CameraTriggerVisualStyle {
            ring_color: [0.2, 0.75, 1.0, 0.95],
            arrow_color: [0.2, 0.9, 0.3, 0.98],
            ring_radius: 0.28,
            ring_tube_radius: 0.025,
            shaft_length: 0.42,
            shaft_radius: 0.035,
            cone_length: 0.22,
            cone_radius: 0.095,
        },
    );

    MeshGeometry::from_vertices(vertices)
}

fn build_transform_trigger_icon_geometry() -> MeshGeometry {
    let mut vertices = Vec::new();
    append_transform_trigger_visual_vertices(
        &mut vertices,
        [0.08, 0.08, 0.08],
        [0.84, 0.84, 0.84],
        [0.0, 0.0, 0.0],
        &TransformTriggerVisualStyle {
            frame_color: [0.18, 1.0, 0.74, 0.95],
            arrow_color: [1.0, 0.95, 0.16, 0.98],
            ring_color: [0.42, 0.66, 1.0, 0.92],
            frame_radius: 0.035,
            shaft_radius: 0.055,
            cone_radius: 0.16,
            ring_thickness: 0.055,
        },
    );

    MeshGeometry::from_vertices(vertices)
}

impl GpuContext {
    /// Renders a single block to an offscreen texture for use as an editor icon.
    pub fn render_block_icon_snapshot(&self, block_id: &str, size: u32) -> Option<wgpu::Texture> {
        let safe_size = size.max(1);
        let dimetric = uses_dimetric_icon_projection(block_id);
        let geometry = build_block_icon_geometry(block_id, dimetric);
        if geometry.is_empty() {
            return None;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block Icon Vertex Buffer"),
                contents: bytemuck::cast_slice(geometry.vertices.as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = geometry.indices.as_ref().map(|indices| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Icon Index Buffer"),
                    contents: bytemuck::cast_slice(indices.as_slice()),
                    usage: wgpu::BufferUsages::INDEX,
                })
        });

        let color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Block Icon Depth Texture"),
            size: wgpu::Extent3d {
                width: safe_size,
                height: safe_size,
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

        let camera = resolve_icon_camera(block_id, dimetric);
        let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
        let view_proj = camera.projection * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_position: [camera.eye.x, camera.eye.y, camera.eye.z, 0.0],
        };
        let camera_uniform_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Icon Camera Uniform Buffer"),
                    contents: bytemuck::bytes_of(&camera_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Block Icon Camera Bind Group"),
            layout: &self.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });
        let icon_color_space_uniform = ColorSpaceUniform {
            apply_gamma_correction: if self.apply_gamma_correction {
                1.0
            } else {
                0.0
            },
            time_seconds: 0.0,
            viewport_size: [safe_size as f32, safe_size as f32],
        };
        let icon_color_space_uniform_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Icon Color Space Uniform Buffer"),
                    contents: bytemuck::bytes_of(&icon_color_space_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let icon_color_space_bind_group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Block Icon Color Space Bind Group"),
                layout: &self.color_space_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: icon_color_space_uniform_buffer.as_entire_binding(),
                }],
            });

        let mut encoder = self
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
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.block_icon_pipeline);
            pass.set_bind_group(0, &camera_bind_group, &[]);
            pass.set_bind_group(1, &self.zero_line_bind_group, &[]);
            pass.set_bind_group(2, &icon_color_space_bind_group, &[]);
            pass.set_bind_group(3, &self.block_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            if let Some(index_buffer) = &index_buffer {
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                let index_count = geometry
                    .indices
                    .as_ref()
                    .map(|indices| indices.len() as u32)
                    .unwrap_or(0);
                pass.draw_indexed(0..index_count, 0, 0..1);
            } else {
                pass.draw(0..geometry.vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Some(color_texture)
    }
}

#[cfg(test)]
fn build_block_icon_vertices(block_id: &str, dimetric: bool) -> Vec<eg_mesh::Vertex> {
    build_block_icon_geometry(block_id, dimetric).to_triangle_vertices()
}

#[cfg(test)]
mod tests {
    use super::{build_block_icon_vertices, resolve_icon_camera, uses_dimetric_icon_projection};

    fn assert_approx_eq(actual: f32, expected: f32, epsilon: f32) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected} +/- {epsilon}, got {actual}"
        );
    }

    #[test]
    fn dimetric_projection_configuration_is_read_from_block_render_json() {
        assert!(uses_dimetric_icon_projection("core/stone"));
        assert!(!uses_dimetric_icon_projection("core/speedportal"));
    }

    #[test]
    fn custom_icon_camera_configuration_controls_speed_portal_view() {
        let camera = resolve_icon_camera("core/speedportal", false);

        assert_approx_eq(camera.eye.x, 0.5, 1e-6);
        assert_approx_eq(camera.eye.y, 4.0, 1e-6);
        assert_approx_eq(camera.eye.z, 0.5, 1e-6);
        assert_approx_eq(camera.target.x, 0.5, 1e-5);
        assert_approx_eq(camera.target.y, 3.0, 1e-5);
        assert_approx_eq(camera.target.z, 0.5, 1e-5);
        assert_approx_eq(camera.up.x, 1.0, 1e-5);
        assert_approx_eq(camera.up.y, 0.0, 1e-5);
        assert_approx_eq(camera.up.z, 0.0, 1e-5);
    }

    #[test]
    fn dimetric_liquid_block_icons_mark_vertices_as_liquid_profile() {
        let liquid_vertices = build_block_icon_vertices("core/lava", true);
        assert!(
            !liquid_vertices.is_empty(),
            "expected liquid block icon vertices"
        );
        assert!(
            liquid_vertices
                .iter()
                .all(|vertex| vertex.render_profile == 1.0),
            "expected dimetric liquid icon vertices to use the liquid render profile tag"
        );

        let solid_vertices = build_block_icon_vertices("core/stone", true);
        assert!(
            solid_vertices
                .iter()
                .all(|vertex| vertex.render_profile == 0.0),
            "expected non-liquid dimetric icon vertices to keep the default render profile"
        );
    }

    #[test]
    fn dimetric_cuboid_block_icon_uses_all_elements() {
        let vertices = build_block_icon_vertices("core/wooden_fence", true);
        assert_eq!(vertices.len(), 3 * 36);
    }

    #[test]
    fn transform_trigger_dimetric_icon_uses_visible_marker_geometry() {
        let vertices = build_block_icon_vertices("core/transform_trigger", true);
        assert!(
            vertices.len() > 36,
            "expected transform trigger icon to include visible marker geometry"
        );
        assert!(vertices.iter().any(|vertex| vertex.color[3] > 0.0));
    }
}
