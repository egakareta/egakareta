use crate::mesh::shapes::append_prism;
use crate::types::{SpawnDirection, Vertex};

pub(crate) fn build_editor_cursor_vertices(cursor: [f32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color_top = [0.2, 0.85, 0.95, 0.4];
    let color_side = [0.1, 0.45, 0.55, 0.4];
    let z_min = cursor[2];
    let z_max = cursor[2] + 1.05;

    let x_min = cursor[0];
    let x_max = x_min + 1.0;
    let y_min = cursor[1];
    let y_max = y_min + 1.0;

    vertices.push(Vertex {
        position: [x_min, y_min, z_max],
        color: color_top,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_max],
        color: color_top,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_top,
    });
    vertices.push(Vertex {
        position: [x_min, y_min, z_max],
        color: color_top,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_top,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_max],
        color: color_top,
    });

    vertices.push(Vertex {
        position: [x_min, y_max, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_max],
        color: color_side,
    });

    vertices.push(Vertex {
        position: [x_max, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_max, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_max],
        color: color_side,
    });

    vertices.push(Vertex {
        position: [x_min, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_min, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_max, z_max],
        color: color_side,
    });

    vertices.push(Vertex {
        position: [x_min, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_min, z_min],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_min, y_min, z_max],
        color: color_side,
    });
    vertices.push(Vertex {
        position: [x_max, y_min, z_max],
        color: color_side,
    });

    vertices
}

pub(crate) fn build_editor_preview_player_vertices(
    position: [f32; 3],
    direction: SpawnDirection,
    is_tapping: bool,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let base_x = position[0];
    let base_y = position[1];
    let base_z = position[2];

    append_prism(
        &mut vertices,
        [base_x + 0.27, base_y + 0.27, base_z + 0.02],
        [base_x + 0.73, base_y + 0.73, base_z + 0.52],
        [0.95, 0.98, 1.0, 1.0],
        [0.45, 0.8, 0.95, 1.0],
    );

    append_prism(
        &mut vertices,
        [base_x + 0.34, base_y + 0.34, base_z + 0.52],
        [base_x + 0.66, base_y + 0.66, base_z + 0.84],
        [0.98, 1.0, 1.0, 1.0],
        [0.72, 0.9, 0.98, 1.0],
    );

    match direction {
        SpawnDirection::Forward => {
            append_prism(
                &mut vertices,
                [base_x + 0.41, base_y + 0.73, base_z + 0.2],
                [base_x + 0.59, base_y + 1.08, base_z + 0.48],
                [0.3, 0.95, 0.6, 1.0],
                [0.15, 0.55, 0.35, 1.0],
            );
        }
        SpawnDirection::Right => {
            append_prism(
                &mut vertices,
                [base_x + 0.73, base_y + 0.41, base_z + 0.2],
                [base_x + 1.08, base_y + 0.59, base_z + 0.48],
                [0.3, 0.95, 0.6, 1.0],
                [0.15, 0.55, 0.35, 1.0],
            );
        }
    }

    if is_tapping {
        append_prism(
            &mut vertices,
            [base_x + 0.1, base_y + 0.1, base_z + 0.9],
            [base_x + 0.9, base_y + 0.9, base_z + 0.96],
            [1.0, 0.68, 0.2, 0.95],
            [0.9, 0.45, 0.15, 0.95],
        );
    }

    vertices
}
