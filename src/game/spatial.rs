/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::block_geometry::{effective_hitbox_cuboids, rotated_cuboid_aabb_xz};
use crate::types::LevelObject;
use std::collections::HashMap;

pub(crate) const GRID_CELL_SIZE: f32 = 4.0;

#[derive(Default)]
pub(crate) struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl SpatialGrid {
    pub(crate) fn clear(&mut self) {
        self.cells.clear();
    }

    pub(crate) fn insert_object(&mut self, index: usize, obj: &LevelObject) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for cuboid in effective_hitbox_cuboids(obj) {
            let [cuboid_min_x, cuboid_max_x, cuboid_min_z, cuboid_max_z] =
                rotated_cuboid_aabb_xz(obj, cuboid);
            min_x = min_x.min(cuboid_min_x);
            max_x = max_x.max(cuboid_max_x);
            min_z = min_z.min(cuboid_min_z);
            max_z = max_z.max(cuboid_max_z);
        }

        if !min_x.is_finite() || !max_x.is_finite() || !min_z.is_finite() || !max_z.is_finite() {
            return;
        }

        let start_x = (min_x / GRID_CELL_SIZE).floor() as i32;
        let end_x = (max_x / GRID_CELL_SIZE).floor() as i32;
        let start_z = (min_z / GRID_CELL_SIZE).floor() as i32;
        let end_z = (max_z / GRID_CELL_SIZE).floor() as i32;

        for gx in start_x..=end_x {
            for gz in start_z..=end_z {
                self.cells.entry((gx, gz)).or_default().push(index);
            }
        }
    }

    pub(crate) fn query_aabb(&self, min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Vec<usize> {
        let start_x = (min_x / GRID_CELL_SIZE).floor() as i32;
        let end_x = (max_x / GRID_CELL_SIZE).floor() as i32;
        let start_z = (min_z / GRID_CELL_SIZE).floor() as i32;
        let end_z = (max_z / GRID_CELL_SIZE).floor() as i32;

        let mut results = Vec::new();
        // Use a small fixed-size buffer or HashSet if we expect many overlapping cells,
        // but for a player-sized AABB, it usually hits 1-4 cells.
        for gx in start_x..=end_x {
            for gz in start_z..=end_z {
                if let Some(indices) = self.cells.get(&(gx, gz)) {
                    for &idx in indices {
                        if !results.contains(&idx) {
                            results.push(idx);
                        }
                    }
                }
            }
        }
        results
    }

    #[cfg(test)]
    pub(crate) fn query_point(&self, x: f32, z: f32) -> &[usize] {
        let gx = (x / GRID_CELL_SIZE).floor() as i32;
        let gz = (z / GRID_CELL_SIZE).floor() as i32;
        self.cells
            .get(&(gx, gz))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
