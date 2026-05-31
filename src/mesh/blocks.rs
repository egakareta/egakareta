/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use rayon::prelude::*;

use crate::block_repository::{
    resolve_block_definition, resolve_block_texture_layers, BlockRenderProfile,
};
use crate::mesh::egmesh::{append_egmesh_geometry, resolve_egmesh};
use crate::mesh::geometry::MeshGeometry;
use crate::mesh::noise::pseudo_random_noise;
use crate::mesh::obj::{append_obj_mesh, resolve_obj_mesh};
use crate::mesh::shapes::{append_prism_with_layers, PrismFaceColors, PrismTextureLayers};
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::platform::parallel::rayon_is_ready;
use crate::types::{LevelObject, Vertex};

const LIQUID_PROFILE_TAG: f32 = 1.0;
const FINISH_RING_PROFILE_TAG: f32 = 2.0;
const PARALLEL_GEOMETRY_OBJECT_THRESHOLD: usize = 256;

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    puffin::profile_scope!("BuildBlockVertices");
    build_block_geometry_from_slice(objects).to_triangle_vertices()
}

pub(crate) fn build_block_geometry(objects: &[LevelObject]) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometry");
    build_block_geometry_from_slice(objects)
}

pub(crate) fn build_block_geometry_from_refs(objects: &[&LevelObject]) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryRefs");
    if should_build_geometry_in_parallel(objects.len()) {
        puffin::profile_scope!("BuildBlockGeometryRefsParallel");
        merge_object_geometries(
            objects
                .par_iter()
                .map(|object| build_block_geometry_for_object(object))
                .collect(),
        )
    } else {
        build_block_geometry_impl(objects.iter().copied())
    }
}

pub(crate) fn build_block_geometry_for_object(object: &LevelObject) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryOne");
    let mut geometry = MeshGeometry::default();
    append_block_geometry(&mut geometry, object);
    geometry
}

fn build_block_geometry_impl<'a, I>(objects: I) -> MeshGeometry
where
    I: Iterator<Item = &'a LevelObject>,
{
    puffin::profile_scope!("BuildBlockGeometryImpl");
    let mut all_geometry = MeshGeometry::default();

    for obj in objects {
        append_block_geometry(&mut all_geometry, obj);
    }

    all_geometry
}

fn build_block_geometry_from_slice(objects: &[LevelObject]) -> MeshGeometry {
    if should_build_geometry_in_parallel(objects.len()) {
        puffin::profile_scope!("BuildBlockGeometryParallel");
        merge_object_geometries(
            objects
                .par_iter()
                .map(build_block_geometry_for_object)
                .collect(),
        )
    } else {
        build_block_geometry_impl(objects.iter())
    }
}

fn merge_object_geometries(object_geometries: Vec<MeshGeometry>) -> MeshGeometry {
    let mut all_geometry = MeshGeometry::default();
    for object_geometry in object_geometries {
        all_geometry.append_geometry(object_geometry);
    }
    all_geometry
}

fn should_build_geometry_in_parallel(object_count: usize) -> bool {
    rayon_is_ready() && object_count >= PARALLEL_GEOMETRY_OBJECT_THRESHOLD
}

struct BlockColors {
    top: [f32; 4],
    side: [f32; 4],
    bottom: [f32; 4],
    outline: [f32; 4],
}

impl BlockColors {
    fn apply_noise(&mut self, factor: f32) {
        for i in 0..3 {
            self.top[i] = (self.top[i] + factor).clamp(0.0, 1.0);
            self.side[i] = (self.side[i] + factor).clamp(0.0, 1.0);
            self.bottom[i] = (self.bottom[i] + factor).clamp(0.0, 1.0);
        }
    }

    fn apply_tint(&mut self, tint_rgb: [f32; 3]) {
        self.top = apply_color_tint(self.top, tint_rgb);
        self.side = apply_color_tint(self.side, tint_rgb);
        self.bottom = apply_color_tint(self.bottom, tint_rgb);
        self.outline = apply_color_tint(self.outline, tint_rgb);
    }
}

