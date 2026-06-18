/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

use crate::block_id::{canonical_block_id_text, DEFAULT_BLOCK_ID};

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

fn default_gem_value() -> u32 {
    0
}

fn default_placeable() -> bool {
    true
}

fn default_noise() -> f32 {
    0.0
}

fn default_icon_dimetric_projection() -> bool {
    true
}

fn default_block_size() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn sanitize_block_size(size: [f32; 3]) -> [f32; 3] {
    size.map(|component| {
        if component.is_finite() {
            component.max(0.01)
        } else {
            1.0
        }
    })
}

fn sanitize_cuboid_bounds(from: [f32; 3], to: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [0.0; 3];
    let mut max = [0.0; 3];
    for axis in 0..3 {
        let a = if from[axis].is_finite() {
            from[axis]
        } else {
            0.0
        }
        .clamp(0.0, 16.0);
        let b = if to[axis].is_finite() { to[axis] } else { 0.0 }.clamp(0.0, 16.0);
        min[axis] = a.min(b);
        max[axis] = a.max(b);
        if (max[axis] - min[axis]).abs() <= f32::EPSILON {
            return None;
        }
    }
    Some((min, max))
}

fn sanitize_color_tint(color_tint: [f32; 3]) -> [f32; 3] {
    color_tint.map(|component| {
        if component.is_finite() {
            component.clamp(0.0, 1.0)
        } else {
            1.0
        }
    })
}

/// JSON-backed definition for a placeable or gameplay block.
#[derive(Clone, Deserialize, Serialize)]
pub struct BlockDefinition {
    /// Canonical block identifier.
    pub id: String,
    /// Human-readable editor label.
    #[serde(default = "default_display_name")]
    pub display_name: String,
    /// Editor grouping for the block.
    pub category: BlockCategory,
    /// Optional visual asset references.
    #[serde(default)]
    pub assets: BlockAssets,
    /// Rendering metadata.
    #[serde(default)]
    pub render: BlockRender,
    /// Gameplay collision and interaction metadata.
    #[serde(default)]
    pub behavior: BlockBehavior,
    /// Optional custom geometry and hitboxes.
    #[serde(default)]
    pub geometry: BlockGeometry,
    /// Whether the block is visible in editor placement UI.
    #[serde(default = "default_placeable")]
    pub placeable: bool,
}

impl BlockDefinition {
    /// Normalizes IDs, display names, dimensions, and custom cuboid bounds.
    pub fn normalize(mut self) -> Option<Self> {
        self.id = canonical_block_id_text(&self.id);
        if self.id.is_empty() {
            return None;
        }
        self.display_name = self.display_name.trim().to_string();
        if self.display_name.is_empty() {
            self.display_name = self.id.clone();
        }
        self.render.default_size = sanitize_block_size(self.render.default_size);
        self.geometry.elements = self
            .geometry
            .elements
            .into_iter()
            .filter_map(BlockCuboid::normalized)
            .collect();
        self.geometry.hitboxes = self
            .geometry
            .hitboxes
            .into_iter()
            .filter_map(BlockCuboid::normalized)
            .collect();
        Some(self)
    }

    /// Returns the default placement size for this block.
    pub fn default_size(&self) -> [f32; 3] {
        self.render.default_size
    }
}

/// Optional mesh, icon, and texture asset references for a block.
#[derive(Clone, Deserialize, Serialize, Default)]
pub struct BlockAssets {
    /// Shared texture for all faces.
    #[serde(default)]
    pub texture: Option<String>,
    /// Top-face texture override.
    #[serde(default)]
    pub texture_top: Option<String>,
    /// Side-face texture override.
    #[serde(default)]
    pub texture_side: Option<String>,
    /// Bottom-face texture override.
    #[serde(default)]
    pub texture_bottom: Option<String>,
    /// Optional custom mesh asset path.
    #[serde(default)]
    pub mesh: Option<String>,
    /// Optional custom icon asset path.
    #[serde(default)]
    pub icon: Option<String>,
}

/// Custom geometry definition for a block.
#[derive(Clone, Deserialize, Serialize, Default)]
pub struct BlockGeometry {
    /// Visual cuboids in 0..16 block-local coordinates.
    #[serde(default)]
    pub elements: Vec<BlockCuboid>,
    /// Collision cuboids in 0..16 block-local coordinates.
    #[serde(default)]
    pub hitboxes: Vec<BlockCuboid>,
}

/// One local-space cuboid in a block definition.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockCuboid {
    /// Minimum-ish corner before normalization, in 0..16 coordinates.
    pub from: [f32; 3],
    /// Maximum-ish corner before normalization, in 0..16 coordinates.
    pub to: [f32; 3],
    /// Optional per-cuboid color tint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_tint: Option<[f32; 3]>,
}

impl BlockCuboid {
    fn normalized(self) -> Option<Self> {
        let (from, to) = sanitize_cuboid_bounds(self.from, self.to)?;
        Some(Self {
            from,
            to,
            color_tint: self.color_tint.map(sanitize_color_tint),
        })
    }
}

