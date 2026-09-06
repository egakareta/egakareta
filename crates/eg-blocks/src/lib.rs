/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Block repository for managing block definitions and behaviors.
//!
//! Provides a catalog of block types loaded from JSON files in assets/blocks/.
//! Each block defines its visual appearance, collision behavior, and gameplay effects.

mod block_id;
mod block_schema;

use std::{collections::HashMap, sync::OnceLock};

pub use block_id::*;
pub use block_schema::*;
use image::imageops::FilterType;
use include_dir::{Dir, include_dir};

static BLOCKS_DIR: Dir<'_> = include_dir!("$EGAKARETA_WORKSPACE_ROOT/assets/blocks");
static BLOCK_DEFINITIONS: OnceLock<Vec<BlockDefinition>> = OnceLock::new();
static BLOCK_DEFINITION_INDEX: OnceLock<HashMap<BlockId, usize>> = OnceLock::new();
static BLOCK_TEXTURE_ATLAS: OnceLock<BlockTextureAtlas> = OnceLock::new();

const BLOCK_TEXTURE_EDGE: u32 = 64;
const DEFAULT_BLOCK_TEXTURE_KEY: &str = "__default_white__";

#[derive(Clone)]
pub struct BlockTextureLayer {
    pub rgba: Vec<u8>,
}

pub struct BlockTextureAtlas {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<BlockTextureLayer>,
    by_key: HashMap<String, u32>,
    default_layer: u32,
}

impl BlockTextureAtlas {
    fn new() -> Self {
        let mut atlas = Self {
            width: BLOCK_TEXTURE_EDGE,
            height: BLOCK_TEXTURE_EDGE,
            layers: Vec::new(),
            by_key: HashMap::new(),
            default_layer: 0,
        };

        atlas.add_layer(
            DEFAULT_BLOCK_TEXTURE_KEY,
            vec![255; (BLOCK_TEXTURE_EDGE * BLOCK_TEXTURE_EDGE * 4) as usize],
        );

        atlas
    }

    fn add_layer(&mut self, key: &str, rgba: Vec<u8>) {
        let index = self.layers.len() as u32;
        let normalized = normalize_texture_key(key);
        self.layers.push(BlockTextureLayer { rgba });

        self.insert_texture_alias(&normalized, index);

        if let Some(trimmed) = normalized.strip_prefix("assets/blocks/") {
            self.insert_texture_alias(trimmed, index);
        }

        if let Some(file_name) = normalized.rsplit('/').next() {
            self.insert_texture_alias(file_name, index);

            if let Some(stem) = file_name.rsplit_once('.') {
                self.insert_texture_alias(stem.0, index);
            }
        }
    }

    fn insert_texture_alias(&mut self, key: &str, index: u32) {
        let normalized = normalize_texture_key(key);
        if normalized.is_empty() {
            return;
        }
        self.by_key.entry(normalized).or_insert(index);
    }

    pub fn resolve_layer(&self, key: &str) -> Option<u32> {
        let normalized = normalize_texture_key(key);
        if normalized.is_empty() {
            return None;
        }

        if let Some(index) = self.by_key.get(&normalized) {
            return Some(*index);
        }

        if !normalized.ends_with(".png") && !normalized.ends_with(".bmp") {
            let with_extension = format!("{normalized}.png");
            if let Some(index) = self.by_key.get(&with_extension) {
                return Some(*index);
            }
            let with_bmp = format!("{normalized}.bmp");
            if let Some(index) = self.by_key.get(&with_bmp) {
                return Some(*index);
            }

            let with_prefix = format!("assets/blocks/{with_extension}");
            if let Some(index) = self.by_key.get(&with_prefix) {
                return Some(*index);
            }
            let with_bmp_prefix = format!("assets/blocks/{with_bmp}");
            if let Some(index) = self.by_key.get(&with_bmp_prefix) {
                return Some(*index);
            }
        }

        let with_prefix = format!("assets/blocks/{normalized}");
        self.by_key.get(&with_prefix).copied()
    }

