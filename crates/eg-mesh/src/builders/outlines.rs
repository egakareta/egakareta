/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_block_geometry::hitbox_cuboids;
use eg_blocks::{BlockCollision, BlockDefinition};
use eg_types::LevelObject;
use glam::{EulerRot, Mat3, Vec3};

use crate::{Vertex, shapes::append_prism, transforms::rotate_vertices_around_euler};

/// 12 edge prisms × 36 vertices per cuboid.
const HITBOX_VERTS_PER_CUBOID: usize = 432;

fn euler_rotation_matrix(degrees: [f32; 3]) -> Mat3 {
    Mat3::from_euler(
        EulerRot::XYZ,
        degrees[0].to_radians(),
        degrees[1].to_radians(),
        degrees[2].to_radians(),
    )
}

fn unit_corner_direction(local: Vec3) -> Vec3 {
    Vec3::new(local.x.signum(), local.y.signum(), local.z.signum())
}

fn build_editor_outline_hull_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation: [f32; 3],
    line_width_pixels: f32,
    color_top: [f32; 4],
    color_side: [f32; 4],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    append_prism(
        &mut vertices,
        position,
        [
            position[0] + size[0],
            position[1] + size[1],
            position[2] + size[2],
        ],
        color_top,
        color_side,
    );

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];
    let center_vec = Vec3::from(center);
    let local_directions = vertices
        .iter()
        .map(|vertex| unit_corner_direction(Vec3::from(vertex.position) - center_vec))
        .collect::<Vec<_>>();

    rotate_vertices_around_euler(&mut vertices, center, rotation);
    let rotation = euler_rotation_matrix(rotation);
    let outline_width_pixels = (line_width_pixels * 1.35).max(1.0);
    for (vertex, local_direction) in vertices.iter_mut().zip(local_directions) {
        let anchor = Vec3::from(vertex.position) - rotation * local_direction;
        vertex.color_outline = [anchor.x, anchor.y, anchor.z, outline_width_pixels];
        vertex.render_profile = 3.0;
    }
    vertices
}

fn append_hitbox_outline_vertices(
    vertices: &mut Vec<Vertex>,
    position: [f32; 3],
    size: [f32; 3],
    rotation: [f32; 3],
    color: [f32; 4],
    thickness: f32,
) {
    let start = vertices.len();
    let x_min = position[0];
    let x_max = position[0] + size[0];
    let y_min = position[1];
    let y_max = position[1] + size[1];
    let z_min = position[2];
    let z_max = position[2] + size[2];
    let half_thickness = (thickness * 0.5).max(0.001);

    for y in [y_min, y_max] {
        for z in [z_min, z_max] {
            append_prism(
                vertices,
                [x_min, y - half_thickness, z - half_thickness],
                [x_max, y + half_thickness, z + half_thickness],
                color,
                color,
            );
        }
    }

    for x in [x_min, x_max] {
        for z in [z_min, z_max] {
            append_prism(
                vertices,
                [x - half_thickness, y_min, z - half_thickness],
                [x + half_thickness, y_max, z + half_thickness],
                color,
                color,
            );
        }
    }

    for x in [x_min, x_max] {
        for y in [y_min, y_max] {
            append_prism(
                vertices,
                [x - half_thickness, y - half_thickness, z_min],
                [x + half_thickness, y + half_thickness, z_max],
                color,
                color,
            );
        }
    }

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];
    rotate_vertices_around_euler(&mut vertices[start..], center, rotation);
}

fn hitbox_outline_color(collision: BlockCollision) -> Option<[f32; 4]> {
    match collision {
        BlockCollision::Solid | BlockCollision::Hazard => Some([1.0, 0.04, 0.06, 0.88]),
        BlockCollision::Portal => Some([0.0, 0.82, 1.0, 0.88]),
        BlockCollision::Collectible => Some([1.0, 0.86, 0.10, 0.9]),
        BlockCollision::PassThrough => None,
    }
}

pub fn build_editor_hitbox_visualization_vertices(
    objects: &[LevelObject],
    player_hitbox: Option<LevelObject>,
) -> Vec<Vertex> {
    // NOTE: intentionally serial. The output is ~1GB for a 19k-object scene,
    // so the build is memory-bandwidth bound: per-thread buffers plus a full
    // merge memcpy add an extra gigabyte of traffic and regress (measured
    // +48% vs serial). A parallel win would need direct indexed writes into
    // one allocation (see block-geometry builders for the pattern cost).
    let mut vertices = Vec::with_capacity(objects.len().saturating_mul(HITBOX_VERTS_PER_CUBOID));

    for object in objects {
        let block = BlockDefinition::get_by_id(&object.block_id);
        let Some(outline_color) = hitbox_outline_color(block.behavior.collision) else {
            continue;
        };

        // Fast path for full-cube blocks (stone, etc.): no custom hitboxes or
        // visuals, so the hitbox is the object bounds. Avoids 19k `Vec`
        // allocations + double `BlockDefinition` lookups inside
        // `hitbox_cuboids` for the common case.
        if block.geometry.hitboxes.is_empty() && block.geometry.elements.is_empty() {
            append_hitbox_outline_vertices(
                &mut vertices,
                object.position,
                object.size,
                object.rotation,
                outline_color,
                0.035,
            );
            continue;
        }

        for cuboid in hitbox_cuboids(object) {
            let size = [
                cuboid.max[0] - cuboid.min[0],
                cuboid.max[1] - cuboid.min[1],
                cuboid.max[2] - cuboid.min[2],
            ];
            append_hitbox_outline_vertices(
                &mut vertices,
                cuboid.min,
                size,
                object.rotation,
                outline_color,
                0.035,
            );
        }
    }

    if let Some(player) = player_hitbox {
        append_hitbox_outline_vertices(
            &mut vertices,
            player.position,
            player.size,
            player.rotation,
            [1.0, 1.0, 1.0, 0.78],
            0.025,
        );
    }

    vertices
}

pub fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let selection_color = [0.098, 0.6, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation,
        line_width,
        selection_color,
        selection_color,
    )
}

pub fn build_editor_transform_trigger_target_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let target_color = [0.72, 0.22, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation,
        line_width,
        target_color,
        target_color,
    )
}

pub fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let hover_color = [0.698, 0.898, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation,
        line_width,
        hover_color,
        hover_color,
    )
}
