use crate::mesh::primitives::{append_prism, append_quad, append_rounded_prism};
use crate::types::Vertex;

pub(crate) fn build_trail_vertices(points: &[[f32; 3]], game_over: bool) -> Vec<Vertex> {
    let mut trail_vertices = Vec::new();
    let width = 0.8;
    let c_top = if game_over {
        [1.0, 0.2, 0.2, 1.0]
    } else {
        [0.8, 0.25, 0.35, 1.0]
    };
    let c_side = if game_over {
        [0.8, 0.1, 0.1, 1.0]
    } else {
        [0.7, 0.2, 0.3, 1.0]
    };

    if points.len() < 2 {
        return trail_vertices;
    }

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];

        if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
            let x_min = p1[0] - width / 2.0;
            let x_max = p1[0] + width / 2.0;
            let y_min = p1[1] - width / 2.0;
            let y_max = p1[1] + width / 2.0;
            let z_base = p1[2].min(p2[2]);
            let z_top = p1[2].max(p2[2]) + width;

            append_rounded_prism(
                &mut trail_vertices,
                [x_min, y_min, z_base],
                [x_max, y_max, z_top],
                c_top,
                c_side,
                width * 0.2,
                4,
            );
            continue;
        }

        let (x_min, x_max, y_min, y_max) = if dx.abs() > dy.abs() {
            (
                p1[0].min(p2[0]) - width / 2.0,
                p1[0].max(p2[0]) + width / 2.0,
                p1[1] - width / 2.0,
                p1[1] + width / 2.0,
            )
        } else {
            (
                p1[0] - width / 2.0,
                p1[0] + width / 2.0,
                p1[1].min(p2[1]) - width / 2.0,
                p1[1].max(p2[1]) + width / 2.0,
            )
        };

        let z_offset = p1[2].min(p2[2]);
        let z_extra = dz.abs() * 0.5;
        let z_min = z_offset;
        let z_max = z_offset + width + z_extra;

        append_rounded_prism(
            &mut trail_vertices,
            [x_min, y_min, z_min],
            [x_max, y_max, z_max],
            c_top,
            c_side,
            width * 0.2,
            4,
        );
    }

    trail_vertices
}

pub(crate) fn build_spawn_marker_vertices(position: [f32; 3], faces_right: bool) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let x = position[0];
    let y = position[1];
    let z = position[2];

    append_prism(
        &mut vertices,
        [x + 0.1, y + 0.1, z],
        [x + 0.9, y + 0.9, z + 0.5],
        [0.25, 0.95, 0.35, 1.0],
        [0.1, 0.45, 0.15, 1.0],
    );

    if faces_right {
        append_prism(
            &mut vertices,
            [x + 0.9, y + 0.35, z],
            [x + 1.3, y + 0.65, z + 0.7],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    } else {
        append_prism(
            &mut vertices,
            [x + 0.35, y + 0.9, z],
            [x + 0.65, y + 1.3, z + 0.7],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    }

    vertices
}

pub(crate) fn build_tap_indicator_vertices(positions: &[[f32; 3]]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color = [0.0, 0.0, 0.0, 1.0]; // Black
    let thickness = 0.05;
    let dash_len = 0.2;
    // Gaps will be (1.0 - 3*0.2) / 2 = 0.2

    for &pos in positions {
        let x_min = pos[0];
        let x_max = x_min + 1.0;
        let y_min = pos[1];
        let y_max = y_min + 1.0;
        let z = pos[2] + 0.1; // 0.1 above ground

        let starts = [0.0, 0.4, 0.8];

        for &start in &starts {
            let end = start + dash_len;

            // Bottom edge
            append_quad(
                &mut vertices,
                [x_min + start, y_min, z],
                [x_min + end, y_min, z],
                [x_min + end, y_min + thickness, z],
                [x_min + start, y_min + thickness, z],
                color,
            );

            // Top edge
            append_quad(
                &mut vertices,
                [x_min + start, y_max - thickness, z],
                [x_min + end, y_max - thickness, z],
                [x_min + end, y_max, z],
                [x_min + start, y_max, z],
                color,
            );

            // Left edge
            append_quad(
                &mut vertices,
                [x_min, y_min + start, z],
                [x_min + thickness, y_min + start, z],
                [x_min + thickness, y_min + end, z],
                [x_min, y_min + end, z],
                color,
            );

            // Right edge
            append_quad(
                &mut vertices,
                [x_max - thickness, y_min + start, z],
                [x_max, y_min + start, z],
                [x_max, y_min + end, z],
                [x_max - thickness, y_min + end, z],
                color,
            );
        }
    }
    vertices
}
