use crate::types::{LevelObject, Vertex};
use include_dir::{include_dir, Dir};
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) static BLOCK_ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/blocks");
pub(crate) static OBJ_MESHES: OnceLock<HashMap<String, ObjMesh>> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct ObjMesh {
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) texcoords: Vec<[f32; 2]>,
    pub(crate) normals: Vec<[f32; 3]>,
    pub(crate) faces: Vec<[ObjFaceVertex; 3]>,
    pub(crate) min: [f32; 3],
    pub(crate) max: [f32; 3],
}

#[derive(Clone, Copy)]
pub(crate) struct ObjFaceVertex {
    pub(crate) position_index: usize,
    pub(crate) texcoord_index: Option<usize>,
    pub(crate) normal_index: Option<usize>,
}

pub(crate) fn resolve_obj_mesh(mesh_path: &str) -> Option<&'static ObjMesh> {
    let key = mesh_path.trim().replace('\\', "/").to_ascii_lowercase();
    let meshes = obj_meshes();
    meshes
        .get(&key)
        .or_else(|| meshes.get(&format!("assets/blocks/{key}")))
}

pub(crate) fn obj_meshes() -> &'static HashMap<String, ObjMesh> {
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

pub(crate) fn parse_obj_mesh(contents: &str) -> Option<ObjMesh> {
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

pub(crate) fn append_obj_mesh(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    mesh: &ObjMesh,
    color: [f32; 4],
) {
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
    use crate::mesh::blocks::build_block_vertices;
    let vertices = build_block_vertices(std::slice::from_ref(level_object));
    build_obj_from_vertices(&vertices, object_name)
}

fn build_obj_from_vertices(vertices: &[Vertex], object_name: &str) -> String {
    use std::fmt::Write as _;
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
