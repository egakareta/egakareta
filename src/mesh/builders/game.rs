/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use crate::mesh::advanced_shapes::{append_cone, append_sphere};
use crate::mesh::obj::{resolve_obj_mesh, ObjMaterial, ObjMesh};
use crate::mesh::shapes::{append_prism, append_quad};
use crate::mesh::MeshGeometry;
use crate::types::{CameraTrigger, CameraTriggerMode, Direction, Vertex};
use glam::{EulerRot, Quat, Vec2, Vec3};

const GEM_SHATTER_DURATION_SECONDS: f32 = 0.48;
const GEM_SHATTER_SHARD_COUNT: usize = 14;
const PRACTICE_CHECKPOINT_FLAG_MESH: &str = "practice_checkpoint_flag.obj";
const PRACTICE_CHECKPOINT_FLAG_SCALE: f32 = 0.4;

pub(crate) struct GemShatterInstance {
    pub(crate) position: [f32; 3],
    pub(crate) size: [f32; 3],
    pub(crate) color_tint: [f32; 3],
    pub(crate) age_seconds: f32,
}

pub(crate) struct PracticeCheckpointFlagInstance {
    pub(crate) position: [f32; 3],
    pub(crate) direction: Direction,
    pub(crate) is_latest: bool,
}

pub(crate) struct TransformTriggerMarker {
    pub(crate) source_position: Option<[f32; 3]>,
    pub(crate) source_size: Option<[f32; 3]>,
    pub(crate) target_position: [f32; 3],
    pub(crate) target_rotation_degrees: [f32; 3],
    pub(crate) target_size: [f32; 3],
    pub(crate) time_seconds: f32,
    pub(crate) duration_seconds: f32,
    pub(crate) is_selected: bool,
}

pub(crate) fn gem_shatter_duration_seconds() -> f32 {
    GEM_SHATTER_DURATION_SECONDS
}

pub(crate) fn build_gem_shatter_vertices(effects: &[GemShatterInstance]) -> Vec<Vertex> {
    puffin::profile_scope!("BuildGemShatterVertices");
    let mut vertices = Vec::new();
    for effect in effects {
        append_gem_shatter_effect(&mut vertices, effect);
    }
    vertices
}

fn append_gem_shatter_effect(vertices: &mut Vec<Vertex>, effect: &GemShatterInstance) {
    let t = (effect.age_seconds / GEM_SHATTER_DURATION_SECONDS).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    let fade = (1.0 - t).clamp(0.0, 1.0);
    let center = Vec3::new(
        effect.position[0] + effect.size[0] * 0.5,
        effect.position[1] + effect.size[1] * 0.5,
        effect.position[2] + effect.size[2] * 0.5,
    );
    let base_radius = effect.size[0]
        .max(effect.size[1])
        .max(effect.size[2])
        .max(0.2)
        * 0.5;
    let base_color = [
        (0.25 + 0.75 * effect.color_tint[0]).clamp(0.0, 1.0),
        (0.55 + 0.45 * effect.color_tint[1]).clamp(0.0, 1.0),
        (0.95 * effect.color_tint[2]).clamp(0.2, 1.0),
        (0.9 * fade).clamp(0.0, 1.0),
    ];

    for shard in 0..GEM_SHATTER_SHARD_COUNT {
        let angle = shard as f32 * std::f32::consts::TAU / GEM_SHATTER_SHARD_COUNT as f32;
        let vertical_phase = ((shard * 37) % 11) as f32 / 10.0;
        let outward = Vec3::new(angle.cos(), vertical_phase * 0.9 + 0.25, angle.sin()).normalize();
        let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos()).normalize();
        let upish = outward.cross(tangent).normalize_or_zero();
        let burst = base_radius * (0.25 + ease * (1.55 + vertical_phase * 0.55));
        let gravity = Vec3::Y * (-0.55 * t * t * base_radius);
        let wobble =
            tangent * ((t * std::f32::consts::TAU + shard as f32).sin() * 0.08 * base_radius);
        let shard_center = center + outward * burst + gravity + wobble;
        let shard_size = base_radius * (0.1 + 0.04 * (shard % 3) as f32) * (1.0 - t * 0.35);
        let sparkle = if shard % 4 == 0 { 1.18 } else { 1.0 };
        let color = [
            (base_color[0] * sparkle).min(1.0),
            (base_color[1] * sparkle).min(1.0),
            (base_color[2] * sparkle).min(1.0),
            base_color[3],
        ];
        let p0 = (shard_center + tangent * shard_size * 1.35).to_array();
        let p1 = (shard_center - tangent * shard_size * 0.8 + upish * shard_size).to_array();
        let p2 =
            (shard_center - tangent * shard_size * 0.65 - upish * shard_size * 0.85).to_array();
        vertices.push(Vertex::untextured(p0, color));
        vertices.push(Vertex::untextured(p1, color));
        vertices.push(Vertex::untextured(p2, color));
    }
}

