/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use serde::{Deserialize, Serialize};

use crate::types::{
    LevelMetadata, LevelObject, MusicMetadata, SpawnMetadata, TimedTrigger, TimingPoint,
};

const LEVEL_MAGIC: [u8; 4] = *b"EGB1";
const LEVEL_CODEC_V1: u16 = 1;
const LEVEL_CODEC_VERSION: u16 = 2;
const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_ZSTD: u8 = 1;
const ZSTD_LEVEL: i32 = 3;

#[derive(Serialize, Deserialize)]
struct BinaryLevelPayloadV1 {
    format_version: u32,
    name: String,
    music_source: String,
    music_title: Option<String>,
    music_author: Option<String>,
    music_extra_entries: Vec<MetadataEntry>,
    spawn: SpawnMetadata,
    tap_times: Vec<f32>,
    timing_points: Vec<TimingPoint>,
    timeline_time_seconds: f32,
    timeline_duration_seconds: f32,
    triggers: Vec<TimedTrigger>,
    simulate_trigger_hitboxes: bool,
    level_extra_entries: Vec<MetadataEntry>,
    palette: Vec<String>,
    object_runs: Vec<ObjectRun>,
    #[serde(default)]
    object_compact_mask: Vec<u8>,
    #[serde(default)]
    grid_min: [i32; 3],
    #[serde(default)]
    grid_dims: [u32; 3],
    #[serde(default)]
    object_linear_indices: Vec<u32>,
    object_positions: Vec<[f32; 3]>,
    object_sizes: Vec<[f32; 3]>,
    object_rotations: Vec<[f32; 3]>,
    object_roundness: Vec<f32>,
    object_color_tints: Vec<[f32; 3]>,
}

#[derive(Serialize, Deserialize)]
struct ObjectRun {
    palette_index: u16,
    run_length: u32,
}

#[derive(Serialize, Deserialize)]
struct MetadataEntry {
    key: String,
    value_json: String,
}

fn push_full_object_streams(
    object: &LevelObject,
    object_positions: &mut Vec<[f32; 3]>,
    object_sizes: &mut Vec<[f32; 3]>,
    object_rotations: &mut Vec<[f32; 3]>,
    object_roundness: &mut Vec<f32>,
    object_color_tints: &mut Vec<[f32; 3]>,
) {
    object_positions.push(object.position);
    object_sizes.push(object.size);
    object_rotations.push(object.rotation_degrees);
    object_roundness.push(object.roundness);
    object_color_tints.push(object.color_tint);
}

