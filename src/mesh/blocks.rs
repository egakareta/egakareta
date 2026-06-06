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
const GEM_PROFILE_TAG: f32 = 4.0;
const PARALLEL_GEOMETRY_OBJECT_THRESHOLD: usize = 256;
const PARALLEL_CHUNK_SIZE: usize = 128;
const VERTICES_PER_PRISM: usize = 36;

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
        let chunk_geometries: Vec<MeshGeometry> = objects
            .par_chunks(PARALLEL_CHUNK_SIZE)
            .map(|chunk| {
                let mut geometry = MeshGeometry::default();
                geometry.vertices.reserve(chunk.len() * VERTICES_PER_PRISM);
                let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
                for object in chunk {
                    append_block_geometry_with_scratch(&mut geometry, object, &mut scratch);
                }
                geometry
            })
            .collect();
        merge_object_geometries(chunk_geometries)
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
    let (lower, _) = objects.size_hint();
    all_geometry
        .vertices
        .reserve(lower.saturating_mul(VERTICES_PER_PRISM));
    let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
    for obj in objects {
        append_block_geometry_with_scratch(&mut all_geometry, obj, &mut scratch);
    }

    all_geometry
}

fn build_block_geometry_from_slice(objects: &[LevelObject]) -> MeshGeometry {
    if should_build_geometry_in_parallel(objects.len()) {
        puffin::profile_scope!("BuildBlockGeometryParallel");
        let chunk_geometries: Vec<MeshGeometry> = objects
            .par_chunks(PARALLEL_CHUNK_SIZE)
            .map(|chunk| {
                let mut geometry = MeshGeometry::default();
                geometry.vertices.reserve(chunk.len() * VERTICES_PER_PRISM);
                let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
                for object in chunk {
                    append_block_geometry_with_scratch(&mut geometry, object, &mut scratch);
                }
                geometry
            })
            .collect();
        merge_object_geometries(chunk_geometries)
    } else {
        build_block_geometry_impl(objects.iter())
    }
}

fn merge_object_geometries(object_geometries: Vec<MeshGeometry>) -> MeshGeometry {
    let total_vertices: usize = object_geometries.iter().map(|g| g.vertices.len()).sum();
    let total_indices: usize = object_geometries
        .iter()
        .filter_map(|g| g.indices.as_ref().map(Vec::len))
        .sum();

    let mut all_geometry = MeshGeometry::default();
    all_geometry.vertices.reserve(total_vertices);
    if total_indices > 0 {
        all_geometry.indices = Some(Vec::with_capacity(total_indices));
    }
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
    append_block_geometry_inner(
        all_geometry,
        obj,
        &mut object_vertices,
        &mut object_geometry,
    );
}

/// Like `append_block_geometry` but reuses `scratch_vertices` across calls to avoid
/// per-object heap allocations. Used in parallel chunk builders and serial loops.
fn append_block_geometry_with_scratch(
    all_geometry: &mut MeshGeometry,
    obj: &LevelObject,
    scratch_vertices: &mut Vec<Vertex>,
) {
    scratch_vertices.clear();
    let mut object_geometry = MeshGeometry::default();
    append_block_geometry_inner(all_geometry, obj, scratch_vertices, &mut object_geometry);
}

fn append_block_geometry_inner(
    all_geometry: &mut MeshGeometry,
    obj: &LevelObject,
    object_vertices: &mut Vec<Vertex>,
    object_geometry: &mut MeshGeometry,
) {
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
            append_egmesh_geometry(object_geometry, obj, mesh, colors.top, texture_layers.side);
        } else if let Some(mesh) = resolve_obj_mesh(mesh_path) {
            append_obj_mesh(object_vertices, obj, mesh, colors.top, texture_layers.side);
        }
    }

    if object_geometry.vertices.is_empty() && object_vertices.is_empty() {
        let prism_colors = PrismFaceColors::new_with_outline(
            colors.top,
            colors.side,
            colors.bottom,
            colors.outline,
        );

        append_prism_with_layers(
            object_vertices,
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
        rotate_vertices_around_euler(object_vertices, center, obj.rotation_degrees);
        // Apply profile tags directly to the vertex buffer before appending.
        apply_block_profile_tags(object_vertices, &block.render.profile, &colors, center);
        all_geometry.append_vertices_from_slice(object_vertices);
        return;
    }

    // egmesh path: profile tags applied to object_geometry vertices.
    apply_block_profile_tags(
        &mut object_geometry.vertices,
        &block.render.profile,
        &colors,
        center,
    );
    all_geometry.append_geometry(std::mem::take(object_geometry));
}

fn apply_block_profile_tags(
    vertices: &mut [Vertex],
    profile: &BlockRenderProfile,
    colors: &BlockColors,
    center: [f32; 3],
) {
    match profile {
        BlockRenderProfile::Neon => {
            for vertex in vertices.iter_mut() {
                vertex.color = colors.top;
            }
        }
        BlockRenderProfile::Liquid => {
            for vertex in vertices.iter_mut() {
                vertex.set_render_profile(LIQUID_PROFILE_TAG);
            }
        }
        BlockRenderProfile::Gem => {
            let phase_seed =
                pseudo_random_noise(center[0], center[1], center[2]) * std::f32::consts::TAU;
            for vertex in vertices.iter_mut() {
                vertex.set_render_profile(GEM_PROFILE_TAG);
                vertex.color_outline = [center[0], center[1], center[2], phase_seed];
            }
        }
        BlockRenderProfile::Solid | BlockRenderProfile::SpeedPortal => {}
    }
}

fn apply_color_tint(color: [f32; 4], tint_rgb: [f32; 3]) -> [f32; 4] {
    [
        color[0] * tint_rgb[0].clamp(0.0, 1.0),
        color[1] * tint_rgb[1].clamp(0.0, 1.0),
        color[2] * tint_rgb[2].clamp(0.0, 1.0),
        color[3],
    ]
}
