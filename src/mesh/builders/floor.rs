use crate::types::Vertex;

pub(crate) fn build_floor_vertices() -> Vec<Vertex> {
    let mut floor_vertices: Vec<Vertex> = Vec::new();
    let tile_color_top = [0.08, 0.08, 0.1, 1.0];
    let tile_color_side = [0.05, 0.05, 0.07, 1.0];
    let extent = 60;
    let tile_height = 0.1;
    let tile_margin = 0.05;

    for x in -extent..extent {
        for y in -extent..extent {
            let x_min = x as f32 + tile_margin;
            let x_max = (x + 1) as f32 - tile_margin;
            let y_min = y as f32 + tile_margin;
            let y_max = (y + 1) as f32 - tile_margin;
            let z_min = -tile_height;
            let z_max = 0.0;

            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_max],
                color: tile_color_top,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_max],
                color: tile_color_top,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_top,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_max],
                color: tile_color_top,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_top,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_max],
                color: tile_color_top,
            });

            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_max],
                color: tile_color_side,
            });

            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_max],
                color: tile_color_side,
            });

            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_max, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_max, y_min, z_max],
                color: tile_color_side,
            });

            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_min],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_min, z_max],
                color: tile_color_side,
            });
            floor_vertices.push(Vertex {
                position: [x_min, y_max, z_max],
                color: tile_color_side,
            });
        }
    }

    floor_vertices
}
