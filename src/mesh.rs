use crate::block_repository::{resolve_block_definition, BlockRenderProfile};
use crate::types::{LevelObject, Vertex};
use include_dir::{include_dir, Dir};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

static BLOCK_ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/blocks");
static OBJ_MESHES: OnceLock<HashMap<String, ObjMesh>> = OnceLock::new();

#[derive(Clone)]
struct ObjMesh {
    positions: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    normals: Vec<[f32; 3]>,
    faces: Vec<[ObjFaceVertex; 3]>,
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy)]
struct ObjFaceVertex {
    position_index: usize,
    texcoord_index: Option<usize>,
    normal_index: Option<usize>,
}

fn rotate_vertices_around_z(vertices: &mut [Vertex], center: [f32; 3], degrees: f32) {
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

fn append_prism(
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

fn append_quad(
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

fn append_top_fan(vertices: &mut Vec<Vertex>, points: &[[f32; 2]], z: f32, color: [f32; 4]) {
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

fn build_rounded_rect_points(
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

fn append_rounded_prism(
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

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    let mut all_vertices = Vec::new();

    for obj in objects {
        let mut object_vertices = Vec::new();
        let vertices = &mut object_vertices;

        let x_min = obj.position[0];
        let x_max = obj.position[0] + obj.size[0];
        let y_min = obj.position[1];
        let y_max = obj.position[1] + obj.size[1];
        let z_min = obj.position[2];
        let z_max = obj.position[2] + obj.size[2];

        let block = resolve_block_definition(&obj.block_id);

        if let Some(mesh_path) = block.assets.mesh.as_deref() {
            if let Some(mesh) = resolve_obj_mesh(mesh_path) {
                append_obj_mesh(vertices, obj, mesh, block.render.color_top);
            }
        }

        if vertices.is_empty() && matches!(block.render.profile, BlockRenderProfile::VoidFrame) {
            let color_fill = block.render.color_fill;
            let color_outline = block.render.color_outline;
            let t = 0.05;

            // Fill
            append_prism(
                vertices,
                [x_min + t, y_min + t, z_min + t],
                [x_max - t, y_max - t, z_max - t],
                color_fill,
                color_fill,
            );

            // Bottom edges
            append_prism(
                vertices,
                [x_min, y_min, z_min],
                [x_max, y_min + t, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_min],
                [x_max, y_max, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_min + t, z_min],
                [x_min + t, y_max - t, z_min + t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min + t, z_min],
                [x_max, y_max - t, z_min + t],
                color_outline,
                color_outline,
            );

            // Top edges
            append_prism(
                vertices,
                [x_min, y_min, z_max - t],
                [x_max, y_min + t, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_max - t],
                [x_max, y_max, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_min + t, z_max - t],
                [x_min + t, y_max - t, z_max],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min + t, z_max - t],
                [x_max, y_max - t, z_max],
                color_outline,
                color_outline,
            );

            // Vertical edges
            append_prism(
                vertices,
                [x_min, y_min, z_min + t],
                [x_min + t, y_min + t, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_min, z_min + t],
                [x_max, y_min + t, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_min, y_max - t, z_min + t],
                [x_min + t, y_max, z_max - t],
                color_outline,
                color_outline,
            );
            append_prism(
                vertices,
                [x_max - t, y_max - t, z_min + t],
                [x_max, y_max, z_max - t],
                color_outline,
                color_outline,
            );
        } else if vertices.is_empty() {
            let color_top = block.render.color_top;
            let color_side = block.render.color_side;

            if obj.roundness > f32::EPSILON {
                append_rounded_prism(
                    vertices,
                    [x_min, y_min, z_min],
                    [x_max, y_max, z_max],
                    color_top,
                    color_side,
                    obj.roundness,
                    5,
                );
            } else {
                append_prism(
                    vertices,
                    [x_min, y_min, z_min],
                    [x_max, y_max, z_max],
                    color_top,
                    color_side,
                );
            }
        }

        let center = [
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        ];
        rotate_vertices_around_z(&mut object_vertices, center, obj.rotation_degrees);
        all_vertices.extend(object_vertices);
    }

    all_vertices
}

fn resolve_obj_mesh(mesh_path: &str) -> Option<&'static ObjMesh> {
    let key = mesh_path.trim().replace('\\', "/").to_ascii_lowercase();
    let meshes = obj_meshes();
    meshes
        .get(&key)
        .or_else(|| meshes.get(&format!("assets/blocks/{key}")))
}

fn obj_meshes() -> &'static HashMap<String, ObjMesh> {
    OBJ_MESHES.get_or_init(|| {
        let mut meshes = HashMap::new();
        collect_obj_meshes(&BLOCK_ASSETS_DIR, &mut meshes);
        meshes
    })
}

fn collect_obj_meshes(dir: &Dir<'_>, meshes: &mut HashMap<String, ObjMesh>) {
    for file in dir.files() {
        let is_obj = file
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("obj"))
            .unwrap_or(false);

        if !is_obj {
            continue;
        }

        let Some(contents) = file.contents_utf8() else {
            continue;
        };

        let Some(mesh) = parse_obj_mesh(contents) else {
            continue;
        };

        let full_path = file
            .path()
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        meshes.insert(full_path, mesh.clone());

        if let Some(name) = file.path().file_name().and_then(|name| name.to_str()) {
            meshes.insert(name.to_ascii_lowercase(), mesh);
        }
    }

    for child in dir.dirs() {
        collect_obj_meshes(child, meshes);
    }
}

fn parse_obj_mesh(contents: &str) -> Option<ObjMesh> {
    let mut positions = Vec::new();
    let mut texcoords = Vec::new();
    let mut normals = Vec::new();
    let mut faces = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("v ") {
            let mut parts = rest.split_whitespace();
            let x = parts.next()?.parse::<f32>().ok()?;
            let y = parts.next()?.parse::<f32>().ok()?;
            let z = parts.next()?.parse::<f32>().ok()?;
            positions.push([x, y, z]);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("vt ") {
            let mut parts = rest.split_whitespace();
            let Some(u_text) = parts.next() else {
                continue;
            };
            let Some(v_text) = parts.next() else {
                continue;
            };
            let Ok(u) = u_text.parse::<f32>() else {
                continue;
            };
            let Ok(v) = v_text.parse::<f32>() else {
                continue;
            };
            texcoords.push([u, v]);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("vn ") {
            let mut parts = rest.split_whitespace();
            let Some(x_text) = parts.next() else {
                continue;
            };
            let Some(y_text) = parts.next() else {
                continue;
            };
            let Some(z_text) = parts.next() else {
                continue;
            };
            let Ok(x) = x_text.parse::<f32>() else {
                continue;
            };
            let Ok(y) = y_text.parse::<f32>() else {
                continue;
            };
            let Ok(z) = z_text.parse::<f32>() else {
                continue;
            };
            normals.push([x, y, z]);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("f ") {
            let mut corners = Vec::new();
            for token in rest.split_whitespace() {
                let mut segments = token.split('/');
                let Some(position_text) = segments.next() else {
                    continue;
                };
                if position_text.is_empty() {
                    continue;
                }

                let Ok(raw_position_index) = position_text.parse::<isize>() else {
                    continue;
                };
                let Some(position_index) = resolve_obj_index(raw_position_index, positions.len())
                else {
                    continue;
                };

                let texcoord_index = segments
                    .next()
                    .and_then(|value| {
                        if value.is_empty() {
                            None
                        } else {
                            value.parse::<isize>().ok()
                        }
                    })
                    .and_then(|raw_index| resolve_obj_index(raw_index, texcoords.len()));

                let normal_index = segments
                    .next()
                    .and_then(|value| {
                        if value.is_empty() {
                            None
                        } else {
                            value.parse::<isize>().ok()
                        }
                    })
                    .and_then(|raw_index| resolve_obj_index(raw_index, normals.len()));

                corners.push(ObjFaceVertex {
                    position_index,
                    texcoord_index,
                    normal_index,
                });
            }

            if corners.len() >= 3 {
                for i in 1..corners.len() - 1 {
                    faces.push([corners[0], corners[i], corners[i + 1]]);
                }
            }
        }
    }

    if positions.is_empty() || faces.is_empty() {
        return None;
    }

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in &positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }

    Some(ObjMesh {
        positions,
        texcoords,
        normals,
        faces,
        min,
        max,
    })
}

fn resolve_obj_index(raw_index: isize, len: usize) -> Option<usize> {
    if len == 0 || raw_index == 0 {
        return None;
    }

    if raw_index > 0 {
        let index = (raw_index as usize).checked_sub(1)?;
        (index < len).then_some(index)
    } else {
        let idx = len as isize + raw_index;
        (idx >= 0).then_some(idx as usize)
    }
}

fn append_obj_mesh(vertices: &mut Vec<Vertex>, obj: &LevelObject, mesh: &ObjMesh, color: [f32; 4]) {
    let span = [
        (mesh.max[0] - mesh.min[0]).max(f32::EPSILON),
        (mesh.max[1] - mesh.min[1]).max(f32::EPSILON),
        (mesh.max[2] - mesh.min[2]).max(f32::EPSILON),
    ];

    for face in &mesh.faces {
        for corner in face {
            let Some(raw) = mesh.positions.get(corner.position_index) else {
                continue;
            };

            let normalized = [
                (raw[0] - mesh.min[0]) / span[0],
                (raw[1] - mesh.min[1]) / span[1],
                (raw[2] - mesh.min[2]) / span[2],
            ];

            let _uv = corner
                .texcoord_index
                .and_then(|index| mesh.texcoords.get(index))
                .copied();

            let normal_tint = corner
                .normal_index
                .and_then(|index| mesh.normals.get(index))
                .map(|normal| {
                    let length =
                        (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
                            .sqrt();
                    if length <= f32::EPSILON {
                        1.0
                    } else {
                        let nx = normal[0] / length;
                        let ny = normal[1] / length;
                        let nz = normal[2] / length;
                        (nx * 0.25 + ny * 0.35 + nz * 0.4).abs().clamp(0.35, 1.0)
                    }
                })
                .unwrap_or(1.0);

            vertices.push(Vertex {
                position: [
                    obj.position[0] + normalized[0] * obj.size[0],
                    obj.position[1] + normalized[1] * obj.size[1],
                    obj.position[2] + normalized[2] * obj.size[2],
                ],
                color: [
                    color[0] * normal_tint,
                    color[1] * normal_tint,
                    color[2] * normal_tint,
                    color[3],
                ],
            });
        }
    }
}

pub(crate) fn build_block_obj(level_object: &LevelObject, object_name: &str) -> String {
    let vertices = build_block_vertices(std::slice::from_ref(level_object));
    build_obj_from_vertices(&vertices, object_name)
}

fn build_obj_from_vertices(vertices: &[Vertex], object_name: &str) -> String {
    let mut output = String::new();
    let _ = writeln!(&mut output, "# line-dash block export");
    let _ = writeln!(&mut output, "o {}", object_name);

    for vertex in vertices {
        let _ = writeln!(
            &mut output,
            "v {:.6} {:.6} {:.6}",
            vertex.position[0], vertex.position[1], vertex.position[2]
        );
    }

    for i in (0..vertices.len()).step_by(3) {
        if i + 2 >= vertices.len() {
            break;
        }
        let a = i + 1;
        let b = i + 2;
        let c = i + 3;
        let _ = writeln!(&mut output, "f {} {} {}", a, b, c);
    }

    output
}

pub(crate) fn build_trail_vertices(points: &[[f32; 3]], game_over: bool) -> Vec<Vertex> {
    let mut trail_vertices = Vec::new();
    let width = 0.8;
    let c_top = if game_over {
        [1.0, 0.2, 0.2, 1.0]
    } else {
        [0.8, 0.25, 0.35, 1.0]
    };
    let c_side = if game_over {
        [0.8, 0.1, 0.1, 1.0]
    } else {
        [0.7, 0.2, 0.3, 1.0]
    };

    if points.len() < 2 {
        return trail_vertices;
    }

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];

        if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
            let x_min = p1[0] - width / 2.0;
            let x_max = p1[0] + width / 2.0;
            let y_min = p1[1] - width / 2.0;
            let y_max = p1[1] + width / 2.0;
            let z_base = p1[2].min(p2[2]);
            let z_top = p1[2].max(p2[2]) + width;

            append_rounded_prism(
                &mut trail_vertices,
                [x_min, y_min, z_base],
                [x_max, y_max, z_top],
                c_top,
                c_side,
                width * 0.2,
                4,
            );
            continue;
        }

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

        let z_offset = p1[2].min(p2[2]);
        let z_extra = dz.abs() * 0.5;
        let z_min = z_offset;
        let z_max = z_offset + width + z_extra;

        append_rounded_prism(
            &mut trail_vertices,
            [x_min, y_min, z_min],
            [x_max, y_max, z_max],
            c_top,
            c_side,
            width * 0.2,
            4,
        );
    }

    trail_vertices
}

