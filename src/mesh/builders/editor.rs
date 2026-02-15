use crate::mesh::primitives::{append_cone, append_prism, append_sphere};
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum GizmoPart {
    MoveX,
    MoveY,
    MoveZ,
    MoveXNeg,
    MoveYNeg,
    MoveZNeg,
    ResizeX,
    ResizeY,
    ResizeZ,
    ResizeXNeg,
    ResizeYNeg,
    ResizeZNeg,
}

pub(crate) fn build_editor_gizmo_vertices(
    position: [f32; 3],
    size: [f32; 3],
    axis_lengths: [f32; 3],
    axis_width: f32,
    active_part: Option<GizmoPart>,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];

    let shaft = axis_width.max(0.0005) * 0.5;
    let tip_length = shaft * 6.0;
    let cone_radius = shaft * 2.5;
    let arm_start_offset = shaft * 2.0;
    let x_length = axis_lengths[0].max(arm_start_offset + tip_length);
    let y_length = axis_lengths[1].max(arm_start_offset + tip_length);
    let z_length = axis_lengths[2].max(arm_start_offset + tip_length);

    let color_x_base = [1.0, 0.05, 0.05, 0.6];
    let color_x_dark = [0.85, 0.0, 0.0, 0.4];
    let color_y_base = [0.05, 1.0, 0.05, 0.6];
    let color_y_dark = [0.0, 0.85, 0.0, 0.4];
    let color_z_base = [0.05, 0.05, 1.0, 0.6];
    let color_z_dark = [0.0, 0.0, 0.85, 0.4];

    let darken = |color: [f32; 4], active: bool| -> [f32; 4] {
        if active {
            [color[0] * 0.6, color[1] * 0.6, color[2] * 0.6, color[3]]
        } else {
            color
        }
    };

    // X move arms
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::MoveXNeg
        } else {
            GizmoPart::MoveX
        };
        let active = active_part == Some(variant);
        let sign = if neg { -1.0 } else { 1.0 };
        let start = center[0] + arm_start_offset * sign;
        let end = center[0] + x_length * sign;
        let (p_min_x, p_max_x) = if neg {
            (end + tip_length, start)
        } else {
            (start, end - tip_length)
        };
        append_prism(
            &mut vertices,
            [p_min_x, center[1] - shaft, center[2] - shaft],
            [p_max_x, center[1] + shaft, center[2] + shaft],
            darken(color_x_base, active),
            darken(color_x_dark, active),
        );
        append_cone(
            &mut vertices,
            [end - tip_length * sign, center[1], center[2]],
            [end, center[1], center[2]],
            cone_radius,
            darken(color_x_base, active),
        );
    }

    // Y move arms
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::MoveYNeg
        } else {
            GizmoPart::MoveY
        };
        let active = active_part == Some(variant);
        let sign = if neg { -1.0 } else { 1.0 };
        let start = center[1] + arm_start_offset * sign;
        let end = center[1] + y_length * sign;
        let (p_min_y, p_max_y) = if neg {
            (end + tip_length, start)
        } else {
            (start, end - tip_length)
        };
        append_prism(
            &mut vertices,
            [center[0] - shaft, p_min_y, center[2] - shaft],
            [center[0] + shaft, p_max_y, center[2] + shaft],
            darken(color_y_base, active),
            darken(color_y_dark, active),
        );
        append_cone(
            &mut vertices,
            [center[0], end - tip_length * sign, center[2]],
            [center[0], end, center[2]],
            cone_radius,
            darken(color_y_base, active),
        );
    }

    // Z move arms
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::MoveZNeg
        } else {
            GizmoPart::MoveZ
        };
        let active = active_part == Some(variant);
        let sign = if neg { -1.0 } else { 1.0 };
        let start = center[2] + arm_start_offset * sign;
        let end = center[2] + z_length * sign;
        let (p_min_z, p_max_z) = if neg {
            (end + tip_length, start)
        } else {
            (start, end - tip_length)
        };
        append_prism(
            &mut vertices,
            [center[0] - shaft, center[1] - shaft, p_min_z],
            [center[0] + shaft, center[1] + shaft, p_max_z],
            darken(color_z_base, active),
            darken(color_z_dark, active),
        );
        append_cone(
            &mut vertices,
            [center[0], center[1], end - tip_length * sign],
            [center[0], center[1], end],
            cone_radius,
            darken(color_z_base, active),
        );
    }

    // Resize handles
    let resize_radius = 0.25;
    let inner_resize_radius = 0.1;
    let inner_color = [0.0, 0.0, 0.0, 0.025];

    // X resize
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::ResizeXNeg
        } else {
            GizmoPart::ResizeX
        };
        let active = active_part == Some(variant);
        let x = if neg {
            position[0] - resize_radius
        } else {
            position[0] + size[0] + resize_radius
        };
        let pos = [x, center[1], center[2]];
        append_sphere(
            &mut vertices,
            pos,
            resize_radius,
            darken(color_x_base, active),
        );
        append_sphere(
            &mut vertices,
            pos,
            inner_resize_radius,
            darken(inner_color, active),
        );
    }

    // Y resize
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::ResizeYNeg
        } else {
            GizmoPart::ResizeY
        };
        let active = active_part == Some(variant);
        let y = if neg {
            position[1] - resize_radius
        } else {
            position[1] + size[1] + resize_radius
        };
        let pos = [center[0], y, center[2]];
        append_sphere(
            &mut vertices,
            pos,
            resize_radius,
            darken(color_y_base, active),
        );
        append_sphere(
            &mut vertices,
            pos,
            inner_resize_radius,
            darken(inner_color, active),
        );
    }

    // Z resize
    for neg in [false, true] {
        let variant = if neg {
            GizmoPart::ResizeZNeg
        } else {
            GizmoPart::ResizeZ
        };
        let active = active_part == Some(variant);
        let z = if neg {
            position[2] - resize_radius
        } else {
            position[2] + size[2] + resize_radius
        };
        let pos = [center[0], center[1], z];
        append_sphere(
            &mut vertices,
            pos,
            resize_radius,
            darken(color_z_base, active),
        );
        append_sphere(
            &mut vertices,
            pos,
            inner_resize_radius,
            darken(inner_color, active),
        );
    }

    vertices
}

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
    let color_top = [0.45, 0.9, 1.0, 1.0];
    let color_side = [0.25, 0.75, 0.9, 1.0];

    // Edges along X
    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            &mut vertices,
            [x0, y - thickness, z - thickness],
            [x1, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Y
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0, z - thickness],
            [x + thickness, y1, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Z
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0],
            [x + thickness, y + thickness, z1],
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
            [x0, y - thickness, z - thickness],
            [x1, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0, z - thickness],
            [x + thickness, y1, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0],
            [x + thickness, y + thickness, z1],
            color_top,
            color_side,
        );
    }

    vertices
}

pub(crate) fn build_editor_preview_player_vertices(
    position: [f32; 3],
    direction: crate::types::SpawnDirection,
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
