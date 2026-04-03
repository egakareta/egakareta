/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::level_repository::{
    build_egz_archive, parse_level_metadata_binary, parse_level_metadata_json,
    read_metadata_from_egz, serialize_level_metadata_binary,
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
    serialize_level_metadata_binary(metadata)
}

pub fn parse_level_binary_import(data: &[u8]) -> Result<LevelMetadata, String> {
    parse_level_metadata_binary(data)
}

/// Converts legacy JSON level metadata into binary metadata bytes.
///
/// This helper is intended for one-time migration tooling.
pub fn convert_level_json_to_binary(json: &str) -> Result<Vec<u8>, String> {
    let metadata = parse_level_metadata_json(json)?;
    serialize_level_metadata_binary(&metadata)
}

/// Re-encodes binary level metadata bytes into the current codec format.
pub fn normalize_level_binary_format(data: &[u8]) -> Result<Vec<u8>, String> {
    let metadata = parse_level_metadata_binary(data)?;
    serialize_level_metadata_binary(&metadata)
}

pub fn parse_level_egz_import(data: &[u8]) -> Result<(LevelMetadata, Option<Vec<u8>>), String> {
    read_metadata_from_egz(data)
}
