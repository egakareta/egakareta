/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Block repository for managing block definitions and behaviors.
//!
//! Provides a catalog of block types loaded from JSON files in assets/blocks/.
//! Each block defines its visual appearance, collision behavior, and gameplay effects.

use std::collections::HashMap;
use std::sync::OnceLock;

use image::imageops::FilterType;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_BLOCK_ID: &str = "core/stone";

static BLOCKS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/blocks");
static BLOCK_CATALOG: OnceLock<BlockCatalog> = OnceLock::new();
static BLOCK_TEXTURE_ATLAS: OnceLock<BlockTextureAtlas> = OnceLock::new();

const BLOCK_TEXTURE_EDGE: u32 = 64;
const DEFAULT_BLOCK_TEXTURE_KEY: &str = "__default_white__";

fn default_display_name() -> String {
    "Standard".to_string()
}

fn default_render_profile() -> BlockRenderProfile {
    BlockRenderProfile::Solid
}

fn default_color_top() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_color_side() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_color_bottom() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_color_fill() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_color_outline() -> [f32; 4] {
    [0.0, 0.0, 0.0, 0.0]
}

fn default_collision() -> BlockCollision {
    BlockCollision::Solid
}

fn default_speed_multiplier() -> f32 {
    1.0
}

fn default_support_surface() -> bool {
    true
}

fn default_consumed_on_overlap() -> bool {
    false
}

fn default_placeable() -> bool {
    true
}