pub(crate) fn encode_level_metadata_binary(metadata: &LevelMetadata) -> Result<Vec<u8>, String> {
    let mut palette = Vec::<String>::new();
    let mut palette_lookup = std::collections::HashMap::<String, u16>::new();
    let mut palette_indices = Vec::<u16>::with_capacity(metadata.objects.len());

    let mut object_positions = Vec::with_capacity(metadata.objects.len());
    let mut object_sizes = Vec::with_capacity(metadata.objects.len());
    let mut object_rotations = Vec::with_capacity(metadata.objects.len());
    let mut object_roundness = Vec::with_capacity(metadata.objects.len());
    let mut object_color_tints = Vec::with_capacity(metadata.objects.len());

    let mut compact_positions = Vec::<[i32; 3]>::new();
    let mut compact_mask = vec![0u8; metadata.objects.len().div_ceil(8)];

    for object in &metadata.objects {
        let palette_index = if let Some(index) = palette_lookup.get(&object.block_id) {
            *index
        } else {
            let next_index = u16::try_from(palette.len())
                .map_err(|_| "Too many unique block ids for binary palette".to_string())?;
            palette_lookup.insert(object.block_id.clone(), next_index);
            palette.push(object.block_id.clone());
            next_index
        };

        palette_indices.push(palette_index);

        if let Some(quantized_position) = quantize_compact_position(object) {
            compact_positions.push(quantized_position);
            set_compact_bit(&mut compact_mask, palette_indices.len() - 1);
        } else {
            push_full_object_streams(
                object,
                &mut object_positions,
                &mut object_sizes,
                &mut object_rotations,
                &mut object_roundness,
                &mut object_color_tints,
            );
        }
    }

    let compact_count = compact_positions.len();
    let object_count = metadata.objects.len();
    let full_count = object_count.saturating_sub(compact_count);

    let (object_compact_mask, grid_min, grid_dims, object_linear_indices) =
        if compact_count > 0 && full_count > 0 {
            let (grid_min, grid_dims) = compute_grid_bounds(&compact_positions)?;
            let mut linear_indices = Vec::with_capacity(compact_count);

            for compact_position in &compact_positions {
                linear_indices.push(position_to_linear_index(
                    *compact_position,
                    grid_min,
                    grid_dims,
                )?);
            }

            (compact_mask, grid_min, grid_dims, linear_indices)
        } else {
            // Keep a single encoding lane when everything is compact or everything is full.
            if full_count == 0 {
                object_positions.clear();
                object_sizes.clear();
                object_rotations.clear();
                object_roundness.clear();
                object_color_tints.clear();

                for object in &metadata.objects {
                    push_full_object_streams(
                        object,
                        &mut object_positions,
                        &mut object_sizes,
                        &mut object_rotations,
                        &mut object_roundness,
                        &mut object_color_tints,
                    );
                }
            }

            (Vec::new(), [0, 0, 0], [0, 0, 0], Vec::new())
        };

    let payload = BinaryLevelPayloadV1 {
        format_version: metadata.format_version,
        name: metadata.name.clone(),
        music_source: metadata.music.source.clone(),
        music_title: metadata.music.title.clone(),
        music_author: metadata.music.author.clone(),
        music_extra_entries: map_to_entries(&metadata.music.extra)?,
        spawn: metadata.spawn.clone(),
        tap_times: metadata.tap_times.clone(),
        timing_points: metadata.timing_points.clone(),
        timeline_time_seconds: metadata.timeline_time_seconds,
        timeline_duration_seconds: metadata.timeline_duration_seconds,
        triggers: metadata.triggers.clone(),
        simulate_trigger_hitboxes: metadata.simulate_trigger_hitboxes,
        level_extra_entries: map_to_entries(&metadata.extra)?,
        palette,
        object_runs: rle_encode_palette_indices(&palette_indices),
        object_compact_mask,
        grid_min,
        grid_dims,
        object_linear_indices,
        object_positions,
        object_sizes,
        object_rotations,
        object_roundness,
        object_color_tints,
    };

    let payload_bytes = serde_cbor::to_vec(&payload).map_err(|error| error.to_string())?;
    let compressed_payload =
        zstd::bulk::compress(&payload_bytes, ZSTD_LEVEL).map_err(|error| error.to_string())?;

    let mut encoded = Vec::with_capacity(4 + 2 + 1 + 4 + 4 + compressed_payload.len());
    encoded.extend_from_slice(&LEVEL_MAGIC);
    encoded.extend_from_slice(&LEVEL_CODEC_VERSION.to_le_bytes());
    encoded.push(COMPRESSION_ZSTD);
    encoded.extend_from_slice(
        &u32::try_from(compressed_payload.len())
            .map_err(|_| "Level payload too large".to_string())?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(
        &u32::try_from(payload_bytes.len())
            .map_err(|_| "Level payload too large".to_string())?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(&compressed_payload);

    Ok(encoded)
}

pub(crate) fn decode_level_metadata_binary(bytes: &[u8]) -> Result<LevelMetadata, String> {
    if bytes.len() < 11 {
        return Err("Level payload is too small".to_string());
    }

    if bytes[0..4] != LEVEL_MAGIC {
        return Err("Invalid level payload magic header".to_string());
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    let payload = match version {
        LEVEL_CODEC_V1 => decode_payload_v1(bytes)?,
        LEVEL_CODEC_VERSION => decode_payload_v2(bytes)?,
        _ => {
            return Err(format!(
                "Unsupported level codec version {version}; expected {LEVEL_CODEC_V1} or {LEVEL_CODEC_VERSION}"
            ));
        }
    };

    decode_payload(payload)
}

fn decode_payload_v1(bytes: &[u8]) -> Result<BinaryLevelPayloadV1, String> {
    let compression = bytes[6];
    let payload_len = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
    let expected_total = 11usize
        .checked_add(payload_len)
        .ok_or_else(|| "Level payload size overflow".to_string())?;

    if bytes.len() != expected_total {
        return Err("Level payload length mismatch".to_string());
    }

    let payload_bytes = match compression {
        COMPRESSION_NONE => bytes[11..].to_vec(),
        COMPRESSION_ZSTD => {
            return Err(
                "zstd-compressed legacy payload missing decompressed-size header".to_string(),
            );
        }
        _ => return Err("Unsupported level compression algorithm".to_string()),
    };

    serde_cbor::from_slice(&payload_bytes).map_err(|error| error.to_string())
}

fn decode_payload_v2(bytes: &[u8]) -> Result<BinaryLevelPayloadV1, String> {
    if bytes.len() < 15 {
        return Err("Level payload is too small for codec v2".to_string());
    }

    let compression = bytes[6];
    let payload_len = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
    let decompressed_len =
        u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]) as usize;
    let expected_total = 15usize
        .checked_add(payload_len)
        .ok_or_else(|| "Level payload size overflow".to_string())?;

    if bytes.len() != expected_total {
        return Err("Level payload length mismatch".to_string());
    }

    let payload_bytes = match compression {
        COMPRESSION_NONE => {
            if decompressed_len != payload_len {
                return Err("Codec v2 uncompressed payload size mismatch".to_string());
            }
            bytes[15..].to_vec()
        }
        COMPRESSION_ZSTD => zstd::bulk::decompress(&bytes[15..], decompressed_len)
            .map_err(|error| error.to_string())?,
        _ => return Err("Unsupported level compression algorithm".to_string()),
    };

    if payload_bytes.len() != decompressed_len {
        return Err("Codec v2 decompressed payload size mismatch".to_string());
    }

    serde_cbor::from_slice(&payload_bytes).map_err(|error| error.to_string())
}

fn decode_payload(payload: BinaryLevelPayloadV1) -> Result<LevelMetadata, String> {
    let palette_indices = rle_decode_palette_indices(&payload.object_runs)?;
    let object_count = palette_indices.len();
    let mut objects = Vec::with_capacity(object_count);

    if payload.object_compact_mask.is_empty() {
        if payload.object_positions.len() != object_count
            || payload.object_sizes.len() != object_count
            || payload.object_rotations.len() != object_count
            || payload.object_roundness.len() != object_count
            || payload.object_color_tints.len() != object_count
        {
            return Err("Object stream length mismatch in binary payload".to_string());
        }

        for (index, palette_index_value) in palette_indices.iter().enumerate() {
            let palette_index = usize::from(*palette_index_value);
            let Some(block_id) = payload.palette.get(palette_index) else {
                return Err("Object palette index out of range".to_string());
            };

            let mut object = LevelObject {
                position: payload.object_positions[index],
                size: payload.object_sizes[index],
                rotation_degrees: payload.object_rotations[index],
                roundness: payload.object_roundness[index],
                block_id: block_id.clone(),
                color_tint: payload.object_color_tints[index],
            };
            object.normalize_block_id();
            objects.push(object);
        }
    } else {
        let compact_count = count_compact_bits(&payload.object_compact_mask, object_count);
        if payload.object_linear_indices.len() != compact_count {
            return Err("Compact object index stream length mismatch".to_string());
        }

        let full_count = object_count.saturating_sub(compact_count);
        if payload.object_positions.len() != full_count
            || payload.object_sizes.len() != full_count
            || payload.object_rotations.len() != full_count
            || payload.object_roundness.len() != full_count
            || payload.object_color_tints.len() != full_count
        {
            return Err("Full object stream length mismatch in compact payload".to_string());
        }

        let mut compact_cursor = 0usize;
        let mut full_cursor = 0usize;

        for (index, palette_index_value) in palette_indices.iter().enumerate() {
            let palette_index = usize::from(*palette_index_value);
            let Some(block_id) = payload.palette.get(palette_index) else {
                return Err("Object palette index out of range".to_string());
            };

            let mut object = if is_compact_bit_set(&payload.object_compact_mask, index) {
                let linear_index = payload
                    .object_linear_indices
                    .get(compact_cursor)
                    .copied()
                    .ok_or_else(|| "Compact object cursor overflow".to_string())?;
                compact_cursor += 1;

                LevelObject {
                    position: linear_index_to_position(
                        linear_index,
                        payload.grid_min,
                        payload.grid_dims,
                    )?,
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    roundness: 0.0,
                    block_id: block_id.clone(),
                    color_tint: [1.0, 1.0, 1.0],
                }
            } else {
                let object = LevelObject {
                    position: *payload
                        .object_positions
                        .get(full_cursor)
                        .ok_or_else(|| "Full object position cursor overflow".to_string())?,
                    size: *payload
                        .object_sizes
                        .get(full_cursor)
                        .ok_or_else(|| "Full object size cursor overflow".to_string())?,
                    rotation_degrees: *payload
                        .object_rotations
                        .get(full_cursor)
                        .ok_or_else(|| "Full object rotation cursor overflow".to_string())?,
                    roundness: *payload
                        .object_roundness
                        .get(full_cursor)
                        .ok_or_else(|| "Full object roundness cursor overflow".to_string())?,
                    block_id: block_id.clone(),
                    color_tint: *payload
                        .object_color_tints
                        .get(full_cursor)
                        .ok_or_else(|| "Full object color cursor overflow".to_string())?,
                };
                full_cursor += 1;
                object
            };

            object.normalize_block_id();
            objects.push(object);
        }
    }

    Ok(LevelMetadata {
        format_version: payload.format_version,
        name: payload.name,
        music: MusicMetadata {
            source: payload.music_source,
            title: payload.music_title,
            author: payload.music_author,
            extra: entries_to_map(payload.music_extra_entries)?,
        },
        spawn: payload.spawn,
        tap_times: payload.tap_times,
        timing_points: payload.timing_points,
        timeline_time_seconds: payload.timeline_time_seconds,
        timeline_duration_seconds: payload.timeline_duration_seconds,
        triggers: payload.triggers,
        simulate_trigger_hitboxes: payload.simulate_trigger_hitboxes,
        objects,
        extra: entries_to_map(payload.level_extra_entries)?,
    })
}

fn quantize_compact_position(object: &LevelObject) -> Option<[i32; 3]> {
    if object.size != [1.0, 1.0, 1.0]
        || object.rotation_degrees != [0.0, 0.0, 0.0]
        || object.roundness != 0.0
        || object.color_tint != [1.0, 1.0, 1.0]
    {
        return None;
    }

    let mut quantized = [0i32; 3];
    for (index, component) in object.position.iter().enumerate() {
        let rounded = component.round();
        if (component - rounded).abs() > 1e-6 {
            return None;
        }

        if rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
            return None;
        }
        quantized[index] = rounded as i32;
    }

    Some(quantized)
}