pub(crate) fn build_editor_cursor_vertices(cursor: [i32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color_top = [0.2, 0.85, 0.95, 0.4];
    let color_side = [0.1, 0.45, 0.55, 0.4];
    let z_min = cursor[2] as f32;
    let z_max = cursor[2] as f32 + 1.05;

    let x_min = cursor[0] as f32;
    let x_max = x_min + 1.0;
    let y_min = cursor[1] as f32;
    let y_max = y_min + 1.0;

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

    vertices
}

pub(crate) fn build_editor_gizmo_vertices(
    position: [f32; 3],
    size: [f32; 3],
    axis_lengths: [f32; 3],
    axis_width: f32,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];

    let shaft = axis_width.max(0.0005) * 0.5;
    let tip = shaft * 3.0;
    let cap = shaft * 3.5;
    let arm_start_offset = shaft * 2.0;
    let x_length = axis_lengths[0].max(arm_start_offset + tip * 2.0);
    let y_length = axis_lengths[1].max(arm_start_offset + tip * 2.0);
    let z_length = axis_lengths[2].max(arm_start_offset + tip * 2.0);

    // X move arm + tip
    let x_arm_start = center[0] + arm_start_offset;
    let x_arm_end = center[0] + x_length;
    append_prism(
        &mut vertices,
        [x_arm_start, center[1] - shaft, center[2] - shaft],
        [x_arm_end, center[1] + shaft, center[2] + shaft],
        [1.0, 0.25, 0.25, 0.72],
        [0.85, 0.15, 0.15, 0.62],
    );
    append_prism(
        &mut vertices,
        [x_arm_end - tip * 2.0, center[1] - cap, center[2] - cap],
        [x_arm_end, center[1] + cap, center[2] + cap],
        [1.0, 0.38, 0.38, 0.74],
        [0.85, 0.2, 0.2, 0.64],
    );

    // Y move arm + tip
    let y_arm_start = center[1] + arm_start_offset;
    let y_arm_end = center[1] + y_length;
    append_prism(
        &mut vertices,
        [center[0] - shaft, y_arm_start, center[2] - shaft],
        [center[0] + shaft, y_arm_end, center[2] + shaft],
        [0.35, 1.0, 0.35, 0.72],
        [0.2, 0.85, 0.2, 0.62],
    );
    append_prism(
        &mut vertices,
        [center[0] - cap, y_arm_end - tip * 2.0, center[2] - cap],
        [center[0] + cap, y_arm_end, center[2] + cap],
        [0.45, 1.0, 0.45, 0.74],
        [0.25, 0.85, 0.25, 0.64],
    );

    // Z move arm + tip
    let z_arm_start = center[2] + arm_start_offset;
    let z_arm_end = center[2] + z_length;
    append_prism(
        &mut vertices,
        [center[0] - shaft, center[1] - shaft, z_arm_start],
        [center[0] + shaft, center[1] + shaft, z_arm_end],
        [0.45, 0.65, 1.0, 0.72],
        [0.3, 0.5, 0.9, 0.62],
    );
    append_prism(
        &mut vertices,
        [center[0] - cap, center[1] - cap, z_arm_end - tip * 2.0],
        [center[0] + cap, center[1] + cap, z_arm_end],
        [0.55, 0.72, 1.0, 0.74],
        [0.35, 0.55, 0.9, 0.64],
    );

    // Resize handles on positive corners of each axis
    let resize = 0.18;
    append_prism(
        &mut vertices,
        [
            position[0] + size[0],
            center[1] - resize,
            center[2] - resize,
        ],
        [
            position[0] + size[0] + resize * 2.0,
            center[1] + resize,
            center[2] + resize,
        ],
        [1.0, 0.55, 0.55, 0.78],
        [0.95, 0.42, 0.42, 0.68],
    );
    append_prism(
        &mut vertices,
        [
            center[0] - resize,
            position[1] + size[1],
            center[2] - resize,
        ],
        [
            center[0] + resize,
            position[1] + size[1] + resize * 2.0,
            center[2] + resize,
        ],
        [0.6, 1.0, 0.6, 0.78],
        [0.45, 0.95, 0.45, 0.68],
    );
    append_prism(
        &mut vertices,
        [
            center[0] - resize,
            center[1] - resize,
            position[2] + size[2],
        ],
        [
            center[0] + resize,
            center[1] + resize,
            position[2] + size[2] + resize * 2.0,
        ],
        [0.65, 0.8, 1.0, 0.78],
        [0.5, 0.65, 0.95, 0.68],
    );

    vertices
}

