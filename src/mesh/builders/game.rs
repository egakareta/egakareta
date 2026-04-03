/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::mesh::advanced_shapes::{append_cone, append_sphere};
use crate::mesh::shapes::{append_prism, append_quad};
use crate::types::{CameraTrigger, CameraTriggerMode, Vertex};
use glam::Vec3;

pub(crate) fn build_trail_vertices(points: &[[f32; 3]], game_over: bool) -> Vec<Vertex> {
    build_trail_vertices_internal(points, game_over, 1.0)
}

pub(crate) fn build_trail_vertices_with_alpha(
    points: &[[f32; 3]],
    game_over: bool,
    alpha: f32,
) -> Vec<Vertex> {
    build_trail_vertices_internal(points, game_over, alpha)
}

fn build_trail_vertices_internal(points: &[[f32; 3]], game_over: bool, alpha: f32) -> Vec<Vertex> {
    let mut trail_vertices = Vec::new();
    let width = 0.8;
    let alpha = alpha.clamp(0.0, 1.0);
    let c_top = if game_over {
        [1.0, 0.2, 0.2, alpha]
    } else {
        [0.8, 0.25, 0.35, alpha]
    };
    let c_side = if game_over {
        [0.8, 0.1, 0.1, alpha]
    } else {
        [0.7, 0.2, 0.3, alpha]
    };

    if points.len() < 2 {
        return trail_vertices;
    }

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];

        if dx.abs() <= f32::EPSILON && dz.abs() <= f32::EPSILON {
            let x_min = p1[0] - width / 2.0;
            let x_max = p1[0] + width / 2.0;
            let z_min = p1[2] - width / 2.0;
            let z_max = p1[2] + width / 2.0;
            let y_base = p1[1].min(p2[1]);
            let y_top = p1[1].max(p2[1]) + width;

            append_prism(
                &mut trail_vertices,
                [x_min, y_base, z_min],
                [x_max, y_top, z_max],
                c_top,
                c_side,
            );
            continue;
        }

        let (x_min, x_max, z_min, z_max) = if dx.abs() > dz.abs() {
            (
                p1[0].min(p2[0]) - width / 2.0,
                p1[0].max(p2[0]) + width / 2.0,
                p1[2] - width / 2.0,
                p1[2] + width / 2.0,
            )
        } else {
            (
                p1[0] - width / 2.0,
                p1[0] + width / 2.0,
                p1[2].min(p2[2]) - width / 2.0,
                p1[2].max(p2[2]) + width / 2.0,
            )
        };

        let y_offset = p1[1].min(p2[1]);
        let y_extra = dy.abs() * 0.5;
        let y_min = y_offset;
        let y_max = y_offset + width + y_extra;

        append_prism(
            &mut trail_vertices,
            [x_min, y_min, z_min],
            [x_max, y_max, z_max],
            c_top,
            c_side,
        );
    }

    trail_vertices
}

