/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::block_geometry::{effective_hitbox_cuboids, rotated_cuboid_xz_polygon};
use crate::types::LevelObject;
use glam::Vec2;

pub(crate) const BASE_PLAYER_SPEED: f32 = 8.0;

pub(crate) fn object_xz_contains(obj: &LevelObject, x: f32, z: f32) -> bool {
    for cuboid in effective_hitbox_cuboids(obj) {
        let polygon = rotated_cuboid_xz_polygon(obj, cuboid);
        if polygon.len() >= 3 && point_in_polygon(Vec2::new(x, z), &polygon) {
            return true;
        }
    }
    false
}

#[cfg(test)]
pub(crate) fn aabb_overlaps_object_xz(
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    obj: &LevelObject,
) -> bool {
    let rect = [
        Vec2::new(min_x, min_z),
        Vec2::new(max_x, min_z),
        Vec2::new(max_x, max_z),
        Vec2::new(min_x, max_z),
    ];

    for cuboid in effective_hitbox_cuboids(obj) {
        let polygon = rotated_cuboid_xz_polygon(obj, cuboid);
        if polygon.len() >= 3 && polygons_overlap_xz(&polygon, &rect) {
            return true;
        }
    }

    false
}

pub(crate) fn aabb_overlaps_cuboid_xz(
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    obj: &LevelObject,
    cuboid: crate::block_geometry::WorldCuboid,
) -> bool {
    let polygon = rotated_cuboid_xz_polygon(obj, cuboid);
    if polygon.len() < 3 {
        return false;
    }

    let rect = [
        Vec2::new(min_x, min_z),
        Vec2::new(max_x, min_z),
        Vec2::new(max_x, max_z),
        Vec2::new(min_x, max_z),
    ];

    polygons_overlap_xz(&polygon, &rect)
}

fn polygons_overlap_xz(polygon: &[Vec2], rect: &[Vec2; 4]) -> bool {
    let mut axes: Vec<Vec2> = Vec::with_capacity(polygon.len() + 2);
    axes.push(Vec2::X);
    axes.push(Vec2::Y);
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let edge = b - a;
        if edge.length_squared() <= f32::EPSILON {
            continue;
        }
        let normal = Vec2::new(-edge.y, edge.x).normalize();
        axes.push(normal);
    }

    for axis in axes {
        let (poly_min, poly_max) = project_points(axis, polygon);
        let (rect_min, rect_max) = project_points(axis, rect.as_slice());
        if poly_max < rect_min || rect_max < poly_min {
            return false;
        }
    }

    true
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    for edge_start_index in 0..polygon.len() {
        let edge_end_index = (edge_start_index + 1) % polygon.len();
        if point_on_segment(point, polygon[edge_start_index], polygon[edge_end_index]) {
            return true;
        }
    }

    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let pi = polygon[i];
        let pj = polygon[j];
        let denom = pj.y - pi.y;
        let intersects = ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / denom + pi.x);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> bool {
    const EPSILON: f32 = 1e-5;

    let segment = end - start;
    let to_point = point - start;
    if cross(segment, to_point).abs() > EPSILON {
        return false;
    }

    let dot = to_point.dot(segment);
    if dot < -EPSILON {
        return false;
    }

    dot <= segment.length_squared() + EPSILON
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn project_points(axis: Vec2, points: &[Vec2]) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for point in points {
        let dot = axis.dot(*point);
        min = min.min(dot);
        max = max.max(dot);
    }
    (min, max)
}
