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
        // Calculate the bounding box of the object in world coordinates
        // For simplicity, we use the AABB even if rotated, as it's just for broad-phase
        let (min_x, max_x, min_z, max_z) = if obj.rotation_degrees.abs() < 0.001 {
            (
                obj.position[0],
                obj.position[0] + obj.size[0],
                obj.position[2],
                obj.position[2] + obj.size[2],
            )
        } else {
            // For rotated blocks, a conservative AABB
            let rad = obj.rotation_degrees.to_radians();
            let cos = rad.cos().abs();
            let sin = rad.sin().abs();
            let half_w = obj.size[0] * 0.5;
            let half_d = obj.size[2] * 0.5;

            let extent_x = half_w * cos + half_d * sin;
            let extent_z = half_w * sin + half_d * cos;

            let center_x = obj.position[0] + obj.size[0] * 0.5;
            let center_z = obj.position[2] + obj.size[2] * 0.5;

            (
                center_x - extent_x,
                center_x + extent_x,
                center_z - extent_z,
                center_z + extent_z,
            )
        };

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

    pub(crate) fn query_point(&self, x: f32, z: f32) -> &[usize] {
        let gx = (x / GRID_CELL_SIZE).floor() as i32;
        let gz = (z / GRID_CELL_SIZE).floor() as i32;
        self.cells
            .get(&(gx, gz))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