pub(crate) fn build_spawn_marker_vertices(position: [f32; 3], faces_right: bool) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let x = position[0];
    let y = position[1];
    let z = position[2];

    append_prism(
        &mut vertices,
        [x + 0.1, y, z + 0.1],
        [x + 0.9, y + 0.5, z + 0.9],
        [0.25, 0.95, 0.35, 1.0],
        [0.1, 0.45, 0.15, 1.0],
    );

    if faces_right {
        append_prism(
            &mut vertices,
            [x + 0.9, y, z + 0.35],
            [x + 1.3, y + 0.7, z + 0.65],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    } else {
        append_prism(
            &mut vertices,
            [x + 0.35, y, z + 0.9],
            [x + 0.65, y + 0.7, z + 1.3],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    }

    vertices
}

pub(crate) fn build_tap_indicator_vertices(positions: &[[f32; 3]]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color = [0.0, 0.0, 0.0, 1.0]; // Black
    let thickness = 0.05;
    let dash_len = 0.2;
    // Gaps will be (1.0 - 3*0.2) / 2 = 0.2

    for &pos in positions {
        let x_min = pos[0];
        let x_max = x_min + 1.0;
        let z_min = pos[2];
        let z_max = z_min + 1.0;
        let y = pos[1] + 0.1; // 0.1 above ground

        let starts = [0.0, 0.4, 0.8];

        for &start in &starts {
            let end = start + dash_len;

            // Bottom edge
            append_quad(
                &mut vertices,
                [x_min + start, y, z_min],
                [x_min + end, y, z_min],
                [x_min + end, y, z_min + thickness],
                [x_min + start, y, z_min + thickness],
                color,
            );

            // Top edge
            append_quad(
                &mut vertices,
                [x_min + start, y, z_max - thickness],
                [x_min + end, y, z_max - thickness],
                [x_min + end, y, z_max],
                [x_min + start, y, z_max],
                color,
            );

            // Left edge
            append_quad(
                &mut vertices,
                [x_min, y, z_min + start],
                [x_min + thickness, y, z_min + start],
                [x_min + thickness, y, z_min + end],
                [x_min, y, z_min + end],
                color,
            );

            // Right edge
            append_quad(
                &mut vertices,
                [x_max - thickness, y, z_min + start],
                [x_max, y, z_min + start],
                [x_max, y, z_min + end],
                [x_max - thickness, y, z_min + end],
                color,
            );
        }
    }
    vertices
}

pub(crate) fn build_camera_trigger_marker_vertices(
    camera_triggers: &[CameraTrigger],
    selected_index: Option<usize>,
    current_camera_eye: Option<Vec3>,
) -> Vec<Vertex> {
    const CAMERA_BASE_DISTANCE: f32 = 24.0;
    const HIDE_DISTANCE_SQUARED: f32 = 0.5 * 0.5;

    let mut vertices = Vec::new();

    for (index, camera_trigger) in camera_triggers.iter().enumerate() {
        let is_selected = selected_index == Some(index);
        let distance = CAMERA_BASE_DISTANCE;

        let (sin_rotation, cos_rotation) = camera_trigger.rotation.sin_cos();
        let (sin_pitch, cos_pitch) = camera_trigger.pitch.sin_cos();

        // Mirrors the editor camera pose: camera triggers are rendered at camera eye position.
        let offset = [
            -cos_pitch * sin_rotation * distance,
            sin_pitch * distance,
            -cos_pitch * cos_rotation * distance,
        ];
        let eye = [
            camera_trigger.target_position[0] + offset[0],
            camera_trigger.target_position[1] + offset[1],
            camera_trigger.target_position[2] + offset[2],
        ];

        // Skip rendering if the camera is inside the trigger marker.
        if let Some(cam_eye) = current_camera_eye {
            let camera_trigger_eye_vec = Vec3::from_array(eye);
            if cam_eye.distance_squared(camera_trigger_eye_vec) < HIDE_DISTANCE_SQUARED {
                continue;
            }
        }

        let forward = if distance > f32::EPSILON {
            [
                -offset[0] / distance,
                -offset[1] / distance,
                -offset[2] / distance,
            ]
        } else {
            [0.0, 0.0, 1.0]
        };

        let (ball_color, arrow_color) = if is_selected {
            ([1.0, 0.9, 0.25, 0.95], [1.0, 0.8, 0.1, 0.95])
        } else if matches!(camera_trigger.mode, CameraTriggerMode::Follow) {
            ([0.95, 0.4, 0.2, 0.85], [1.0, 0.55, 0.25, 0.9])
        } else {
            ([0.2, 0.75, 1.0, 0.85], [0.25, 0.85, 1.0, 0.9])
        };

        let ball_radius = if is_selected { 0.42 } else { 0.34 };
        append_sphere(&mut vertices, eye, ball_radius, ball_color);

        let shaft_start = [
            eye[0] + forward[0] * (ball_radius * 1.05),
            eye[1] + forward[1] * (ball_radius * 1.05),
            eye[2] + forward[2] * (ball_radius * 1.05),
        ];
        let shaft_end = [
            shaft_start[0] + forward[0] * 1.2,
            shaft_start[1] + forward[1] * 1.2,
            shaft_start[2] + forward[2] * 1.2,
        ];
        let tip = [
            shaft_end[0] + forward[0] * 0.6,
            shaft_end[1] + forward[1] * 0.6,
            shaft_end[2] + forward[2] * 0.6,
        ];

        append_cone(&mut vertices, shaft_start, shaft_end, 0.09, arrow_color);
        append_cone(&mut vertices, shaft_end, tip, 0.22, arrow_color);
    }

    vertices
}
