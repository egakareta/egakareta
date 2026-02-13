use crate::types::{BlockKind, LevelObject, Vertex};

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

        if obj.kind == BlockKind::Void {
            let color_fill = [0.0, 0.0, 0.0, 1.0];
            let color_outline = [0.8, 0.8, 0.9, 1.0];
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
        } else if obj.kind == BlockKind::SpeedPortal {
            let cx = (x_min + x_max) / 2.0;
            let cy = (y_min + y_max) / 2.0;
            let cz = (z_min + z_max) / 2.0;
            let arrow_color = [1.0, 1.0, 0.0, 1.0];
            let arrow_len = 0.6;
            let arrow_width = 0.4;
            let thickness = 0.1;

            for offset_y in [-0.3, 0.3] {
                let center_y = cy + offset_y;
                let y_tip = center_y + arrow_len / 2.0;
                let y_base = center_y - arrow_len / 2.0;
                let v0 = [cx, y_tip];
                let v1 = [cx - arrow_width / 2.0, y_base];
                let v2 = [cx, y_base + arrow_len * 0.3]; // Indent
                let v3 = [cx + arrow_width / 2.0, y_base];

                let z_top = cz + thickness / 2.0;
                let z_bot = cz - thickness / 2.0;

                // Top face
                vertices.push(Vertex {
                    position: [v0[0], v0[1], z_top],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v1[0], v1[1], z_top],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v2[0], v2[1], z_top],
                    color: arrow_color,
                });

                vertices.push(Vertex {
                    position: [v0[0], v0[1], z_top],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v2[0], v2[1], z_top],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v3[0], v3[1], z_top],
                    color: arrow_color,
                });

                // Bottom face
                vertices.push(Vertex {
                    position: [v0[0], v0[1], z_bot],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v2[0], v2[1], z_bot],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v1[0], v1[1], z_bot],
                    color: arrow_color,
                });

                vertices.push(Vertex {
                    position: [v0[0], v0[1], z_bot],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v3[0], v3[1], z_bot],
                    color: arrow_color,
                });
                vertices.push(Vertex {
                    position: [v2[0], v2[1], z_bot],
                    color: arrow_color,
                });

                // Sides
                let perimeter = [v0, v1, v2, v3];
                for i in 0..4 {
                    let p1 = perimeter[i];
                    let p2 = perimeter[(i + 1) % 4];

                    vertices.push(Vertex {
                        position: [p1[0], p1[1], z_top],
                        color: arrow_color,
                    });
                    vertices.push(Vertex {
                        position: [p2[0], p2[1], z_top],
                        color: arrow_color,
                    });
                    vertices.push(Vertex {
                        position: [p2[0], p2[1], z_bot],
                        color: arrow_color,
                    });

                    vertices.push(Vertex {
                        position: [p1[0], p1[1], z_top],
                        color: arrow_color,
                    });
                    vertices.push(Vertex {
                        position: [p2[0], p2[1], z_bot],
                        color: arrow_color,
                    });
                    vertices.push(Vertex {
                        position: [p1[0], p1[1], z_bot],
                        color: arrow_color,
                    });
                }
            }
        } else {
            let (color_top, color_side) = match obj.kind {
                BlockKind::Standard => ([0.4, 0.4, 0.45, 1.0], [0.2, 0.2, 0.25, 1.0]),
                BlockKind::Grass => ([0.1, 0.6, 0.1, 1.0], [0.35, 0.25, 0.15, 1.0]),
                BlockKind::Dirt => ([0.4, 0.3, 0.2, 1.0], [0.35, 0.25, 0.15, 1.0]),
                _ => ([0.5, 0.5, 0.5, 1.0], [0.5, 0.5, 0.5, 1.0]),
            };

            append_prism(
                vertices,
                [x_min, y_min, z_min],
                [x_max, y_max, z_max],
                color_top,
                color_side,
            );
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

            append_prism(
                &mut trail_vertices,
                [x_min, y_min, z_base],
                [x_max, y_max, z_top],
                c_top,
                c_side,
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

pub(crate) fn build_editor_gizmo_vertices(position: [f32; 3], size: [f32; 3]) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let center = [
        position[0] + size[0] * 0.5,
        position[1] + size[1] * 0.5,
        position[2] + size[2] * 0.5,
    ];

    let arm_length = size[0].max(size[1]).max(size[2]).max(1.0) * 0.9;
    let shaft = 0.06;
    let tip = 0.18;
    let cap = 0.22;

    // X move arm + tip
    let x_arm_start = center[0] + 0.08;
    let x_arm_end = center[0] + arm_length;
    append_prism(
        &mut vertices,
        [x_arm_start, center[1] - shaft, center[2] - shaft],
        [x_arm_end, center[1] + shaft, center[2] + shaft],
        [1.0, 0.25, 0.25, 0.72],
        [0.85, 0.15, 0.15, 0.62],
    );
    append_prism(
        &mut vertices,
        [x_arm_end - tip, center[1] - cap, center[2] - cap],
        [x_arm_end + tip, center[1] + cap, center[2] + cap],
        [1.0, 0.38, 0.38, 0.74],
        [0.85, 0.2, 0.2, 0.64],
    );

    // Y move arm + tip
    let y_arm_start = center[1] + 0.08;
    let y_arm_end = center[1] + arm_length;
    append_prism(
        &mut vertices,
        [center[0] - shaft, y_arm_start, center[2] - shaft],
        [center[0] + shaft, y_arm_end, center[2] + shaft],
        [0.35, 1.0, 0.35, 0.72],
        [0.2, 0.85, 0.2, 0.62],
    );
    append_prism(
        &mut vertices,
        [center[0] - cap, y_arm_end - tip, center[2] - cap],
        [center[0] + cap, y_arm_end + tip, center[2] + cap],
        [0.45, 1.0, 0.45, 0.74],
        [0.25, 0.85, 0.25, 0.64],
    );

    // Z move arm + tip
    let z_arm_start = center[2] + 0.08;
    let z_arm_end = center[2] + arm_length;
    append_prism(
        &mut vertices,
        [center[0] - shaft, center[1] - shaft, z_arm_start],
        [center[0] + shaft, center[1] + shaft, z_arm_end],
        [0.45, 0.65, 1.0, 0.72],
        [0.3, 0.5, 0.9, 0.62],
    );
    append_prism(
        &mut vertices,
        [center[0] - cap, center[1] - cap, z_arm_end - tip],
        [center[0] + cap, center[1] + cap, z_arm_end + tip],
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
