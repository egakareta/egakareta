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

pub(crate) fn builtin_level_names() -> Vec<String> {
    builtin_levels()
        .iter()
        .map(|metadata| metadata.name.clone())
        .collect()
}

pub(crate) fn load_builtin_level_metadata(level_name: &str) -> Option<LevelMetadata> {
    builtin_levels()
        .iter()
        .find(|metadata| metadata.name == level_name)
        .cloned()
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
        assert!(names.contains(&"Flowerfield".to_string()));
        assert!(names.contains(&"Golden Haze".to_string()));
    }

    #[test]
    fn loads_known_level_metadata() {
        let metadata = load_builtin_level_metadata("Flowerfield");
        assert!(metadata.is_some());
    }

    #[test]
    fn parses_objects_without_kind_using_default() {
        let mut metadata = load_builtin_level_metadata("Flowerfield").expect("missing level");
        metadata.objects[0].kind = crate::types::BlockKind::Standard;

        let mut json_value = serde_json::from_str::<serde_json::Value>(
            &serialize_level_metadata_pretty(&metadata).unwrap(),
        )
        .unwrap();
        json_value["objects"][0]
            .as_object_mut()
            .unwrap()
            .remove("kind");

        let result = parse_level_metadata_json(&json_value.to_string()).expect("valid metadata");
        assert!(matches!(
            result.objects[0].kind,
            crate::types::BlockKind::Standard
        ));
    }
}
