use crate::types::Vertex;

pub(crate) fn pseudo_random_noise(x: f32, y: f32, z: f32) -> f32 {
    let seed = ((x as i32).wrapping_mul(73856093)
        ^ (y as i32).wrapping_mul(19349663)
        ^ (z as i32).wrapping_mul(83492791)) as u32;
    let mut h = seed;
    h = (h ^ (h >> 13)).wrapping_mul(0x5bd1e995);
    (h ^ (h >> 15)) as f32 / 4294967295.0
}

pub(crate) fn rotate_vertices_around_z(vertices: &mut [Vertex], center: [f32; 3], degrees: f32) {
    if degrees.abs() <= f32::EPSILON {
        return;
    }

    let radians = degrees.to_radians();
    let (sin_theta, cos_theta) = radians.sin_cos();

    for vertex in vertices.iter_mut() {
        let dx = vertex.position[0] - center[0];
        let dy = vertex.position[1] - center[1];
        vertex.position[0] = center[0] + dx * cos_theta - dy * sin_theta;
        vertex.position[1] = center[1] + dx * sin_theta + dy * cos_theta;
    }
}

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

pub(crate) fn apply_lighting(base_color: [f32; 4], normal: [f32; 3]) -> [f32; 4] {
    let light_dir = [0.57735, 0.57735, 0.57735];
    let ambient = 0.6;
    let diffuse = 0.4;

    let dot = normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2];
    let intensity = ambient + diffuse * dot.max(0.0);

    [
        (base_color[0] * intensity).min(1.0),
        (base_color[1] * intensity).min(1.0),
        (base_color[2] * intensity).min(1.0),
        base_color[3],
    ]
}

pub(crate) fn append_cone(
    vertices: &mut Vec<Vertex>,
    base_center: [f32; 3],
    tip: [f32; 3],
    radius: f32,
    color: [f32; 4],
) {
    let segments = 16;
    let axis_vec = [
        tip[0] - base_center[0],
        tip[1] - base_center[1],
        tip[2] - base_center[2],
    ];

    let height =
        (axis_vec[0] * axis_vec[0] + axis_vec[1] * axis_vec[1] + axis_vec[2] * axis_vec[2]).sqrt();
    let axis_norm = if height > 1e-6 {
        [
            axis_vec[0] / height,
            axis_vec[1] / height,
            axis_vec[2] / height,
        ]
    } else {
        [0.0, 0.0, 1.0]
    };

    let arbitrary = if axis_norm[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };

    let mut u = [
        axis_norm[1] * arbitrary[2] - axis_norm[2] * arbitrary[1],
        axis_norm[2] * arbitrary[0] - axis_norm[0] * arbitrary[2],
        axis_norm[0] * arbitrary[1] - axis_norm[1] * arbitrary[0],
    ];
    let u_len = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    u = [u[0] / u_len, u[1] / u_len, u[2] / u_len];

    let v = [
        axis_norm[1] * u[2] - axis_norm[2] * u[1],
        axis_norm[2] * u[0] - axis_norm[0] * u[2],
        axis_norm[0] * u[1] - axis_norm[1] * u[0],
    ];

    let mut base_points = Vec::with_capacity(segments);
    let mut normals = Vec::with_capacity(segments);

    for i in 0..segments {
        let angle = (i as f32) * std::f32::consts::TAU / (segments as f32);
        let (sin, cos) = angle.sin_cos();

        // Radial vector
        let rx = cos * u[0] + sin * v[0];
        let ry = cos * u[1] + sin * v[1];
        let rz = cos * u[2] + sin * v[2];

        let x = base_center[0] + radius * rx;
        let y = base_center[1] + radius * ry;
        let z = base_center[2] + radius * rz;
        base_points.push([x, y, z]);

        let nx = height * rx + radius * axis_norm[0];
        let ny = height * ry + radius * axis_norm[1];
        let nz = height * rz + radius * axis_norm[2];
        let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
        normals.push([nx / nlen, ny / nlen, nz / nlen]);
    }

    // Sides
    for i in 0..segments {
        let next = (i + 1) % segments;

        let c_base_i = apply_lighting(color, normals[i]);
        let c_base_next = apply_lighting(color, normals[next]);

        let n_tip = [
            (normals[i][0] + normals[next][0]) * 0.5,
            (normals[i][1] + normals[next][1]) * 0.5,
            (normals[i][2] + normals[next][2]) * 0.5,
        ];
        let n_tip_len = (n_tip[0] * n_tip[0] + n_tip[1] * n_tip[1] + n_tip[2] * n_tip[2]).sqrt();
        let n_tip = [
            n_tip[0] / n_tip_len,
            n_tip[1] / n_tip_len,
            n_tip[2] / n_tip_len,
        ];
        let c_tip = apply_lighting(color, n_tip);

        vertices.push(Vertex {
            position: base_points[i],
            color: c_base_i,
        });
        vertices.push(Vertex {
            position: base_points[next],
            color: c_base_next,
        });
        vertices.push(Vertex {
            position: tip,
            color: c_tip,
        });
    }

    // Base cap
    let base_normal = [-axis_norm[0], -axis_norm[1], -axis_norm[2]];
    let c_base = apply_lighting(color, base_normal);

    for i in 0..segments {
        let next = (i + 1) % segments;
        vertices.push(Vertex {
            position: base_center,
            color: c_base,
        });
        vertices.push(Vertex {
            position: base_points[next],
            color: c_base,
        });
        vertices.push(Vertex {
            position: base_points[i],
            color: c_base,
        });
    }
}

