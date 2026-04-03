/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::mesh::lighting::apply_lighting;
use crate::types::Vertex;

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
