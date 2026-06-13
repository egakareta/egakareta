/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use rayon::prelude::*;

use crate::block_geometry::visual_cuboids;
use crate::block_repository::{
    resolve_block_definition, resolve_block_texture_layers, BlockRenderProfile,
};
use crate::mesh::advanced_shapes::append_cone;
use crate::mesh::builders::game::{
    append_cylinder_segment, append_oriented_box_edges, append_xz_ring, object_center,
    transform_marker_rotation,
};
use crate::mesh::egmesh::{append_egmesh_geometry, resolve_egmesh};
use crate::mesh::geometry::MeshGeometry;
use crate::mesh::noise::pseudo_random_noise;
use crate::mesh::obj::{append_obj_mesh, resolve_obj_mesh};
use crate::mesh::shapes::{append_prism_with_layers, PrismFaceColors, PrismTextureLayers};
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::platform::parallel::rayon_is_ready;
use crate::triggers::{
    camera_trigger_eye_from_object, camera_trigger_forward_from_rotation_degrees, TimedTrigger,
    TimedTriggerAction, TimedTriggerTarget,
};
use crate::types::{LevelObject, Vertex};
use glam::Vec3;

const LIQUID_PROFILE_TAG: f32 = 1.0;
const GEM_PROFILE_TAG: f32 = 4.0;
const CAMERA_TRIGGER_PROFILE_TAG: f32 = 6.0;
const PARALLEL_GEOMETRY_OBJECT_THRESHOLD: usize = 256;
const PARALLEL_CHUNK_SIZE: usize = 128;
const VERTICES_PER_PRISM: usize = 36;

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    puffin::profile_scope!("BuildBlockVertices");
    build_block_geometry_from_slice(objects, None).to_triangle_vertices()
}

pub(crate) fn build_block_geometry(objects: &[LevelObject]) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometry");
    build_block_geometry_from_slice(objects, None)
}

pub(crate) fn build_block_geometry_at_time(
    objects: &[LevelObject],
    current_time_seconds: f32,
) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryAtTime");
    build_block_geometry_from_slice(objects, Some(current_time_seconds))
}

pub(crate) fn build_block_geometry_from_refs(objects: &[&LevelObject]) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryRefs");
    build_block_geometry_from_refs_at_time(objects, None)
}

pub(crate) fn build_block_geometry_from_refs_at_time(
    objects: &[&LevelObject],
    current_time_seconds: Option<f32>,
) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryRefsAtTime");
    if should_build_geometry_in_parallel(objects.len()) {
        puffin::profile_scope!("BuildBlockGeometryRefsParallel");
        let chunk_geometries: Vec<MeshGeometry> = objects
            .par_chunks(PARALLEL_CHUNK_SIZE)
            .map(|chunk| {
                let mut geometry = MeshGeometry::default();
                geometry.vertices.reserve(chunk.len() * VERTICES_PER_PRISM);
                let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
                for object in chunk {
                    append_block_geometry_with_scratch(
                        &mut geometry,
                        object,
                        &mut scratch,
                        current_time_seconds,
                    );
                }
                geometry
            })
            .collect();
        merge_object_geometries(chunk_geometries)
    } else {
        build_block_geometry_impl(objects.iter().copied(), current_time_seconds)
    }
}

pub(crate) fn build_block_geometry_for_object(object: &LevelObject) -> MeshGeometry {
    puffin::profile_scope!("BuildBlockGeometryOne");
    let mut geometry = MeshGeometry::default();
    append_block_geometry(&mut geometry, object, None);
    geometry
}

fn build_block_geometry_impl<'a, I>(objects: I, current_time_seconds: Option<f32>) -> MeshGeometry
where
    I: Iterator<Item = &'a LevelObject>,
{
    puffin::profile_scope!("BuildBlockGeometryImpl");
    let mut all_geometry = MeshGeometry::default();
    let (lower, _) = objects.size_hint();
    all_geometry
        .vertices
        .reserve(lower.saturating_mul(VERTICES_PER_PRISM));
    let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
    for obj in objects {
        append_block_geometry_with_scratch(
            &mut all_geometry,
            obj,
            &mut scratch,
            current_time_seconds,
        );
    }

    all_geometry
}

