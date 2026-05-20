/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::shapes::append_prism;
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::types::Vertex;

fn build_editor_outline_hull_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
    color_top: [f32; 4],
    color_side: [f32; 4],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let expansion = (line_width * 1.35).max(0.018);

    append_prism(
        &mut vertices,
        [
            position[0] - expansion,
            position[1] - expansion,
            position[2] - expansion,
        ],
        [
            position[0] + size[0] + expansion,
            position[1] + size[1] + expansion,
            position[2] + size[2] + expansion,
        ],
        color_top,
        color_side,
    );

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];
    rotate_vertices_around_euler(&mut vertices, center, rotation_degrees);
    vertices
}

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        [0.06, 0.72, 1.0, 0.72],
        [0.02, 0.34, 0.92, 0.52],
    )
}

pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        [1.0, 0.82, 0.28, 0.58],
        [0.9, 0.5, 0.12, 0.42],
    )
}