fn default_noise() -> f32 {
    0.0
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct BlockDefinition {
    pub(crate) id: String,
    #[serde(default = "default_display_name")]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) assets: BlockAssets,
    #[serde(default)]
    pub(crate) render: BlockRender,
    #[serde(default)]
    pub(crate) behavior: BlockBehavior,
    #[serde(default = "default_placeable")]
    pub(crate) placeable: bool,
}

impl BlockDefinition {
    /// Normalizes the block definition by trimming and lowercasing the ID,
    /// and ensuring the display name is not empty.
    /// Returns None if the ID is empty after trimming.
    pub(crate) fn normalize(mut self) -> Option<Self> {
        self.id = self.id.trim().to_ascii_lowercase();
        if self.id.is_empty() {
            return None;
        }
        self.display_name = self.display_name.trim().to_string();
        if self.display_name.is_empty() {
            self.display_name = self.id.clone();
        }
        Some(self)
    }
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub(crate) struct BlockAssets {
    #[serde(default)]
    pub(crate) texture: Option<String>,
    #[serde(default)]
    pub(crate) texture_top: Option<String>,
    #[serde(default)]
    pub(crate) texture_side: Option<String>,
    #[serde(default)]
    pub(crate) texture_bottom: Option<String>,
    #[serde(default)]
    pub(crate) mesh: Option<String>,
    #[serde(default)]
    pub(crate) icon: Option<String>,
}

#[derive(Clone)]
pub(crate) struct BlockTextureLayer {
    pub(crate) rgba: Vec<u8>,
}

pub(crate) struct BlockTextureAtlas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layers: Vec<BlockTextureLayer>,
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

    pub(crate) fn resolve_layer(&self, key: &str) -> Option<u32> {
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

    pub(crate) fn default_layer(&self) -> u32 {
        self.default_layer
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BlockTextureLayers {
    pub(crate) top: u32,
    pub(crate) side: u32,
    pub(crate) bottom: u32,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockRenderProfile {
    Solid,
    Liquid,
    SpeedPortal,
    FinishRing,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct BlockRender {
    #[serde(default = "default_render_profile")]
    pub(crate) profile: BlockRenderProfile,
    #[serde(default = "default_color_top")]
    pub(crate) color_top: [f32; 4],
    #[serde(default = "default_color_side")]
    pub(crate) color_side: [f32; 4],
    #[serde(default = "default_color_bottom")]
    pub(crate) color_bottom: [f32; 4],
    #[serde(default = "default_color_fill")]
    pub(crate) color_fill: [f32; 4],
    #[serde(default = "default_color_outline")]
    pub(crate) color_outline: [f32; 4],
    #[serde(default = "default_noise")]
    pub(crate) noise: f32,
}

impl Default for BlockRender {
    fn default() -> Self {
        Self {
            profile: default_render_profile(),
            color_top: default_color_top(),
            color_side: default_color_side(),
            color_bottom: default_color_bottom(),
            color_fill: default_color_fill(),
            color_outline: default_color_outline(),
            noise: default_noise(),
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockCollision {
    Solid,
    PassThrough,
    Hazard,
    Portal,
    Finish,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct BlockBehavior {
    #[serde(default = "default_collision")]
    pub(crate) collision: BlockCollision,
    #[serde(default = "default_speed_multiplier")]
    pub(crate) speed_multiplier: f32,
    #[serde(default = "default_support_surface")]
    pub(crate) support_surface: bool,
    #[serde(default = "default_consumed_on_overlap")]
    pub(crate) consumed_on_overlap: bool,
}

impl Default for BlockBehavior {
    fn default() -> Self {
        Self {
            collision: default_collision(),
            speed_multiplier: default_speed_multiplier(),
            support_surface: default_support_surface(),
            consumed_on_overlap: default_consumed_on_overlap(),
        }
    }
}

pub(crate) struct BlockCatalog {
    definitions: Vec<BlockDefinition>,
    by_id: HashMap<String, usize>,
    fallback_id: String,
}

impl BlockCatalog {
    fn from_definitions(mut definitions: Vec<BlockDefinition>) -> Self {
        if definitions.is_empty() {
            definitions.push(Self::fallback_definition());
        }

        let mut by_id = HashMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            by_id.insert(definition.id.clone(), index);
        }

        if !by_id.contains_key(DEFAULT_BLOCK_ID) {
            let fallback = Self::fallback_definition();
            let index = definitions.len();
            by_id.insert(fallback.id.clone(), index);
            definitions.push(fallback);
        }

        Self {
            definitions,
            by_id,
            fallback_id: DEFAULT_BLOCK_ID.to_string(),
        }
    }

    fn fallback_definition() -> BlockDefinition {
        BlockDefinition {
            id: DEFAULT_BLOCK_ID.to_string(),
            display_name: "Standard".to_string(),
            assets: BlockAssets::default(),
            render: BlockRender::default(),
            behavior: BlockBehavior::default(),
            placeable: true,
        }
    }

    fn resolve_id<'a>(&'a self, block_id: &'a str) -> &'a str {
        let normalized = block_id.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return self.fallback_id.as_str();
        }
        self.by_id
            .get(&normalized)
            .map(|&index| self.definitions[index].id.as_str())
            .unwrap_or(self.fallback_id.as_str())
    }

    fn block_for_id(&self, block_id: &str) -> &BlockDefinition {
        let resolved = self.resolve_id(block_id);
        self.by_id
            .get(resolved)
            .and_then(|index| self.definitions.get(*index))
            .or_else(|| {
                self.by_id
                    .get(&self.fallback_id)
                    .and_then(|index| self.definitions.get(*index))
            })
            .expect("block catalog must contain fallback block")
    }
}

fn block_catalog() -> &'static BlockCatalog {
    BLOCK_CATALOG.get_or_init(|| {
        let mut definitions = Vec::new();
        collect_builtin_blocks(&BLOCKS_DIR, &mut definitions);
        definitions.sort_unstable_by(|left, right| left.display_name.cmp(&right.display_name));
        BlockCatalog::from_definitions(definitions)
    })
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
                if let Some(normalized) = definition.normalize() {
                    definitions.push(normalized);
                }
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

pub(crate) fn block_texture_atlas() -> &'static BlockTextureAtlas {
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

pub(crate) fn resolve_block_texture_layers(block_id: &str) -> BlockTextureLayers {
    let block = resolve_block_definition(block_id);
    let atlas = block_texture_atlas();

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

    BlockTextureLayers {
        top: resolve_layer(top_key, "top"),
        side: resolve_layer(side_key, "side"),
        bottom: resolve_layer(bottom_key, "bottom"),
    }
}

/// Returns a slice of all block definitions available for placement in the editor.
pub(crate) fn all_placeable_blocks() -> &'static [BlockDefinition] {
    &block_catalog().definitions
}

/// Resolves a block ID to its definition, returning the default block if not found.
pub(crate) fn resolve_block_definition(block_id: &str) -> &'static BlockDefinition {
    block_catalog().block_for_id(block_id)
}

/// Normalizes a block ID to its canonical form (e.g., adding "core/" prefix if needed).
pub(crate) fn normalize_block_id(block_id: &str) -> String {
    block_catalog().resolve_id(block_id).to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        all_placeable_blocks, block_texture_atlas, normalize_block_id, resolve_block_definition,
        resolve_block_texture_layers, BlockAssets, BlockBehavior, BlockDefinition, BlockRender,
    };

    #[test]
    fn configured_texture_assets_resolve_to_non_default_layers() {
        let atlas = block_texture_atlas();
        let default_layer = atlas.default_layer();

        for block in all_placeable_blocks() {
            let layers = resolve_block_texture_layers(&block.id);

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
    fn normalize_block_definition_trims_and_falls_back_display_name() {
        let normalized = BlockDefinition {
            id: "  CORE/STONE  ".to_string(),
            display_name: "   ".to_string(),
            assets: BlockAssets::default(),
            render: BlockRender::default(),
            behavior: BlockBehavior::default(),
            placeable: true,
        }
        .normalize()
        .expect("normalize");
        assert_eq!(normalized.id, "core/stone");
        assert_eq!(normalized.display_name, "core/stone");

        let missing_id = BlockDefinition {
            id: "   ".to_string(),
            display_name: "name".to_string(),
            assets: BlockAssets::default(),
            render: BlockRender::default(),
            behavior: BlockBehavior::default(),
            placeable: true,
        }
        .normalize();
        assert!(missing_id.is_none());
    }

    #[test]
    fn block_id_resolution_prefers_catalog_and_falls_back_to_default() {
        let placeable = all_placeable_blocks();
        assert!(!placeable.is_empty());

        let resolved_known = normalize_block_id("CORE/STONE");
        assert_eq!(resolved_known, "core/stone");

        let resolved_unknown = normalize_block_id("does/not/exist");
        assert_eq!(resolved_unknown, "core/stone");

        let unknown_definition = resolve_block_definition("does/not/exist");
        assert_eq!(unknown_definition.id, "core/stone");
    }

    #[test]
    fn texture_atlas_resolve_layer_handles_aliases_and_empty_keys() {
        let atlas = block_texture_atlas();
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
}