fn build_block_geometry_from_slice(
    objects: &[LevelObject],
    current_time_seconds: Option<f32>,
) -> MeshGeometry {
    if should_build_geometry_in_parallel(objects.len()) {
        puffin::profile_scope!("BuildBlockGeometryParallel");
        let chunk_geometries: Vec<MeshGeometry> = objects
            .par_chunks(PARALLEL_CHUNK_SIZE)
            .map(|chunk| {
                let mut geometry = MeshGeometry::default();
                geometry.vertices.reserve(chunk.len() * VERTICES_PER_PRISM);
                let mut scratch = Vec::with_capacity(VERTICES_PER_PRISM);
                for object in chunk {
                    append_block_geometry_with_scratch(
                        &mut geometry,
                        object,
                        &mut scratch,
                        current_time_seconds,
                    );
                }
                geometry
            })
            .collect();
        merge_object_geometries(chunk_geometries)
    } else {
        build_block_geometry_impl(objects.iter(), current_time_seconds)
    }
}

fn merge_object_geometries(object_geometries: Vec<MeshGeometry>) -> MeshGeometry {
    let total_vertices: usize = object_geometries.iter().map(|g| g.vertices.len()).sum();
    let total_indices: usize = object_geometries
        .iter()
        .filter_map(|g| g.indices.as_ref().map(Vec::len))
        .sum();

    let mut all_geometry = MeshGeometry::default();
    all_geometry.vertices.reserve(total_vertices);
    if total_indices > 0 {
        all_geometry.indices = Some(Vec::with_capacity(total_indices));
    }
    for object_geometry in object_geometries {
        all_geometry.append_geometry(object_geometry);
    }
    all_geometry
}

fn should_build_geometry_in_parallel(object_count: usize) -> bool {
    rayon_is_ready() && object_count >= PARALLEL_GEOMETRY_OBJECT_THRESHOLD
}

struct BlockColors {
    top: [f32; 4],
    side: [f32; 4],
    bottom: [f32; 4],
    outline: [f32; 4],
}

pub(crate) struct TransformTriggerVisualStyle {
    pub(crate) frame_color: [f32; 4],
    pub(crate) arrow_color: [f32; 4],
    pub(crate) ring_color: [f32; 4],
    pub(crate) frame_radius: f32,
    pub(crate) shaft_radius: f32,
    pub(crate) cone_radius: f32,
    pub(crate) ring_thickness: f32,
}

pub(crate) struct CameraTriggerVisualStyle {
    pub(crate) ring_color: [f32; 4],
    pub(crate) arrow_color: [f32; 4],
    pub(crate) ring_radius: f32,
    pub(crate) ring_tube_radius: f32,
    pub(crate) shaft_length: f32,
    pub(crate) shaft_radius: f32,
    pub(crate) cone_length: f32,
    pub(crate) cone_radius: f32,
}

impl BlockColors {
    fn apply_noise(&mut self, factor: f32) {
        for i in 0..3 {
            self.top[i] = (self.top[i] + factor).clamp(0.0, 1.0);
            self.side[i] = (self.side[i] + factor).clamp(0.0, 1.0);
            self.bottom[i] = (self.bottom[i] + factor).clamp(0.0, 1.0);
        }
    }

    fn apply_tint(&mut self, tint_rgb: [f32; 3]) {
        self.top = apply_color_tint(self.top, tint_rgb);
        self.side = apply_color_tint(self.side, tint_rgb);
        self.bottom = apply_color_tint(self.bottom, tint_rgb);
        self.outline = apply_color_tint(self.outline, tint_rgb);
    }
}

fn append_block_geometry(
    all_geometry: &mut MeshGeometry,
    obj: &LevelObject,
    current_time_seconds: Option<f32>,
) {
    let mut object_geometry = MeshGeometry::default();
    let mut object_vertices = Vec::new();
    append_block_geometry_inner(
        all_geometry,
        obj,
        &mut object_vertices,
        &mut object_geometry,
        current_time_seconds,
    );
}

/// Like `append_block_geometry` but reuses `scratch_vertices` across calls to avoid
/// per-object heap allocations. Used in parallel chunk builders and serial loops.
fn append_block_geometry_with_scratch(
    all_geometry: &mut MeshGeometry,
    obj: &LevelObject,
    scratch_vertices: &mut Vec<Vertex>,
    current_time_seconds: Option<f32>,
) {
    scratch_vertices.clear();
    let mut object_geometry = MeshGeometry::default();
    append_block_geometry_inner(
        all_geometry,
        obj,
        scratch_vertices,
        &mut object_geometry,
        current_time_seconds,
    );
}

