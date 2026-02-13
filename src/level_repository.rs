use std::io::{Read, Write as _};

use crate::types::LevelMetadata;

pub(crate) fn builtin_level_names() -> Vec<String> {
    ["Flowerfield", "Golden Haze"]
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}

pub(crate) fn load_builtin_level_metadata(level_name: &str) -> Option<LevelMetadata> {
    builtin_level_metadata_str(level_name).and_then(|json| parse_level_metadata_json(json).ok())
}

pub(crate) fn parse_level_metadata_json(json: &str) -> Result<LevelMetadata, String> {
    serde_json::from_str(json).map_err(|error| error.to_string())
}

pub(crate) fn serialize_level_metadata_pretty(metadata: &LevelMetadata) -> Result<String, String> {
    serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())
}

pub(crate) fn build_ldz_archive(
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

pub(crate) fn read_metadata_from_ldz(data: &[u8]) -> Result<LevelMetadata, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;

    let mut metadata_file = archive
        .by_name("metadata.json")
        .map_err(|error| error.to_string())?;
    let mut metadata_json = String::new();
    metadata_file
        .read_to_string(&mut metadata_json)
        .map_err(|error| error.to_string())?;

    parse_level_metadata_json(&metadata_json)
}

fn builtin_level_metadata_str(level_name: &str) -> Option<&'static str> {
    match level_name {
        "Flowerfield" => Some(include_str!("../assets/levels/Flowerfield/metadata.json")),
        "Golden Haze" => Some(include_str!("../assets/levels/Golden Haze/metadata.json")),
        _ => None,
    }
}
