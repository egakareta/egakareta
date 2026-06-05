/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::mesh::geometry::MeshGeometry;
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::types::{LevelObject, Vertex};

const EGMESH_MAGIC: [u8; 4] = *b"EGM1";
const EGMESH_CODEC_VERSION: u16 = 1;
const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_ZSTD: u8 = 1;
#[cfg(test)]
const ZSTD_LEVEL: i32 = 3;

static EGMESHES: OnceLock<HashMap<String, EgMesh>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct EgMeshVertex {
    pub(crate) position: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) normal_tint: f32,
    #[serde(default = "default_material_color")]
    pub(crate) material_color: [f32; 4],
}

fn default_material_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct EgMesh {
    pub(crate) vertices: Vec<EgMeshVertex>,
    pub(crate) indices: Vec<u32>,
    pub(crate) min: [f32; 3],
    pub(crate) max: [f32; 3],
}

#[cfg(test)]
pub(crate) fn encode_egmesh(mesh: &EgMesh) -> Result<Vec<u8>, String> {
    let payload_bytes = serde_cbor::to_vec(mesh).map_err(|error| error.to_string())?;
    let compressed_payload =
        zstd::bulk::compress(&payload_bytes, ZSTD_LEVEL).map_err(|error| error.to_string())?;

    let mut encoded = Vec::with_capacity(15 + compressed_payload.len());
    encoded.extend_from_slice(&EGMESH_MAGIC);
    encoded.extend_from_slice(&EGMESH_CODEC_VERSION.to_le_bytes());
    encoded.push(COMPRESSION_ZSTD);
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

pub(crate) fn decode_egmesh(bytes: &[u8]) -> Result<EgMesh, String> {
    if bytes.len() < 15 {
        return Err("Egmesh payload is too small".to_string());
    }

    if bytes[0..4] != EGMESH_MAGIC {
        return Err("Invalid egmesh payload magic header".to_string());
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != EGMESH_CODEC_VERSION {
        return Err(format!(
            "Unsupported egmesh codec version {version}; expected {EGMESH_CODEC_VERSION}"
        ));
    }

    let compression = bytes[6];
    let payload_len = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
    let decompressed_len =
        u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]) as usize;
    let expected_total = 15usize
        .checked_add(payload_len)
        .ok_or_else(|| "Egmesh payload size overflow".to_string())?;

    if bytes.len() != expected_total {
        return Err("Egmesh payload length mismatch".to_string());
    }

    let payload_bytes = match compression {
        COMPRESSION_NONE => bytes[15..].to_vec(),
        COMPRESSION_ZSTD => zstd::bulk::decompress(&bytes[15..], decompressed_len)
            .map_err(|error| error.to_string())?,
        _ => return Err(format!("Unsupported egmesh compression mode {compression}")),
    };

    let mesh: EgMesh = serde_cbor::from_slice(&payload_bytes).map_err(|error| error.to_string())?;
    if mesh
        .indices
        .iter()
        .any(|index| *index as usize >= mesh.vertices.len())
    {
        return Err("Egmesh index references missing vertex".to_string());
    }
    Ok(mesh)
}

pub(crate) fn resolve_egmesh(mesh_path: &str) -> Option<&'static EgMesh> {
    let key = normalize_mesh_key(mesh_path);
    let meshes = egmeshes();
    meshes
        .get(&key)
        .or_else(|| meshes.get(&format!("assets/blocks/{key}")))
}

pub(crate) fn append_egmesh_geometry(
    geometry: &mut MeshGeometry,
    obj: &LevelObject,
    mesh: &EgMesh,
    color: [f32; 4],
    texture_layer: u32,
) {
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];

    // Recover the original OBJ aspect ratio from the stored bounds.
    let aspect = [
        (mesh.max[0] - mesh.min[0]).max(f32::EPSILON),
        (mesh.max[1] - mesh.min[1]).max(f32::EPSILON),
        (mesh.max[2] - mesh.min[2]).max(f32::EPSILON),
    ];
    // Uniform scale to contain the mesh within the block bounds (preserving aspect ratio).
    let scale = (obj.size[0] / aspect[0])
        .min(obj.size[1] / aspect[1])
        .min(obj.size[2] / aspect[2]);
    let offset = [
        (obj.size[0] - aspect[0] * scale) / 2.0,
        (obj.size[1] - aspect[1] * scale) / 2.0,
        (obj.size[2] - aspect[2] * scale) / 2.0,
    ];

    let mut vertices = Vec::with_capacity(mesh.vertices.len());
    for vertex in &mesh.vertices {
        vertices.push(Vertex::textured(
            [
                obj.position[0] + vertex.position[0] * aspect[0] * scale + offset[0],
                obj.position[1] + vertex.position[1] * aspect[1] * scale + offset[1],
                obj.position[2] + vertex.position[2] * aspect[2] * scale + offset[2],
            ],
            [
                color[0] * vertex.material_color[0] * vertex.normal_tint,
                color[1] * vertex.material_color[1] * vertex.normal_tint,
                color[2] * vertex.material_color[2] * vertex.normal_tint,
                color[3] * vertex.material_color[3],
            ],
            vertex.uv,
            texture_layer,
        ));
    }

    rotate_vertices_around_euler(&mut vertices, center, obj.rotation_degrees);
    geometry.append_indexed(vertices, &mesh.indices);
}