fn append_block_geometry_inner(
    all_geometry: &mut MeshGeometry,
    obj: &LevelObject,
    object_vertices: &mut Vec<Vertex>,
    object_geometry: &mut MeshGeometry,
    current_time_seconds: Option<f32>,
) {
    let x_min = obj.position[0];
    let x_max = obj.position[0] + obj.size[0];
    let y_min = obj.position[1];
    let y_max = obj.position[1] + obj.size[1];
    let z_min = obj.position[2];
    let z_max = obj.position[2] + obj.size[2];

    let block = resolve_block_definition(&obj.block_id);
    let texture_layers = resolve_block_texture_layers(&obj.block_id);

    let mut colors = BlockColors {
        top: block.render.color_top,
        side: block.render.color_side,
        bottom: block.render.color_bottom,
        outline: block.render.color_outline,
    };

    if block.render.noise.abs() > f32::EPSILON {
        let noise = pseudo_random_noise(obj.position[0], obj.position[1], obj.position[2]);
        let factor = (noise * 2.0 - 1.0) * block.render.noise;
        colors.apply_noise(factor);
    }

    colors.apply_tint(obj.color_tint);

    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];

    if block.render.profile == BlockRenderProfile::CameraTrigger {
        build_camera_trigger_block_vertices(object_vertices, obj, &colors, current_time_seconds);
        all_geometry.append_vertices_from_slice(object_vertices);
        return;
    }

    if block.render.profile == BlockRenderProfile::TransformTrigger {
        build_transform_trigger_block_vertices(object_vertices, obj, &colors);
    } else if let Some(mesh_path) = block.assets.mesh.as_deref() {
        if let Some(mesh) = resolve_egmesh(mesh_path) {
            append_egmesh_geometry(object_geometry, obj, mesh, colors.top, texture_layers.side);
        } else if let Some(mesh) = resolve_obj_mesh(mesh_path) {
            append_obj_mesh(object_vertices, obj, mesh, colors.top, texture_layers.side);
        }
    }

    if object_geometry.vertices.is_empty() && object_vertices.is_empty() {
        let prism_colors = PrismFaceColors::new_with_outline(
            colors.top,
            colors.side,
            colors.bottom,
            colors.outline,
        );
        let visual_cuboids = visual_cuboids(obj);
        if visual_cuboids.is_empty() {
            append_prism_with_layers(
                object_vertices,
                [x_min, y_min, z_min],
                [x_max, y_max, z_max],
                prism_colors,
                PrismTextureLayers::new(
                    texture_layers.top,
                    texture_layers.side,
                    texture_layers.bottom,
                ),
            );
        } else {
            for cuboid in visual_cuboids {
                append_prism_with_layers(
                    object_vertices,
                    cuboid.min,
                    cuboid.max,
                    prism_colors,
                    PrismTextureLayers::new(
                        texture_layers.top,
                        texture_layers.side,
                        texture_layers.bottom,
                    ),
                );
            }
        }
    }

    if !object_vertices.is_empty() {
        rotate_vertices_around_euler(object_vertices, center, obj.rotation_degrees);
        // Apply profile tags directly to the vertex buffer before appending.
        apply_block_profile_tags(object_vertices, &block.render.profile, &colors, center);
        all_geometry.append_vertices_from_slice(object_vertices);
        return;
    }

    // egmesh path: profile tags applied to object_geometry vertices.
    apply_block_profile_tags(
        &mut object_geometry.vertices,
        &block.render.profile,
        &colors,
        center,
    );
    all_geometry.append_geometry(std::mem::take(object_geometry));
}

fn apply_block_profile_tags(
    vertices: &mut [Vertex],
    profile: &BlockRenderProfile,
    colors: &BlockColors,
    center: [f32; 3],
) {
    match profile {
        BlockRenderProfile::Neon => {
            for vertex in vertices.iter_mut() {
                vertex.color = colors.top;
            }
        }
        BlockRenderProfile::Liquid => {
            for vertex in vertices.iter_mut() {
                vertex.set_render_profile(LIQUID_PROFILE_TAG);
            }
        }
        BlockRenderProfile::Gem => {
            let phase_seed =
                pseudo_random_noise(center[0], center[1], center[2]) * std::f32::consts::TAU;
            for vertex in vertices.iter_mut() {
                vertex.set_render_profile(GEM_PROFILE_TAG);
                vertex.color_outline = [center[0], center[1], center[2], phase_seed];
            }
        }
        BlockRenderProfile::Solid
        | BlockRenderProfile::SpeedPortal
        | BlockRenderProfile::TransformTrigger
        | BlockRenderProfile::CameraTrigger => {}
    }
}

