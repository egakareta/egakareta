use crate::types::LevelObject;
use glam::{EulerRot, Mat3};
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
        let center_x = obj.position[0] + obj.size[0] * 0.5;
        let center_z = obj.position[2] + obj.size[2] * 0.5;
        let half_x = obj.size[0] * 0.5;
        let half_y = obj.size[1] * 0.5;
        let half_z = obj.size[2] * 0.5;
        let rotation = Mat3::from_euler(
            EulerRot::XYZ,
            obj.rotation_degrees[0].to_radians(),
            obj.rotation_degrees[1].to_radians(),
            obj.rotation_degrees[2].to_radians(),
        );
        let matrix = rotation.to_cols_array_2d();

        let extent_x =
            matrix[0][0].abs() * half_x + matrix[1][0].abs() * half_y + matrix[2][0].abs() * half_z;
        let extent_z =
            matrix[0][2].abs() * half_x + matrix[1][2].abs() * half_y + matrix[2][2].abs() * half_z;

        let min_x = center_x - extent_x;
        let max_x = center_x + extent_x;
        let min_z = center_z - extent_z;
        let max_z = center_z + extent_z;

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