pub(crate) fn build_trail_vertices(points: &[[f32; 3]], game_over: bool) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTrailVertices");
    build_trail_vertices_internal(points, game_over, 1.0)
}

pub(crate) fn build_trail_vertices_with_alpha(
    points: &[[f32; 3]],
    game_over: bool,
    alpha: f32,
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTrailVerticesAlpha");
    build_trail_vertices_internal(points, game_over, alpha)
}

fn build_trail_vertices_internal(points: &[[f32; 3]], game_over: bool, alpha: f32) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTrailVerticesInternal");
    let mut trail_vertices = Vec::new();
    let width = 0.8;
    const GHOST_Y_BIAS_STEP: f32 = 0.0002;
    const GHOST_Y_BIAS_MAX: f32 = 0.003;
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
        let y_bias = if alpha < 0.999 {
            (i as f32 * GHOST_Y_BIAS_STEP).min(GHOST_Y_BIAS_MAX)
        } else {
            0.0
        };

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];

        if dx.abs() <= f32::EPSILON && dz.abs() <= f32::EPSILON {
            let x_min = p1[0] - width / 2.0;
            let x_max = p1[0] + width / 2.0;
            let z_min = p1[2] - width / 2.0;
            let z_max = p1[2] + width / 2.0;
            let y_base = p1[1].min(p2[1]) + y_bias;
            let y_top = p1[1].max(p2[1]) + width + y_bias;

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

        let y_offset = p1[1].min(p2[1]) + y_bias;
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
    puffin::profile_scope!("BuildSpawnMarkerVertices");
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

pub(crate) fn build_practice_checkpoint_flag_geometry(
    checkpoints: &[PracticeCheckpointFlagInstance],
) -> MeshGeometry {
    puffin::profile_scope!("BuildPracticeCheckpointFlags");
    let mut geometry = MeshGeometry::default();

    for checkpoint in checkpoints {
        append_practice_checkpoint_flag(&mut geometry, checkpoint);
    }

    geometry
}

fn append_practice_checkpoint_flag(
    geometry: &mut MeshGeometry,
    checkpoint: &PracticeCheckpointFlagInstance,
) {
    let Some(mesh) = resolve_obj_mesh(PRACTICE_CHECKPOINT_FLAG_MESH) else {
        return;
    };
    let flag_top = if checkpoint.is_latest {
        [1.0, 0.87, 0.24, 1.0]
    } else {
        [0.25, 0.78, 1.0, 0.92]
    };
    let flag_bottom = if checkpoint.is_latest {
        [1.0, 0.3, 0.42, 1.0]
    } else {
        [0.24, 0.42, 1.0, 0.78]
    };

    append_practice_checkpoint_flag_mesh(
        geometry,
        mesh,
        Vec3::from_array(checkpoint.position),
        checkpoint.direction,
        flag_top,
        flag_bottom,
    );
}

#[derive(Hash, PartialEq, Eq)]
struct PracticeCheckpointFlagVertexKey {
    position_index: usize,
    texcoord_index: Option<usize>,
    normal_index: Option<usize>,
    material_index: Option<usize>,
}

fn append_practice_checkpoint_flag_mesh(
    geometry: &mut MeshGeometry,
    mesh: &ObjMesh,
    anchor: Vec3,
    direction: Direction,
    flag_top: [f32; 4],
    flag_bottom: [f32; 4],
) {
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(mesh.faces.len() * 3);
    let mut lookup = HashMap::<PracticeCheckpointFlagVertexKey, u32>::new();

    for face in &mesh.faces {
        for corner in face {
            let key = PracticeCheckpointFlagVertexKey {
                position_index: corner.position_index,
                texcoord_index: corner.texcoord_index,
                normal_index: corner.normal_index,
                material_index: corner.material_index,
            };
            if let Some(index) = lookup.get(&key) {
                indices.push(*index);
                continue;
            }

            let Some(raw_position) = mesh.positions.get(corner.position_index) else {
                continue;
            };
            let material = corner
                .material_index
                .and_then(|index| mesh.materials.get(index));
            let color = practice_checkpoint_flag_material_color(material, flag_top, flag_bottom);
            let position = transform_practice_checkpoint_flag_position(
                Vec3::from_array(*raw_position),
                anchor,
                direction,
            );

            let Ok(index) = u32::try_from(vertices.len()) else {
                return;
            };
            vertices.push(Vertex::untextured(position.to_array(), color));
            lookup.insert(key, index);
            indices.push(index);
        }
    }

    geometry.append_indexed(vertices, &indices);
}

fn transform_practice_checkpoint_flag_position(
    local_position: Vec3,
    anchor: Vec3,
    direction: Direction,
) -> Vec3 {
    let oriented = match direction {
        Direction::Forward => local_position,
        Direction::Right => Vec3::new(local_position.z, local_position.y, -local_position.x),
    };
    anchor + (oriented * PRACTICE_CHECKPOINT_FLAG_SCALE) + Vec3::new(0.0, 1.0, 0.0)
}

fn practice_checkpoint_flag_material_color(
    material: Option<&ObjMaterial>,
    flag_top: [f32; 4],
    flag_bottom: [f32; 4],
) -> [f32; 4] {
    match material.map(|material| material.name.as_str()) {
        Some("pole") => [0.92, 0.83, 0.58, 1.0],
        Some("base") => [0.46, 0.35, 0.18, 1.0],
        Some("cap") => flag_top,
        Some("flag_band_top") => mix_practice_checkpoint_flag_color(flag_top, flag_bottom, 0.17),
        Some("flag_band_mid") => mix_practice_checkpoint_flag_color(flag_top, flag_bottom, 0.5),
        Some("flag_band_bottom") => mix_practice_checkpoint_flag_color(flag_top, flag_bottom, 0.83),
        _ => material
            .map(|material| material.color.diffuse)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]),
    }
}

