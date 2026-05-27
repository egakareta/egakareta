/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

pub(crate) fn build_editor_cursor_vertices(cursor: [f32; 3], size: [f32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color_top = [0.2, 0.85, 0.95, 0.4];
    let color_side = [0.1, 0.45, 0.55, 0.4];
    let x_min = cursor[0];
    let x_max = x_min + size[0];
    let y_min = cursor[1];
    let y_max = y_min + size[1] + 0.05;
    let z_min = cursor[2];
    let z_max = z_min + size[2];

    append_prism(
        &mut vertices,
        [x_min, y_min, z_min],
        [x_max, y_max, z_max],
        color_top,
        color_side,
    );

    vertices
}

#[cfg(test)]
mod tests {
    use super::build_editor_cursor_vertices;

    #[test]
    fn cursor_vertices_follow_requested_size() {
        let vertices = build_editor_cursor_vertices([1.0, 2.0, 3.0], [2.0, 0.25, 1.5]);
        let max_x = vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = vertices
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_z = vertices
            .iter()
            .map(|vertex| vertex.position[2])
            .fold(f32::NEG_INFINITY, f32::max);

        assert!((max_x - 3.0).abs() <= 1e-6);
        assert!((max_y - 2.3).abs() <= 1e-6);
        assert!((max_z - 4.5).abs() <= 1e-6);
    }
}
