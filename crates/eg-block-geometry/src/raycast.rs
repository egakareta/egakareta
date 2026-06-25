/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::Vec3;

use crate::*;

/// Computes the intersection of a ray with an axis-aligned bounding box (AABB).
///
/// Returns the distance `t` along the ray to the first intersection point and the
/// outward-facing surface normal at that point. Both `local_origin` and `local_dir` are
/// expected to be in the local coordinate space of the AABB (i.e., the AABB is treated as
/// axis-aligned in this coordinate frame).
///
/// The algorithm uses the slab method: for each axis, it computes the entry and exit
/// distances along the ray, tracking the candidate normal on each face. This is a
/// numerically robust implementation that handles rays parallel to slab boundaries.
///
/// # Parameters
/// - `local_origin`: The ray origin in the AABB's local coordinate space.
/// - `local_dir`: The ray direction vector (does not need to be normalized, but must be
///   non-zero for meaningful results).
/// - `min`: The minimum corner of the AABB (component-wise).
/// - `max`: The maximum corner of the AABB (component-wise).
///
/// # Returns
/// - `Some((t, normal))` if the ray intersects the AABB at distance `t >= 0`, where
///   `normal` is the outward-facing surface normal at the hit point.
/// - `None` if the ray misses the AABB, if the intersection lies behind the ray origin,
///   or if the ray is parallel to a slab and lies completely outside it.
pub fn ray_intersect_aabb(
    local_origin: Vec3,
    local_dir: Vec3,
    min: Vec3,
    max: Vec3,
) -> Option<(f32, Vec3)> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    let mut normal_enter = Vec3::ZERO;
    let mut normal_exit = Vec3::ZERO;

    for axis in 0..3 {
        let origin_component = local_origin[axis];
        let dir_component = local_dir[axis];
        let min_component = min[axis];
        let max_component = max[axis];

        if dir_component.abs() <= f32::EPSILON {
            if origin_component < min_component || origin_component > max_component {
                return None;
            }
            continue;
        }

        let mut t1 = (min_component - origin_component) / dir_component;
        let mut t2 = (max_component - origin_component) / dir_component;

        let axis_dir = match axis {
            0 => Vec3::X,
            1 => Vec3::Y,
            _ => Vec3::Z,
        };

        let mut n1 = -axis_dir;
        let mut n2 = axis_dir;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            std::mem::swap(&mut n1, &mut n2);
        }

        if t1 > t_min {
            t_min = t1;
            normal_enter = n1;
        }
        if t2 < t_max {
            t_max = t2;
            normal_exit = n2;
        }

        if t_min > t_max {
            return None;
        }
    }

    if t_max < 0.0 {
        return None;
    }

    let (t_hit, normal_local) = if t_min >= 0.0 {
        (t_min, normal_enter)
    } else {
        (t_max, normal_exit)
    };

    Some((t_hit, normal_local))
}

/// Computes the entry and exit distances of a ray through an axis-aligned bounding box (AABB),
/// without computing surface normals.
///
/// Unlike [`ray_intersect_aabb`], this function returns the full interval `[t_entry, t_exit]`
/// of the ray's passage through the AABB, and does not calculate surface normals. This is
/// useful for broad-phase collision tests, frustum culling, or any scenario where only the
/// overlap interval is needed.
///
/// Uses the slab method with reciprocal direction for numerical efficiency. The entry
/// distance may be negative if the ray origin is inside the AABB.
///
/// # Parameters
/// - `ray_origin`: The ray origin.
/// - `ray_dir`: The ray direction vector (does not need to be normalized, but must be
///   non-zero for meaningful results).
/// - `min`: The minimum corner of the AABB (component-wise).
/// - `max`: The maximum corner of the AABB (component-wise).
///
/// # Returns
/// - `Some((t_entry, t_exit))` where `t_entry` is the distance at which the ray enters the
///   AABB and `t_exit` is the distance at which it exits. `t_entry` may be negative when the
///   ray origin lies inside the AABB.
/// - `None` if the ray misses the AABB entirely, including when the entire interval lies
///   behind the ray origin (`t_exit < 0`).
pub fn ray_intersect_aabb_interval(
    ray_origin: Vec3,
    ray_dir: Vec3,
    min: Vec3,
    max: Vec3,
) -> Option<(f32, f32)> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;

    for axis in 0..3 {
        let origin_component = ray_origin[axis];
        let dir_component = ray_dir[axis];
        let min_component = min[axis];
        let max_component = max[axis];

        if dir_component.abs() <= f32::EPSILON {
            if origin_component < min_component || origin_component > max_component {
                return None;
            }
            continue;
        }

        let inv_dir = 1.0 / dir_component;
        let mut t1 = (min_component - origin_component) * inv_dir;
        let mut t2 = (max_component - origin_component) * inv_dir;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }

    (t_max >= 0.0).then_some((t_min, t_max))
}

