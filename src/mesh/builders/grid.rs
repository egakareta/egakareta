use crate::types::Vertex;

pub(crate) fn build_grid_vertices() -> Vec<Vertex> {
    let mut grid_vertices: Vec<Vertex> = Vec::new();
    let extent = 60.0;
    let step = 1.0;
    let grid_color = [0.2, 0.22, 0.26, 1.0];
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
