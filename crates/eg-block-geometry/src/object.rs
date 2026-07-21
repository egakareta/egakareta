/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_blocks::{BlockCuboid, BlockDefinition};
use eg_types::LevelObject;
use glam::{Vec2, Vec3};

/// A cuboid in world space, derived from block definitions and object transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCuboid {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub color_tint: Option<[f32; 3]>,
}

impl WorldCuboid {
    pub fn top(self) -> f32 {
        self.max[1]
    }
}

/// Obtain world-space cuboids for the object's visuals and hitboxes, based on block definitions and object transforms.
pub fn visual_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let block = BlockDefinition::get(&object.block_id);
    cuboids_from_definition(object, &block.geometry.elements)
}

/// Hitboxes are defined separately from visuals in block definitions. If hitboxes are missing, the caller can fall back to visuals or a full object cuboid.
fn raw_hitbox_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let block = BlockDefinition::get(&object.block_id);
    cuboids_from_definition(object, &block.geometry.hitboxes)
}

/// Obtain the world-space AABB of a rotated cuboid, used for spatial grid insertion and broad-phase collision checks.
/// Falls back to visual cuboids if hitboxes are missing, and to a single cuboid matching the object's size if visuals are also missing.
pub fn hitbox_cuboids(object: &LevelObject) -> Vec<WorldCuboid> {
    let hitboxes = raw_hitbox_cuboids(object);
    if hitboxes.is_empty() {
        let visual_cuboids = visual_cuboids(object);
        if visual_cuboids.is_empty() {
            vec![WorldCuboid {
                min: object.position,
                max: [
                    object.position[0] + object.size[0],
                    object.position[1] + object.size[1],
                    object.position[2] + object.size[2],
                ],
                color_tint: None,
            }]
        } else {
            visual_cuboids
        }
    } else {
        hitboxes
    }
}

/// Computes the 2D polygon of a rotated cuboid's footprint on the XZ plane, used for collision detection and rendering.
pub fn rotated_cuboid_xz_polygon(object: &LevelObject, cuboid: WorldCuboid) -> Vec<Vec2> {
    let object_center = object.center();
    let rotation = object.basis();
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

/// Computes the axis-aligned bounding box of a rotated cuboid's XZ footprint, used for spatial grid insertion and broad-phase collision checks.
pub fn rotated_cuboid_aabb_xz(object: &LevelObject, cuboid: WorldCuboid) -> [f32; 4] {
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

/// Converts block cuboid definitions to world cuboids based on the object's position and size.
/// Internally uses `block_local_to_world`.
fn cuboids_from_definition(object: &LevelObject, cuboids: &[BlockCuboid]) -> Vec<WorldCuboid> {
    cuboids
        .iter()
        .map(|cuboid| WorldCuboid {
            min: object.local_to_world(cuboid.from),
            max: object.local_to_world(cuboid.to),
            color_tint: cuboid.color_tint,
        })
        .collect()
}

/// Computes the convex hull of a set of 2D points using the monotone chain algorithm.
/// This is used to get the 2D footprint of a rotated cuboid for collision detection and rendering.
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

/// 2D cross product.
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use eg_blocks::BlockCuboid;
    use eg_types::LevelObject;

    use super::*;

    #[test]
    fn cuboids_from_definition_preserves_color_tint() {
        let object = LevelObject {
            position: [10.0, 20.0, 30.0],
            size: [2.0, 4.0, 6.0],
            ..LevelObject::default()
        };
        let cuboids = cuboids_from_definition(
            &object,
            &[BlockCuboid {
                from: [4.0, 8.0, 12.0],
                to: [12.0, 16.0, 16.0],
                color_tint: Some([0.25, 0.5, 0.75]),
            }],
        );

        assert_eq!(cuboids.len(), 1);
        assert_eq!(cuboids[0].min, [10.5, 22.0, 34.5]);
        assert_eq!(cuboids[0].max, [11.5, 24.0, 36.0]);
        assert_eq!(cuboids[0].color_tint, Some([0.25, 0.5, 0.75]));
    }
}
