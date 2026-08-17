/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy mesh conversion for egakareta's generated geometry.
//!
//! Geometry generation remains renderer-independent in [`crate::MeshGeometry`].
//! This module is the engine-facing adapter: it preserves every attribute used
//! by the block shader in a `bevy::render::mesh::Mesh`, ready for the custom
//! Bevy material introduced by the renderer cutover.

use bevy::{
    asset::RenderAssetUsages,
    render::{
        mesh::{Indices, Mesh, MeshVertexAttribute},
        render_resource::{PrimitiveTopology, VertexFormat},
    },
};

use crate::{BlockInstanceBatch, MeshGeometry, Vertex};

/// Normalized texture coordinate used by the block texture atlas.
pub const ATTRIBUTE_UV_NORM: MeshVertexAttribute =
    MeshVertexAttribute::new("EgUvNorm", 0xE6A0_0001, VertexFormat::Float32x2);
/// Texture-array layer selected by the block shader.
pub const ATTRIBUTE_TEXTURE_LAYER: MeshVertexAttribute =
    MeshVertexAttribute::new("EgTextureLayer", 0xE6A0_0002, VertexFormat::Float32);
/// Outline color used by the editor and block outline paths.
pub const ATTRIBUTE_COLOR_OUTLINE: MeshVertexAttribute =
    MeshVertexAttribute::new("EgColorOutline", 0xE6A0_0003, VertexFormat::Float32x4);
/// Block render-profile discriminator.
pub const ATTRIBUTE_RENDER_PROFILE: MeshVertexAttribute =
    MeshVertexAttribute::new("EgRenderProfile", 0xE6A0_0004, VertexFormat::Float32);
/// World-space idle-animation anchor and phase seed.
pub const ATTRIBUTE_ANIMATION_ANCHOR: MeshVertexAttribute =
    MeshVertexAttribute::new("EgAnimationAnchor", 0xE6A0_0005, VertexFormat::Float32x4);
/// First packed block-animation parameter group.
pub const ATTRIBUTE_ANIMATION_PARAMS: MeshVertexAttribute =
    MeshVertexAttribute::new("EgAnimationParams", 0xE6A0_0006, VertexFormat::Float32x4);
/// Second packed block-animation parameter group.
pub const ATTRIBUTE_ANIMATION_PARAMS2: MeshVertexAttribute =
    MeshVertexAttribute::new("EgAnimationParams2", 0xE6A0_0007, VertexFormat::Float32x4);

/// Convert renderer-independent generated geometry to a Bevy triangle mesh.
///
/// The mesh retains CPU data as well as its render-world copy so changed
/// chunks can be inspected and incrementally rebuilt by the presentation
/// systems during the cutover.
pub fn mesh_geometry_to_bevy_mesh(geometry: &MeshGeometry) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.color)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.uv)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_UV_NORM,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.uv_norm)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_TEXTURE_LAYER,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.texture_layer)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_COLOR_OUTLINE,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.color_outline)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_RENDER_PROFILE,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.render_profile)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_ANCHOR,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.animation_anchor)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_PARAMS,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.animation_params)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_PARAMS2,
        geometry
            .vertices
            .iter()
            .map(|vertex| vertex.animation_params2)
            .collect::<Vec<_>>(),
    );
    if let Some(indices) = &geometry.indices {
        mesh.insert_indices(Indices::U32(bevy_triangle_indices(indices.iter().copied())));
    } else {
        mesh.insert_indices(Indices::U32(bevy_triangle_indices(
            0..geometry.vertices.len() as u32,
        )));
    }

    mesh
}