fn mix_practice_checkpoint_flag_color(top: [f32; 4], bottom: [f32; 4], progress: f32) -> [f32; 4] {
    [
        top[0] + (bottom[0] - top[0]) * progress,
        top[1] + (bottom[1] - top[1]) * progress,
        top[2] + (bottom[2] - top[2]) * progress,
        top[3] + (bottom[3] - top[3]) * progress,
    ]
}

pub(crate) fn build_colored_tap_indicator_vertices(
    indicators: &[([f32; 3], [f32; 4])],
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTapIndicatorVertices");
    let mut vertices = Vec::new();
    let thickness = 0.05;
    let dash_len = 0.2;
    // Gaps will be (1.0 - 3*0.2) / 2 = 0.2

    for &(pos, color) in indicators {
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

pub(crate) fn build_tap_division_preview_vertices(
    previews: &[([f32; 3], [f32; 4])],
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTapDivisionPreviewVertices");
    build_tap_division_cross_vertices(previews, 0.18, 0.06)
}

pub(crate) fn build_tap_division_tap_marker_vertices(
    indicators: &[([f32; 3], [f32; 4])],
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTapDivisionTapMarkerVertices");
    build_tap_division_cross_vertices(indicators, 0.18, 0.045)
}

fn build_tap_division_cross_vertices(
    indicators: &[([f32; 3], [f32; 4])],
    inset: f32,
    thickness: f32,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    for &(pos, color) in indicators {
        let x_min = pos[0] + inset;
        let x_max = pos[0] + 1.0 - inset;
        let z_min = pos[2] + inset;
        let z_max = pos[2] + 1.0 - inset;
        let y = pos[1] + 0.1;

        append_flat_xz_segment(
            &mut vertices,
            Vec2::new(x_min, z_min),
            Vec2::new(x_max, z_max),
            y,
            thickness,
            color,
        );
        append_flat_xz_segment(
            &mut vertices,
            Vec2::new(x_min, z_max),
            Vec2::new(x_max, z_min),
            y,
            thickness,
            color,
        );
    }

    vertices
}

fn append_flat_xz_segment(
    vertices: &mut Vec<Vertex>,
    start: Vec2,
    end: Vec2,
    y: f32,
    thickness: f32,
    color: [f32; 4],
) {
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }

    let normal = Vec2::new(-delta.y, delta.x) / length * (thickness * 0.5);
    append_quad(
        vertices,
        [start.x + normal.x, y, start.y + normal.y],
        [end.x + normal.x, y, end.y + normal.y],
        [end.x - normal.x, y, end.y - normal.y],
        [start.x - normal.x, y, start.y - normal.y],
        color,
    );
}

pub(crate) fn build_camera_trigger_marker_vertices(
    camera_triggers: &[CameraTrigger],
    selected_index: Option<usize>,
    current_camera_eye: Option<Vec3>,
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildCameraTriggerMarkerVertices");
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

pub(crate) fn build_transform_trigger_marker_vertices(
    markers: &[TransformTriggerMarker],
    current_time_seconds: f32,
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildTransformTriggerMarkerVertices");
    let mut vertices = Vec::new();

    for marker in markers {
        append_transform_trigger_marker(&mut vertices, marker, current_time_seconds);
    }

    vertices
}

fn append_transform_trigger_marker(
    vertices: &mut Vec<Vertex>,
    marker: &TransformTriggerMarker,
    current_time_seconds: f32,
) {
    let source_center = marker
        .source_position
        .zip(marker.source_size)
        .map(|(pos, size)| object_center(pos, size));
    let target_center = object_center(marker.target_position, marker.target_size);
    let progress = transform_trigger_countdown_progress(
        marker.time_seconds,
        marker.duration_seconds,
        current_time_seconds,
    );

    let selected_boost = if marker.is_selected { 0.1 } else { 0.0 };
    let ring_radius = 0.46 - progress * 0.31 + selected_boost;
    let ring_color = if current_time_seconds >= marker.time_seconds {
        [1.0, 0.52, 0.18, 0.88]
    } else if marker.is_selected {
        [1.0, 0.84, 0.24, 0.96]
    } else {
        [0.64, 0.22, 1.0, 0.9]
    };

    if let Some(source_center) = source_center {
        append_xz_ring(
            vertices,
            source_center,
            ring_radius.max(0.12),
            0.055,
            ring_color,
        );

        let connector_color = [0.72, 0.42, 1.0, 0.72];
        append_cylinder_segment(
            vertices,
            source_center,
            target_center,
            0.035,
            connector_color,
        );
    }

    let rotation = transform_marker_rotation(marker.target_rotation_degrees);
    let forward = (rotation * Vec3::Z).normalize_or_zero();
    let arrow_direction = if forward.length_squared() > f32::EPSILON {
        forward
    } else {
        Vec3::Z
    };
    let target_extent = marker
        .target_size
        .iter()
        .copied()
        .fold(0.0_f32, f32::max)
        .max(0.75);
    let arrow_base = Vec3::from_array(target_center) - arrow_direction * (target_extent * 0.3);
    let arrow_shaft_end =
        Vec3::from_array(target_center) + arrow_direction * (target_extent * 0.42);
    let arrow_tip = arrow_shaft_end + arrow_direction * 0.45;
    let arrow_color = if marker.is_selected {
        [1.0, 0.8, 0.18, 0.96]
    } else {
        [0.22, 0.9, 1.0, 0.88]
    };

    append_cylinder_segment(
        vertices,
        arrow_base.to_array(),
        arrow_shaft_end.to_array(),
        0.055,
        arrow_color,
    );
    append_cone(
        vertices,
        arrow_shaft_end.to_array(),
        arrow_tip.to_array(),
        0.17,
        arrow_color,
    );
    append_oriented_box_edges(
        vertices,
        marker.target_position,
        marker.target_size,
        rotation,
        0.028,
        [0.18, 1.0, 0.62, 0.58],
    );
}

fn transform_trigger_countdown_progress(
    time_seconds: f32,
    duration_seconds: f32,
    current_time_seconds: f32,
) -> f32 {
    if current_time_seconds >= time_seconds {
        return 1.0;
    }

    let window_seconds = duration_seconds.max(1.0);
    let remaining = time_seconds - current_time_seconds;
    1.0 - (remaining / window_seconds).clamp(0.0, 1.0)
}

pub(crate) fn object_center(position: [f32; 3], size: [f32; 3]) -> [f32; 3] {
    [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ]
}

pub(crate) fn transform_marker_rotation(rotation_degrees: [f32; 3]) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        rotation_degrees[0].to_radians(),
        rotation_degrees[1].to_radians(),
        rotation_degrees[2].to_radians(),
    )
}

