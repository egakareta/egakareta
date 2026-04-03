/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::Vertex;

pub(crate) fn append_prism(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    color_top: [f32; 4],
    color_side: [f32; 4],
) {
    append_prism_with_layers(vertices, min, max, color_top, color_side, 0, 0);
}

pub(crate) fn append_prism_with_layers(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    color_top: [f32; 4],
    color_side: [f32; 4],
    top_layer: u32,
    side_layer: u32,
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    let mut push_face = |p0: [f32; 3],
                         p1: [f32; 3],
                         p2: [f32; 3],
                         p3: [f32; 3],
                         color: [f32; 4],
                         texture_layer: u32| {
        vertices.push(Vertex::textured(p0, color, [0.0, 0.0], texture_layer));
        vertices.push(Vertex::textured(p1, color, [0.0, 1.0], texture_layer));
        vertices.push(Vertex::textured(p2, color, [1.0, 1.0], texture_layer));
        vertices.push(Vertex::textured(p0, color, [0.0, 0.0], texture_layer));
        vertices.push(Vertex::textured(p2, color, [1.0, 1.0], texture_layer));
        vertices.push(Vertex::textured(p3, color, [1.0, 0.0], texture_layer));
    };

    // Top (+Y)
    push_face(
        [x_min, y_max, z_min],
        [x_min, y_max, z_max],
        [x_max, y_max, z_max],
        [x_max, y_max, z_min],
        color_top,
        top_layer,
    );

    // +X
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        color_side,
        side_layer,
    );

    // -X
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        color_side,
        side_layer,
    );

    // +Z
    push_face(
        [x_min, y_min, z_max],
        [x_max, y_min, z_max],
        [x_max, y_max, z_max],
        [x_min, y_max, z_max],
        color_side,
        side_layer,
    );

    // -Z
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_max, z_min],
        [x_max, y_max, z_min],
        [x_max, y_min, z_min],
        color_side,
        side_layer,
    );

    // Bottom (-Y)
    push_face(
        [x_min, y_min, z_min],
        [x_max, y_min, z_min],
        [x_max, y_min, z_max],
        [x_min, y_min, z_max],
        color_side,
        side_layer,
    );
}

pub(crate) fn append_quad(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    color: [f32; 4],
) {
    vertices.push(Vertex::untextured(p0, color));
    vertices.push(Vertex::untextured(p1, color));
    vertices.push(Vertex::untextured(p2, color));
    vertices.push(Vertex::untextured(p0, color));
    vertices.push(Vertex::untextured(p2, color));
    vertices.push(Vertex::untextured(p3, color));
}