/// Convert a flat list of GPU-ready vertices into a Bevy triangle mesh.
///
/// This is the lightweight counterpart of [`mesh_geometry_to_bevy_mesh`]
/// for overlay geometry (trail, cursor, gizmo, selection outlines, etc.)
/// that lives in a `Vec<Vertex>` instead of a full `MeshGeometry`.
///
/// The resulting mesh reuses the same vertex layout and custom attributes
/// as block geometry, so it renders through the same `BlockMaterial`.
pub fn vertices_to_bevy_mesh(vertices: &[Vertex]) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vertices.iter().map(|v| v.color).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_UV_NORM,
        vertices.iter().map(|v| v.uv_norm).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_TEXTURE_LAYER,
        vertices.iter().map(|v| v.texture_layer).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_COLOR_OUTLINE,
        vertices.iter().map(|v| v.color_outline).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_RENDER_PROFILE,
        vertices
            .iter()
            .map(|v| v.render_profile)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_ANCHOR,
        vertices
            .iter()
            .map(|v| v.animation_anchor)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_PARAMS,
        vertices
            .iter()
            .map(|v| v.animation_params)
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        ATTRIBUTE_ANIMATION_PARAMS2,
        vertices
            .iter()
            .map(|v| v.animation_params2)
            .collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(bevy_triangle_indices(
        0..vertices.len() as u32,
    )));

    mesh
}

fn bevy_triangle_indices(indices: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut indices = indices.into_iter().collect::<Vec<_>>();
    for triangle in indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    indices
}

/// Expand one compact block-instance batch into ordinary geometry.
///
/// Bevy's first presentation pass keeps one `Mesh3d` per render chunk. Until
/// the dedicated instancing path is ported, this preserves the existing
/// instance transform, tint/noise, and animation-anchor semantics without
/// dropping any authored blocks.
pub fn expand_block_instance_batch(batch: &BlockInstanceBatch) -> MeshGeometry {
    let mut geometry = MeshGeometry::default();
    for instance in &batch.instances {
        let mut transformed = batch.template.clone();
        for vertex in &mut transformed.vertices {
            vertex.position = instance.transform_position(vertex.position);
            for channel in 0..3 {
                let base =
                    (vertex.color[channel] + instance.color_tint_and_noise[3]).clamp(0.0, 1.0);
                vertex.color[channel] = base * instance.color_tint_and_noise[channel];
                vertex.color_outline[channel] *= instance.color_tint_and_noise[channel];
            }
            vertex.animation_anchor = instance.animation_anchor;
        }
        geometry.append_geometry(transformed);
    }
    geometry
}

#[cfg(test)]
mod tests {
    use bevy::render::mesh::{Indices, Mesh, VertexAttributeValues};
    use glam::Vec3;

    use super::{
        ATTRIBUTE_ANIMATION_ANCHOR, ATTRIBUTE_ANIMATION_PARAMS, ATTRIBUTE_ANIMATION_PARAMS2,
        ATTRIBUTE_COLOR_OUTLINE, ATTRIBUTE_RENDER_PROFILE, ATTRIBUTE_TEXTURE_LAYER,
        ATTRIBUTE_UV_NORM, mesh_geometry_to_bevy_mesh, vertices_to_bevy_mesh,
    };
    use crate::{BlockInstance, BlockInstanceBatch, MeshGeometry, Vertex};

