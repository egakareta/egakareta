use crate::mesh::advanced_shapes::{append_cone, append_sphere};
use crate::mesh::shapes::append_prism;
use crate::types::{GizmoPart, Vertex};

pub(crate) struct GizmoParams {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub axis_lengths: [f32; 3],
    pub axis_width: f32,
    pub resize_radius: f32,
    pub resize_offsets: [f32; 3],
    pub show_move_handles: bool,
    pub show_scale_handles: bool,
    pub show_rotate_handles: bool,
    pub hovered_part: Option<GizmoPart>,
    pub dragged_part: Option<GizmoPart>,
}

pub(crate) fn build_editor_gizmo_vertices(
    GizmoParams {
        position,
        size,
        axis_lengths,
        axis_width,
        resize_radius,
        resize_offsets,
        show_move_handles,
        show_scale_handles,
        show_rotate_handles,
        hovered_part,
        dragged_part,
    }: GizmoParams,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];

    let color_x_normal = [0.804, 0.0, 0.0, 0.6];
    let color_x_active = [1.0, 0.0, 0.0, 1.0];
    let color_y_normal = [0.0, 0.804, 0.0, 0.6];
    let color_y_active = [0.0, 1.0, 0.0, 1.0];
    let color_z_normal = [0.0, 0.0, 0.6, 0.6];
    let color_z_active = [0.0, 0.0, 1.0, 1.0];

    if show_move_handles {
        // X move arms
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::MoveXNeg
            } else {
                GizmoPart::MoveX
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let width_mult = if is_hovered && !is_dragged { 1.35 } else { 1.0 };

            let base_shaft = axis_width.max(0.0005) * 0.5;
            let base_tip_length = base_shaft * 10.0;
            let arm_start_offset = base_shaft * 6.0;

            let shaft = base_shaft * width_mult;
            let tip_length = shaft * 10.0;
            let cone_radius = shaft * 2.5;

            let base_x_length = axis_lengths[0].max(arm_start_offset + base_tip_length);
            let shaft_length = base_x_length - arm_start_offset - base_tip_length;
            let x_length = arm_start_offset + shaft_length + tip_length;

            let color = if active {
                color_x_active
            } else {
                color_x_normal
            };
            let color_dark = [
                color[0] * 0.8,
                color[1] * 0.8,
                color[2] * 0.8,
                color[3] * 0.8,
            ];

            let sign = if neg { -1.0 } else { 1.0 };
            let origin = center[0];
            let start = origin + arm_start_offset * sign;
            let end = origin + x_length * sign;
            let (p_min_x, p_max_x) = if neg {
                (end + tip_length, start)
            } else {
                (start, end - tip_length)
            };
            append_prism(
                &mut vertices,
                [p_min_x, center[1] - shaft, center[2] - shaft],
                [p_max_x, center[1] + shaft, center[2] + shaft],
                color,
                color_dark,
            );
            append_cone(
                &mut vertices,
                [end - tip_length * sign, center[1], center[2]],
                [end, center[1], center[2]],
                cone_radius,
                color,
            );
        }

        // Y move arms
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::MoveYNeg
            } else {
                GizmoPart::MoveY
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let width_mult = if is_hovered && !is_dragged { 1.35 } else { 1.0 };

            let base_shaft = axis_width.max(0.0005) * 0.5;
            let base_tip_length = base_shaft * 10.0;
            let arm_start_offset = base_shaft * 6.0;

            let shaft = base_shaft * width_mult;
            let tip_length = shaft * 10.0;
            let cone_radius = shaft * 2.5;

            let base_y_length = axis_lengths[1].max(arm_start_offset + base_tip_length);
            let shaft_length = base_y_length - arm_start_offset - base_tip_length;
            let y_length = arm_start_offset + shaft_length + tip_length;

            let color = if active {
                color_y_active
            } else {
                color_y_normal
            };
            let color_dark = [
                color[0] * 0.8,
                color[1] * 0.8,
                color[2] * 0.8,
                color[3] * 0.8,
            ];

            let sign = if neg { -1.0 } else { 1.0 };
            let origin = center[1];
            let start = origin + arm_start_offset * sign;
            let end = origin + y_length * sign;
            let (p_min_y, p_max_y) = if neg {
                (end + tip_length, start)
            } else {
                (start, end - tip_length)
            };
            append_prism(
                &mut vertices,
                [center[0] - shaft, p_min_y, center[2] - shaft],
                [center[0] + shaft, p_max_y, center[2] + shaft],
                color,
                color_dark,
            );
            append_cone(
                &mut vertices,
                [center[0], end - tip_length * sign, center[2]],
                [center[0], end, center[2]],
                cone_radius,
                color,
            );
        }

        // Z move arms
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::MoveZNeg
            } else {
                GizmoPart::MoveZ
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let width_mult = if is_hovered && !is_dragged { 1.35 } else { 1.0 };

            let base_shaft = axis_width.max(0.0005) * 0.5;
            let base_tip_length = base_shaft * 10.0;
            let arm_start_offset = base_shaft * 6.0;

            let shaft = base_shaft * width_mult;
            let tip_length = shaft * 10.0;
            let cone_radius = shaft * 2.5;

            let base_z_length = axis_lengths[2].max(arm_start_offset + base_tip_length);
            let shaft_length = base_z_length - arm_start_offset - base_tip_length;
            let z_length = arm_start_offset + shaft_length + tip_length;

            let color = if active {
                color_z_active
            } else {
                color_z_normal
            };
            let color_dark = [
                color[0] * 0.8,
                color[1] * 0.8,
                color[2] * 0.8,
                color[3] * 0.8,
            ];

            let sign = if neg { -1.0 } else { 1.0 };
            let origin = center[2];
            let start = origin + arm_start_offset * sign;
            let end = origin + z_length * sign;
            let (p_min_z, p_max_z) = if neg {
                (end + tip_length, start)
            } else {
                (start, end - tip_length)
            };
            append_prism(
                &mut vertices,
                [center[0] - shaft, center[1] - shaft, p_min_z],
                [center[0] + shaft, center[1] + shaft, p_max_z],
                color,
                color_dark,
            );
            append_cone(
                &mut vertices,
                [center[0], center[1], end - tip_length * sign],
                [center[0], center[1], end],
                cone_radius,
                color,
            );
        }
    }

    // Resize handles
    let inner_resize_radius = resize_radius * 0.4;
    let inner_color = [0.0, 0.0, 0.0, 0.025];

    if show_scale_handles {
        // X resize
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::ResizeXNeg
            } else {
                GizmoPart::ResizeX
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let current_radius = if is_hovered && !is_dragged {
                resize_radius * 1.35
            } else {
                resize_radius
            };

            let color = if active {
                color_x_active
            } else {
                color_x_normal
            };

            let x = if neg {
                position[0] - resize_offsets[0]
            } else {
                position[0] + size[0] + resize_offsets[0]
            };
            let pos = [x, center[1], center[2]];
            append_sphere(&mut vertices, pos, current_radius, color);
            append_sphere(&mut vertices, pos, inner_resize_radius, inner_color);
        }

        // Y resize
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::ResizeYNeg
            } else {
                GizmoPart::ResizeY
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let current_radius = if is_hovered && !is_dragged {
                resize_radius * 1.35
            } else {
                resize_radius
            };

            let color = if active {
                color_y_active
            } else {
                color_y_normal
            };

            let y = if neg {
                position[1] - resize_offsets[1]
            } else {
                position[1] + size[1] + resize_offsets[1]
            };
            let pos = [center[0], y, center[2]];
            append_sphere(&mut vertices, pos, current_radius, color);
            append_sphere(&mut vertices, pos, inner_resize_radius, inner_color);
        }

        // Z resize
        for neg in [false, true] {
            let variant = if neg {
                GizmoPart::ResizeZNeg
            } else {
                GizmoPart::ResizeZ
            };
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let current_radius = if is_hovered && !is_dragged {
                resize_radius * 1.35
            } else {
                resize_radius
            };

            let color = if active {
                color_z_active
            } else {
                color_z_normal
            };

            let z = if neg {
                position[2] - resize_offsets[2]
            } else {
                position[2] + size[2] + resize_offsets[2]
            };
            let pos = [center[0], center[1], z];
            append_sphere(&mut vertices, pos, current_radius, color);
            append_sphere(&mut vertices, pos, inner_resize_radius, inner_color);
        }
    }

    if show_rotate_handles {
        let rotate_radius = resize_radius * 1.2;
        let inner_rotate_radius = rotate_radius * 0.5;
        let rotate_offset = [
            axis_lengths[0] * 1.3,
            axis_lengths[1] * 1.3,
            axis_lengths[2] * 1.3,
        ];

        for (variant, pos, normal, active_color) in [
            (
                GizmoPart::RotateX,
                [center[0] + rotate_offset[0], center[1], center[2]],
                color_x_normal,
                color_x_active,
            ),
            (
                GizmoPart::RotateY,
                [center[0], center[1] + rotate_offset[1], center[2]],
                color_y_normal,
                color_y_active,
            ),
            (
                GizmoPart::RotateZ,
                [center[0], center[1], center[2] + rotate_offset[2]],
                color_z_normal,
                color_z_active,
            ),
        ] {
            let is_hovered = hovered_part == Some(variant);
            let is_dragged = dragged_part == Some(variant);
            let active = is_hovered || is_dragged;
            let current_radius = if is_hovered && !is_dragged {
                rotate_radius * 1.25
            } else {
                rotate_radius
            };
            let color = if active { active_color } else { normal };
            append_sphere(&mut vertices, pos, current_radius, color);
            append_sphere(&mut vertices, pos, inner_rotate_radius, inner_color);
        }
    }

    vertices
}
