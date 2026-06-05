/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const EGMESH_MAGIC: [u8; 4] = *b"EGM1";
const EGMESH_CODEC_VERSION: u16 = 1;
const EGMESH_COMPRESSION_ZSTD: u8 = 1;
const EGMESH_ZSTD_LEVEL: i32 = 3;

#[derive(Clone, Copy)]
struct BuildObjFaceVertex {
    position_index: usize,
    texcoord_index: Option<usize>,
    normal_index: Option<usize>,
    material_index: Option<usize>,
}

struct BuildObjMesh {
    positions: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    normals: Vec<[f32; 3]>,
    materials: Vec<BuildObjMaterial>,
    faces: Vec<[BuildObjFaceVertex; 3]>,
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy)]
struct BuildObjMaterial {
    diffuse: [f32; 4],
}

#[derive(Clone, Copy, Deserialize, Serialize)]
struct BuildEgMeshVertex {
    position: [f32; 3],
    uv: [f32; 2],
    normal_tint: f32,
    material_color: [f32; 4],
}

#[derive(Deserialize, Serialize)]
struct BuildEgMesh {
    vertices: Vec<BuildEgMeshVertex>,
    indices: Vec<u32>,
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Hash, PartialEq, Eq)]
struct EgMeshVertexKey {
    position: [u32; 3],
    uv: [u32; 2],
    normal_tint: u32,
    material_color: [u32; 4],
}

fn load_wrangler_vars(build_env: &str) -> Vec<(String, String)> {
    let content = match std::fs::read_to_string("wrangler.jsonc") {
        Ok(c) => c,
        Err(_) => {
            println!("cargo:warning=wrangler.jsonc not found, skipping var baking");
            return vec![];
        }
    };

    // Strip block and line comments
    let stripped = strip_jsonc_comments(&content);

    let json: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=Failed to parse wrangler.jsonc: {}", e);
            return vec![];
        }
    };

    let vars = json
        .pointer(&format!("/env/{}/vars", build_env))
        .or_else(|| json.pointer("/vars")); // fallback to top-level vars

    match vars {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        _ => {
            vec![]
        }
    }
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            if c == '\\' {
                out.push(c);
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                if c == '"' {
                    in_string = false;
                }
                out.push(c);
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.by_ref().take_while(|&c| c != '\n').for_each(drop);
                    out.push('\n');
                }
                Some('*') => {
                    chars.next();
                    loop {
                        match chars.next() {
                            Some('*') if chars.peek() == Some(&'/') => {
                                chars.next();
                                break;
                            }
                            None => break,
                            _ => {}
                        }
                    }
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn collect_obj_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_obj_paths(&path, paths);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("obj"))
        {
            paths.push(path);
        }
    }
}

fn collect_mtl_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_mtl_paths(&path, paths);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mtl"))
        {
            paths.push(path);
        }
    }
}