pub(crate) fn append_xz_ring(
    vertices: &mut Vec<Vertex>,
    center: [f32; 3],
    radius: f32,
    thickness: f32,
    color: [f32; 4],
) {
    let segments = 32;
    let outer_radius = radius + thickness * 0.5;
    let inner_radius = (radius - thickness * 0.5).max(0.02);
    let center = Vec3::from_array(center);

    for segment in 0..segments {
        let a0 = segment as f32 * std::f32::consts::TAU / segments as f32;
        let a1 = (segment + 1) as f32 * std::f32::consts::TAU / segments as f32;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let outer0 = center + Vec3::new(c0 * outer_radius, 0.0, s0 * outer_radius);
        let outer1 = center + Vec3::new(c1 * outer_radius, 0.0, s1 * outer_radius);
        let inner0 = center + Vec3::new(c0 * inner_radius, 0.0, s0 * inner_radius);
        let inner1 = center + Vec3::new(c1 * inner_radius, 0.0, s1 * inner_radius);
        append_quad(
            vertices,
            outer0.to_array(),
            outer1.to_array(),
            inner1.to_array(),
            inner0.to_array(),
            color,
        );
    }
}

pub(crate) fn append_oriented_box_edges(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    rotation: Quat,
    radius: f32,
    color: [f32; 4],
) {
    let center = Vec3::from_array(object_center(position, size));
    let half = Vec3::new(size[0] * 0.5, size[1] * 0.5, size[2] * 0.5);
    let corners = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ]
    .map(|corner| center + rotation * corner);
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    for (start, end) in edges {
        append_cylinder_segment(
            vertices,
            corners[start].to_array(),
            corners[end].to_array(),
            radius,
            color,
        );
    }
}

