use crate::block_repository::{resolve_block_definition, BlockRenderProfile};
use crate::mesh::noise::pseudo_random_noise;
use crate::mesh::obj::{append_obj_mesh, resolve_obj_mesh};
use crate::mesh::shapes::{append_prism, append_rounded_prism};
use crate::mesh::transforms::rotate_vertices_around_z;
use crate::types::{LevelObject, Vertex};

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    build_block_vertices_from_refs(objects.iter())
}

pub(crate) fn build_block_vertices_from_refs<'a, I>(objects: I) -> Vec<Vertex>
where
    I: IntoIterator<Item = &'a LevelObject>,
{
    build_block_vertices_with_phase_from_refs(objects, 0.0)
}

pub(crate) fn build_block_vertices_with_phase(
    objects: &[LevelObject],
    pulse_phase_seconds: f32,
) -> Vec<Vertex> {
    build_block_vertices_with_phase_from_refs(objects.iter(), pulse_phase_seconds)
}

pub(crate) fn build_block_vertices_with_phase_from_refs<'a, I>(
    objects: I,
    pulse_phase_seconds: f32,
) -> Vec<Vertex>
where
    I: IntoIterator<Item = &'a LevelObject>,
{
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

        if vertices.is_empty() && matches!(block.render.profile, BlockRenderProfile::FinishRing) {
            append_finish_ring(
                vertices,
                obj,
                block.render.color_top,
                block.render.color_outline,
                pulse_phase_seconds,
            );
        } else if vertices.is_empty()
            && matches!(block.render.profile, BlockRenderProfile::VoidFrame)
        {
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

fn append_finish_ring(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    color_outer: [f32; 4],
    color_inner: [f32; 4],
    pulse_phase_seconds: f32,
) {
    const SEGMENTS: usize = 28;
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];

    let phase_offset = (obj.position[0] * 0.37 + obj.position[1] * 0.21) * std::f32::consts::PI;
    let pulse = 1.0 + 0.14 * (pulse_phase_seconds * 5.0 + phase_offset).sin();

    let base_radius = (obj.size[0].min(obj.size[1]) * 0.5 * 0.85).max(0.15);
    let outer_radius = base_radius * pulse;
    let inner_radius = (outer_radius * 0.56).max(0.08);
    let half_thickness = (obj.size[2] * 0.16).clamp(0.03, 0.14);
    let z_top = center[2] + half_thickness;
    let z_bottom = center[2] - half_thickness;

    let mut funnel_color = color_inner;
    funnel_color[3] = (funnel_color[3] * 0.72).clamp(0.0, 1.0);
    let sink_point = [
        center[0],
        center[1],
        obj.position[2] - obj.size[2] * 0.9 - 0.25,
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
            center[1] + sin0 * outer_radius,
            z_top,
        ];
        let outer_top_1 = [
            center[0] + cos1 * outer_radius,
            center[1] + sin1 * outer_radius,
            z_top,
        ];
        let inner_top_0 = [
            center[0] + cos0 * inner_radius,
            center[1] + sin0 * inner_radius,
            z_top,
        ];
        let inner_top_1 = [
            center[0] + cos1 * inner_radius,
            center[1] + sin1 * inner_radius,
            z_top,
        ];

        let outer_bottom_0 = [
            center[0] + cos0 * outer_radius,
            center[1] + sin0 * outer_radius,
            z_bottom,
        ];
        let outer_bottom_1 = [
            center[0] + cos1 * outer_radius,
            center[1] + sin1 * outer_radius,
            z_bottom,
        ];
        let inner_bottom_0 = [
            center[0] + cos0 * inner_radius,
            center[1] + sin0 * inner_radius,
            z_bottom,
        ];
        let inner_bottom_1 = [
            center[0] + cos1 * inner_radius,
            center[1] + sin1 * inner_radius,
            z_bottom,
        ];

        push_triangle(vertices, outer_top_0, outer_top_1, inner_top_1, color_outer);
        push_triangle(vertices, outer_top_0, inner_top_1, inner_top_0, color_outer);

        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_1,
            outer_bottom_1,
            color_outer,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_0,
            inner_bottom_1,
            color_outer,
        );

        push_triangle(
            vertices,
            outer_bottom_0,
            outer_bottom_1,
            outer_top_1,
            color_inner,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            outer_top_1,
            outer_top_0,
            color_inner,
        );

        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_1,
            inner_bottom_1,
            color_inner,
        );
        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_0,
            inner_top_1,
            color_inner,
        );

        if index % 2 == 0 {
            push_triangle(
                vertices,
                inner_bottom_0,
                inner_bottom_1,
                sink_point,
                funnel_color,
            );
        }
    }
}

fn push_triangle(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 4],
) {
    vertices.push(Vertex {
        position: p0,
        color,
    });
    vertices.push(Vertex {
        position: p1,
        color,
    });
    vertices.push(Vertex {
        position: p2,
        color,
    });
}