fn egmeshes() -> &'static HashMap<String, EgMesh> {
    EGMESHES.get_or_init(|| {
        let mut meshes = HashMap::new();
        for (key, bytes) in generated_egmesh_assets() {
            match decode_egmesh(bytes) {
                Ok(mesh) => {
                    meshes.entry(normalize_mesh_key(key)).or_insert(mesh);
                }
                Err(error) => {
                    crate::platform::io::log_platform_error(&format!(
                        "Failed to decode egmesh asset {key}: {error}"
                    ));
                }
            }
        }
        meshes
    })
}

fn generated_egmesh_assets() -> &'static [(&'static str, &'static [u8])] {
    include!(concat!(env!("OUT_DIR"), "/egmesh_manifest.rs"))
}

fn normalize_mesh_key(mesh_path: &str) -> String {
    mesh_path.trim().replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{decode_egmesh, encode_egmesh, EgMesh, EgMeshVertex};
    use crate::test_utils::assert_approx_eq;

    fn assert_float_array_approx_eq<const N: usize>(actual: [f32; N], expected: [f32; N]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_approx_eq(actual, expected, 1e-6);
        }
    }

    fn assert_mesh_approx_eq(actual: &EgMesh, expected: &EgMesh) {
        assert_eq!(actual.indices, expected.indices);
        assert_eq!(actual.vertices.len(), expected.vertices.len());
        assert_float_array_approx_eq(actual.min, expected.min);
        assert_float_array_approx_eq(actual.max, expected.max);
        for (actual, expected) in actual.vertices.iter().zip(&expected.vertices) {
            assert_float_array_approx_eq(actual.position, expected.position);
            assert_float_array_approx_eq(actual.uv, expected.uv);
            assert_approx_eq(actual.normal_tint, expected.normal_tint, 1e-6);
            assert_float_array_approx_eq(actual.material_color, expected.material_color);
        }
    }

    #[test]
    fn egmesh_roundtrip_preserves_payload() {
        let mesh = EgMesh {
            vertices: vec![
                EgMeshVertex {
                    position: [0.0, 0.0, 0.0],
                    uv: [0.0, 0.0],
                    normal_tint: 1.0,
                    material_color: [1.0, 0.5, 0.25, 1.0],
                },
                EgMeshVertex {
                    position: [1.0, 0.0, 0.0],
                    uv: [1.0, 0.0],
                    normal_tint: 0.75,
                    material_color: [0.5, 1.0, 0.25, 0.8],
                },
                EgMeshVertex {
                    position: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                    normal_tint: 0.5,
                    material_color: [0.25, 0.5, 1.0, 0.6],
                },
            ],
            indices: vec![0, 1, 2],
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 0.0],
        };

        let encoded = encode_egmesh(&mesh).expect("mesh should encode");
        let decoded = decode_egmesh(&encoded).expect("mesh should decode");
        assert_mesh_approx_eq(&decoded, &mesh);
    }

    #[test]
    fn egmesh_decode_rejects_out_of_range_indices() {
        let bytes = encode_egmesh(&EgMesh {
            vertices: vec![EgMeshVertex {
                position: [0.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                normal_tint: 1.0,
                material_color: [1.0, 1.0, 1.0, 1.0],
            }],
            indices: vec![0, 1],
            min: [0.0; 3],
            max: [0.0; 3],
        })
        .expect("mesh should encode");

        let error = decode_egmesh(&bytes).expect_err("out-of-range index should fail");
        assert!(error.contains("Egmesh index references missing vertex"));
    }

    #[test]
    fn egmesh_decode_rejects_invalid_magic() {
        let mut bytes = encode_egmesh(&EgMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            min: [0.0; 3],
            max: [0.0; 3],
        })
        .expect("mesh should encode");
        bytes[0] = b'X';

        let error = decode_egmesh(&bytes).expect_err("invalid magic should fail");
        assert!(error.contains("Invalid egmesh payload magic header"));
    }
}