pub(crate) fn build_editor_selection_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let x0 = position[0] - 0.015;
    let x1 = position[0] + size[0] + 0.015;
    let y0 = position[1] - 0.015;
    let y1 = position[1] + size[1] + 0.015;
    let z0 = position[2] - 0.015;
    let z1 = position[2] + size[2] + 0.015;

    let thickness = 0.045;
    let color_top = [0.45, 0.9, 1.0, 1.0];
    let color_side = [0.25, 0.75, 0.9, 1.0];

    // Edges along X
    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            &mut vertices,
            [x0, y - thickness, z - thickness],
            [x1, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Y
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0, z - thickness],
            [x + thickness, y1, z + thickness],
            color_top,
            color_side,
        );
    }

    // Edges along Z
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0],
            [x + thickness, y + thickness, z1],
            color_top,
            color_side,
        );
    }

    vertices
}

pub(crate) fn build_editor_hover_outline_vertices(
    position: [f32; 3],
    size: [f32; 3],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let x0 = position[0] - 0.01;
    let x1 = position[0] + size[0] + 0.01;
    let y0 = position[1] - 0.01;
    let y1 = position[1] + size[1] + 0.01;
    let z0 = position[2] - 0.01;
    let z1 = position[2] + size[2] + 0.01;

    let thickness = 0.03;
    let color_top = [0.62, 0.9, 1.0, 0.45];
    let color_side = [0.45, 0.82, 0.95, 0.38];

    for (y, z) in [(y0, z0), (y1, z0), (y0, z1), (y1, z1)] {
        append_prism(
            &mut vertices,
            [x0, y - thickness, z - thickness],
            [x1, y + thickness, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y0, z - thickness],
            [x + thickness, y1, z + thickness],
            color_top,
            color_side,
        );
    }

    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        append_prism(
            &mut vertices,
            [x - thickness, y - thickness, z0],
            [x + thickness, y + thickness, z1],
            color_top,
            color_side,
        );
    }

    vertices
}

