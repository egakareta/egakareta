use crate::types::Vertex;

pub(crate) fn append_prism(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    color_top: [f32; 4],
    color_side: [f32; 4],
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

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

pub(crate) fn append_quad(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    color: [f32; 4],
) {
    vertices.push(Vertex {
        position: p0,
        color,
    });
    vertices.push(Vertex {
        position: p1,
        color,
    });
    vertices.push(Vertex {
        position: p2,
        color,
    });
    vertices.push(Vertex {
        position: p0,
        color,
    });
    vertices.push(Vertex {
        position: p2,
        color,
    });
    vertices.push(Vertex {
        position: p3,
        color,
    });
}

pub(crate) fn append_top_fan(
    vertices: &mut Vec<Vertex>,
    points: &[[f32; 2]],
    z: f32,
    color: [f32; 4],
) {
    if points.len() < 3 {
        return;
    }

    let center = {
        let mut cx = 0.0;
        let mut cy = 0.0;
        for p in points {
            cx += p[0];
            cy += p[1];
        }
        let inv = 1.0 / points.len() as f32;
        [cx * inv, cy * inv, z]
    };

    for i in 0..points.len() {
        let next = (i + 1) % points.len();
        vertices.push(Vertex {
            position: center,
            color,
        });
        vertices.push(Vertex {
            position: [points[i][0], points[i][1], z],
            color,
        });
        vertices.push(Vertex {
            position: [points[next][0], points[next][1], z],
            color,
        });
    }
}

pub(crate) fn build_rounded_rect_points(
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    radius: f32,
    corner_segments: usize,
) -> Vec<[f32; 2]> {
    let half_w = (x_max - x_min) * 0.5;
    let half_h = (y_max - y_min) * 0.5;
    let r = radius.clamp(0.0, half_w.min(half_h));

    if r <= f32::EPSILON || corner_segments == 0 {
        return vec![
            [x_max, y_min],
            [x_max, y_max],
            [x_min, y_max],
            [x_min, y_min],
        ];
    }

    let mut points = Vec::with_capacity(corner_segments * 4);
    let arc_defs = [
        (x_max - r, y_min + r, -90.0f32, 0.0f32),
        (x_max - r, y_max - r, 0.0f32, 90.0f32),
        (x_min + r, y_max - r, 90.0f32, 180.0f32),
        (x_min + r, y_min + r, 180.0f32, 270.0f32),
    ];

    for (arc_index, (cx, cy, start_deg, end_deg)) in arc_defs.into_iter().enumerate() {
        for step in 0..=corner_segments {
            if arc_index > 0 && step == 0 {
                continue;
            }
            let t = step as f32 / corner_segments as f32;
            let angle = (start_deg + (end_deg - start_deg) * t).to_radians();
            points.push([cx + r * angle.cos(), cy + r * angle.sin()]);
        }
    }

    points
}

pub(crate) fn append_rounded_prism(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    color_top: [f32; 4],
    color_side: [f32; 4],
    corner_radius: f32,
    corner_segments: usize,
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    let points =
        build_rounded_rect_points(x_min, x_max, y_min, y_max, corner_radius, corner_segments);
    let bevel_height = ((z_max - z_min) * 0.22).min(corner_radius * 0.9).max(0.0);
    let z_bevel = (z_max - bevel_height).max(z_min);
    let inset = (corner_radius * 0.28)
        .min((x_max - x_min) * 0.25)
        .min((y_max - y_min) * 0.25)
        .max(0.0);

    let top_points = build_rounded_rect_points(
        x_min + inset,
        x_max - inset,
        y_min + inset,
        y_max - inset,
        (corner_radius - inset).max(0.0),
        corner_segments,
    );

    append_top_fan(vertices, &top_points, z_max, color_top);

    if points.len() == top_points.len() {
        for i in 0..points.len() {
            let next = (i + 1) % points.len();
            append_quad(
                vertices,
                [points[i][0], points[i][1], z_bevel],
                [points[next][0], points[next][1], z_bevel],
                [top_points[next][0], top_points[next][1], z_max],
                [top_points[i][0], top_points[i][1], z_max],
                color_side,
            );
        }
    }

    for i in 0..points.len() {
        let next = (i + 1) % points.len();
        append_quad(
            vertices,
            [points[i][0], points[i][1], z_min],
            [points[next][0], points[next][1], z_min],
            [points[next][0], points[next][1], z_bevel],
            [points[i][0], points[i][1], z_bevel],
            color_side,
        );
    }
}
