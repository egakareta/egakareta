/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::block_repository::{resolve_block_definition, BlockCollision};
use crate::mesh::shapes::append_prism;
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::types::{LevelObject, Vertex};
use glam::{EulerRot, Mat3, Vec3};

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
    rotation_degrees: [f32; 3],
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

    rotate_vertices_around_euler(&mut vertices, center, rotation_degrees);
    let rotation = euler_rotation_matrix(rotation_degrees);
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
    object: &LevelObject,
    color: [f32; 4],
    thickness: f32,
) {
    let start = vertices.len();
    let x_min = object.position[0];
    let x_max = object.position[0] + object.size[0];
    let y_min = object.position[1];
    let y_max = object.position[1] + object.size[1];
    let z_min = object.position[2];
    let z_max = object.position[2] + object.size[2];
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
        object.position[0] + object.size[0] * 0.5,
        object.position[1] + object.size[1] * 0.5,
        object.position[2] + object.size[2] * 0.5,
    ];
    rotate_vertices_around_euler(&mut vertices[start..], center, object.rotation_degrees);
}

pub(crate) fn build_editor_hitbox_visualization_vertices(
    objects: &[LevelObject],
    player_hitbox: Option<LevelObject>,
) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(objects.len().saturating_mul(432));

    for object in objects {
        let collision = resolve_block_definition(&object.block_id)
            .behavior
            .collision;
        let outline_color = match collision {
            BlockCollision::Solid | BlockCollision::Hazard => [1.0, 0.04, 0.06, 0.88],
            BlockCollision::Portal => [0.0, 0.82, 1.0, 0.88],
            BlockCollision::Collectible => [1.0, 0.86, 0.10, 0.9],
            BlockCollision::PassThrough => continue,
        };

        append_hitbox_outline_vertices(&mut vertices, object, outline_color, 0.035);
    }

    if let Some(player) = player_hitbox {
        append_hitbox_outline_vertices(&mut vertices, &player, [1.0, 1.0, 1.0, 0.78], 0.025);
    }

    vertices
}

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let selection_color = [0.098, 0.6, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        selection_color,
        selection_color,
    )
}

pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
    rotation_degrees: [f32; 3],
    line_width: f32,
) -> Vec<Vertex> {
    let hover_color = [0.698, 0.898, 1.0, 1.0];
    build_editor_outline_hull_vertices(
        position,
        size,
        rotation_degrees,
        line_width,
        hover_color,
        hover_color,
    )
}
