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
    line_width_pixels: f32,
    color_top: [f32; 4],
    color_side: [f32; 4],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    append_prism(
        &mut vertices,
        position,
        [
            position[0] + size[0],
            position[1] + size[1],
            position[2] + size[2],
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
    let outline_width_pixels = (line_width_pixels * 1.35).max(1.0);
    for vertex in &mut vertices {
        vertex.color_outline = [center[0], center[1], center[2], outline_width_pixels];
        vertex.render_profile = 3.0;
    }
    vertices
}

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let selection_color = [0.098, 0.6, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        selection_color,
        selection_color,
    )
}

pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let hover_color = [0.698, 0.898, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        hover_color,
        hover_color,
    )
}