fn parse_build_obj_mesh(contents: &str, obj_path: &Path) -> Result<BuildObjMesh, String> {
    let mut positions = Vec::new();
    let mut texcoords = Vec::new();
    let mut normals = Vec::new();
    let mut materials = Vec::new();
    let mut material_indices = HashMap::new();
    let mut current_material_index = None;
    let mut faces = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("mtllib ") {
            for material_path in rest.split_whitespace() {
                let resolved_path = obj_path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(material_path);
                match fs::read_to_string(&resolved_path) {
                    Ok(material_contents) => append_build_mtl_materials(
                        &material_contents,
                        &mut materials,
                        &mut material_indices,
                    ),
                    Err(error) => println!(
                        "cargo:warning=Failed to read material library {}: {error}",
                        resolved_path.display()
                    ),
                }
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("usemtl ") {
            current_material_index = material_indices.get(rest.trim()).copied();
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("v ") {
            let mut parts = rest.split_whitespace();
            let x = parts
                .next()
                .ok_or_else(|| "OBJ vertex missing x".to_string())?
                .parse::<f32>()
                .map_err(|error| error.to_string())?;
            let y = parts
                .next()
                .ok_or_else(|| "OBJ vertex missing y".to_string())?
                .parse::<f32>()
                .map_err(|error| error.to_string())?;
            let z = parts
                .next()
                .ok_or_else(|| "OBJ vertex missing z".to_string())?
                .parse::<f32>()
                .map_err(|error| error.to_string())?;
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
                let Some(position_index) =
                    resolve_build_obj_index(raw_position_index, positions.len())
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
                    .and_then(|raw_index| resolve_build_obj_index(raw_index, texcoords.len()));

                let normal_index = segments
                    .next()
                    .and_then(|value| {
                        if value.is_empty() {
                            None
                        } else {
                            value.parse::<isize>().ok()
                        }
                    })
                    .and_then(|raw_index| resolve_build_obj_index(raw_index, normals.len()));

                corners.push(BuildObjFaceVertex {
                    position_index,
                    texcoord_index,
                    normal_index,
                    material_index: current_material_index,
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
        return Err("OBJ mesh must contain positions and faces".to_string());
    }

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for face in &faces {
        for corner in face {
            let position = positions
                .get(corner.position_index)
                .ok_or_else(|| "OBJ face references missing position".to_string())?;
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
    }

    Ok(BuildObjMesh {
        positions,
        texcoords,
        normals,
        materials,
        faces,
        min,
        max,
    })
}

fn append_build_mtl_materials(
    contents: &str,
    materials: &mut Vec<BuildObjMaterial>,
    material_indices: &mut HashMap<String, usize>,
) {
    let mut current_name = None::<String>;
    let mut current_diffuse = [1.0, 1.0, 1.0, 1.0];

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("newmtl ") {
            flush_build_material(
                &mut current_name,
                current_diffuse,
                materials,
                material_indices,
            );
            current_name = Some(rest.trim().to_string());
            current_diffuse = [1.0, 1.0, 1.0, 1.0];
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("Kd ") {
            let mut parts = rest.split_whitespace();
            let Some(r) = parts.next().and_then(|value| value.parse::<f32>().ok()) else {
                continue;
            };
            let Some(g) = parts.next().and_then(|value| value.parse::<f32>().ok()) else {
                continue;
            };
            let Some(b) = parts.next().and_then(|value| value.parse::<f32>().ok()) else {
                continue;
            };
            current_diffuse[0] = r.clamp(0.0, 1.0);
            current_diffuse[1] = g.clamp(0.0, 1.0);
            current_diffuse[2] = b.clamp(0.0, 1.0);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("d ") {
            if let Some(alpha) = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<f32>().ok())
            {
                current_diffuse[3] = alpha.clamp(0.0, 1.0);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("Tr ") {
            if let Some(transparency) = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<f32>().ok())
            {
                current_diffuse[3] = (1.0 - transparency).clamp(0.0, 1.0);
            }
        }
    }

    flush_build_material(
        &mut current_name,
        current_diffuse,
        materials,
        material_indices,
    );
}

fn flush_build_material(
    current_name: &mut Option<String>,
    diffuse: [f32; 4],
    materials: &mut Vec<BuildObjMaterial>,
    material_indices: &mut HashMap<String, usize>,
) {
    let Some(name) = current_name.take() else {
        return;
    };
    if name.is_empty() || material_indices.contains_key(&name) {
        return;
    }

    let index = materials.len();
    materials.push(BuildObjMaterial { diffuse });
    material_indices.insert(name, index);
}

fn resolve_build_obj_index(raw_index: isize, len: usize) -> Option<usize> {
    if len == 0 || raw_index == 0 {
        return None;
    }

    if raw_index > 0 {
        let index = (raw_index as usize).checked_sub(1)?;
        (index < len).then_some(index)
    } else {
        let index = len as isize + raw_index;
        (index >= 0).then_some(index as usize)
    }
}

fn convert_obj_to_egmesh(mesh: &BuildObjMesh) -> Result<BuildEgMesh, String> {
    let span = [
        (mesh.max[0] - mesh.min[0]).max(f32::EPSILON),
        (mesh.max[1] - mesh.min[1]).max(f32::EPSILON),
        (mesh.max[2] - mesh.min[2]).max(f32::EPSILON),
    ];
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(mesh.faces.len() * 3);
    let mut lookup = HashMap::<EgMeshVertexKey, u32>::new();

    for face in &mesh.faces {
        for corner in face {
            let raw = mesh
                .positions
                .get(corner.position_index)
                .ok_or_else(|| "OBJ face references missing position".to_string())?;
            let normalized = [
                (raw[0] - mesh.min[0]) / span[0],
                (raw[1] - mesh.min[1]) / span[1],
                (raw[2] - mesh.min[2]) / span[2],
            ];
            let uv = corner
                .texcoord_index
                .and_then(|index| mesh.texcoords.get(index))
                .copied()
                .unwrap_or([normalized[0], 1.0 - normalized[1]]);
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
                        // Light direction (0.25, 0.35, 0.4), normalized.
                        let l_len = 0.58738_f32;
                        let dot = (nx * 0.25 + ny * 0.35 + nz * 0.4) / l_len;
                        dot.abs().clamp(0.35, 1.0)
                    }
                })
                .unwrap_or(1.0);
            let material_color = corner
                .material_index
                .and_then(|index| mesh.materials.get(index))
                .map(|material| material.diffuse)
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            let key = EgMeshVertexKey {
                position: [
                    normalized[0].to_bits(),
                    normalized[1].to_bits(),
                    normalized[2].to_bits(),
                ],
                uv: [uv[0].to_bits(), uv[1].to_bits()],
                normal_tint: normal_tint.to_bits(),
                material_color: [
                    material_color[0].to_bits(),
                    material_color[1].to_bits(),
                    material_color[2].to_bits(),
                    material_color[3].to_bits(),
                ],
            };
            let index = if let Some(index) = lookup.get(&key) {
                *index
            } else {
                let index = u32::try_from(vertices.len())
                    .map_err(|_| "Egmesh has too many vertices".to_string())?;
                vertices.push(BuildEgMeshVertex {
                    position: normalized,
                    uv,
                    normal_tint,
                    material_color,
                });
                lookup.insert(key, index);
                index
            };
            indices.push(index);
        }
    }

    Ok(BuildEgMesh {
        vertices,
        indices,
        min: mesh.min,
        max: mesh.max,
    })
}

fn encode_build_egmesh(mesh: &BuildEgMesh) -> Result<Vec<u8>, String> {
    let payload_bytes = serde_cbor::to_vec(mesh).map_err(|error| error.to_string())?;
    let compressed_payload = zstd::bulk::compress(&payload_bytes, EGMESH_ZSTD_LEVEL)
        .map_err(|error| error.to_string())?;
    let mut encoded = Vec::with_capacity(15 + compressed_payload.len());
    encoded.extend_from_slice(&EGMESH_MAGIC);
    encoded.extend_from_slice(&EGMESH_CODEC_VERSION.to_le_bytes());
    encoded.push(EGMESH_COMPRESSION_ZSTD);
    encoded.extend_from_slice(
        &u32::try_from(compressed_payload.len())
            .map_err(|_| "Egmesh payload too large".to_string())?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(
        &u32::try_from(payload_bytes.len())
            .map_err(|_| "Egmesh payload too large".to_string())?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(&compressed_payload);
    Ok(encoded)
}

fn egmesh_aliases(path: &Path) -> Vec<String> {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let mut aliases = Vec::new();
    aliases.push(normalized.clone());
    aliases.push(format!("assets/blocks/{normalized}"));
    if let Some(stem) = normalized.strip_suffix(".obj") {
        aliases.push(format!("{stem}.egmesh"));
        aliases.push(format!("assets/blocks/{stem}.egmesh"));
    }
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        let file_name = file_name.to_ascii_lowercase();
        aliases.push(file_name.clone());
        if let Some(stem) = file_name.strip_suffix(".obj") {
            aliases.push(format!("{stem}.egmesh"));
        }
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn generated_egmesh_file_name(relative_path: &Path) -> String {
    let normalized = relative_path
        .to_string_lossy()
        .replace(['\\', '/', '.'], "_")
        .to_ascii_lowercase();
    format!("{normalized}.egmesh")
}

fn rust_raw_string(value: &str) -> String {
    format!("r#\"{value}\"#")
}

fn generate_egmesh_manifest() {
    let out_dir = PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR should be set by Cargo for build scripts"),
    );
    let manifest_path = out_dir.join("egmesh_manifest.rs");
    let generated_dir = out_dir.join("egmesh");
    let assets_dir = Path::new("assets/blocks");
    let mut manifest = String::from("&[\n");
    let mut converted_count = 0usize;
    let mut skipped_count = 0usize;

    if let Err(error) = fs::create_dir_all(&generated_dir) {
        println!("cargo:warning=Failed to create egmesh output directory: {error}");
    }

    let mut obj_paths = Vec::new();
    collect_obj_paths(assets_dir, &mut obj_paths);
    obj_paths.sort();

    let mut mtl_paths = Vec::new();
    collect_mtl_paths(assets_dir, &mut mtl_paths);
    mtl_paths.sort();
    for mtl_path in mtl_paths {
        println!("cargo:rerun-if-changed={}", mtl_path.display());
    }

    for obj_path in obj_paths {
        println!("cargo:rerun-if-changed={}", obj_path.display());
        let relative_path = obj_path.strip_prefix(assets_dir).unwrap_or(&obj_path);
        let output_path = generated_dir.join(generated_egmesh_file_name(relative_path));
        let conversion = fs::read_to_string(&obj_path)
            .map_err(|error| error.to_string())
            .and_then(|contents| parse_build_obj_mesh(&contents, &obj_path))
            .and_then(|mesh| convert_obj_to_egmesh(&mesh))
            .and_then(|mesh| encode_build_egmesh(&mesh));

        match conversion {
            Ok(bytes) => {
                if let Err(error) = fs::write(&output_path, bytes) {
                    println!(
                        "cargo:warning=Failed to write generated egmesh for {}: {error}",
                        obj_path.display()
                    );
                    skipped_count += 1;
                    continue;
                }
                for alias in egmesh_aliases(relative_path) {
                    manifest.push_str("    (");
                    manifest.push_str(&rust_raw_string(&alias));
                    manifest.push_str(", include_bytes!(");
                    manifest.push_str(&rust_raw_string(&output_path.to_string_lossy()));
                    manifest.push_str(") as &[u8]),\n");
                }
                converted_count += 1;
            }
            Err(error) => {
                println!(
                    "cargo:warning=Skipping egmesh generation for {}: {error}",
                    obj_path.display()
                );
                skipped_count += 1;
            }
        }
    }

    manifest.push_str("]\n");
    if let Err(error) = fs::write(&manifest_path, manifest) {
        println!("cargo:warning=Failed to write egmesh manifest: {error}");
    }
    if skipped_count > 0 {
        println!(
            "cargo:warning=Generated {} egmesh asset(s), skipped {}",
            converted_count, skipped_count
        );
    }
}

// Whitelist of keys to bake into the binary
const BAKE_KEYS: &[&str] = &["API_URL", "PUBLISHABLE_KEY", "AUTH_BASE_URL"];

fn main() {
    println!("cargo:rerun-if-changed=.env.local");
    println!("cargo:rerun-if-changed=wrangler.jsonc");
    println!("cargo:rerun-if-env-changed=BUILD_ENV");
    println!("cargo:rerun-if-changed=assets/levels");
    println!("cargo:rerun-if-changed=assets/blocks");
    generate_egmesh_manifest();

    let build_env = std::env::var("BUILD_ENV").unwrap_or_else(|_| "local".to_string());

    let env_file = match build_env.as_str() {
        "preview" => ".env.preview",
        "production" => ".env.production",
        _ => ".env.local",
    };

    let allowed: HashSet<&str> = BAKE_KEYS.iter().copied().collect();

    let mut wrangler_count = 0;
    // Use wrangler.jsonc first
    for (key, value) in load_wrangler_vars(&build_env) {
        if allowed.contains(key.as_str()) {
            println!("cargo:rustc-env={}={}", key, value);
            wrangler_count += 1;
        }
    }
    println!(
        "cargo:warning=Baking {} keys from wrangler.jsonc",
        wrangler_count
    );

    let mut env_count = 0;
    match std::fs::read_to_string(env_file) {
        Ok(c) => {
            for line in c.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    if allowed.contains(key) {
                        println!("cargo:rustc-env={}={}", key, value);
                        env_count += 1;
                    }
                }
            }
        }
        Err(_) => {
            println!("cargo:warning={} not found", env_file);
        }
    };
    println!("cargo:warning=Baking {} keys from {}", env_count, env_file);

    let levels_dir = Path::new("assets/levels");
    if levels_dir.is_dir() {
        let level_count = std::fs::read_dir(levels_dir)
            .map(|entries| {
                entries
                    .filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false))
                    .count()
            })
            .unwrap_or(0);
        println!("cargo:warning=Using {} levels", level_count);
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "windows" && target_arch != "wasm32" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/favicon.ico");
        res.compile().expect("Failed to compile Windows resource");
    }
}
