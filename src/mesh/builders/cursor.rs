/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

pub(crate) fn build_editor_cursor_vertices(cursor: [f32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color_top = [0.2, 0.85, 0.95, 0.4];
    let color_side = [0.1, 0.45, 0.55, 0.4];
    let x_min = cursor[0];
    let x_max = x_min + 1.0;
    let y_min = cursor[1];
    let y_max = y_min + 1.05;
    let z_min = cursor[2];
    let z_max = z_min + 1.0;

    append_prism(
        &mut vertices,
        [x_min, y_min, z_min],
        [x_max, y_max, z_max],
        color_top,
        color_side,
    );

    vertices
}
