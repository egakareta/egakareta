/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
//! Level repository for managing built-in and user-created levels.
//!
//! Provides functionality to load level metadata from JSON files,
//! serialize/deserialize levels, and manage the level catalog.

use std::io::{Read, Write as _};
use std::sync::OnceLock;

use include_dir::{include_dir, Dir};

use crate::types::LevelMetadata;

static LEVELS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/levels");
static BUILTIN_LEVELS: OnceLock<Vec<LevelMetadata>> = OnceLock::new();

fn builtin_levels() -> &'static [LevelMetadata] {
    BUILTIN_LEVELS.get_or_init(|| {
        let mut levels = Vec::new();
        collect_builtin_levels(&LEVELS_DIR, &mut levels);
        levels.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        levels
    })
}

/// Returns the names of all built-in levels.
pub(crate) fn builtin_level_names() -> Vec<String> {
    builtin_levels()
        .iter()
        .map(|metadata| metadata.name.clone())
        .collect()
}

/// Loads metadata for a built-in level by name.
pub(crate) fn load_builtin_level_metadata(level_name: &str) -> Option<LevelMetadata> {
    builtin_levels()
        .iter()
        .find(|metadata| metadata.name == level_name)
        .cloned()
}

/// Retrieves the raw audio bytes for a built-in level's music file.
pub(crate) fn get_builtin_audio(level_name: &str, music_source: &str) -> Option<&'static [u8]> {
    let path = format!("{}/{}", level_name, music_source);
    LEVELS_DIR.get_file(path).map(|f| f.contents())
}

/// Parses level metadata from a JSON string.
pub(crate) fn parse_level_metadata_json(json: &str) -> Result<LevelMetadata, String> {
    let mut metadata: LevelMetadata =
        serde_json::from_str(json).map_err(|error| error.to_string())?;

    for object in &mut metadata.objects {
        object.normalize_block_id();
    }
    Ok(metadata)
}

/// Serializes level metadata to a pretty-printed JSON string.
pub(crate) fn serialize_level_metadata_pretty(metadata: &LevelMetadata) -> Result<String, String> {
    serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())
}

/// Builds an egz archive containing level metadata and optional audio data.
pub(crate) fn build_egz_archive(
    metadata: &LevelMetadata,
    audio_file: Option<(&str, &[u8])>,
) -> Result<Vec<u8>, String> {
    let metadata_json = serialize_level_metadata_pretty(metadata)?;

    let mut buffer = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("metadata.json", options)
        .map_err(|error| error.to_string())?;
    zip.write_all(metadata_json.as_bytes())
        .map_err(|error| error.to_string())?;

    if let Some((filename, bytes)) = audio_file {
        zip.start_file(filename, options)
            .map_err(|error| error.to_string())?;
        zip.write_all(bytes).map_err(|error| error.to_string())?;
    }

    zip.finish().map_err(|error| error.to_string())?;
    Ok(buffer)
}

/// Reads level metadata and optional audio data from an egz archive.
pub(crate) fn read_metadata_from_egz(
    data: &[u8],
) -> Result<(LevelMetadata, Option<Vec<u8>>), String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;

    let mut metadata_file = archive
        .by_name("metadata.json")
        .map_err(|error| error.to_string())?;
    let mut metadata_json = String::new();
    metadata_file
        .read_to_string(&mut metadata_json)
        .map_err(|error| error.to_string())?;
    drop(metadata_file); // Drop to allow another borrow

    let metadata = parse_level_metadata_json(&metadata_json)?;

    // Try to read the audio file if it exists
    let audio_bytes = if let Ok(mut audio_file) = archive.by_name(&metadata.music.source) {
        let mut bytes = Vec::new();
        audio_file
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Some(bytes)
    } else {
        None
    };

    Ok((metadata, audio_bytes))
}

fn collect_builtin_levels(dir: &Dir<'_>, levels: &mut Vec<LevelMetadata>) {
    for file in dir.files() {
        let is_metadata = file
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("metadata.json"))
            .unwrap_or(false);

        if !is_metadata {
            continue;
        }

        if let Some(json) = file.contents_utf8() {
            if let Ok(metadata) = parse_level_metadata_json(json) {
                levels.push(metadata);
            }
        }
    }

    for child in dir.dirs() {
        collect_builtin_levels(child, levels);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_level_names, load_builtin_level_metadata, parse_level_metadata_json,
        serialize_level_metadata_pretty,
    };

    #[test]
    fn discovers_builtin_levels_from_assets_directory() {
        let names = builtin_level_names();
        assert!(!names.is_empty());
    }

    #[test]
    fn loads_known_level_metadata() {
        let metadata = load_builtin_level_metadata("Flowerfield");
        assert!(metadata.is_some());
    }

    #[test]
    fn parses_objects_without_kind_using_default() {
        let mut metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");
        metadata.objects[0].block_id = "core/stone".to_string();

        let mut json_value = serde_json::from_str::<serde_json::Value>(
            &serialize_level_metadata_pretty(&metadata).unwrap(),
        )
        .unwrap();
        json_value["objects"][0]
            .as_object_mut()
            .unwrap()
            .remove("block_id");

        let result = parse_level_metadata_json(&json_value.to_string()).expect("valid metadata");
        assert_eq!(result.objects[0].block_id, "core/stone");
    }
}
