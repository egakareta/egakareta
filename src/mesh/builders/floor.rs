/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::mesh::shapes::append_prism;
use crate::types::Vertex;

pub(crate) fn build_floor_vertices() -> Vec<Vertex> {
    let mut floor_vertices: Vec<Vertex> = Vec::new();
    let tile_color_top = [0.08, 0.08, 0.1, 1.0];
    let tile_color_side = [0.05, 0.05, 0.07, 1.0];
    let extent = 60;
    let tile_height = 0.1;
    let tile_margin = 0.05;

    for x in -extent..extent {
        for z in -extent..extent {
            let x_min = x as f32 + tile_margin;
            let x_max = (x + 1) as f32 - tile_margin;
            let z_min = z as f32 + tile_margin;
            let z_max = (z + 1) as f32 - tile_margin;
            let y_min = -tile_height;
            let y_max = 0.0;

            append_prism(
                &mut floor_vertices,
                [x_min, y_min, z_min],
                [x_max, y_max, z_max],
                tile_color_top,
                tile_color_side,
            );
        }
    }

    floor_vertices
}