/// Editor category for a block definition.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockCategory {
    /// Structural or decorative blocks.
    Building,
    /// Hazard blocks.
    Danger,
    /// Mechanical or technical blocks.
    Tech,
    /// Trigger marker blocks.
    Trigger,
}

/// Render profile used by mesh generation and editor previews.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockRenderProfile {
    /// Standard solid block shading.
    Solid,
    /// Fluid-like animated block shading.
    Liquid,
    /// Speed portal rendering.
    SpeedPortal,
    /// Emissive neon rendering.
    Neon,
    /// Collectible gem rendering.
    Gem,
    /// Transform trigger marker rendering.
    TransformTrigger,
    /// Camera trigger marker rendering.
    CameraTrigger,
}

/// Camera override used when rendering block icons.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct BlockIconCamera {
    /// Optional camera position.
    #[serde(default)]
    pub position: Option<[f32; 3]>,
    /// Optional camera rotation in degrees.
    #[serde(default, alias = "rotation_degrees")]
    pub rotation: Option<[f32; 3]>,
    /// Optional orthographic half extent.
    #[serde(default)]
    pub orthographic_half_extent: Option<f32>,
}

/// Rendering metadata for a block definition.
#[derive(Clone, Deserialize, Serialize)]
pub struct BlockRender {
    /// Mesh/rendering profile.
    #[serde(default = "default_render_profile")]
    pub profile: BlockRenderProfile,
    /// Whether generated icons use the default dimetric camera.
    #[serde(default = "default_icon_dimetric_projection")]
    pub icon_dimetric_projection: bool,
    /// Optional icon camera override.
    #[serde(default)]
    pub icon_camera: BlockIconCamera,
    /// Default editor placement size.
    #[serde(default = "default_block_size")]
    pub default_size: [f32; 3],
    /// Top face color.
    #[serde(default = "default_color_top")]
    pub color_top: [f32; 4],
    /// Side face color.
    #[serde(default = "default_color_side")]
    pub color_side: [f32; 4],
    /// Bottom face color.
    #[serde(default = "default_color_bottom")]
    pub color_bottom: [f32; 4],
    /// Fill color for profile-specific meshes.
    #[serde(default = "default_color_fill")]
    pub color_fill: [f32; 4],
    /// Outline color for profile-specific meshes.
    #[serde(default = "default_color_outline")]
    pub color_outline: [f32; 4],
    /// Procedural noise amount used by some block meshes.
    #[serde(default = "default_noise")]
    pub noise: f32,
}

impl Default for BlockRender {
    fn default() -> Self {
        Self {
            profile: default_render_profile(),
            icon_dimetric_projection: default_icon_dimetric_projection(),
            icon_camera: BlockIconCamera::default(),
            default_size: default_block_size(),
            color_top: default_color_top(),
            color_side: default_color_side(),
            color_bottom: default_color_bottom(),
            color_fill: default_color_fill(),
            color_outline: default_color_outline(),
            noise: default_noise(),
        }
    }
}

/// Collision category for gameplay simulation.
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockCollision {
    /// Normal solid obstacle/support collision.
    Solid,
    /// No blocking collision.
    PassThrough,
    /// Hazard collision.
    Hazard,
    /// Portal collision.
    Portal,
    /// Collectible collision.
    Collectible,
}

/// Gameplay behavior metadata for a block definition.
#[derive(Clone, Deserialize, Serialize)]
pub struct BlockBehavior {
    /// Collision behavior.
    #[serde(default = "default_collision")]
    pub collision: BlockCollision,
    /// Movement speed multiplier while interacting with the block.
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f32,
    /// Whether this block can support player movement.
    #[serde(default = "default_support_surface")]
    pub support_surface: bool,
    /// Whether the block is consumed on overlap.
    #[serde(default = "default_consumed_on_overlap")]
    pub consumed_on_overlap: bool,
    /// Gem value awarded by collectible blocks.
    #[serde(default = "default_gem_value")]
    pub gem_value: u32,
}

impl Default for BlockBehavior {
    fn default() -> Self {
        Self {
            collision: default_collision(),
            speed_multiplier: default_speed_multiplier(),
            support_surface: default_support_surface(),
            consumed_on_overlap: default_consumed_on_overlap(),
            gem_value: default_gem_value(),
        }
    }
}

/// Returns the built-in fallback block definition used by runtime catalogs.
pub fn fallback_block_definition() -> BlockDefinition {
    BlockDefinition {
        id: DEFAULT_BLOCK_ID.to_string(),
        display_name: "Standard".to_string(),
        category: BlockCategory::Building,
        assets: BlockAssets::default(),
        render: BlockRender::default(),
        behavior: BlockBehavior::default(),
        geometry: BlockGeometry::default(),
        placeable: true,
    }
}