fn append_block_geometry(all_geometry: &mut MeshGeometry, obj: &LevelObject) {
    let mut object_geometry = MeshGeometry::default();
    let mut object_vertices = Vec::new();
    let vertices = &mut object_vertices;

    let x_min = obj.position[0];
    let x_max = obj.position[0] + obj.size[0];
    let y_min = obj.position[1];
    let y_max = obj.position[1] + obj.size[1];
    let z_min = obj.position[2];
    let z_max = obj.position[2] + obj.size[2];

    let block = resolve_block_definition(&obj.block_id);
    let texture_layers = resolve_block_texture_layers(&obj.block_id);

    let mut colors = BlockColors {
        top: block.render.color_top,
        side: block.render.color_side,
        bottom: block.render.color_bottom,
        outline: block.render.color_outline,
    };

    if block.render.noise.abs() > f32::EPSILON {
        let noise = pseudo_random_noise(obj.position[0], obj.position[1], obj.position[2]);
        let factor = (noise * 2.0 - 1.0) * block.render.noise;
        colors.apply_noise(factor);
    }

    colors.apply_tint(obj.color_tint);

    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];
    if let Some(mesh_path) = block.assets.mesh.as_deref() {
        if let Some(mesh) = resolve_egmesh(mesh_path) {
            append_egmesh_geometry(
                &mut object_geometry,
                obj,
                mesh,
                colors.top,
                texture_layers.side,
            );
        } else if let Some(mesh) = resolve_obj_mesh(mesh_path) {
            append_obj_mesh(vertices, obj, mesh, colors.top, texture_layers.side);
        }
    }

    if object_geometry.vertices.is_empty()
        && vertices.is_empty()
        && matches!(block.render.profile, BlockRenderProfile::FinishRing)
    {
        append_finish_ring(
            vertices,
            obj,
            colors.top,
            colors.outline,
            texture_layers.side,
        );
    } else if object_geometry.vertices.is_empty() && vertices.is_empty() {
        let prism_colors = PrismFaceColors::new_with_outline(
            colors.top,
            colors.side,
            colors.bottom,
            colors.outline,
        );

        append_prism_with_layers(
            vertices,
            [x_min, y_min, z_min],
            [x_max, y_max, z_max],
            prism_colors,
            PrismTextureLayers::new(
                texture_layers.top,
                texture_layers.side,
                texture_layers.bottom,
            ),
        );
    }

    if !object_vertices.is_empty() {
        rotate_vertices_around_euler(&mut object_vertices, center, obj.rotation_degrees);
        object_geometry.append_vertices(object_vertices);
    }

    if matches!(block.render.profile, BlockRenderProfile::Neon) {
        // Neon: use raw specified colors, strip any normal_tint lighting from mesh vertices.
        for vertex in &mut object_geometry.vertices {
            vertex.color = colors.top;
        }
    }

    if matches!(block.render.profile, BlockRenderProfile::Liquid) {
        for vertex in &mut object_geometry.vertices {
            vertex.set_render_profile(LIQUID_PROFILE_TAG);
        }
    }

    all_geometry.append_geometry(object_geometry);
}

fn apply_color_tint(color: [f32; 4], tint_rgb: [f32; 3]) -> [f32; 4] {
    [
        color[0] * tint_rgb[0].clamp(0.0, 1.0),
        color[1] * tint_rgb[1].clamp(0.0, 1.0),
        color[2] * tint_rgb[2].clamp(0.0, 1.0),
        color[3],
    ]
}