pub(crate) fn build_spawn_marker_vertices(position: [f32; 3], faces_right: bool) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let x = position[0];
    let y = position[1];
    let z = position[2];

    append_prism(
        &mut vertices,
        [x + 0.1, y + 0.1, z],
        [x + 0.9, y + 0.9, z + 0.5],
        [0.25, 0.95, 0.35, 1.0],
        [0.1, 0.45, 0.15, 1.0],
    );

    if faces_right {
        append_prism(
            &mut vertices,
            [x + 0.9, y + 0.35, z],
            [x + 1.3, y + 0.65, z + 0.7],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    } else {
        append_prism(
            &mut vertices,
            [x + 0.35, y + 0.9, z],
            [x + 0.65, y + 1.3, z + 0.7],
            [0.2, 0.9, 0.3, 1.0],
            [0.1, 0.45, 0.15, 1.0],
        );
    }

    vertices
}

#[cfg(test)]
mod tests {
    use super::{
        build_block_vertices, build_editor_gizmo_vertices, build_editor_hover_outline_vertices,
        parse_obj_mesh,
    };
    use crate::types::LevelObject;

    fn bounds_xy(vertices: &[[f32; 3]]) -> (f32, f32, f32, f32) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for pos in vertices {
            min_x = min_x.min(pos[0]);
            max_x = max_x.max(pos[0]);
            min_y = min_y.min(pos[1]);
            max_y = max_y.max(pos[1]);
        }
        (min_x, max_x, min_y, max_y)
    }

    #[test]
    fn block_vertices_rotate_about_center() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 1.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        };
        let vertices = build_block_vertices(&[obj]);
        let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
        let (min_x, max_x, min_y, max_y) = bounds_xy(&positions);

        assert!((min_x - 0.5).abs() < 1e-5);
        assert!((max_x - 1.5).abs() < 1e-5);
        assert!((min_y - -0.5).abs() < 1e-5);
        assert!((max_y - 1.5).abs() < 1e-5);
    }

    #[test]
    fn gizmo_vertices_generate_with_screen_scaled_inputs() {
        let vertices =
            build_editor_gizmo_vertices([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], [3.0, 4.0, 5.0], 0.1);
        assert!(!vertices.is_empty());

        let max_x = vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_x >= 4.0);
    }

    #[test]
    fn hover_outline_vertices_are_translucent() {
        let vertices = build_editor_hover_outline_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert!(!vertices.is_empty());
        assert!(vertices.iter().any(|v| v.color[3] < 1.0));
    }

    #[test]
    fn obj_parser_supports_uvs_and_normals() {
        let obj = r#"
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
"#;

        let mesh = parse_obj_mesh(obj).expect("valid mesh");
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.texcoords.len(), 3);
        assert_eq!(mesh.normals.len(), 1);
        assert_eq!(mesh.faces.len(), 1);
        assert_eq!(mesh.faces[0][0].texcoord_index, Some(0));
        assert_eq!(mesh.faces[0][0].normal_index, Some(0));
    }
}
