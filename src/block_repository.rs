use std::collections::HashMap;
use std::sync::OnceLock;

use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_BLOCK_ID: &str = "core/standard";

static BLOCKS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/blocks");
static BLOCK_CATALOG: OnceLock<BlockCatalog> = OnceLock::new();

fn default_display_name() -> String {
    "Standard".to_string()
}

fn default_render_profile() -> BlockRenderProfile {
    BlockRenderProfile::Solid
}

fn default_color_top() -> [f32; 4] {
    [0.4, 0.4, 0.45, 1.0]
}

fn default_color_side() -> [f32; 4] {
    [0.2, 0.2, 0.25, 1.0]
}

fn default_color_fill() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_color_outline() -> [f32; 4] {
    [0.8, 0.8, 0.9, 1.0]
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
    pub(crate) mesh: Option<String>,
    #[serde(default)]
    pub(crate) icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockRenderProfile {
    Solid,
    VoidFrame,
    SpeedPortal,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct BlockRender {
    #[serde(default = "default_render_profile")]
    pub(crate) profile: BlockRenderProfile,
    #[serde(default = "default_color_top")]
    pub(crate) color_top: [f32; 4],
    #[serde(default = "default_color_side")]
    pub(crate) color_side: [f32; 4],
    #[serde(default = "default_color_fill")]
    pub(crate) color_fill: [f32; 4],
    #[serde(default = "default_color_outline")]
    pub(crate) color_outline: [f32; 4],
}

impl Default for BlockRender {
    fn default() -> Self {
        Self {
            profile: default_render_profile(),
            color_top: default_color_top(),
            color_side: default_color_side(),
            color_fill: default_color_fill(),
            color_outline: default_color_outline(),
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

pub(crate) fn all_placeable_blocks() -> &'static [BlockDefinition] {
    &block_catalog().definitions
}

pub(crate) fn resolve_block_definition(block_id: &str) -> &'static BlockDefinition {
    block_catalog().block_for_id(block_id)
}

pub(crate) fn normalize_block_id(block_id: &str) -> String {
    block_catalog().resolve_id(block_id).to_string()
}
