/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use is_default::IsDefault;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::BlockId;

/// JSON-backed definition for a placeable or gameplay block.
#[derive(Clone, Deserialize, SmartDefault, IsDefault)]
#[serde(default)]
pub struct BlockDefinition {
    /// Canonical block identifier.
    pub id: BlockId,
    /// Human-readable editor label.
    pub display_name: String,
    /// Editor grouping for the block.
    pub category: BlockCategory,
    /// Optional visual asset references.
    pub assets: BlockAssets,
    /// Rendering metadata.
    pub render: BlockRender,
    /// Gameplay collision and interaction metadata.
    pub behavior: BlockBehavior,
    /// Optional custom geometry and hitboxes.
    pub geometry: BlockGeometry,
    /// Optional inline animation and visual effect metadata.
    pub animation: BlockAnimation,
    /// Whether the block is visible in editor placement UI.
    #[default = true]
    pub placeable: bool,
}

/// Optional icon and texture asset references for a block.
#[derive(Clone, Deserialize, Serialize, Default, IsDefault)]
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
    /// Optional custom icon asset path.
    #[serde(default)]
    pub icon: Option<String>,
}

/// Custom geometry definition for a block.
#[derive(Clone, Deserialize, Default, IsDefault)]
pub struct BlockGeometry {
    /// Visual cuboids in 0..16 block-local coordinates.
    #[serde(default)]
    pub elements: Vec<BlockCuboid>,
    /// Collision cuboids in 0..16 block-local coordinates.
    #[serde(default)]
    pub hitboxes: Vec<BlockCuboid>,
}

/// One local-space cuboid in a block definition.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Default, IsDefault)]
#[serde(default)]
pub struct BlockCuboid {
    /// Minimum-ish corner before normalization, in 0..16 coordinates.
    pub from: [f32; 3],
    /// Maximum-ish corner before normalization, in 0..16 coordinates.
    pub to: [f32; 3],
    /// Optional per-cuboid color tint.
    pub color_tint: Option<[f32; 3]>,
}

/// Inline animation and visual effect metadata for a block definition.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, IsDefault)]
pub struct BlockAnimation {
    /// Idle, shader-side motion applied while the block is visible.
    #[serde(default)]
    pub idle: BlockIdleAnimation,
    /// Effect spawned when gameplay consumes this block.
    #[serde(default)]
    pub on_consumed: Option<BlockEffect>,
    /// Effect spawned by future break/destruction actions.
    #[serde(default)]
    pub on_break: Option<BlockEffect>,
}

/// Shader-side idle animation presets for a block.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, IsDefault)]
#[serde(default)]
pub struct BlockIdleAnimation {
    /// Optional center-based spin animation.
    pub spin: Option<BlockSpinAnimation>,
    /// Optional axis-aligned bob animation.
    pub bob: Option<BlockBobAnimation>,
    /// Optional center-based pulse/scale animation.
    pub pulse: Option<BlockPulseAnimation>,
    /// How per-object animation phase is seeded.
    pub phase_seed: BlockAnimationPhaseSeed,
}

impl BlockIdleAnimation {
    pub fn has_motion(&self) -> bool {
        self.spin
            .as_ref()
            .is_some_and(BlockSpinAnimation::is_active)
            || self.bob.as_ref().is_some_and(BlockBobAnimation::is_active)
            || self
                .pulse
                .as_ref()
                .is_some_and(BlockPulseAnimation::is_active)
    }
}

/// Axis used by shader-side block animations.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockAnimationAxis {
    /// X-axis motion or rotation.
    X,
    /// Y-axis motion or rotation.
    #[default]
    Y,
    /// Z-axis motion or rotation.
    Z,
}

/// How repeated blocks receive idle animation phase offsets.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockAnimationPhaseSeed {
    /// Every instance starts with the same phase.
    None,
    /// Seed phase from world position so repeated blocks do not move identically.
    #[default]
    Position,
}

/// Center-based spin animation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Default, IsDefault)]
#[serde(default)]
pub struct BlockSpinAnimation {
    /// Rotation axis.
    pub axis: BlockAnimationAxis,
    /// Angular speed in radians per second.
    pub speed: f32,
}

impl BlockSpinAnimation {
    pub fn is_active(&self) -> bool {
        self.speed.abs() > f32::EPSILON
    }
}