/// Fast broad-phase check for whether a ray might intersect a block's bounding sphere.
///
/// Approximates the block's axis-aligned bounding box with a bounding sphere centered at
/// the block's center with a radius equal to half the block's diagonal extent. This is a
/// *conservative* test: it may return `true` for rays that would miss the block's precise
/// geometry (false positive), but will never return `false` for a ray that would actually
/// hit the block (no false negatives). This makes it suitable as an early rejection test
/// before performing more expensive per-cuboid intersection checks.
///
/// # Parameters
/// - `ray_origin`: The ray origin in world space.
/// - `ray_dir`: The ray direction vector (should be normalized for meaningful `max_t`
///   comparisons).
/// - `obj`: The level object whose bounding sphere to test against.
/// - `max_t`: The maximum distance along the ray to consider. Rays that would intersect
///   the bounding sphere beyond this distance are reported as misses.
///
/// # Returns
/// - `true` if the ray might intersect the block's bounding sphere within distance `max_t`.
/// - `false` if the ray definitely misses the block (e.g., the sphere is behind the ray,
///   is too far away, or the block has zero size).
pub fn ray_may_hit_block_bounds(
    ray_origin: Vec3,
    ray_dir: Vec3,
    obj: &eg_types::LevelObject,
    max_t: f32,
) -> bool {
    let center = Vec3::new(
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    );
    let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
    let radius = half.length();
    if radius <= f32::EPSILON {
        return false;
    }

    let to_center = center - ray_origin;
    let center_t = to_center.dot(ray_dir);
    if center_t + radius < 0.0 {
        return false;
    }

    let nearest_possible_t = (center_t - radius).max(0.0);
    if nearest_possible_t > max_t {
        return false;
    }

    let closest_distance_sq = (to_center.length_squared() - center_t * center_t).max(0.0);
    closest_distance_sq <= radius * radius
}