    pub fn default_layer(&self) -> u32 {
        self.default_layer
    }

    pub fn get() -> &'static Self {
        BLOCK_TEXTURE_ATLAS.get_or_init(|| {
            let mut atlas = BlockTextureAtlas::new();
            let mut files = Vec::new();
            collect_builtin_texture_files(&BLOCKS_DIR, &mut files);
            files.sort_unstable_by(|left, right| left.0.cmp(&right.0));

            if files.is_empty() {
                log::warn!(
                    "No block textures (PNG/BMP) were discovered in embedded assets; textured blocks will fall back to flat color."
                );
            }

            for (path, bytes) in files {
                let decoded = match image::load_from_memory(&bytes) {
                    Ok(image) => image,
                    Err(error) => {
                        log::warn!("Failed to decode block texture {path}: {error}");
                        continue;
                    }
                };

                let rgba = decoded.to_rgba8();
                let resized =
                    if rgba.width() != BLOCK_TEXTURE_EDGE || rgba.height() != BLOCK_TEXTURE_EDGE {
                        image::imageops::resize(
                            &rgba,
                            BLOCK_TEXTURE_EDGE,
                            BLOCK_TEXTURE_EDGE,
                            FilterType::Nearest,
                        )
                    } else {
                        rgba
                    };

                atlas.add_layer(&path, resized.into_raw());
            }

            log::info!(
                "Loaded {} block texture layer(s) from embedded assets.",
                atlas.layers.len().saturating_sub(1)
            );

            atlas
        })
    }
}

#[derive(Clone, Copy)]
pub struct BlockTextureLayers {
    pub top: u32,
    pub side: u32,
    pub bottom: u32,
}

impl BlockTextureLayers {
    pub fn get(block_id: &str) -> Self {
        let block = BlockDefinition::get(block_id);
        let atlas = BlockTextureAtlas::get();

        let resolve_layer = |key: Option<&str>, face: &str| -> u32 {
            match key {
                Some(value) => match atlas.resolve_layer(value) {
                    Some(layer) => layer,
                    None => {
                        log::warn!(
                            "Missing {face} texture for block {}: {}. Falling back to default.",
                            block.id,
                            value
                        );
                        atlas.default_layer()
                    }
                },
                None => atlas.default_layer(),
            }
        };

        let top_key = block
            .assets
            .texture_top
            .as_deref()
            .or(block.assets.texture.as_deref());
        let side_key = block
            .assets
            .texture_side
            .as_deref()
            .or(block.assets.texture.as_deref());
        let bottom_key = block
            .assets
            .texture_bottom
            .as_deref()
            .or(block.assets.texture_side.as_deref())
            .or(block.assets.texture.as_deref());

        Self {
            top: resolve_layer(top_key, "top"),
            side: resolve_layer(side_key, "side"),
            bottom: resolve_layer(bottom_key, "bottom"),
        }
    }
}

impl BlockDefinition {
    /// Returns a slice of all block definitions available for placement in the editor.
    pub fn all_placeable() -> &'static [Self] {
        block_definitions()
    }

    /// Resolves a block ID to its definition, returning the default block if not found.
    pub fn get(block_id: &str) -> &'static Self {
        block_definition_for_id(&BlockId::new(block_id))
    }

    /// Resolves an already-normalized [`BlockId`] without allocating a new ID.
    ///
    /// Hot paths that already hold a `BlockId` (e.g. per-object render
    /// extraction over ~19k objects) must use this instead of [`Self::get`]
    /// to avoid one `String` allocation + normalization per object.
    pub fn get_by_id(block_id: &BlockId) -> &'static Self {
        block_definition_for_id(block_id)
    }
}

fn block_definitions() -> &'static [BlockDefinition] {
    BLOCK_DEFINITIONS.get_or_init(|| {
        let mut definitions = Vec::new();
        collect_builtin_blocks(&BLOCKS_DIR, &mut definitions);

        if definitions.is_empty() {
            definitions.push(BlockDefinition::default());
        }

        if !definitions
            .iter()
            .any(|definition| definition.id == BlockId::default())
        {
            definitions.push(BlockDefinition::default());
        }

        definitions.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));
        definitions
    })
}

