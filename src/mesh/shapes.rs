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
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    let mut push_face =
        |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], color: [f32; 4]| {
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
            vertices.push(Vertex {
                position: p0,
                color,
            });
            vertices.push(Vertex {
                position: p2,
                color,
            });
            vertices.push(Vertex {
                position: p3,
                color,
            });
        };

    // Top (+Y)
    push_face(
        [x_min, y_max, z_min],
        [x_min, y_max, z_max],
        [x_max, y_max, z_max],
        [x_max, y_max, z_min],
        color_top,
    );

    // +X
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        color_side,
    );

    // -X
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        color_side,
    );

    // +Z
    push_face(
        [x_min, y_min, z_max],
        [x_max, y_min, z_max],
        [x_max, y_max, z_max],
        [x_min, y_max, z_max],
        color_side,
    );

    // -Z
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_max, z_min],
        [x_max, y_max, z_min],
        [x_max, y_min, z_min],
        color_side,
    );

    // Bottom (-Y)
    push_face(
        [x_min, y_min, z_min],
        [x_max, y_min, z_min],
        [x_max, y_min, z_max],
        [x_min, y_min, z_max],
        color_side,
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
    vertices.push(Vertex {
        position: p0,
        color,
    });
    vertices.push(Vertex {
        position: p2,
        color,
    });
    vertices.push(Vertex {
        position: p3,
        color,
    });
}
