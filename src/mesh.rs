use crate::types::{LevelObject, Vertex};

pub(crate) fn build_floor_vertices() -> Vec<Vertex> {
    let mut floor_vertices: Vec<Vertex> = Vec::new();
    let tile_color_top = [0.08, 0.08, 0.1];
    let tile_color_side = [0.05, 0.05, 0.07];
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

pub(crate) fn build_grid_vertices() -> Vec<Vertex> {
    let mut grid_vertices: Vec<Vertex> = Vec::new();
    let extent = 60.0;
    let step = 1.0;
    let grid_color = [0.2, 0.22, 0.26];
    let line_width = 0.02;
    let grid_z = 0.01;

    let mut x = -extent;
    while x <= extent {
        let x_min = x - line_width;
        let x_max = x + line_width;
        grid_vertices.push(Vertex {
            position: [x_min, -extent, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, -extent, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, extent, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_min, -extent, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_max, extent, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [x_min, extent, grid_z],
            color: grid_color,
        });
        x += step;
    }

    let mut y = -extent;
    while y <= extent {
        let y_min = y - line_width;
        let y_max = y + line_width;
        grid_vertices.push(Vertex {
            position: [-extent, y_min, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, y_min, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, y_max, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [-extent, y_min, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [extent, y_max, grid_z],
            color: grid_color,
        });
        grid_vertices.push(Vertex {
            position: [-extent, y_max, grid_z],
            color: grid_color,
        });
        y += step;
    }

    grid_vertices
}

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color_top = [0.4, 0.4, 0.45];
    let color_side = [0.2, 0.2, 0.25];
    let z_min = 0.0;
    let z_max = 1.0;

    for obj in objects {
        let x_min = obj.position[0];
        let x_max = obj.position[0] + obj.size[0];
        let y_min = obj.position[1];
        let y_max = obj.position[1] + obj.size[1];

        vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: color_top,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: color_top,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_top,
        });
        vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: color_top,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_top,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: color_top,
        });

        vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: color_side,
        });

        vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: color_side,
        });

        vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: color_side,
        });

        vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: color_side,
        });
        vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: color_side,
        });
    }

    vertices
}

pub(crate) fn build_trail_vertices(points: &[[f32; 2]], game_over: bool) -> Vec<Vertex> {
    let mut trail_vertices = Vec::new();
    let width = 0.8;
    let z_min = 0.3;
    let z_max = 0.8;
    let c_top = if game_over {
        [1.0, 0.2, 0.2]
    } else {
        [0.8, 0.25, 0.35]
    };
    let c_side = if game_over {
        [0.8, 0.1, 0.1]
    } else {
        [0.7, 0.2, 0.3]
    };

    if points.len() < 2 {
        return trail_vertices;
    }

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];

        let (x_min, x_max, y_min, y_max) = if dx.abs() > dy.abs() {
            (
                p1[0].min(p2[0]) - width / 2.0,
                p1[0].max(p2[0]) + width / 2.0,
                p1[1] - width / 2.0,
                p1[1] + width / 2.0,
            )
        } else {
            (
                p1[0] - width / 2.0,
                p1[0] + width / 2.0,
                p1[1].min(p2[1]) - width / 2.0,
                p1[1].max(p2[1]) + width / 2.0,
            )
        };

        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: c_top,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: c_top,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_top,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: c_top,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_top,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: c_top,
        });

        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: c_side,
        });

        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: c_side,
        });

        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_max, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_max, y_min, z_max],
            color: c_side,
        });

        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_min],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_min, z_max],
            color: c_side,
        });
        trail_vertices.push(Vertex {
            position: [x_min, y_max, z_max],
            color: c_side,
        });
    }

    trail_vertices
}
