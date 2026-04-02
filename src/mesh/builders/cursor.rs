use super::game::build_trail_vertices_with_alpha;
use crate::mesh::shapes::append_prism;
use crate::types::{SpawnDirection, Vertex};

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

pub(crate) fn build_editor_preview_player_vertices(
    position: [f32; 3],
    direction: SpawnDirection,
    is_tapping: bool,
) -> Vec<Vertex> {
    let center = [position[0] + 0.5, position[1], position[2] + 0.5];
    let head_length = if is_tapping { 0.44 } else { 0.32 };
    let alpha = if is_tapping { 0.62 } else { 0.42 };
    let dir = match direction {
        SpawnDirection::Forward => [0.0, 1.0],
        SpawnDirection::Right => [1.0, 0.0],
    };
    let head_start = [
        center[0] - dir[0] * head_length,
        center[1],
        center[2] - dir[1] * head_length,
    ];

    build_trail_vertices_with_alpha(&[head_start, center], false, alpha)
}
