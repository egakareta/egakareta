/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::level_codec::{decode_level_metadata_binary, encode_level_metadata_binary};
use crate::level_repository::{
    build_egz_archive, parse_level_metadata_json, read_metadata_from_egz,
};
use crate::types::LevelMetadata;

pub fn build_level_export(
    metadata: &LevelMetadata,
    audio_bytes: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let audio_file = audio_bytes
        .as_ref()
        .map(|bytes| (metadata.music.source.as_str(), bytes.as_slice()));
    build_egz_archive(metadata, audio_file)
}

pub fn build_level_binary_export(metadata: &LevelMetadata) -> Result<Vec<u8>, String> {
    encode_level_metadata_binary(metadata)
}

pub fn parse_level_binary_import(data: &[u8]) -> Result<LevelMetadata, String> {
    decode_level_metadata_binary(data)
}

/// Converts legacy JSON level metadata into binary metadata bytes.
///
/// This helper is intended for one-time migration tooling.
pub fn convert_level_json_to_binary(json: &str) -> Result<Vec<u8>, String> {
    let metadata = parse_level_metadata_json(json)?;
    encode_level_metadata_binary(&metadata)
}

/// Converts binary level metadata into JSON level metadata.
pub fn convert_level_binary_to_json(data: &[u8]) -> Result<String, String> {
    let metadata = decode_level_metadata_binary(data)?;
    serde_json::to_string_pretty(&metadata).map_err(|error| error.to_string())
}

/// Re-encodes binary level metadata bytes into the current codec format.
pub fn normalize_level_binary_format(data: &[u8]) -> Result<Vec<u8>, String> {
    let metadata = decode_level_metadata_binary(data)?;
    encode_level_metadata_binary(&metadata)
}

pub fn parse_level_egz_import(data: &[u8]) -> Result<(LevelMetadata, Option<Vec<u8>>), String> {
    read_metadata_from_egz(data)
}

#[cfg(test)]
mod tests {
    use super::{
        build_level_binary_export, build_level_export, convert_level_binary_to_json,
        convert_level_json_to_binary, normalize_level_binary_format, parse_level_binary_import,
        parse_level_egz_import,
    };
    use crate::types::{
        EditorStateParams, LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata,
    };

    fn sample_metadata() -> LevelMetadata {
        LevelMetadata::from_editor_state(EditorStateParams {
            name: "Service Test".to_string(),
            music: MusicMetadata {
                source: "audio.mp3".to_string(),
                title: Some("Track".to_string()),
                author: Some("Composer".to_string()),
                extra: serde_json::Map::new(),
            },
            spawn: SpawnMetadata {
                position: [1.0, 2.0, 3.0],
                direction: SpawnDirection::Right,
            },
            tap_times: vec![0.25, 0.75],
            timing_points: Vec::new(),
            timeline_time_seconds: 0.5,
            timeline_duration_seconds: 12.0,
            triggers: Vec::new(),
            simulate_trigger_hitboxes: true,
            objects: vec![LevelObject {
                position: [4.0, 0.0, 2.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 45.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 0.8, 0.7],
            }],
        })
    }

    #[test]
    fn roundtrips_egz_export_with_audio() {
        let metadata = sample_metadata();
        let audio = vec![1_u8, 3, 3, 7, 9, 9];

        let bytes = build_level_export(&metadata, Some(audio.clone())).expect("build export");
        let (decoded, decoded_audio) = parse_level_egz_import(&bytes).expect("parse export");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.music.source, metadata.music.source);
        assert_eq!(decoded.spawn, metadata.spawn);
        assert_eq!(decoded.objects, metadata.objects);
        assert_eq!(decoded_audio, Some(audio));
    }

    #[test]
    fn binary_and_json_conversion_roundtrip_preserves_metadata() {
        let metadata = sample_metadata();

        let binary = build_level_binary_export(&metadata).expect("binary encode");
        let decoded = parse_level_binary_import(&binary).expect("binary decode");
        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.objects, metadata.objects);

        let json = convert_level_binary_to_json(&binary).expect("binary to json");
        let binary_from_json = convert_level_json_to_binary(&json).expect("json to binary");
        let decoded_from_json =
            parse_level_binary_import(&binary_from_json).expect("decode re-encoded binary");

        assert_eq!(decoded_from_json.name, metadata.name);
        assert_eq!(decoded_from_json.spawn, metadata.spawn);
        assert_eq!(decoded_from_json.tap_times, metadata.tap_times);
    }

    #[test]
    fn normalize_binary_format_keeps_semantics() {
        let metadata = sample_metadata();
        let binary = build_level_binary_export(&metadata).expect("binary encode");

        let normalized = normalize_level_binary_format(&binary).expect("normalize");
        let decoded = parse_level_binary_import(&normalized).expect("decode normalized");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.music, metadata.music);
        assert_eq!(decoded.objects, metadata.objects);
    }

    #[test]
    fn convert_level_json_to_binary_normalizes_block_ids() {
        let json = r#"{
            "name": "Legacy",
            "music": { "source": "audio.mp3" },
            "objects": [
                {
                    "position": [0.0, 0.0, 0.0],
                    "size": [1.0, 1.0, 1.0],
                    "kind": "stone"
                }
            ]
        }"#;

        let binary = convert_level_json_to_binary(json).expect("convert");
        let decoded = parse_level_binary_import(&binary).expect("decode");

        assert_eq!(decoded.objects.len(), 1);
        assert_eq!(decoded.objects[0].block_id, "core/stone");
    }

    #[test]
    fn rejects_invalid_payloads_for_binary_and_egz_inputs() {
        let invalid = b"not a valid payload";

        assert!(parse_level_binary_import(invalid).is_err());
        assert!(convert_level_binary_to_json(invalid).is_err());
        assert!(parse_level_egz_import(invalid).is_err());
    }
}
