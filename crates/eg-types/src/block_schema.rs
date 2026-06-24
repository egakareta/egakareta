/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

use crate::BlockId;

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

fn default_spin_speed() -> f32 {
    0.0
}

fn default_bob_amplitude() -> f32 {
    0.0
}

fn default_bob_frequency() -> f32 {
    1.0
}

fn default_pulse_amplitude() -> f32 {
    0.0
}

fn default_pulse_frequency() -> f32 {
    1.0
}

fn default_effect_duration_seconds() -> f32 {
    0.48
}

fn default_effect_shard_count() -> u32 {
    14
}

fn default_effect_speed_scale() -> f32 {
    1.0
}

fn default_effect_gravity_scale() -> f32 {
    1.0
}

fn default_effect_wobble_scale() -> f32 {
    1.0
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

fn sanitize_finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn sanitize_nonnegative(value: f32, fallback: f32) -> f32 {
    sanitize_finite(value, fallback).max(0.0)
}

/// JSON-backed definition for a placeable or gameplay block.
#[derive(Clone, Deserialize, Serialize)]
pub struct BlockDefinition {
    /// Canonical block identifier.
    pub id: BlockId,
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
    /// Optional inline animation and visual effect metadata.
    #[serde(default)]
    pub animation: BlockAnimation,
    /// Whether the block is visible in editor placement UI.
    #[serde(default = "default_placeable")]
    pub placeable: bool,
}

impl BlockDefinition {
    /// Normalizes IDs, display names, dimensions, and custom cuboid bounds.
    pub fn normalize(mut self) -> Option<Self> {
        if self.id.is_empty() {
            return None;
        }
        self.display_name = self.display_name.trim().to_string();
        if self.display_name.is_empty() {
            self.display_name = self.id.to_string();
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
        self.animation = self.animation.normalized();
        Some(self)
    }

    /// Returns the default placement size for this block.
    pub fn default_size(&self) -> [f32; 3] {
        self.render.default_size
    }
}

/// Optional icon and texture asset references for a block.
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

/// Inline animation and visual effect metadata for a block definition.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct BlockAnimation {
    /// Idle, shader-side motion applied while the block is visible.
    #[serde(default)]
    pub idle: BlockIdleAnimation,
    /// Effect spawned when gameplay consumes this block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_consumed: Option<BlockEffect>,
    /// Effect spawned by future break/destruction actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_break: Option<BlockEffect>,
}

impl BlockAnimation {
    fn normalized(mut self) -> Self {
        self.idle = self.idle.normalized();
        self.on_consumed = self.on_consumed.map(BlockEffect::normalized);
        self.on_break = self.on_break.map(BlockEffect::normalized);
        self
    }
}

/// Shader-side idle animation presets for a block.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct BlockIdleAnimation {
    /// Optional center-based spin animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spin: Option<BlockSpinAnimation>,
    /// Optional axis-aligned bob animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bob: Option<BlockBobAnimation>,
    /// Optional center-based pulse/scale animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<BlockPulseAnimation>,
    /// How per-object animation phase is seeded.
    #[serde(default)]
    pub phase_seed: BlockAnimationPhaseSeed,
}

impl BlockIdleAnimation {
    fn normalized(mut self) -> Self {
        self.spin = self.spin.map(BlockSpinAnimation::normalized);
        self.bob = self.bob.map(BlockBobAnimation::normalized);
        self.pulse = self.pulse.map(BlockPulseAnimation::normalized);
        self
    }

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
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockAnimationPhaseSeed {
    /// Every instance starts with the same phase.
    None,
    /// Seed phase from world position so repeated blocks do not move identically.
    #[default]
    Position,
}

/// Center-based spin animation.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockSpinAnimation {
    /// Rotation axis.
    #[serde(default)]
    pub axis: BlockAnimationAxis,
    /// Angular speed in radians per second.
    #[serde(default = "default_spin_speed")]
    pub speed: f32,
}

impl BlockSpinAnimation {
    fn normalized(mut self) -> Self {
        self.speed = sanitize_finite(self.speed, default_spin_speed());
        self
    }

    pub fn is_active(&self) -> bool {
        self.speed.abs() > f32::EPSILON
    }
}

impl Default for BlockSpinAnimation {
    fn default() -> Self {
        Self {
            axis: BlockAnimationAxis::Y,
            speed: default_spin_speed(),
        }
    }
}

