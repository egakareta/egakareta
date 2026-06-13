/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{EulerRot, Mat3, Vec2, Vec3};

use crate::block_repository::{resolve_block_definition, BlockCuboid};
use crate::types::LevelObject;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WorldCuboid {
    pub(crate) min: [f32; 3],
    pub(crate) max: [f32; 3],
}

impl WorldCuboid {
    pub(crate) fn top(self) -> f32 {
        self.max[1]
    }
}

pub(crate) fn object_center(object: &LevelObject) -> Vec3 {
    Vec3::new(
        object.position[0] + object.size[0] * 0.5,
        object.position[1] + object.size[1] * 0.5,
        object.position[2] + object.size[2] * 0.5,
    )
}

pub(crate) fn object_rotation(object: &LevelObject) -> Mat3 {
    Mat3::from_euler(
        EulerRot::XYZ,
        object.rotation_degrees[0].to_radians(),
        object.rotation_degrees[1].to_radians(),
        object.rotation_degrees[2].to_radians(),
    )
}

pub(crate) fn visual_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let block = resolve_block_definition(&object.block_id);
    cuboids_from_definition(object, &block.geometry.elements)
}

pub(crate) fn hitbox_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let block = resolve_block_definition(&object.block_id);
    cuboids_from_definition(object, &block.geometry.hitboxes)
}

pub(crate) fn effective_hitbox_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let hitboxes = hitbox_cuboids(object);
    if hitboxes.is_empty() {
        let visual_cuboids = visual_cuboids(object);
        if visual_cuboids.is_empty() {
            vec![full_object_cuboid(object)]
        } else {
            visual_cuboids
        }
    } else {
        hitboxes
    }
}

pub(crate) fn full_object_cuboid(object: &LevelObject) -> WorldCuboid {
    WorldCuboid {
        min: object.position,
        max: [
            object.position[0] + object.size[0],
            object.position[1] + object.size[1],
            object.position[2] + object.size[2],
        ],
    }
}

pub(crate) fn rotated_cuboid_xz_polygon(object: &LevelObject, cuboid: WorldCuboid) -> Vec<Vec2> {
    let object_center = object_center(object);
    let rotation = object_rotation(object);
    let min = Vec3::from(cuboid.min);
    let max = Vec3::from(cuboid.max);
    let mut points = Vec::with_capacity(8);

    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let world = object_center + rotation * (Vec3::new(x, y, z) - object_center);
                points.push(Vec2::new(world.x, world.z));
            }
        }
    }

    convex_hull(points)
}

pub(crate) fn rotated_cuboid_aabb_xz(object: &LevelObject, cuboid: WorldCuboid) -> [f32; 4] {
    let polygon = rotated_cuboid_xz_polygon(object, cuboid);
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for point in polygon {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_z = min_z.min(point.y);
        max_z = max_z.max(point.y);
    }
    [min_x, max_x, min_z, max_z]
}

fn cuboids_from_definition(object: &LevelObject, cuboids: &[BlockCuboid]) -> Vec<WorldCuboid> {
    cuboids
        .iter()
        .map(|cuboid| WorldCuboid {
            min: block_local_to_world(object, cuboid.from),
            max: block_local_to_world(object, cuboid.to),
        })
        .collect()
}

fn block_local_to_world(object: &LevelObject, local: [f32; 3]) -> [f32; 3] {
    [
        object.position[0] + object.size[0] * (local[0] / 16.0),
        object.position[1] + object.size[1] * (local[1] / 16.0),
        object.position[2] + object.size[2] * (local[2] / 16.0),
    ]
}

fn convex_hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    if points.len() <= 1 {
        return points;
    }

    points.sort_by(|a, b| {
        let cmp_x = f32::total_cmp(&a.x, &b.x);
        if cmp_x.is_eq() {
            f32::total_cmp(&a.y, &b.y)
        } else {
            cmp_x
        }
    });
    points.dedup_by(|a, b| (a.x - b.x).abs() <= 1e-6 && (a.y - b.y).abs() <= 1e-6);

    if points.len() <= 2 {
        return points;
    }

    let mut lower = Vec::new();
    for point in &points {
        while lower.len() >= 2
            && cross(
                lower[lower.len() - 1] - lower[lower.len() - 2],
                *point - lower[lower.len() - 1],
            ) <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }

    let mut upper = Vec::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && cross(
                upper[upper.len() - 1] - upper[upper.len() - 2],
                *point - upper[upper.len() - 1],
            ) <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}
