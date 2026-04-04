/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
//! Level repository for managing built-in and user-created levels.
//!
//! Provides functionality to load level metadata from binary files,
//! serialize/deserialize levels, and manage the level catalog.

use std::io::{Read, Write as _};
use std::sync::OnceLock;

use include_dir::{include_dir, Dir};

use crate::level_codec::{decode_level_metadata_binary, encode_level_metadata_binary};
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
///
/// This is retained for one-time migration utilities.
pub(crate) fn parse_level_metadata_json(json: &str) -> Result<LevelMetadata, String> {
    let mut metadata: LevelMetadata =
        serde_json::from_str(json).map_err(|error| error.to_string())?;

    for object in &mut metadata.objects {
        object.normalize_block_id();
    }
    Ok(metadata)
}

/// Builds an egz archive containing level metadata and optional audio data.
pub(crate) fn build_egz_archive(
    metadata: &LevelMetadata,
    audio_file: Option<(&str, &[u8])>,
) -> Result<Vec<u8>, String> {
    let metadata_binary = encode_level_metadata_binary(metadata)?;

    let mut buffer = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("metadata.egb", options)
        .map_err(|error| error.to_string())?;
    zip.write_all(&metadata_binary)
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
        .by_name("metadata.egb")
        .map_err(|error| error.to_string())?;
    let mut metadata_binary = Vec::new();
    metadata_file
        .read_to_end(&mut metadata_binary)
        .map_err(|error| error.to_string())?;
    drop(metadata_file); // Drop to allow another borrow

    let metadata = decode_level_metadata_binary(&metadata_binary)?;

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
            .map(|name| name.eq_ignore_ascii_case("metadata.egb"))
            .unwrap_or(false);

        if !is_metadata {
            continue;
        }

        if let Ok(metadata) = decode_level_metadata_binary(file.contents()) {
            levels.push(metadata);
        }
    }

    for child in dir.dirs() {
        collect_builtin_levels(child, levels);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::{
        build_egz_archive, builtin_level_names, get_builtin_audio, load_builtin_level_metadata,
        parse_level_metadata_json, read_metadata_from_egz,
    };
    use crate::level_codec::{decode_level_metadata_binary, encode_level_metadata_binary};

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
    fn roundtrips_binary_metadata() {
        let metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");
        let encoded = encode_level_metadata_binary(&metadata).expect("serialize");
        let decoded = decode_level_metadata_binary(&encoded).expect("parse");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.objects.len(), metadata.objects.len());
    }

    #[test]
    fn parses_json_metadata_and_normalizes_legacy_block_kind() {
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

        let parsed = parse_level_metadata_json(json).expect("parse json");
        assert_eq!(parsed.objects.len(), 1);
        assert_eq!(parsed.objects[0].block_id, "core/stone");
    }

    #[test]
    fn builds_and_reads_egz_archive_without_audio() {
        let metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");

        let archive_bytes = build_egz_archive(&metadata, None).expect("build egz");
        let (decoded, audio) = read_metadata_from_egz(&archive_bytes).expect("read egz");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.objects.len(), metadata.objects.len());
        assert!(audio.is_none());
    }

    #[test]
    fn builds_and_reads_egz_archive_with_audio() {
        let mut metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");
        metadata.music.source = "custom.mp3".to_string();
        let audio = vec![8_u8, 6, 7, 5, 3, 0, 9];

        let archive_bytes =
            build_egz_archive(&metadata, Some(("custom.mp3", audio.as_slice()))).expect("build");
        let (decoded, decoded_audio) = read_metadata_from_egz(&archive_bytes).expect("read");

        assert_eq!(decoded.name, metadata.name);
        assert_eq!(decoded.music.source, "custom.mp3");
        assert_eq!(decoded_audio, Some(audio));
    }

    #[test]
    fn reads_builtin_audio_and_handles_missing_audio() {
        let metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");

        let bytes = get_builtin_audio("Flowerfield", &metadata.music.source)
            .expect("expected built-in audio bytes");
        assert!(!bytes.is_empty());

        assert!(get_builtin_audio("Flowerfield", "missing.mp3").is_none());
        assert!(get_builtin_audio("UnknownLevel", "audio.mp3").is_none());
    }

    #[test]
    fn read_metadata_from_egz_fails_when_metadata_file_missing() {
        let mut buffer = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buffer);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("payload.bin", options)
                .expect("start payload file");
            zip.write_all(b"not metadata").expect("write payload");
            zip.finish().expect("finalize zip");
        }

        assert!(read_metadata_from_egz(&buffer).is_err());
    }
}
