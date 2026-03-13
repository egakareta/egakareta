use crate::mesh::advanced_shapes::{append_cone, append_sphere};
use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

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
    show_move_handles: bool,
    show_scale_handles: bool,
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

    if show_move_handles {
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
    }

    // Resize handles
    let resize_radius = 0.25;
    let inner_resize_radius = 0.1;
    let inner_color = [0.0, 0.0, 0.0, 0.025];

    if show_scale_handles {
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
    }

    vertices
}