/// Axis-aligned sinusoidal bob animation.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockBobAnimation {
    /// Translation axis.
    #[serde(default)]
    pub axis: BlockAnimationAxis,
    /// Translation amplitude in world units.
    #[serde(default = "default_bob_amplitude")]
    pub amplitude: f32,
    /// Frequency multiplier in radians per second.
    #[serde(default = "default_bob_frequency")]
    pub frequency: f32,
}

impl BlockBobAnimation {
    fn normalized(mut self) -> Self {
        self.amplitude = sanitize_nonnegative(self.amplitude, default_bob_amplitude());
        self.frequency = sanitize_nonnegative(self.frequency, default_bob_frequency());
        self
    }

    pub fn is_active(&self) -> bool {
        self.amplitude > f32::EPSILON && self.frequency > f32::EPSILON
    }
}

impl Default for BlockBobAnimation {
    fn default() -> Self {
        Self {
            axis: BlockAnimationAxis::Y,
            amplitude: default_bob_amplitude(),
            frequency: default_bob_frequency(),
        }
    }
}

/// Center-based sinusoidal scale animation.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockPulseAnimation {
    /// Scale amplitude as a fraction of local size.
    #[serde(default = "default_pulse_amplitude")]
    pub amplitude: f32,
    /// Frequency multiplier in radians per second.
    #[serde(default = "default_pulse_frequency")]
    pub frequency: f32,
}

impl BlockPulseAnimation {
    fn normalized(mut self) -> Self {
        self.amplitude = sanitize_nonnegative(self.amplitude, default_pulse_amplitude()).min(0.95);
        self.frequency = sanitize_nonnegative(self.frequency, default_pulse_frequency());
        self
    }

    pub fn is_active(&self) -> bool {
        self.amplitude > f32::EPSILON && self.frequency > f32::EPSILON
    }
}

impl Default for BlockPulseAnimation {
    fn default() -> Self {
        Self {
            amplitude: default_pulse_amplitude(),
            frequency: default_pulse_frequency(),
        }
    }
}

/// Runtime visual effect preset for block events.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockEffectPreset {
    /// Triangular shards burst outward and fade.
    #[default]
    Shatter,
}

/// Color source for a block visual effect.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockEffectColorMode {
    /// Derive effect color from the consumed object's `color_tint`.
    #[default]
    Tint,
    /// Derive effect color from the block render colors.
    Render,
}

/// Visual effect metadata for consumed or broken blocks.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockEffect {
    /// Effect generator preset.
    #[serde(default)]
    pub preset: BlockEffectPreset,
    /// Effect lifetime in seconds.
    #[serde(default = "default_effect_duration_seconds")]
    pub duration_seconds: f32,
    /// Number of generated shards for shatter-style effects.
    #[serde(default = "default_effect_shard_count")]
    pub shard_count: u32,
    /// Outward burst speed scale.
    #[serde(default = "default_effect_speed_scale")]
    pub speed_scale: f32,
    /// Gravity scale applied to shards.
    #[serde(default = "default_effect_gravity_scale")]
    pub gravity_scale: f32,
    /// Tangential wobble scale applied to shards.
    #[serde(default = "default_effect_wobble_scale")]
    pub wobble_scale: f32,
    /// Color source for generated shards.
    #[serde(default)]
    pub color_mode: BlockEffectColorMode,
}

impl BlockEffect {
    fn normalized(mut self) -> Self {
        self.duration_seconds =
            sanitize_nonnegative(self.duration_seconds, default_effect_duration_seconds())
                .max(0.01);
        self.shard_count = self.shard_count.clamp(1, 128);
        self.speed_scale = sanitize_nonnegative(self.speed_scale, default_effect_speed_scale());
        self.gravity_scale =
            sanitize_nonnegative(self.gravity_scale, default_effect_gravity_scale());
        self.wobble_scale = sanitize_nonnegative(self.wobble_scale, default_effect_wobble_scale());
        self
    }
}

impl Default for BlockEffect {
    fn default() -> Self {
        Self {
            preset: BlockEffectPreset::Shatter,
            duration_seconds: default_effect_duration_seconds(),
            shard_count: default_effect_shard_count(),
            speed_scale: default_effect_speed_scale(),
            gravity_scale: default_effect_gravity_scale(),
            wobble_scale: default_effect_wobble_scale(),
            color_mode: BlockEffectColorMode::Tint,
        }
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
        id: BlockId::default(),
        display_name: "Standard".to_string(),
        category: BlockCategory::Building,
        assets: BlockAssets::default(),
        render: BlockRender::default(),
        behavior: BlockBehavior::default(),
        geometry: BlockGeometry::default(),
        animation: BlockAnimation::default(),
        placeable: true,
    }
}