pub(crate) fn append_cylinder_segment(
    vertices: &mut Vec<Vertex>,
    start: [f32; 3],
    end: [f32; 3],
    radius: f32,
    color: [f32; 4],
) {
    let start = Vec3::from_array(start);
    let end = Vec3::from_array(end);
    let axis = end - start;
    let length = axis.length();
    if length <= f32::EPSILON {
        return;
    }

    let forward = axis / length;
    let arbitrary = if forward.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let right = forward.cross(arbitrary).normalize_or_zero();
    if right.length_squared() <= f32::EPSILON {
        return;
    }
    let up = right.cross(forward).normalize_or_zero();
    let segments = 12;

    for segment in 0..segments {
        let a0 = segment as f32 * std::f32::consts::TAU / segments as f32;
        let a1 = (segment + 1) as f32 * std::f32::consts::TAU / segments as f32;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let offset0 = (right * c0 + up * s0) * radius;
        let offset1 = (right * c1 + up * s1) * radius;

        append_quad(
            vertices,
            (start + offset0).to_array(),
            (end + offset0).to_array(),
            (end + offset1).to_array(),
            (start + offset1).to_array(),
            color,
        );
    }
}

pub(crate) fn build_camera_arrow_vertices(
    eye: [f32; 3],
    forward: [f32; 3],
    editor_camera_eye: [f32; 3],
) -> Vec<Vertex> {
    puffin::profile_scope!("BuildCameraArrowVertices");
    const BASE_ALPHA: f32 = 0.55;
    const FADE_START_DISTANCE: f32 = 3.0;
    const FADE_END_DISTANCE: f32 = 1.0;

    let dist = {
        let dx = editor_camera_eye[0] - eye[0];
        let dy = editor_camera_eye[1] - eye[1];
        let dz = editor_camera_eye[2] - eye[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    };
    let alpha = if dist >= FADE_START_DISTANCE {
        BASE_ALPHA
    } else if dist <= FADE_END_DISTANCE {
        0.0
    } else {
        BASE_ALPHA * (dist - FADE_END_DISTANCE) / (FADE_START_DISTANCE - FADE_END_DISTANCE)
    };
    let arrow_color: [f32; 4] = [0.8, 0.25, 0.35, alpha];

    let mut vertices = Vec::new();
    if alpha <= f32::EPSILON {
        return vertices;
    }
    let shaft_start = [
        eye[0] + forward[0],
        eye[1] + forward[1],
        eye[2] + forward[2],
    ];
    append_cone(&mut vertices, eye, shaft_start, 0.5, arrow_color);

    vertices
}

#[cfg(test)]
mod tests {
    use super::{
        build_gem_shatter_vertices, build_practice_checkpoint_flag_geometry,
        build_tap_division_preview_vertices, build_tap_division_tap_marker_vertices,
        build_trail_vertices, build_trail_vertices_with_alpha,
        build_transform_trigger_marker_vertices, gem_shatter_duration_seconds, GemShatterInstance,
        PracticeCheckpointFlagInstance, TransformTriggerMarker,
    };
    use crate::types::{Direction, Vertex};

    const PRISM_VERTEX_COUNT: usize = 36;

    fn approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {a} to be within {eps} of {b} (delta: {})",
            (a - b).abs()
        );
    }

    fn segment_y_bounds(vertices: &[Vertex]) -> Vec<(f32, f32)> {
        assert_eq!(
            vertices.len() % PRISM_VERTEX_COUNT,
            0,
            "trail should be composed of full prism segments"
        );

        vertices
            .chunks(PRISM_VERTEX_COUNT)
            .map(|segment| {
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for vertex in segment {
                    min_y = min_y.min(vertex.position[1]);
                    max_y = max_y.max(vertex.position[1]);
                }
                (min_y, max_y)
            })
            .collect()
    }

    #[test]
    fn gem_shatter_vertices_emit_fading_shards() {
        let fresh = build_gem_shatter_vertices(&[GemShatterInstance {
            position: [1.0, 2.0, 3.0],
            size: [0.72, 0.86, 0.72],
            color_tint: [0.8, 0.9, 1.0],
            age_seconds: 0.0,
        }]);
        let fading = build_gem_shatter_vertices(&[GemShatterInstance {
            position: [1.0, 2.0, 3.0],
            size: [0.72, 0.86, 0.72],
            color_tint: [0.8, 0.9, 1.0],
            age_seconds: gem_shatter_duration_seconds() * 0.75,
        }]);

        assert_eq!(fresh.len(), 14 * 3);
        assert_eq!(fading.len(), fresh.len());
        assert!(fresh.iter().all(|vertex| vertex.color[3] > 0.8));
        assert!(fading.iter().all(|vertex| vertex.color[3] < 0.3));
    }

    #[test]
    fn practice_checkpoint_flags_use_compact_indexed_geometry() {
        let geometry = build_practice_checkpoint_flag_geometry(&[
            PracticeCheckpointFlagInstance {
                position: [2.0, 1.0, 3.0],
                direction: Direction::Forward,
                is_latest: false,
            },
            PracticeCheckpointFlagInstance {
                position: [5.0, 2.0, 6.0],
                direction: Direction::Right,
                is_latest: true,
            },
        ]);

        assert!(!geometry.vertices.is_empty());
        assert!(geometry
            .indices
            .as_ref()
            .is_some_and(|indices| !indices.is_empty()));
        assert!(geometry.vertex_count() < 160);
        assert!(geometry.draw_count() < 900);
        let max_latest_flag_y = geometry
            .vertices
            .iter()
            .filter(|vertex| vertex.color[0] > 0.8)
            .map(|vertex| vertex.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_latest_flag_y > 3.9);
        assert!(max_latest_flag_y < 4.1);
    }

    #[test]
    fn transform_trigger_marker_contains_countdown_ring_connector_arrow_and_scale_cage() {
        let marker = TransformTriggerMarker {
            source_position: Some([0.0, 0.0, 0.0]),
            source_size: Some([1.0, 1.0, 1.0]),
            target_position: [3.0, 0.0, 2.0],
            target_rotation_degrees: [0.0, 90.0, 0.0],
            target_size: [2.0, 1.5, 0.75],
            time_seconds: 4.0,
            duration_seconds: 2.0,
            is_selected: false,
        };
        let early = build_transform_trigger_marker_vertices(std::slice::from_ref(&marker), 2.0);
        let late = build_transform_trigger_marker_vertices(std::slice::from_ref(&marker), 4.0);

        assert!(!early.is_empty());
        assert!(!late.is_empty());
        assert_eq!(early.len(), late.len());

        let source_center = [0.5, 0.5, 0.5];
        let ring_outer_radius = |vertices: &[Vertex]| {
            vertices
                .iter()
                .take(32 * 6)
                .map(|vertex| {
                    let dx = vertex.position[0] - source_center[0];
                    let dz = vertex.position[2] - source_center[2];
                    (dx * dx + dz * dz).sqrt()
                })
                .fold(0.0_f32, f32::max)
        };
        assert!(ring_outer_radius(&late) < ring_outer_radius(&early));

        let max_target_x = early
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_target_x = early
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        assert!(max_target_x > 4.2);
        assert!(min_target_x < 0.4);
    }

    #[test]
    fn trail_vertices_empty_for_short_paths() {
        let empty: [[f32; 3]; 0] = [];
        assert!(build_trail_vertices(&empty, false).is_empty());

        let one = [[0.0, 0.0, 0.0]];
        assert!(build_trail_vertices(&one, false).is_empty());
    }

    #[test]
    fn trail_emits_one_prism_per_segment() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ];

        let opaque = build_trail_vertices(&points, false);
        let translucent = build_trail_vertices_with_alpha(&points, false, 0.5);

        assert_eq!(opaque.len(), (points.len() - 1) * PRISM_VERTEX_COUNT);
        assert_eq!(translucent.len(), (points.len() - 1) * PRISM_VERTEX_COUNT);
    }

    #[test]
    fn translucent_trail_staggers_segment_heights_through_turns() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ];

        let vertices = build_trail_vertices_with_alpha(&points, false, 0.45);
        let y_bounds = segment_y_bounds(&vertices);

        assert_eq!(y_bounds.len(), 3);
        for i in 1..y_bounds.len() {
            assert!(
                y_bounds[i].0 > y_bounds[i - 1].0,
                "segment {} base y ({}) should be above previous segment base y ({})",
                i,
                y_bounds[i].0,
                y_bounds[i - 1].0
            );
            assert!(
                y_bounds[i].1 > y_bounds[i - 1].1,
                "segment {} top y ({}) should be above previous segment top y ({})",
                i,
                y_bounds[i].1,
                y_bounds[i - 1].1
            );
        }
    }

    #[test]
    fn opaque_trail_keeps_segments_coplanar() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ];

        let vertices = build_trail_vertices(&points, false);
        let y_bounds = segment_y_bounds(&vertices);
        let (first_min_y, first_max_y) = y_bounds[0];

        for (min_y, max_y) in y_bounds.iter().copied().skip(1) {
            approx_eq(min_y, first_min_y, 1e-6);
            approx_eq(max_y, first_max_y, 1e-6);
        }
    }

    #[test]
    fn translucent_trail_bias_caps_for_long_paths() {
        let points: Vec<[f32; 3]> = (0..40).map(|i| [i as f32, 0.0, 0.0]).collect();

        let vertices = build_trail_vertices_with_alpha(&points, false, 0.6);
        let y_bounds = segment_y_bounds(&vertices);
        let first_min_y = y_bounds.first().expect("expected first segment").0;
        let last_min_y = y_bounds.last().expect("expected last segment").0;

        approx_eq(last_min_y - first_min_y, 0.003, 1e-6);
        for (min_y, _) in y_bounds.iter().copied().skip(16) {
            approx_eq(min_y, 0.003, 1e-6);
        }
    }

    #[test]
    fn translucent_trail_alpha_is_clamped() {
        let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];

        let low = build_trail_vertices_with_alpha(&points, false, -1.0);
        assert!(!low.is_empty());
        assert!(low.iter().all(|v| v.color[3] <= f32::EPSILON));

        let high = build_trail_vertices_with_alpha(&points, false, 5.0);
        assert!(!high.is_empty());
        assert!(high
            .iter()
            .all(|v| (v.color[3] - 1.0).abs() <= f32::EPSILON));
    }

    #[test]
    fn full_alpha_builder_matches_opaque_builder() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ];

        let opaque = build_trail_vertices(&points, false);
        let full_alpha = build_trail_vertices_with_alpha(&points, false, 1.0);

        assert_eq!(opaque.len(), full_alpha.len());
        for (a, b) in opaque.iter().zip(full_alpha.iter()) {
            for i in 0..3 {
                approx_eq(a.position[i], b.position[i], 1e-6);
            }
            for i in 0..4 {
                approx_eq(a.color[i], b.color[i], 1e-6);
            }
        }
    }

    #[test]
    fn game_over_trail_uses_red_palette() {
        let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];

        let normal = build_trail_vertices(&points, false);
        let game_over = build_trail_vertices(&points, true);

        assert!(!normal.is_empty());
        assert!(!game_over.is_empty());

        let normal_first = normal[0].color;
        let game_over_first = game_over[0].color;

        assert!(
            game_over_first[0] > normal_first[0],
            "game over trail should be redder than normal trail"
        );
        assert!(
            game_over_first[1] < normal_first[1],
            "game over trail should reduce green channel"
        );
        assert!(
            game_over_first[2] < normal_first[2],
            "game over trail should reduce blue channel"
        );
    }

    #[test]
    fn tap_division_preview_vertices_build_translucent_x_crosses() {
        let color = [0.05, 0.48, 0.95, 0.24];
        let vertices = build_tap_division_preview_vertices(&[([2.0, 1.0, 3.0], color)]);

        assert_eq!(vertices.len(), 12);
        assert!(vertices.iter().all(|vertex| vertex.color == color));
        assert!(vertices
            .iter()
            .all(|vertex| (vertex.position[1] - 1.1).abs() <= f32::EPSILON));

        let min_x = vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_z = vertices
            .iter()
            .map(|vertex| vertex.position[2])
            .fold(f32::INFINITY, f32::min);
        let max_z = vertices
            .iter()
            .map(|vertex| vertex.position[2])
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(min_x > 2.0);
        assert!(max_x < 3.0);
        assert!(min_z > 3.0);
        assert!(max_z < 4.0);
    }

    #[test]
    fn tap_division_tap_marker_vertices_build_smaller_opaque_crosses() {
        let color = [0.0, 0.0, 0.0, 1.0];
        let preview_vertices = build_tap_division_preview_vertices(&[([2.0, 1.0, 3.0], color)]);
        let marker_vertices = build_tap_division_tap_marker_vertices(&[([2.0, 1.0, 3.0], color)]);

        assert_eq!(marker_vertices.len(), 12);
        assert!(marker_vertices.iter().all(|vertex| vertex.color == color));

        let preview_width = preview_vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - preview_vertices
                .iter()
                .map(|vertex| vertex.position[0])
                .fold(f32::INFINITY, f32::min);
        let marker_width = marker_vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - marker_vertices
                .iter()
                .map(|vertex| vertex.position[0])
                .fold(f32::INFINITY, f32::min);

        assert!(marker_width < preview_width);
    }
}