fn append_finish_ring(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    color_outer: [f32; 4],
    color_inner: [f32; 4],
    texture_layer: u32,
) {
    const SEGMENTS: usize = 28;
    let vertex_start = vertices.len();
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];

    let phase_offset = (obj.position[0] * 0.37 + obj.position[2] * 0.21) * std::f32::consts::PI;

    let base_radius = (obj.size[0].min(obj.size[2]) * 0.5 * 0.85).max(0.15);
    let outer_radius = base_radius;
    let inner_radius = (outer_radius * 0.56).max(0.08);
    let half_thickness = (obj.size[1] * 0.16).clamp(0.03, 0.14);
    let y_top = center[1] + half_thickness;
    let y_bottom = center[1] - half_thickness;

    let mut funnel_color = color_inner;
    funnel_color[3] = (funnel_color[3] * 0.72).clamp(0.0, 1.0);
    let sink_point = [
        center[0],
        obj.position[1] - obj.size[1] * 0.9 - 0.25,
        center[2],
    ];

    for index in 0..SEGMENTS {
        let t0 = index as f32 / SEGMENTS as f32;
        let t1 = (index + 1) as f32 / SEGMENTS as f32;
        let a0 = t0 * std::f32::consts::TAU;
        let a1 = t1 * std::f32::consts::TAU;

        let (cos0, sin0) = (a0.cos(), a0.sin());
        let (cos1, sin1) = (a1.cos(), a1.sin());

        let outer_top_0 = [
            center[0] + cos0 * outer_radius,
            y_top,
            center[2] + sin0 * outer_radius,
        ];
        let outer_top_1 = [
            center[0] + cos1 * outer_radius,
            y_top,
            center[2] + sin1 * outer_radius,
        ];
        let inner_top_0 = [
            center[0] + cos0 * inner_radius,
            y_top,
            center[2] + sin0 * inner_radius,
        ];
        let inner_top_1 = [
            center[0] + cos1 * inner_radius,
            y_top,
            center[2] + sin1 * inner_radius,
        ];

        let outer_bottom_0 = [
            center[0] + cos0 * outer_radius,
            y_bottom,
            center[2] + sin0 * outer_radius,
        ];
        let outer_bottom_1 = [
            center[0] + cos1 * outer_radius,
            y_bottom,
            center[2] + sin1 * outer_radius,
        ];
        let inner_bottom_0 = [
            center[0] + cos0 * inner_radius,
            y_bottom,
            center[2] + sin0 * inner_radius,
        ];
        let inner_bottom_1 = [
            center[0] + cos1 * inner_radius,
            y_bottom,
            center[2] + sin1 * inner_radius,
        ];

        push_triangle(
            vertices,
            outer_top_0,
            outer_top_1,
            inner_top_1,
            color_outer,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_top_0,
            inner_top_1,
            inner_top_0,
            color_outer,
            texture_layer,
        );

        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_1,
            outer_bottom_1,
            color_outer,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_0,
            inner_bottom_1,
            color_outer,
            texture_layer,
        );

        push_triangle(
            vertices,
            outer_bottom_0,
            outer_bottom_1,
            outer_top_1,
            color_inner,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            outer_top_1,
            outer_top_0,
            color_inner,
            texture_layer,
        );

        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_1,
            inner_bottom_1,
            color_inner,
            texture_layer,
        );
        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_0,
            inner_top_1,
            color_inner,
            texture_layer,
        );

        if index % 2 == 0 {
            push_triangle(
                vertices,
                inner_bottom_0,
                inner_bottom_1,
                sink_point,
                funnel_color,
                texture_layer,
            );
        }
    }

    let pulse_metadata = [center[0], center[2], phase_offset, 0.0];
    for vertex in vertices.iter_mut().skip(vertex_start) {
        vertex.set_render_profile(FINISH_RING_PROFILE_TAG);
        vertex.color_outline = pulse_metadata;
    }
}

fn push_triangle(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 4],
    texture_layer: u32,
) {
    vertices.push(Vertex::textured(p0, color, [0.0, 0.0], texture_layer));
    vertices.push(Vertex::textured(p1, color, [1.0, 0.0], texture_layer));
    vertices.push(Vertex::textured(p2, color, [0.5, 1.0], texture_layer));
}
