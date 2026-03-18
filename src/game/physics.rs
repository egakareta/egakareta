use crate::types::LevelObject;
use glam::{EulerRot, Mat3, Vec2, Vec3};

pub(crate) const BASE_PLAYER_SPEED: f32 = 8.0;

pub(crate) fn object_xz_contains(obj: &LevelObject, x: f32, z: f32) -> bool {
    let polygon = object_projected_xz_polygon(obj);
    if polygon.len() < 3 {
        return false;
    }
    point_in_polygon(Vec2::new(x, z), &polygon)
}

pub(crate) fn aabb_overlaps_object_xz(
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    obj: &LevelObject,
) -> bool {
    let polygon = object_projected_xz_polygon(obj);
    if polygon.len() < 3 {
        return false;
    }

    let rect = [
        Vec2::new(min_x, min_z),
        Vec2::new(max_x, min_z),
        Vec2::new(max_x, max_z),
        Vec2::new(min_x, max_z),
    ];

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
        let (poly_min, poly_max) = project_points(axis, &polygon);
        let (rect_min, rect_max) = project_points(axis, &rect);
        if poly_max < rect_min || rect_max < poly_min {
            return false;
        }
    }

    true
}

fn object_projected_xz_polygon(obj: &LevelObject) -> Vec<Vec2> {
    let center = Vec3::new(
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    );
    let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
    let rotation = Mat3::from_euler(
        EulerRot::XYZ,
        obj.rotation_degrees[0].to_radians(),
        obj.rotation_degrees[1].to_radians(),
        obj.rotation_degrees[2].to_radians(),
    );

    let mut points: Vec<Vec2> = Vec::with_capacity(8);
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let local = Vec3::new(half.x * sx, half.y * sy, half.z * sz);
                let world = center + rotation * local;
                points.push(Vec2::new(world.x, world.z));
            }
        }
    }

    convex_hull(points)
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
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

    let mut lower: Vec<Vec2> = Vec::new();
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

    let mut upper: Vec<Vec2> = Vec::new();
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