fn build_camera_trigger_block_vertices(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    colors: &BlockColors,
    current_time_seconds: Option<f32>,
) {
    let base_ring_radius = 0.52;
    let progress = current_time_seconds
        .and_then(|time_seconds| {
            obj.trigger
                .as_ref()
                .and_then(|trigger| camera_trigger_countdown_progress(trigger, time_seconds))
        })
        .unwrap_or(0.0);
    let style = CameraTriggerVisualStyle {
        ring_color: colors.side,
        arrow_color: colors.top,
        ring_radius: camera_trigger_countdown_ring_radius(base_ring_radius, progress),
        ring_tube_radius: 0.035,
        shaft_length: 1.15,
        shaft_radius: 0.055,
        cone_length: 0.48,
        cone_radius: 0.16,
    };

    append_camera_trigger_visual_vertices(
        vertices,
        camera_trigger_eye_from_object(obj),
        obj.rotation_degrees,
        &style,
    );
}

fn camera_trigger_countdown_progress(
    trigger: &TimedTrigger,
    current_time_seconds: f32,
) -> Option<f32> {
    if !current_time_seconds.is_finite()
        || !trigger.time_seconds.is_finite()
        || !matches!(trigger.target, TimedTriggerTarget::Camera)
    {
        return None;
    }

    let transition_interval_seconds = match trigger.action {
        TimedTriggerAction::CameraPose {
            transition_interval_seconds,
            ..
        }
        | TimedTriggerAction::CameraFollow {
            transition_interval_seconds,
            ..
        } => transition_interval_seconds,
        TimedTriggerAction::TransformObjects { .. } => return None,
    };

    if current_time_seconds >= trigger.time_seconds {
        return Some(1.0);
    }

    let window_seconds = transition_interval_seconds.max(1.0);
    let remaining = trigger.time_seconds - current_time_seconds.max(0.0);
    Some(1.0 - (remaining / window_seconds).clamp(0.0, 1.0))
}

fn camera_trigger_countdown_ring_radius(base_radius: f32, progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    (base_radius - progress * base_radius * 0.67).max(base_radius * 0.33)
}

fn apply_color_tint(color: [f32; 4], tint_rgb: [f32; 3]) -> [f32; 4] {
    [
        color[0] * tint_rgb[0].clamp(0.0, 1.0),
        color[1] * tint_rgb[1].clamp(0.0, 1.0),
        color[2] * tint_rgb[2].clamp(0.0, 1.0),
        color[3],
    ]
}

fn build_transform_trigger_block_vertices(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    colors: &BlockColors,
) {
    let style = TransformTriggerVisualStyle {
        frame_color: colors.side,
        arrow_color: colors.top,
        ring_color: colors.top,
        frame_radius: 0.035,
        shaft_radius: 0.06,
        cone_radius: 0.18,
        ring_thickness: 0.05,
    };

    append_transform_trigger_visual_vertices(
        vertices,
        obj.position,
        obj.size,
        obj.rotation_degrees,
        &style,
    );
}

pub(crate) fn append_camera_trigger_visual_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    rotation_degrees: [f32; 3],
    style: &CameraTriggerVisualStyle,
) {
    let eye = Vec3::from_array(position);
    let forward = Vec3::from_array(camera_trigger_forward_from_rotation_degrees(
        rotation_degrees,
    ));
    let arrow_direction = if forward.length_squared() > f32::EPSILON {
        forward
    } else {
        Vec3::Z
    };

    append_camera_trigger_ring(
        vertices,
        eye,
        arrow_direction,
        style.ring_radius,
        style.ring_tube_radius,
        style.ring_color,
    );

    let shaft_start = eye + arrow_direction * (style.ring_radius * 0.7);
    let shaft_end = shaft_start + arrow_direction * style.shaft_length;
    let arrow_tip = shaft_end + arrow_direction * style.cone_length;

    append_cylinder_segment(
        vertices,
        shaft_start.to_array(),
        shaft_end.to_array(),
        style.shaft_radius,
        style.arrow_color,
    );
    append_cone(
        vertices,
        shaft_end.to_array(),
        arrow_tip.to_array(),
        style.cone_radius,
        style.arrow_color,
    );

    for vertex in vertices {
        vertex.set_render_profile(CAMERA_TRIGGER_PROFILE_TAG);
        vertex.color_outline = [eye.x, eye.y, eye.z, 0.0];
    }
}

