/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::Vertex;

pub(crate) fn build_grid_vertices() -> Vec<Vertex> {
    let mut grid_vertices: Vec<Vertex> = Vec::new();
    let extent = 60.0;
    let step = 1.0;
    let grid_color = [0.2, 0.22, 0.26, 1.0];
    let line_width = 0.02;
    let grid_y = 0.01;

    let mut x = -extent;
    while x <= extent {
        let x_min = x - line_width;
        let x_max = x + line_width;
        grid_vertices.push(Vertex {
            position: [x_min, grid_y, -extent],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, grid_y, -extent],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, grid_y, extent],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_min, grid_y, -extent],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, grid_y, extent],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_min, grid_y, extent],
            color: grid_color,
        });
        x += step;
    }

    let mut z = -extent;
    while z <= extent {
        let z_min = z - line_width;
        let z_max = z + line_width;
        grid_vertices.push(Vertex {
            position: [-extent, grid_y, z_min],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, grid_y, z_min],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, grid_y, z_max],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [-extent, grid_y, z_min],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, grid_y, z_max],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [-extent, grid_y, z_max],
            color: grid_color,
        });
        z += step;
    }

    grid_vertices
}