fn compute_grid_bounds(positions: &[[i32; 3]]) -> Result<([i32; 3], [u32; 3]), String> {
    let mut min = positions[0];
    let mut max = positions[0];

    for position in &positions[1..] {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }

    let mut dims = [0u32; 3];
    for axis in 0..3 {
        let span = i64::from(max[axis]) - i64::from(min[axis]) + 1;
        if span <= 0 {
            return Err("Invalid compact grid span".to_string());
        }
        dims[axis] = u32::try_from(span).map_err(|_| "Compact grid span too large".to_string())?;
    }

    Ok((min, dims))
}

fn position_to_linear_index(
    position: [i32; 3],
    grid_min: [i32; 3],
    grid_dims: [u32; 3],
) -> Result<u32, String> {
    let x = u64::try_from(i64::from(position[0]) - i64::from(grid_min[0]))
        .map_err(|_| "Compact x offset underflow".to_string())?;
    let y = u64::try_from(i64::from(position[1]) - i64::from(grid_min[1]))
        .map_err(|_| "Compact y offset underflow".to_string())?;
    let z = u64::try_from(i64::from(position[2]) - i64::from(grid_min[2]))
        .map_err(|_| "Compact z offset underflow".to_string())?;

    let dy = u64::from(grid_dims[1]);
    let dz = u64::from(grid_dims[2]);
    let linear = x
        .checked_mul(dy)
        .and_then(|value| value.checked_mul(dz))
        .and_then(|value| value.checked_add(y.checked_mul(dz)?))
        .and_then(|value| value.checked_add(z))
        .ok_or_else(|| "Compact linear index overflow".to_string())?;

    u32::try_from(linear).map_err(|_| "Compact linear index too large".to_string())
}

