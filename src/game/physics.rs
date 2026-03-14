use crate::types::LevelObject;

pub(crate) const BASE_PLAYER_SPEED: f32 = 8.0;

pub(crate) fn rotate_point_around_center_2d(
    point: [f32; 2],
    center: [f32; 2],
    radians: f32,
) -> [f32; 2] {
    let sin = radians.sin();
    let cos = radians.cos();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    [
        center[0] + (dx * cos - dy * sin),
        center[1] + (dx * sin + dy * cos),
    ]
}

pub(crate) fn object_xz_contains(obj: &LevelObject, x: f32, z: f32) -> bool {
    // Fast path for axis-aligned objects (most common case)
    if obj.rotation_degrees.abs() < 0.001 {
        return x >= obj.position[0]
            && x < obj.position[0] + obj.size[0]
            && z >= obj.position[2]
            && z < obj.position[2] + obj.size[2];
    }
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];
    let local = rotate_point_around_center_2d([x, z], center, -obj.rotation_degrees.to_radians());
    local[0] >= obj.position[0]
        && local[0] < obj.position[0] + obj.size[0]
        && local[1] >= obj.position[2]
        && local[1] < obj.position[2] + obj.size[2]
}

pub(crate) fn aabb_overlaps_object_xz(
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    obj: &LevelObject,
) -> bool {
    // Fast path for axis-aligned objects — simple AABB vs AABB
    if obj.rotation_degrees.abs() < 0.001 {
        let obj_max_x = obj.position[0] + obj.size[0];
        let obj_max_z = obj.position[2] + obj.size[2];
        return max_x > obj.position[0]
            && min_x < obj_max_x
            && max_z > obj.position[2]
            && min_z < obj_max_z;
    }

    let aabb_center_x = (min_x + max_x) * 0.5;
    let aabb_center_z = (min_z + max_z) * 0.5;
    let aabb_half_x = (max_x - min_x) * 0.5;
    let aabb_half_z = (max_z - min_z) * 0.5;

    let rect_center_x = obj.position[0] + obj.size[0] * 0.5;
    let rect_center_z = obj.position[2] + obj.size[2] * 0.5;
    let rect_half_x = obj.size[0] * 0.5;
    let rect_half_z = obj.size[2] * 0.5;

    let theta = obj.rotation_degrees.to_radians();
    let axis_u = [theta.cos(), theta.sin()];
    let axis_v = [-theta.sin(), theta.cos()];

    let axes = [[1.0, 0.0], [0.0, 1.0], axis_u, axis_v];
    for axis in axes {
        let aabb_proj_center = aabb_center_x * axis[0] + aabb_center_z * axis[1];
        let aabb_proj_radius = aabb_half_x * axis[0].abs() + aabb_half_z * axis[1].abs();

        let rect_proj_center = rect_center_x * axis[0] + rect_center_z * axis[1];
        let rect_proj_radius = rect_half_x * (axis_u[0] * axis[0] + axis_u[1] * axis[1]).abs()
            + rect_half_z * (axis_v[0] * axis[0] + axis_v[1] * axis[1]).abs();

        if (aabb_proj_center - rect_proj_center).abs() > aabb_proj_radius + rect_proj_radius {
            return false;
        }
    }

    true
}
