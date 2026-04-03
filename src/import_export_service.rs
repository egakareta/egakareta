/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::level_repository::{
    build_egz_archive, parse_level_metadata_json, read_metadata_from_egz,
    serialize_level_metadata_pretty,
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

pub fn build_level_json_export(metadata: &LevelMetadata) -> String {
    serialize_level_metadata_pretty(metadata).unwrap_or_default()
}

pub fn parse_level_import(json: &str) -> Result<LevelMetadata, String> {
    parse_level_metadata_json(json)
}

pub fn parse_level_egz_import(data: &[u8]) -> Result<(LevelMetadata, Option<Vec<u8>>), String> {
    read_metadata_from_egz(data)
}
