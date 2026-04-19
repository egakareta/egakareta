/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

const OUTLINE_PRISMS_PER_BLOCK: usize = 12;
const PRISM_VERTICES: usize = 36;
pub(crate) const OUTLINE_VERTICES_PER_BLOCK: usize = OUTLINE_PRISMS_PER_BLOCK * PRISM_VERTICES;
const FAST_HOVER_PRISMS_PER_BLOCK: usize = 1;
pub(crate) const FAST_HOVER_VERTICES_PER_BLOCK: usize =
    FAST_HOVER_PRISMS_PER_BLOCK * PRISM_VERTICES;

fn append_editor_outline_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    line_width: f32,
    color_top: [f32; 4],
    color_side: [f32; 4],
) {
    let offset = line_width / 6.0;
    let x0 = position[0] - offset;
    let x1 = position[0] + size[0] + offset;
    let y0 = position[1] - offset;
    let y1 = position[1] + size[1] + offset;
    let z0 = position[2] - offset;
    let z1 = position[2] + size[2] + offset;

    let thickness = line_width * 0.5;

    // Edges along X
    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            vertices,
            [x0 - thickness, y - thickness, z - thickness],
            [x1 + thickness, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Y
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            vertices,
            [x - thickness, y0 + thickness, z - thickness],
            [x + thickness, y1 - thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Z
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            vertices,
            [x - thickness, y - thickness, z0 + thickness],
            [x + thickness, y + thickness, z1 - thickness],
            color_top,
            color_side,
        );
    }
}

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(OUTLINE_VERTICES_PER_BLOCK);
    let color_top = [0.098, 0.6, 1.0, 0.8];
    let color_side = [0.078, 0.48, 0.8, 0.8];
    append_editor_outline_vertices(
        &mut vertices,
        position,
        size,
        line_width,
        color_top,
        color_side,
    );

    vertices
}

pub(crate) fn append_editor_hover_outline_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    line_width: f32,
) {
    let color_top = [0.62, 0.9, 1.0, 0.45];
    let color_side = [0.45, 0.82, 0.95, 0.38];
    append_editor_outline_vertices(vertices, position, size, line_width, color_top, color_side);
}

pub(crate) fn append_editor_hover_proxy_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    line_width: f32,
) {
    let expansion = (line_width * 0.45).max(0.01);
    let min = [
        position[0] - expansion,
        position[1] - expansion,
        position[2] - expansion,
    ];
    let max = [
        position[0] + size[0] + expansion,
        position[1] + size[1] + expansion,
        position[2] + size[2] + expansion,
    ];

    // Lightweight shell used during marquee drag so every overlapped block stays visible.
    append_prism(
        vertices,
        min,
        max,
        [0.62, 0.9, 1.0, 0.14],
        [0.45, 0.82, 0.95, 0.1],
    );
}

#[cfg(test)]
pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(OUTLINE_VERTICES_PER_BLOCK);
    append_editor_hover_outline_vertices(&mut vertices, position, size, line_width);

    vertices
}