fn linear_index_to_position(
    linear_index: u32,
    grid_min: [i32; 3],
    grid_dims: [u32; 3],
) -> Result<[f32; 3], String> {
    if grid_dims.contains(&0) {
        return Err("Invalid compact grid dimensions".to_string());
    }

    let dy = u64::from(grid_dims[1]);
    let dz = u64::from(grid_dims[2]);
    let yz = dy
        .checked_mul(dz)
        .ok_or_else(|| "Compact grid dimension overflow".to_string())?;

    let mut remainder = u64::from(linear_index);
    let x = remainder / yz;
    remainder %= yz;
    let y = remainder / dz;
    let z = remainder % dz;

    if x >= u64::from(grid_dims[0]) || y >= dy || z >= dz {
        return Err("Compact linear index out of bounds".to_string());
    }

    Ok([
        (i64::from(grid_min[0]) + i64::try_from(x).map_err(|_| "x overflow".to_string())?) as f32,
        (i64::from(grid_min[1]) + i64::try_from(y).map_err(|_| "y overflow".to_string())?) as f32,
        (i64::from(grid_min[2]) + i64::try_from(z).map_err(|_| "z overflow".to_string())?) as f32,
    ])
}

fn set_compact_bit(mask: &mut [u8], index: usize) {
    let byte_index = index / 8;
    let bit_index = index % 8;
    mask[byte_index] |= 1u8 << bit_index;
}

