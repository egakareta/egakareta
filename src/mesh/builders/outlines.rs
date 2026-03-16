use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let x0 = position[0] - 0.015;
    let x1 = position[0] + size[0] + 0.015;
    let y0 = position[1] - 0.015;
    let y1 = position[1] + size[1] + 0.015;
    let z0 = position[2] - 0.015;
    let z1 = position[2] + size[2] + 0.015;

    let thickness = 0.045;
    let color_top = [0.45, 0.9, 1.0, 0.8];
    let color_side = [0.25, 0.75, 0.9, 0.8];

    // Edges along X
    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            &mut vertices,
            [x0 - thickness, y - thickness, z - thickness],
            [x1 + thickness, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Y
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0 + thickness, z - thickness],
            [x + thickness, y1 - thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Z
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0 + thickness],
            [x + thickness, y + thickness, z1 - thickness],
            color_top,
            color_side,
        );
    }

    vertices
}

pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let x0 = position[0] - 0.01;
    let x1 = position[0] + size[0] + 0.01;
    let y0 = position[1] - 0.01;
    let y1 = position[1] + size[1] + 0.01;
    let z0 = position[2] - 0.01;
    let z1 = position[2] + size[2] + 0.01;

    let thickness = 0.03;
    let color_top = [0.62, 0.9, 1.0, 0.45];
    let color_side = [0.45, 0.82, 0.95, 0.38];

    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            &mut vertices,
            [x0 - thickness, y - thickness, z - thickness],
            [x1 + thickness, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0 + thickness, z - thickness],
            [x + thickness, y1 - thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0 + thickness],
            [x + thickness, y + thickness, z1 - thickness],
            color_top,
            color_side,
        );
    }

    vertices
}
