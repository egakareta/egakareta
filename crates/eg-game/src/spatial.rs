/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use bevy_ecs::prelude::*;
use eg_block_geometry::{WorldCuboid, hitbox_cuboids, rotated_cuboid_aabb_xz};
use eg_types::LevelObject;

pub const GRID_CELL_SIZE: f32 = 4.0;

/// Spatial grid keyed by `Entity` (Bevy entity id). Replaces the
/// `HashMap<(i32, i32), Vec<usize>>` from the pre-ECS version.
#[derive(Default)]
pub struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
}

impl SpatialGrid {
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert_object(&mut self, entity: Entity, obj: &LevelObject) {
        let hitboxes = hitbox_cuboids(obj);
        self.insert_object_with_hitboxes(entity, obj, &hitboxes);
    }

    pub fn insert_object_with_hitboxes(
        &mut self,
        entity: Entity,
        obj: &LevelObject,
        hitboxes: &[WorldCuboid],
    ) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for &cuboid in hitboxes {
            let [cuboid_min_x, cuboid_max_x, cuboid_min_z, cuboid_max_z] =
                rotated_cuboid_aabb_xz(obj, cuboid);
            min_x = min_x.min(cuboid_min_x);
            max_x = max_x.max(cuboid_max_x);
            min_z = min_z.min(cuboid_min_z);
            max_z = max_z.max(cuboid_max_z);
        }

        self.insert_object_in_bounds(entity, min_x, max_x, min_z, max_z);
    }

    pub fn insert_object_with_cached_hitboxes(
        &mut self,
        entity: Entity,
        hitboxes: &crate::components::CachedBlockHitboxes,
    ) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for hitbox in hitboxes.iter() {
            for point in hitbox.polygon() {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_z = min_z.min(point.y);
                max_z = max_z.max(point.y);
            }
        }

        self.insert_object_in_bounds(entity, min_x, max_x, min_z, max_z);
    }

    fn insert_object_in_bounds(
        &mut self,
        entity: Entity,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) {
        if !min_x.is_finite() || !max_x.is_finite() || !min_z.is_finite() || !max_z.is_finite() {
            return;
        }

        let start_x = (min_x / GRID_CELL_SIZE).floor() as i32;
        let end_x = (max_x / GRID_CELL_SIZE).floor() as i32;
        let start_z = (min_z / GRID_CELL_SIZE).floor() as i32;
        let end_z = (max_z / GRID_CELL_SIZE).floor() as i32;

        for gx in start_x..=end_x {
            for gz in start_z..=end_z {
                self.cells.entry((gx, gz)).or_default().push(entity);
            }
        }
    }

    /// Query the grid for entities whose AABBs intersect the given AABB, and
    /// append their ids to `results`. Filters duplicates that may be
    /// inserted at cell boundaries.
    pub fn query_aabb_into(
        &self,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        results: &mut Vec<Entity>,
    ) {
        let start_x = (min_x / GRID_CELL_SIZE).floor() as i32;
        let end_x = (max_x / GRID_CELL_SIZE).floor() as i32;
        let start_z = (min_z / GRID_CELL_SIZE).floor() as i32;
        let end_z = (max_z / GRID_CELL_SIZE).floor() as i32;

        for gx in start_x..=end_x {
            for gz in start_z..=end_z {
                if let Some(entities) = self.cells.get(&(gx, gz)) {
                    for &entity in entities {
                        if !results.contains(&entity) {
                            results.push(entity);
                        }
                    }
                }
            }
        }
    }

    /// Convenience: query and return a new `Vec`. Allocates - prefer
    /// `query_aabb_into` in hot paths.
    pub fn query_aabb(&self, min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Vec<Entity> {
        let mut results = Vec::new();
        self.query_aabb_into(min_x, max_x, min_z, max_z, &mut results);
        results
    }

    /// Point query - returns the entities whose AABB cell contains `(x, z)`.
    /// Used by editor hit-tests.
    pub fn query_point(&self, x: f32, z: f32) -> &[Entity] {
        let gx = (x / GRID_CELL_SIZE).floor() as i32;
        let gz = (z / GRID_CELL_SIZE).floor() as i32;
        self.cells
            .get(&(gx, gz))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