fn is_compact_bit_set(mask: &[u8], index: usize) -> bool {
    let byte_index = index / 8;
    if byte_index >= mask.len() {
        return false;
    }

    let bit_index = index % 8;
    (mask[byte_index] & (1u8 << bit_index)) != 0
}

fn count_compact_bits(mask: &[u8], object_count: usize) -> usize {
    let mut count = 0usize;
    for index in 0..object_count {
        if is_compact_bit_set(mask, index) {
            count += 1;
        }
    }
    count
}

fn map_to_entries(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<MetadataEntry>, String> {
    let mut entries = Vec::with_capacity(map.len());
    for (key, value) in map {
        let value_json = serde_json::to_string(value).map_err(|error| error.to_string())?;
        entries.push(MetadataEntry {
            key: key.clone(),
            value_json,
        });
    }
    Ok(entries)
}

fn entries_to_map(
    entries: Vec<MetadataEntry>,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut map = serde_json::Map::new();
    for entry in entries {
        let value = serde_json::from_str::<serde_json::Value>(&entry.value_json)
            .map_err(|error| error.to_string())?;
        map.insert(entry.key, value);
    }
    Ok(map)
}

fn rle_encode_palette_indices(indices: &[u16]) -> Vec<ObjectRun> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut current_value = indices[0];
    let mut current_len = 1u32;

    for &value in &indices[1..] {
        if value == current_value && current_len < u32::MAX {
            current_len += 1;
            continue;
        }

        runs.push(ObjectRun {
            palette_index: current_value,
            run_length: current_len,
        });
        current_value = value;
        current_len = 1;
    }

    runs.push(ObjectRun {
        palette_index: current_value,
        run_length: current_len,
    });

    runs
}