/// Axis-aligned sinusoidal bob animation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, SmartDefault, IsDefault)]
#[serde(default)]
pub struct BlockBobAnimation {
    /// Translation axis.
    pub axis: BlockAnimationAxis,
    /// Translation amplitude in world units.
    pub amplitude: f32,
    /// Frequency multiplier in radians per second.
    #[default = 1.0]
    pub frequency: f32,
}

impl BlockBobAnimation {
    pub fn is_active(&self) -> bool {
        self.amplitude > f32::EPSILON && self.frequency > f32::EPSILON
    }
}

/// Center-based sinusoidal scale animation.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, SmartDefault, IsDefault)]
pub struct BlockPulseAnimation {
    /// Scale amplitude as a fraction of local size.
    pub amplitude: f32,
    /// Frequency multiplier in radians per second.
    #[default = 1.0]
    pub frequency: f32,
}

impl BlockPulseAnimation {
    pub fn is_active(&self) -> bool {
        self.amplitude > f32::EPSILON && self.frequency > f32::EPSILON
    }
}

/// Runtime visual effect preset for block events.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockEffectPreset {
    /// Triangular shards burst outward and fade.
    #[default]
    Shatter,
}

/// Color source for a block visual effect.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockEffectColorMode {
    /// Derive effect color from the consumed object's `color_tint`.
    #[default]
    Tint,
    /// Derive effect color from the block render colors.
    Render,
}

/// Visual effect metadata for consumed or broken blocks.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, SmartDefault, IsDefault)]
#[serde(default)]
pub struct BlockEffect {
    /// Effect generator preset.
    pub preset: BlockEffectPreset,
    /// Effect lifetime in seconds.
    #[default = 0.48]
    pub duration_seconds: f32,
    /// Number of generated shards for shatter-style effects.
    #[default = 14]
    pub shard_count: u32,
    /// Outward burst speed scale.
    #[default = 1.0]
    pub speed_scale: f32,
    /// Gravity scale applied to shards.
    #[default = 1.0]
    pub gravity_scale: f32,
    /// Tangential wobble scale applied to shards.
    #[default = 1.0]
    pub wobble_scale: f32,
    /// Color source for generated shards.
    pub color_mode: BlockEffectColorMode,
}

/// Editor category for a block definition.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockCategory {
    /// Structural or decorative blocks.
    #[default]
    Building,
    /// Hazard blocks.
    Danger,
    /// Mechanical or technical blocks.
    Tech,
    /// Trigger marker blocks.
    Trigger,
}

/// Render profile used by mesh generation and editor previews.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockRenderProfile {
    /// Standard solid block shading.
    #[default]
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

/// Rendering metadata for a block definition.
#[derive(Clone, Deserialize, SmartDefault, IsDefault)]
#[serde(default)]
pub struct BlockRender {
    /// Mesh/rendering profile.
    pub profile: BlockRenderProfile,
    /// Default editor placement size.
    #[default([1.0, 1.0, 1.0])]
    pub default_size: [f32; 3],
    /// Top face color.
    #[default([1.0, 1.0, 1.0, 1.0])]
    pub color_top: [f32; 4],
    /// Side face color.
    #[default([1.0, 1.0, 1.0, 1.0])]
    pub color_side: [f32; 4],
    /// Bottom face color.
    #[default([1.0, 1.0, 1.0, 1.0])]
    pub color_bottom: [f32; 4],
    /// Fill color for profile-specific meshes.
    #[default([0.0, 0.0, 0.0, 1.0])]
    pub color_fill: [f32; 4],
    /// Outline color for profile-specific meshes.
    pub color_outline: [f32; 4],
    /// Procedural noise amount used by some block meshes.
    pub noise: f32,
}

/// Collision category for gameplay simulation.
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum BlockCollision {
    /// Normal solid obstacle/support collision.
    #[default]
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
#[derive(Clone, Deserialize, Serialize, SmartDefault, IsDefault)]
pub struct BlockBehavior {
    /// Collision behavior.
    #[serde(default)]
    pub collision: BlockCollision,

    /// Movement speed multiplier while interacting with the block.
    #[default = 1.0]
    #[serde(default)]
    pub speed_multiplier: f32,

    /// Whether this block can support player movement.
    #[default = true]
    #[serde(default)]
    pub support_surface: bool,

    /// Whether the block is consumed on overlap.
    #[serde(default)]
    pub consumed_on_overlap: bool,

    /// Gem value awarded by collectible blocks.
    #[serde(default)]
    pub gem_value: u32,
}