fn block_definition_index() -> &'static HashMap<BlockId, usize> {
    BLOCK_DEFINITION_INDEX.get_or_init(|| {
        block_definitions()
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.id.clone(), index))
            .collect()
    })
}

fn block_definition_for_id(block_id: &BlockId) -> &'static BlockDefinition {
    block_definition_index()
        .get(block_id)
        .or_else(|| block_definition_index().get(&BlockId::default()))
        .and_then(|index| block_definitions().get(*index))
        .expect("block definitions must contain fallback block")
}

fn collect_builtin_blocks(dir: &Dir<'_>, definitions: &mut Vec<BlockDefinition>) {
    for file in dir.files() {
        let is_json = file
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("json"))
            .unwrap_or(false);

        if !is_json {
            continue;
        }

        let Some(contents) = file.contents_utf8() else {
            continue;
        };

        match serde_json::from_str::<BlockDefinition>(contents) {
            Ok(definition) => {
                definitions.push(definition);
            }
            Err(error) => {
                log::warn!(
                    "Failed to parse block definition {:?}: {error}",
                    file.path()
                );
            }
        }
    }

    for child in dir.dirs() {
        collect_builtin_blocks(child, definitions);
    }
}

fn normalize_texture_key(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn collect_builtin_texture_files(dir: &Dir<'_>, files: &mut Vec<(String, Vec<u8>)>) {
    for file in dir.files() {
        let is_texture = file
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| {
                extension.eq_ignore_ascii_case("png") || extension.eq_ignore_ascii_case("bmp")
            })
            .unwrap_or(false);

        if !is_texture {
            continue;
        }

        let normalized_path = normalize_texture_key(file.path().to_string_lossy().as_ref());
        files.push((normalized_path, file.contents().to_vec()));
    }

    for child in dir.dirs() {
        collect_builtin_texture_files(child, files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_texture_assets_resolve_to_non_default_layers() {
        let atlas = BlockTextureAtlas::get();
        let default_layer = atlas.default_layer();

        for block in BlockDefinition::all_placeable() {
            let layers = BlockTextureLayers::get(&block.id);

            let top_configured =
                block.assets.texture_top.is_some() || block.assets.texture.is_some();
            if top_configured {
                assert_ne!(
                    layers.top, default_layer,
                    "Top texture for block '{}' resolved to the default layer.",
                    block.id
                );
            }

            let side_configured =
                block.assets.texture_side.is_some() || block.assets.texture.is_some();
            if side_configured {
                assert_ne!(
                    layers.side, default_layer,
                    "Side texture for block '{}' resolved to the default layer.",
                    block.id
                );
            }

            let bottom_configured =
                block.assets.texture_bottom.is_some() || block.assets.texture.is_some();
            if bottom_configured {
                assert_ne!(
                    layers.bottom, default_layer,
                    "Bottom texture for block '{}' resolved to the default layer.",
                    block.id
                );
            }
        }
    }

    #[test]
    fn block_id_resolution_prefers_definition_lookup_and_falls_back_to_default() {
        let placeable = BlockDefinition::all_placeable();
        assert!(!placeable.is_empty());

        let unknown_definition = BlockDefinition::get("does/not/exist");
        assert_eq!(unknown_definition.id, "core/stone");
    }

    #[test]
    fn texture_atlas_resolve_layer_handles_aliases_and_empty_keys() {
        let atlas = BlockTextureAtlas::get();
        let default_layer = atlas.default_layer();

        let grass_top = atlas
            .resolve_layer("grass_top.png")
            .expect("known texture path");
        assert_ne!(grass_top, default_layer);

        let by_stem = atlas.resolve_layer("grass_top");
        assert_eq!(by_stem, Some(grass_top));

        let by_filename = atlas.resolve_layer("grass_top.png");
        assert_eq!(by_filename, Some(grass_top));

        assert_eq!(atlas.resolve_layer(""), None);
        assert_eq!(atlas.resolve_layer("   "), None);
    }

    #[test]
    fn torch_block_definition_is_passthrough_and_liquid() {
        let torch = BlockDefinition::get("core/torch");
        assert_eq!(torch.id, "core/torch");
        assert!(matches!(
            torch.behavior.collision,
            BlockCollision::PassThrough
        ));
        assert!(!torch.behavior.support_surface);
        assert!(matches!(torch.render.profile, BlockRenderProfile::Liquid));
    }

    #[test]
    fn practice_checkpoint_flag_is_passthrough_cuboid_block() {
        let flag = BlockDefinition::get("core/practice_checkpoint_flag");
        assert_eq!(flag.id, "core/practice_checkpoint_flag");
        assert!(matches!(
            flag.behavior.collision,
            BlockCollision::PassThrough
        ));
        assert!(!flag.behavior.support_surface);
        assert_eq!(flag.geometry.elements.len(), 6);
        assert!(flag.placeable);
    }

    #[test]
    fn block_default_size_is_loaded() {
        let speed_portal = BlockDefinition::get("core/speedportal");
        assert_eq!(speed_portal.render.default_size, [2.0, 1.0, 1.0]);
    }

    #[test]
    fn block_geometry_cuboids_are_loaded_and_normalized() {
        let fence = BlockDefinition::get("core/wooden_fence");
        assert_eq!(fence.geometry.elements.len(), 3);
        assert_eq!(fence.geometry.hitboxes.len(), 0);
        assert_eq!(fence.geometry.elements[0].from, [6.0, 0.0, 6.0]);
        assert_eq!(fence.geometry.elements[0].to, [10.0, 16.0, 10.0]);
    }

    #[test]
    fn block_geometry_cuboid_color_tint_deserializes() {
        let cuboid: BlockCuboid = serde_json::from_str(
            r#"{
                "from": [0.0, 0.0, 0.0],
                "to": [16.0, 16.0, 16.0],
                "color_tint": [0.25, 0.5, 0.75]
            }"#,
        )
        .expect("cuboid json");

        assert_eq!(cuboid.color_tint, Some([0.25, 0.5, 0.75]));
    }

    #[test]
    fn gem_block_uses_json_geometry_and_inline_animation() {
        let gem = BlockDefinition::get("core/gem");

        assert_eq!(gem.geometry.elements.len(), 5);
        assert_eq!(gem.geometry.hitboxes.len(), 1);

        let spin = gem.animation.idle.spin.expect("gem spin animation");
        approx::assert_abs_diff_eq!(spin.speed, 1.6, epsilon = 1e-6);
        let bob = gem.animation.idle.bob.expect("gem bob animation");
        approx::assert_abs_diff_eq!(bob.amplitude, 0.055, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(bob.frequency, 2.4, epsilon = 1e-6);
        let pulse = gem.animation.idle.pulse.expect("gem pulse animation");
        approx::assert_abs_diff_eq!(pulse.amplitude, 0.025, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(pulse.frequency, 3.2, epsilon = 1e-6);
        let consumed = gem.animation.on_consumed.expect("gem consumed effect");
        assert_eq!(consumed.shard_count, 14);
        approx::assert_abs_diff_eq!(consumed.duration_seconds, 0.48, epsilon = 1e-6);
    }

    #[test]
    fn block_categories_are_loaded_from_definitions() {
        let stone = BlockDefinition::get("core/stone");
        let lava = BlockDefinition::get("core/lava");
        let void = BlockDefinition::get("core/void");
        let speed_portal = BlockDefinition::get("core/speedportal");
        let torch = BlockDefinition::get("core/torch");

        assert_eq!(stone.category, BlockCategory::Building);
        assert_eq!(lava.category, BlockCategory::Danger);
        assert_eq!(void.category, BlockCategory::Danger);
        assert_eq!(speed_portal.category, BlockCategory::Tech);
        assert_eq!(torch.category, BlockCategory::Tech);
    }
}