fn rle_decode_palette_indices(runs: &[ObjectRun]) -> Result<Vec<u16>, String> {
    let mut total_len = 0usize;
    for run in runs {
        let run_length = usize::try_from(run.run_length)
            .map_err(|_| "Invalid run length in binary payload".to_string())?;
        total_len = total_len
            .checked_add(run_length)
            .ok_or_else(|| "Palette run length overflow".to_string())?;
    }

    let mut values = Vec::with_capacity(total_len);
    for run in runs {
        if run.run_length == 0 {
            return Err("Invalid zero-length run in binary payload".to_string());
        }

        values.extend(std::iter::repeat_n(
            run.palette_index,
            run.run_length as usize,
        ));
    }

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{
        count_compact_bits, decode_level_metadata_binary, encode_level_metadata_binary,
        BinaryLevelPayloadV1, COMPRESSION_NONE, COMPRESSION_ZSTD, LEVEL_CODEC_V1,
        LEVEL_CODEC_VERSION, LEVEL_MAGIC,
    };
    use crate::level_repository::load_builtin_level_metadata;
    use crate::types::LevelObject;

    #[test]
    fn binary_codec_roundtrip_preserves_objects() {
        let mut metadata =
            load_builtin_level_metadata("Flowerfield").expect("missing built-in level");
        metadata.name = "Binary Roundtrip".to_string();
        metadata.objects = vec![
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [1.0, 2.0, 3.0],
                size: [2.0, 2.0, 2.0],
                rotation_degrees: [0.0, 90.0, 0.0],
                roundness: 0.25,
                color_tint: [0.8, 0.7, 0.6],
            },
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [4.0, 5.0, 6.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                block_id: "core/grass".to_string(),
                position: [7.0, 8.0, 9.0],
                size: [3.0, 1.0, 3.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
        ];

        let encoded = encode_level_metadata_binary(&metadata).expect("encode");
        let decoded = decode_level_metadata_binary(&encoded).expect("decode");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.objects, metadata.objects);
    }

    #[test]
    fn hybrid_compact_stream_uses_linear_indices_and_roundtrips() {
        let mut metadata =
            load_builtin_level_metadata("Flowerfield").expect("missing built-in level");
        metadata.name = "Hybrid Compact".to_string();
        metadata.objects = vec![
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [10.0, 20.0, 30.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                block_id: "core/grass".to_string(),
                position: [11.25, 21.0, 31.0],
                size: [2.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
        ];

        let encoded = encode_level_metadata_binary(&metadata).expect("encode");
        let payload = decode_v2_payload_for_test(&encoded);

        assert!(!payload.object_compact_mask.is_empty());
        assert_eq!(count_compact_bits(&payload.object_compact_mask, 2), 1);
        assert_eq!(payload.object_linear_indices.len(), 1);
        assert_eq!(payload.object_positions.len(), 1);
        assert_eq!(payload.object_sizes.len(), 1);

        let decoded = decode_level_metadata_binary(&encoded).expect("decode");
        assert_eq!(decoded.objects, metadata.objects);
    }

    #[test]
    fn decodes_legacy_v1_uncompressed_payload() {
        let mut metadata =
            load_builtin_level_metadata("Golden Haze").expect("missing built-in level");
        metadata.name = "Legacy Decode".to_string();
        metadata.objects = vec![LevelObject {
            block_id: "core/stone".to_string(),
            position: [2.0, 3.0, 4.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.0,
            color_tint: [1.0, 1.0, 1.0],
        }];

        let encoded_v2 = encode_level_metadata_binary(&metadata).expect("encode v2");
        let payload = decode_v2_payload_for_test(&encoded_v2);
        let legacy_bytes = encode_v1_uncompressed_for_test(&payload);

        let decoded = decode_level_metadata_binary(&legacy_bytes).expect("decode v1");
        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.objects, metadata.objects);
    }

    #[test]
    fn rejects_corrupt_compact_stream_lengths() {
        let mut metadata =
            load_builtin_level_metadata("Flowerfield").expect("missing built-in level");
        metadata.objects = vec![
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                block_id: "core/grass".to_string(),
                position: [0.5, 0.0, 0.0],
                size: [2.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.0,
                color_tint: [1.0, 1.0, 1.0],
            },
        ];

        let encoded = encode_level_metadata_binary(&metadata).expect("encode");
        let mut payload = decode_v2_payload_for_test(&encoded);
        payload.object_linear_indices.clear();

        let corrupted = encode_v2_uncompressed_for_test(&payload);
        let error = match decode_level_metadata_binary(&corrupted) {
            Ok(_) => panic!("expected decode failure"),
            Err(error) => error,
        };
        assert!(error.contains("Compact object index stream length mismatch"));
    }

    fn decode_v2_payload_for_test(bytes: &[u8]) -> BinaryLevelPayloadV1 {
        assert!(bytes.len() >= 15);
        assert_eq!(&bytes[0..4], &LEVEL_MAGIC);

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(version, LEVEL_CODEC_VERSION);

        let compression = bytes[6];
        let payload_len = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
        let decompressed_len =
            u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]) as usize;
        assert_eq!(bytes.len(), 15 + payload_len);

        let payload_bytes = match compression {
            COMPRESSION_NONE => {
                assert_eq!(decompressed_len, payload_len);
                bytes[15..].to_vec()
            }
            COMPRESSION_ZSTD => {
                zstd::bulk::decompress(&bytes[15..], decompressed_len).expect("decompress")
            }
            _ => panic!("unexpected compression flag"),
        };

        serde_cbor::from_slice(&payload_bytes).expect("deserialize payload")
    }

    fn encode_v1_uncompressed_for_test(payload: &BinaryLevelPayloadV1) -> Vec<u8> {
        let payload_bytes = serde_cbor::to_vec(payload).expect("serialize payload");

        let mut encoded = Vec::with_capacity(4 + 2 + 1 + 4 + payload_bytes.len());
        encoded.extend_from_slice(&LEVEL_MAGIC);
        encoded.extend_from_slice(&LEVEL_CODEC_V1.to_le_bytes());
        encoded.push(COMPRESSION_NONE);
        encoded.extend_from_slice(
            &u32::try_from(payload_bytes.len())
                .expect("payload size")
                .to_le_bytes(),
        );
        encoded.extend_from_slice(&payload_bytes);
        encoded
    }

    fn encode_v2_uncompressed_for_test(payload: &BinaryLevelPayloadV1) -> Vec<u8> {
        let payload_bytes = serde_cbor::to_vec(payload).expect("serialize payload");

        let mut encoded = Vec::with_capacity(4 + 2 + 1 + 4 + 4 + payload_bytes.len());
        encoded.extend_from_slice(&LEVEL_MAGIC);
        encoded.extend_from_slice(&LEVEL_CODEC_VERSION.to_le_bytes());
        encoded.push(COMPRESSION_NONE);
        encoded.extend_from_slice(
            &u32::try_from(payload_bytes.len())
                .expect("payload size")
                .to_le_bytes(),
        );
        encoded.extend_from_slice(
            &u32::try_from(payload_bytes.len())
                .expect("payload size")
                .to_le_bytes(),
        );
        encoded.extend_from_slice(&payload_bytes);
        encoded
    }
}