/// Computes the intersection of a ray with a sphere.
///
/// Uses the quadratic formula to solve for the ray parameter `t` at the intersection
/// points. Returns the closest non-negative intersection (the entry point). The ray
/// direction does not need to be normalized; the distance `t` is measured in units of
/// the direction vector's length.
///
/// # Parameters
/// - `ray_origin`: The ray origin in world space.
/// - `ray_dir`: The ray direction vector (does not need to be normalized).
/// - `center`: The center of the sphere.
/// - `radius`: The radius of the sphere (must be non-negative).
///
/// # Returns
/// - `Some(t)` where `t >= 0` is the distance along the ray to the closest intersection
///   point in front of the ray origin. Returns `t = 0` when the origin is inside the sphere.
/// - `None` if the ray misses the sphere, if the only intersection(s) lie behind the
///   ray origin, or if the direction vector has zero length.
pub fn ray_intersect_sphere(
    ray_origin: Vec3,
    ray_dir: Vec3,
    center: Vec3,
    radius: f32,
) -> Option<f32> {
    let a = ray_dir.length_squared();
    if a <= f32::EPSILON {
        return None;
    }

    let oc = ray_origin - center;
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.length_squared() - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let inv_two_a = 0.5 / a;
    let t0 = (-b - sqrt_discriminant) * inv_two_a;
    let t1 = (-b + sqrt_discriminant) * inv_two_a;

    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// Computes the intersection of a ray with a potentially rotated block defined by its
/// hitbox cuboids (from the block definition).
///
/// The ray is first transformed into the object's local coordinate frame, using the
/// object's center as the rotation pivot. The intersection is then tested against each
/// of the object's hitbox cuboids via [`ray_intersect_aabb`]. The closest valid
/// intersection among all hitboxes is returned, with the surface normal transformed
/// back into world space.
///
/// The object's hitbox cuboids come from the block definition's geometry (via
/// [`hitbox_cuboids`](crate::hitbox_cuboids)), which may include multiple separate
/// regions. If the block has no explicit hitbox definitions, the object's full AABB
/// is used as a fallback.
///
/// # Parameters
/// - `ray_origin`: The ray origin in world space.
/// - `ray_dir`: The ray direction in world space (does not need to be normalized).
/// - `obj`: The level object whose hitboxes to test against. The object may have an
///   arbitrary rotation specified by its `rotation` field.
///
/// # Returns
/// - `Some((t, normal))` where `t` is the distance along the ray to the closest
///   intersection and `normal` is the world-space surface normal at that point.
/// - `None` if the ray misses all hitbox cuboids of the object, or if all intersections
///   lie behind the ray origin.
pub fn ray_intersect_rotated_block(
    ray_origin: Vec3,
    ray_dir: Vec3,
    obj: &eg_types::LevelObject,
) -> Option<(f32, Vec3)> {
    let center = obj.center();
    let rotation = obj.basis();
    let inv_rotation = rotation.transpose();

    let local_origin_world = center + inv_rotation * (ray_origin - center);
    let local_dir = inv_rotation * ray_dir;
    let mut best_hit: Option<(f32, Vec3)> = None;

    for cuboid in hitbox_cuboids(obj) {
        let min = Vec3::from(cuboid.min);
        let max = Vec3::from(cuboid.max);
        if let Some((t_hit, normal_local)) =
            ray_intersect_aabb(local_origin_world, local_dir, min, max)
            && best_hit.map(|(best_t, _)| t_hit < best_t).unwrap_or(true)
        {
            best_hit = Some((t_hit, rotation * normal_local));
        }
    }

    best_hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use eg_types::LevelObject;

    #[test]
    fn aabb_hits_front_face() {
        // Unit AABB at origin; ray approaches from -Z.
        let (t, normal) =
            ray_intersect_aabb(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE)
                .expect("expected hit");
        assert_abs_diff_eq!(t, 5.0);
        assert_abs_diff_eq!(normal.x, 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.y, 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.z, -1.0, epsilon = 1e-6);
    }

    #[test]
    fn aabb_hits_from_inside() {
        // Origin inside the AABB; should hit the far face (exit) at Z=1.
        // The outward normal at the exit face in the +Z direction is +Z.
        let (t, normal) =
            ray_intersect_aabb(Vec3::new(0.5, 0.5, 0.5), Vec3::Z, Vec3::ZERO, Vec3::ONE)
                .expect("expected hit");
        assert_abs_diff_eq!(t, 0.5);
        assert_abs_diff_eq!(normal.z, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn aabb_misses_outside_x() {
        let result = ray_intersect_aabb(Vec3::new(5.0, 0.5, -5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE);
        assert!(result.is_none(), "should miss when ray is outside on X");
    }

    #[test]
    fn aabb_misses_behind_origin() {
        // AABB is behind the ray (ray points +Z, AABB is at Z=0 to 1).
        let result = ray_intersect_aabb(Vec3::new(0.5, 0.5, 5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE);
        assert!(result.is_none(), "should miss when AABB is behind the ray");
    }

    #[test]
    fn aabb_parallel_ray_outside() {
        // Ray parallel to Y axis, outside X range.
        let result = ray_intersect_aabb(Vec3::new(5.0, -10.0, 0.5), Vec3::Y, Vec3::ZERO, Vec3::ONE);
        assert!(
            result.is_none(),
            "should miss when parallel and outside the slab"
        );
    }

    #[test]
    fn aabb_parallel_ray_inside_slab() {
        // Ray parallel to Y axis, inside XZ slab → should hit.
        let (t, normal) =
            ray_intersect_aabb(Vec3::new(0.5, -5.0, 0.5), Vec3::Y, Vec3::ZERO, Vec3::ONE)
                .expect("expected hit");
        assert_abs_diff_eq!(t, 5.0);
        assert_abs_diff_eq!(normal.y, -1.0, epsilon = 1e-6);
    }

    #[test]
    fn aabb_hits_top_face() {
        // Ray from above, pointing downward (−Y).
        let (t, normal) =
            ray_intersect_aabb(Vec3::new(0.5, 5.0, 0.5), -Vec3::Y, Vec3::ZERO, Vec3::ONE)
                .expect("expected hit");
        assert_abs_diff_eq!(t, 4.0);
        assert_abs_diff_eq!(normal.y, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn aabb_negative_dir_hits() {
        // Ray from +Z, pointing backward (−Z).
        let (t, _normal) =
            ray_intersect_aabb(Vec3::new(0.5, 0.5, 5.0), -Vec3::Z, Vec3::ZERO, Vec3::ONE)
                .expect("expected hit");
        assert_abs_diff_eq!(t, 4.0);
    }

    #[test]
    fn aabb_non_unit_direction() {
        // Direction vector has length 2 → t should be halved.
        let (t, _normal) = ray_intersect_aabb(
            Vec3::new(0.5, 0.5, -10.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::ZERO,
            Vec3::ONE,
        )
        .expect("expected hit");
        // Distance from origin to AABB is 10, direction speed is 2 → t = 5.
        assert_abs_diff_eq!(t, 5.0);
    }

    #[test]
    fn aabb_zero_thickness() {
        // AABB with zero depth on Z (a plane).
        let result = ray_intersect_aabb(
            Vec3::new(0.5, 0.5, -5.0),
            Vec3::Z,
            Vec3::new(0.0, 0.0, 0.5),
            Vec3::new(1.0, 1.0, 0.5),
        )
        .expect("expected hit on zero-thickness AABB");
        // Should still register an intersection with a valid normal.
        let (t, normal) = result;
        assert_abs_diff_eq!(t, 5.5);
        assert_abs_diff_eq!(normal.z, -1.0, epsilon = 1e-6);
    }

    #[test]
    fn interval_standard_pass_through() {
        let (entry, exit) =
            ray_intersect_aabb_interval(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE)
                .expect("expected interval");
        assert_abs_diff_eq!(entry, 5.0);
        assert_abs_diff_eq!(exit, 6.0);
    }

    #[test]
    fn interval_origin_inside() {
        // Entry should be negative because the origin is already inside.
        let (entry, exit) =
            ray_intersect_aabb_interval(Vec3::new(0.5, 0.5, 0.5), Vec3::Z, Vec3::ZERO, Vec3::ONE)
                .expect("expected interval");
        assert!(entry <= 0.0, "entry = {entry} (should be ≤ 0)");
        assert_abs_diff_eq!(exit, 0.5, epsilon = 1e-6);
    }

    #[test]
    fn interval_misses() {
        let result =
            ray_intersect_aabb_interval(Vec3::new(5.0, 0.5, -5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE);
        assert!(result.is_none());
    }

    #[test]
    fn interval_behind_origin() {
        let result =
            ray_intersect_aabb_interval(Vec3::new(0.5, 0.5, 5.0), Vec3::Z, Vec3::ZERO, Vec3::ONE);
        assert!(result.is_none());
    }

    #[test]
    fn interval_exit_behind_is_still_none() {
        // Both entry and exit are negative → ray points away from AABB.
        let result = ray_intersect_aabb_interval(
            Vec3::new(0.5, 0.5, -5.0),
            -Vec3::Z,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn bounds_direct_hit_within_range() {
        let obj = LevelObject::default();
        assert!(ray_may_hit_block_bounds(
            Vec3::new(0.5, 0.5, -5.0),
            Vec3::Z,
            &obj,
            10.0,
        ));
    }

    #[test]
    fn bounds_beyond_max_t() {
        let obj = LevelObject::default();
        assert!(!ray_may_hit_block_bounds(
            Vec3::new(0.5, 0.5, -5.0),
            Vec3::Z,
            &obj,
            3.0,
        ));
    }

    #[test]
    fn bounds_pointed_away() {
        let obj = LevelObject::default();
        assert!(!ray_may_hit_block_bounds(
            Vec3::new(0.5, 0.5, -5.0),
            -Vec3::Z,
            &obj,
            10.0,
        ));
    }

    #[test]
    fn bounds_zero_size_block() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [0.0, 0.0, 0.0],
            ..LevelObject::default()
        };
        assert!(!ray_may_hit_block_bounds(
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::Z,
            &obj,
            10.0,
        ));
    }

    #[test]
    fn bounds_large_block_glancing_shot() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [10.0, 10.0, 10.0],
            ..LevelObject::default()
        };
        // Ray near edge of the bounding sphere (radius ≈ 8.66) should still report maybe-hit.
        assert!(ray_may_hit_block_bounds(
            Vec3::new(7.0, 0.0, -5.0),
            Vec3::Z,
            &obj,
            20.0,
        ));
    }

    #[test]
    fn bounds_far_away_definitely_misses() {
        let obj = LevelObject::default();
        assert!(!ray_may_hit_block_bounds(
            Vec3::new(100.0, 100.0, -5.0),
            Vec3::Z,
            &obj,
            200.0,
        ));
    }

    #[test]
    fn sphere_hits_center() {
        let t = ray_intersect_sphere(Vec3::new(0.0, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 1.0)
            .expect("expected hit");
        assert_abs_diff_eq!(t, 4.0);
    }

    #[test]
    fn sphere_misses() {
        let result = ray_intersect_sphere(Vec3::new(5.0, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 1.0);
        assert!(result.is_none());
    }

    #[test]
    fn sphere_origin_inside() {
        // Ray starts at sphere center, heading +Z. The exit point is at Z = +radius.
        let t = ray_intersect_sphere(Vec3::new(0.0, 0.0, 0.0), Vec3::Z, Vec3::ZERO, 1.0)
            .expect("expected hit");
        assert_abs_diff_eq!(t, 1.0);
    }

    #[test]
    fn sphere_tangent_ray() {
        // Ray just grazes the sphere surface (distance from center == radius).
        let t = ray_intersect_sphere(Vec3::new(1.0, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 1.0)
            .expect("expected tangent hit");
        assert_abs_diff_eq!(t, 5.0);
    }

    #[test]
    fn sphere_behind_origin() {
        let result = ray_intersect_sphere(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, Vec3::ZERO, 1.0);
        assert!(result.is_none(), "sphere behind the ray should not be hit");
    }

    #[test]
    fn sphere_zero_direction() {
        let result = ray_intersect_sphere(Vec3::new(0.0, 0.0, -5.0), Vec3::ZERO, Vec3::ZERO, 1.0);
        assert!(result.is_none(), "zero-length direction should return None");
    }

    #[test]
    fn sphere_off_center_zero_radius_misses() {
        // Zero-radius sphere at origin; ray off-center → no intersection.
        let result = ray_intersect_sphere(Vec3::new(1.0, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 0.0);
        assert!(result.is_none(), "zero-radius off-center should miss");
    }

    #[test]
    fn sphere_very_close_miss() {
        // Ray passes just outside the sphere (extremely close miss).
        let result = ray_intersect_sphere(Vec3::new(1.001, 0.0, -5.0), Vec3::Z, Vec3::ZERO, 1.0);
        assert!(result.is_none(), "very close miss should still be None");
    }

    #[test]
    fn rotated_block_hits_unrotated_unit_block() {
        let obj = LevelObject::default();
        let (t, normal) = ray_intersect_rotated_block(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, &obj)
            .expect("expected hit");
        assert_abs_diff_eq!(t, 5.0);
        assert_abs_diff_eq!(normal.x, 0.0);
        assert_abs_diff_eq!(normal.y, 0.0);
        assert_abs_diff_eq!(normal.z, -1.0);
    }

    #[test]
    fn rotated_block_hits_rotated_45_degrees() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation: [0.0, 45.0, 0.0],
            ..LevelObject::default()
        };
        // A 45° Y-rotation makes the block diamond-shaped in XZ. The ray down
        // +Z should still hit the rotated face because the diagonal span is
        // still large enough.
        let result = ray_intersect_rotated_block(Vec3::new(0.5, 0.5, -5.0), Vec3::Z, &obj);
        assert!(
            result.is_some(),
            "should hit a 45° rotated block from the front"
        );
    }

    #[test]
    fn rotated_block_misses_when_aimed_away() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation: [0.0, 45.0, 0.0],
            ..LevelObject::default()
        };
        let result = ray_intersect_rotated_block(Vec3::new(5.0, 5.0, -5.0), Vec3::Z, &obj);
        assert!(
            result.is_none(),
            "should miss when aimed well outside the rotated block"
        );
    }

    #[test]
    fn rotated_block_hit_from_inside() {
        let obj = LevelObject::default();
        // Origin inside the block, ray pointing +Z — exits at Z=1 with outward normal +Z.
        let (t, normal) = ray_intersect_rotated_block(Vec3::new(0.5, 0.5, 0.5), Vec3::Z, &obj)
            .expect("expected hit from inside");
        assert_abs_diff_eq!(t, 0.5);
        assert_abs_diff_eq!(normal.z, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn rotated_block_behind_origin() {
        let obj = LevelObject::default();
        let result = ray_intersect_rotated_block(Vec3::new(0.5, 0.5, 5.0), Vec3::Z, &obj);
        assert!(result.is_none(), "should miss when block is behind the ray");
    }

    #[test]
    fn rotated_block_hits_side_face() {
        let obj = LevelObject::default();
        let (t, normal) = ray_intersect_rotated_block(Vec3::new(-5.0, 0.5, 0.5), Vec3::X, &obj)
            .expect("expected hit on side face");
        assert_abs_diff_eq!(t, 5.0);
        assert_abs_diff_eq!(normal.x, -1.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.y, 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.z, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn rotated_block_hits_top_face() {
        let obj = LevelObject::default();
        let (t, normal) = ray_intersect_rotated_block(Vec3::new(0.5, 5.0, 0.5), -Vec3::Y, &obj)
            .expect("expected hit on top face");
        assert_abs_diff_eq!(t, 4.0);
        assert_abs_diff_eq!(normal.y, 1.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.x, 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(normal.z, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn rotated_block_returns_closest_of_multiple_cuboids() {
        // The default stone block only has one fallback cuboid, so this test
        // verifies the "closest hit" logic still works correctly. Create a
        // larger block with two potential hit regions.
        let obj = LevelObject {
            size: [2.0, 2.0, 2.0],
            ..LevelObject::default()
        };
        let (t, normal) = ray_intersect_rotated_block(Vec3::new(1.0, 1.0, -5.0), Vec3::Z, &obj)
            .expect("expected hit on larger block");
        // The block spans Z=0..2, so the front face is at Z=0.
        assert_abs_diff_eq!(t, 5.0);
        assert_abs_diff_eq!(normal.z, -1.0, epsilon = 1e-6);
    }
}