fn append_camera_trigger_ring(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    forward: Vec3,
    radius: f32,
    tube_radius: f32,
    color: [f32; 4],
) {
    let basis_seed = if forward.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let right = basis_seed.cross(forward).normalize_or_zero();
    if right.length_squared() <= f32::EPSILON {
        return;
    }
    let up = forward.cross(right).normalize_or_zero();
    if up.length_squared() <= f32::EPSILON {
        return;
    }

    let segments = 24;
    for segment in 0..segments {
        let angle_start = segment as f32 * std::f32::consts::TAU / segments as f32;
        let angle_end = (segment + 1) as f32 * std::f32::consts::TAU / segments as f32;
        let point_start =
            center + right * (angle_start.cos() * radius) + up * (angle_start.sin() * radius);
        let point_end =
            center + right * (angle_end.cos() * radius) + up * (angle_end.sin() * radius);
        append_cylinder_segment(
            vertices,
            point_start.to_array(),
            point_end.to_array(),
            tube_radius,
            color,
        );
    }
}

pub(crate) fn append_transform_trigger_visual_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    style: &TransformTriggerVisualStyle,
) {
    let center = Vec3::from_array(object_center(position, size));
    let rotation = transform_marker_rotation(rotation_degrees);
    append_transform_trigger_visual_vertices_with_rotation(
        vertices, position, size, center, rotation, style,
    );
}

pub(crate) fn append_transform_trigger_visual_vertices_with_rotation(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    center: Vec3,
    rotation: glam::Quat,
    style: &TransformTriggerVisualStyle,
) {
    let forward = (rotation * Vec3::Z).normalize_or_zero();
    let arrow_direction = if forward.length_squared() > f32::EPSILON {
        forward
    } else {
        Vec3::Z
    };

    let extent = size.iter().copied().fold(0.0_f32, f32::max).max(0.75);

    append_oriented_box_edges(
        vertices,
        position,
        size,
        rotation,
        style.frame_radius,
        style.frame_color,
    );

    let arrow_base = center - arrow_direction * (extent * 0.3);
    let arrow_shaft_end = center + arrow_direction * (extent * 0.42);
    let arrow_tip = arrow_shaft_end + arrow_direction * 0.45;

    append_cylinder_segment(
        vertices,
        arrow_base.to_array(),
        arrow_shaft_end.to_array(),
        style.shaft_radius,
        style.arrow_color,
    );
    append_cone(
        vertices,
        arrow_shaft_end.to_array(),
        arrow_tip.to_array(),
        style.cone_radius,
        style.arrow_color,
    );

    let ring_radius = extent * 0.42;
    let ring_center = [center.x, center.y, center.z];
    append_xz_ring(
        vertices,
        ring_center,
        ring_radius,
        style.ring_thickness,
        style.ring_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CAMERA_TRIGGER_BLOCK_ID;

    fn camera_trigger_object(time_seconds: f32, transition_interval_seconds: f32) -> LevelObject {
        LevelObject {
            block_id: CAMERA_TRIGGER_BLOCK_ID.to_string(),
            trigger: Some(TimedTrigger {
                time_seconds,
                duration_seconds: 0.0,
                easing: crate::triggers::TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Camera,
                action: TimedTriggerAction::CameraPose {
                    transition_interval_seconds,
                    use_full_segment_transition: false,
                    target_position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    pitch: 0.0,
                },
            }),
            ..LevelObject::default()
        }
    }

    fn ring_outer_radius(vertices: &[Vertex], center: [f32; 3], ring_color: [f32; 4]) -> f32 {
        vertices
            .iter()
            .filter(|vertex| vertex.color == ring_color)
            .map(|vertex| {
                let dx = vertex.position[0] - center[0];
                let dy = vertex.position[1] - center[1];
                let dz = vertex.position[2] - center[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            })
            .fold(0.0_f32, f32::max)
    }

    #[test]
    fn camera_trigger_ring_shrinks_as_activation_approaches() {
        let object = camera_trigger_object(4.0, 2.0);
        let colors = BlockColors {
            top: [0.9, 0.8, 0.2, 1.0],
            side: [0.2, 0.7, 1.0, 1.0],
            bottom: [0.0, 0.0, 0.0, 1.0],
            outline: [1.0, 1.0, 1.0, 1.0],
        };
        let mut early = Vec::new();
        let mut late = Vec::new();

        build_camera_trigger_block_vertices(&mut early, &object, &colors, Some(2.0));
        build_camera_trigger_block_vertices(&mut late, &object, &colors, Some(4.0));

        let center = camera_trigger_eye_from_object(&object);
        assert!(
            ring_outer_radius(&late, center, colors.side)
                < ring_outer_radius(&early, center, colors.side)
        );
    }
}