    #[test]
    fn conversion_preserves_block_shader_attributes_and_indices() {
        let mut vertex = Vertex::textured_with_outline(
            [1.0, 2.0, 3.0],
            [0.2, 0.3, 0.4, 1.0],
            [0.25, 0.75],
            [0.5, 0.125],
            7,
            [0.8, 0.7, 0.6, 0.5],
        );
        vertex.set_render_profile(2.0);
        vertex.set_animation(
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
        );
        let geometry = MeshGeometry {
            vertices: vec![vertex],
            indices: Some(vec![0, 0, 0]),
        };

        let mesh = mesh_geometry_to_bevy_mesh(&geometry);

        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            Some(&VertexAttributeValues::Float32x3(vec![vertex.position]))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR),
            Some(&VertexAttributeValues::Float32x4(vec![vertex.color]))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            Some(&VertexAttributeValues::Float32x2(vec![vertex.uv]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_UV_NORM),
            Some(&VertexAttributeValues::Float32x2(vec![vertex.uv_norm]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_TEXTURE_LAYER),
            Some(&VertexAttributeValues::Float32(vec![vertex.texture_layer]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_COLOR_OUTLINE),
            Some(&VertexAttributeValues::Float32x4(vec![
                vertex.color_outline
            ]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_RENDER_PROFILE),
            Some(&VertexAttributeValues::Float32(vec![vertex.render_profile]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_ANIMATION_ANCHOR),
            Some(&VertexAttributeValues::Float32x4(vec![
                vertex.animation_anchor
            ]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_ANIMATION_PARAMS),
            Some(&VertexAttributeValues::Float32x4(vec![
                vertex.animation_params
            ]))
        );
        assert_eq!(
            mesh.attribute(ATTRIBUTE_ANIMATION_PARAMS2),
            Some(&VertexAttributeValues::Float32x4(vec![
                vertex.animation_params2
            ]))
        );
        assert_eq!(mesh.indices(), Some(&Indices::U32(vec![0, 0, 0])));
    }

    #[test]
    fn conversion_normalizes_generated_winding_to_bevy_counter_clockwise() {
        let clockwise_vertices = [
            Vertex::untextured([0.0, 0.0, 0.0], [1.0; 4]),
            Vertex::untextured([0.0, 1.0, 0.0], [1.0; 4]),
            Vertex::untextured([1.0, 0.0, 0.0], [1.0; 4]),
        ];
        let indexed_geometry = MeshGeometry {
            vertices: clockwise_vertices.to_vec(),
            indices: Some(vec![0, 1, 2]),
        };

        let indexed_mesh = mesh_geometry_to_bevy_mesh(&indexed_geometry);
        let flat_mesh = vertices_to_bevy_mesh(&clockwise_vertices);

        assert_eq!(indexed_mesh.indices(), Some(&Indices::U32(vec![0, 2, 1])));
        assert_eq!(flat_mesh.indices(), Some(&Indices::U32(vec![0, 2, 1])));
    }

    #[test]
    fn trail_conversion_keeps_all_snake_faces_outward() {
        let vertices = crate::build_trail_vertices(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], false);
        let mesh = vertices_to_bevy_mesh(&vertices);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("trail mesh must use u32 triangle indices");
        };
        let expected_outward_normals = [
            Vec3::Y,
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::NEG_Y,
        ];

        for (face_indices, expected_normal) in indices.chunks_exact(6).zip(expected_outward_normals)
        {
            for triangle in face_indices.chunks_exact(3) {
                let p0 = Vec3::from(vertices[triangle[0] as usize].position);
                let p1 = Vec3::from(vertices[triangle[1] as usize].position);
                let p2 = Vec3::from(vertices[triangle[2] as usize].position);
                let normal = (p1 - p0).cross(p2 - p0).normalize();

                assert!(normal.is_finite());
                assert!(normal.dot(expected_normal) > 0.99);
            }
        }
    }

    #[test]
    fn instance_batch_expansion_preserves_transform_tint_and_animation_anchor() {
        let batch = BlockInstanceBatch {
            block_id: "solid".to_owned(),
            template: MeshGeometry::from_vertices(vec![Vertex::untextured(
                [1.0, 2.0, 3.0],
                [0.2, 0.3, 0.4, 1.0],
            )]),
            instances: vec![BlockInstance {
                model_rows: [
                    [1.0, 0.0, 0.0, 10.0],
                    [0.0, 1.0, 0.0, 20.0],
                    [0.0, 0.0, 1.0, 30.0],
                ],
                color_tint_and_noise: [0.5, 0.25, 1.0, 0.1],
                animation_anchor: [10.0, 20.0, 30.0, 0.75],
            }],
        };

        let expanded = super::expand_block_instance_batch(&batch);

        assert_eq!(expanded.vertices.len(), 1);
        assert_eq!(expanded.vertices[0].position, [11.0, 22.0, 33.0]);
        assert_eq!(expanded.vertices[0].color, [0.15, 0.1, 0.5, 1.0]);
        assert_eq!(
            expanded.vertices[0].animation_anchor,
            [10.0, 20.0, 30.0, 0.75]
        );
    }
}
