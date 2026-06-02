/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::blocks::build_block_geometry_for_object;
use crate::mesh::builders::game::build_colored_tap_indicator_vertices;
use crate::types::{LevelObject, Vertex};

const EDITOR_CURSOR_TOP_COLOR: [f32; 4] = [0.2, 0.85, 0.95, 0.6];
const EDITOR_CURSOR_OUTLINE_COLOR: [f32; 4] = [0.08, 0.48, 0.62, 0.45];
const EDITOR_CURSOR_TINT_WEIGHT: f32 = 0.65;

pub(crate) fn build_editor_cursor_vertices(
    cursor: [f32; 3],
    size: [f32; 3],
    block_id: &str,
    rotation_degrees: [f32; 3],
) -> Vec<Vertex> {
    let object = LevelObject {
        position: cursor,
        size,
        rotation_degrees,
        block_id: block_id.to_string(),
        color_tint: [1.0, 1.0, 1.0],
    };

    let mut vertices = build_block_geometry_for_object(&object).to_triangle_vertices();
    apply_editor_cursor_tint(&mut vertices);

    vertices
}

fn apply_editor_cursor_tint(vertices: &mut [Vertex]) {
    let base_weight = 1.0 - EDITOR_CURSOR_TINT_WEIGHT;
    for vertex in vertices {
        for (index, cursor_color) in EDITOR_CURSOR_TOP_COLOR.iter().copied().enumerate().take(3) {
            vertex.color[index] = (vertex.color[index] * base_weight
                + cursor_color * EDITOR_CURSOR_TINT_WEIGHT)
                .clamp(0.0, 1.0);
        }
        vertex.color[3] = EDITOR_CURSOR_TOP_COLOR[3];
        vertex.color_outline = EDITOR_CURSOR_OUTLINE_COLOR;
        vertex.render_profile = 0.0;
    }
}

pub(crate) fn build_editor_tap_cursor_vertices(cursor: [f32; 3]) -> Vec<Vertex> {
    build_colored_tap_indicator_vertices(&[(cursor, EDITOR_CURSOR_TOP_COLOR)])
}

#[cfg(test)]
mod tests {
    use super::{build_editor_cursor_vertices, build_editor_tap_cursor_vertices};
    use crate::mesh::blocks::build_block_geometry_for_object;
    use crate::types::LevelObject;

    fn object(position: [f32; 3], size: [f32; 3], block_id: &str) -> LevelObject {
        LevelObject {
            position,
            size,
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: block_id.to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn cursor_vertices_follow_requested_size() {
        let vertices = build_editor_cursor_vertices(
            [1.0, 2.0, 3.0],
            [2.0, 0.25, 1.5],
            "core/stone",
            [0.0, 0.0, 0.0],
        );
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
        assert!((max_y - 2.25).abs() <= 1e-6);
        assert!((max_z - 4.5).abs() <= 1e-6);
    }

    #[test]
    fn cursor_vertices_use_actual_block_shape_with_blue_translucent_tint() {
        let position = [1.0, 2.0, 3.0];
        let size = [2.0, 0.25, 1.0];
        let vertices =
            build_editor_cursor_vertices(position, size, "core/speedportal", [0.0, 0.0, 0.0]);
        let block_vertices =
            build_block_geometry_for_object(&object(position, size, "core/speedportal"))
                .to_triangle_vertices();

        assert_eq!(vertices.len(), block_vertices.len());
        assert_ne!(vertices.len(), 36);
        for (cursor_vertex, block_vertex) in vertices.iter().zip(block_vertices) {
            assert_eq!(cursor_vertex.position, block_vertex.position);
            assert_eq!(cursor_vertex.uv, block_vertex.uv);
            assert_eq!(cursor_vertex.texture_layer, block_vertex.texture_layer);
            assert_eq!(cursor_vertex.color[3], 0.6);
            assert_eq!(cursor_vertex.color_outline, [0.08, 0.48, 0.62, 0.45]);
            assert_eq!(cursor_vertex.render_profile, 0.0);
        }
    }

    #[test]
    fn cursor_vertices_apply_requested_rotation() {
        let position = [1.0, 2.0, 3.0];
        let size = [2.0, 0.25, 1.0];
        let rotation_degrees = [0.0, 90.0, 0.0];
        let vertices =
            build_editor_cursor_vertices(position, size, "core/speedportal", rotation_degrees);
        let block_vertices = build_block_geometry_for_object(&LevelObject {
            position,
            size,
            rotation_degrees,
            block_id: "core/speedportal".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        })
        .to_triangle_vertices();

        assert_eq!(vertices.len(), block_vertices.len());
        for (cursor_vertex, block_vertex) in vertices.iter().zip(block_vertices) {
            assert_eq!(cursor_vertex.position, block_vertex.position);
        }
    }

    #[test]
    fn tap_cursor_uses_flat_tap_indicator_shape_and_ghost_color() {
        let vertices = build_editor_tap_cursor_vertices([1.0, 2.0, 3.0]);

        assert_eq!(vertices.len(), 72);
        assert!(vertices
            .iter()
            .all(|vertex| vertex.color == [0.2, 0.85, 0.95, 0.6]));
        assert!(vertices
            .iter()
            .all(|vertex| (vertex.position[1] - 2.1).abs() <= f32::EPSILON));
    }
}
