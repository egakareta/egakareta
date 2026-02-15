use crate::block_repository::{resolve_block_definition, BlockRenderProfile};
use crate::mesh::obj::{append_obj_mesh, resolve_obj_mesh};
use crate::mesh::primitives::{
    append_prism, append_rounded_prism, pseudo_random_noise, rotate_vertices_around_z,
};
use crate::types::{LevelObject, Vertex};

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    let mut all_vertices = Vec::new();

    for obj in objects {
        let mut object_vertices = Vec::new();
        let vertices = &mut object_vertices;

        let x_min = obj.position[0];
        let x_max = obj.position[0] + obj.size[0];
        let y_min = obj.position[1];
        let y_max = obj.position[1] + obj.size[1];
        let z_min = obj.position[2];
        let z_max = obj.position[2] + obj.size[2];

        let block = resolve_block_definition(&obj.block_id);

        let mut color_top = block.render.color_top;
        let mut color_side = block.render.color_side;

        if block.render.noise.abs() > f32::EPSILON {
            let noise = pseudo_random_noise(obj.position[0], obj.position[1], obj.position[2]);
            let factor = (noise * 2.0 - 1.0) * block.render.noise;
            for i in 0..3 {
                color_top[i] = (color_top[i] + factor).clamp(0.0, 1.0);
                color_side[i] = (color_side[i] + factor).clamp(0.0, 1.0);
            }
        }

        if let Some(mesh_path) = block.assets.mesh.as_deref() {
            if let Some(mesh) = resolve_obj_mesh(mesh_path) {
                append_obj_mesh(vertices, obj, mesh, color_top);
            }
        }

        if vertices.is_empty() && matches!(block.render.profile, BlockRenderProfile::VoidFrame) {
            let color_fill = block.render.color_fill;
            let color_outline = block.render.color_outline;
            let t = 0.05;

            // Fill
            append_prism(
                vertices,
                [x_min + t, y_min + t, z_min + t],
                [x_max - t, y_max - t, z_max - t],
                color_fill,
                color_fill,
            );

            // Bottom edges
            append_prism(
                vertices,
                [x_min, y_min, z_min],
                [x_max, y_min + t, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_min],
                [x_max, y_max, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_min + t, z_min],
                [x_min + t, y_max - t, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min + t, z_min],
                [x_max, y_max - t, z_min + t],
                color_outline,
                color_outline,
            );

            // Top edges
            append_prism(
                vertices,
                [x_min, y_min, z_max - t],
                [x_max, y_min + t, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_max - t],
                [x_max, y_max, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_min + t, z_max - t],
                [x_min + t, y_max - t, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min + t, z_max - t],
                [x_max, y_max - t, z_max],
                color_outline,
                color_outline,
            );

            // Vertical edges
            append_prism(
                vertices,
                [x_min, y_min, z_min + t],
                [x_min + t, y_min + t, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min, z_min + t],
                [x_max, y_min + t, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_min + t],
                [x_min + t, y_max, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_max - t, z_min + t],
                [x_max, y_max, z_max - t],
                color_outline,
                color_outline,
            );
        } else if vertices.is_empty() {
            if obj.roundness > f32::EPSILON {
                append_rounded_prism(
                    vertices,
                    [x_min, y_min, z_min],
                    [x_max, y_max, z_max],
                    color_top,
                    color_side,
                    obj.roundness,
                    5,
                );
            } else {
                append_prism(
                    vertices,
                    [x_min, y_min, z_min],
                    [x_max, y_max, z_max],
                    color_top,
                    color_side,
                );
            }
        }

        let center = [
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        ];
        rotate_vertices_around_z(&mut object_vertices, center, obj.rotation_degrees);
        all_vertices.extend(object_vertices);
    }

    all_vertices
}