pub(crate) fn append_sphere(
    vertices: &mut Vec<Vertex>,
    center: [f32; 3],
    radius: f32,
    color: [f32; 4],
) {
    let lat_segments = 12;
    let lon_segments = 12;

    for i in 0..lat_segments {
        let lat0 = std::f32::consts::PI * (-0.5 + (i as f32) / (lat_segments as f32));
        let z0 = lat0.sin();
        let zr0 = lat0.cos();

        let lat1 = std::f32::consts::PI * (-0.5 + ((i + 1) as f32) / (lat_segments as f32));
        let z1 = lat1.sin();
        let zr1 = lat1.cos();

        for j in 0..lon_segments {
            let lon0 = 2.0 * std::f32::consts::PI * (j as f32) / (lon_segments as f32);
            let x0 = lon0.cos();
            let y0 = lon0.sin();

            let lon1 = 2.0 * std::f32::consts::PI * ((j + 1) as f32) / (lon_segments as f32);
            let x1 = lon1.cos();
            let y1 = lon1.sin();

            let p00 = [
                center[0] + radius * x0 * zr0,
                center[1] + radius * y0 * zr0,
                center[2] + radius * z0,
            ];
            let p10 = [
                center[0] + radius * x1 * zr0,
                center[1] + radius * y1 * zr0,
                center[2] + radius * z0,
            ];
            let p01 = [
                center[0] + radius * x0 * zr1,
                center[1] + radius * y0 * zr1,
                center[2] + radius * z1,
            ];
            let p11 = [
                center[0] + radius * x1 * zr1,
                center[1] + radius * y1 * zr1,
                center[2] + radius * z1,
            ];

            let n00 = [x0 * zr0, y0 * zr0, z0];
            let n10 = [x1 * zr0, y1 * zr0, z0];
            let n01 = [x0 * zr1, y0 * zr1, z1];
            let n11 = [x1 * zr1, y1 * zr1, z1];

            let c00 = apply_lighting(color, n00);
            let c10 = apply_lighting(color, n10);
            let c01 = apply_lighting(color, n01);
            let c11 = apply_lighting(color, n11);

            vertices.push(Vertex {
                position: p00,
                color: c00,
            });
            vertices.push(Vertex {
                position: p10,
                color: c10,
            });
            vertices.push(Vertex {
                position: p01,
                color: c01,
            });

            vertices.push(Vertex {
                position: p10,
                color: c10,
            });
            vertices.push(Vertex {
                position: p11,
                color: c11,
            });
            vertices.push(Vertex {
                position: p01,
                color: c01,
            });
        }
    }
}
