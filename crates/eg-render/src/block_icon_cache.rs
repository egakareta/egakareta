/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use eg_blocks::{
    BlockAnimationAxis, BlockAnimationPhaseSeed, BlockBobAnimation, BlockCuboid, BlockDefinition,
    BlockEffect, BlockEffectColorMode, BlockEffectPreset, BlockId, BlockPulseAnimation,
    BlockRenderProfile, BlockSpinAnimation, BlockTextureLayers,
};
use egui::TextureId;
use egui_wgpu::Renderer as EguiRenderer;

use crate::GpuContext;

const BLOCK_ICON_SIZE: u32 = 96;

struct CachedBlockIcon {
    texture: wgpu::Texture,
    texture_id: TextureId,
    signature: u64,
}

/// Caches offscreen-rendered block icons and exposes `egui` texture ids for UI use.
pub struct BlockIconCache {
    entries: HashMap<String, CachedBlockIcon>,
}

impl BlockIconCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn refresh_icons(&mut self, gpu: &GpuContext, egui_renderer: &mut EguiRenderer) {
        let mut seen_ids = HashSet::new();

        for block in BlockDefinition::all_placeable()
            .iter()
            .filter(|block| block.placeable)
        {
            seen_ids.insert(block.id.clone());
            let signature = block_icon_signature(block);
            let should_refresh = self
                .entries
                .get(&block.id.to_string())
                .is_none_or(|cached| cached.signature != signature);

            if !should_refresh {
                continue;
            }

            let Some(texture) =
                gpu.render_block_icon_snapshot(&block.id.to_string(), BLOCK_ICON_SIZE)
            else {
                if let Some(removed) = self.entries.remove(&block.id.to_string()) {
                    egui_renderer.free_texture(&removed.texture_id);
                }
                continue;
            };

            self.upsert_block_icon(
                gpu,
                egui_renderer,
                &block.id.to_string(),
                texture,
                signature,
            );
        }

        let stale_ids: Vec<String> = self
            .entries
            .keys()
            .filter(|id| !seen_ids.contains(&BlockId::new(id)))
            .cloned()
            .collect();
        for id in stale_ids {
            if let Some(removed) = self.entries.remove(&id) {
                egui_renderer.free_texture(&removed.texture_id);
            }
        }
    }

    pub fn texture_ids(&self) -> HashMap<String, TextureId> {
        self.entries
            .iter()
            .map(|(id, cached)| (id.clone(), cached.texture_id))
            .collect()
    }

    fn upsert_block_icon(
        &mut self,
        gpu: &GpuContext,
        egui_renderer: &mut EguiRenderer,
        block_id: &str,
        texture: wgpu::Texture,
        signature: u64,
    ) {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if let Some(existing) = self.entries.get_mut(block_id) {
            egui_renderer.update_egui_texture_from_wgpu_texture(
                &gpu.device,
                &view,
                wgpu::FilterMode::Linear,
                existing.texture_id,
            );
            existing.texture = texture;
            existing.signature = signature;
            return;
        }

        let texture_id =
            egui_renderer.register_native_texture(&gpu.device, &view, wgpu::FilterMode::Linear);
        self.entries.insert(
            block_id.to_string(),
            CachedBlockIcon {
                texture,
                texture_id,
                signature,
            },
        );
    }
}

impl Default for BlockIconCache {
    fn default() -> Self {
        Self::new()
    }
}

fn block_icon_signature(block: &BlockDefinition) -> u64 {
    let mut hasher = DefaultHasher::new();
    let layers = BlockTextureLayers::get(&block.id.to_string());
    layers.top.hash(&mut hasher);
    layers.side.hash(&mut hasher);
    layers.bottom.hash(&mut hasher);
    block.assets.icon.hash(&mut hasher);
    hash_cuboids(&block.geometry.elements, &mut hasher);
    hash_cuboids(&block.geometry.hitboxes, &mut hasher);
    hash_animation(block, &mut hasher);
    render_profile_tag(&block.render.profile).hash(&mut hasher);
    for value in block.render.color_top {
        value.to_bits().hash(&mut hasher);
    }
    for value in block.render.color_side {
        value.to_bits().hash(&mut hasher);
    }
    for value in block.render.color_bottom {
        value.to_bits().hash(&mut hasher);
    }
    for value in block.render.color_fill {
        value.to_bits().hash(&mut hasher);
    }
    for value in block.render.color_outline {
        value.to_bits().hash(&mut hasher);
    }
    block.render.noise.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn hash_cuboids(cuboids: &[BlockCuboid], hasher: &mut DefaultHasher) {
    cuboids.len().hash(hasher);
    for cuboid in cuboids {
        for value in cuboid.from {
            value.to_bits().hash(hasher);
        }
        for value in cuboid.to {
            value.to_bits().hash(hasher);
        }
        cuboid.color_tint.is_some().hash(hasher);
        if let Some(color_tint) = cuboid.color_tint {
            for value in color_tint {
                value.to_bits().hash(hasher);
            }
        }
    }
}

fn hash_animation(block: &BlockDefinition, hasher: &mut DefaultHasher) {
    hash_optional_spin(block.animation.idle.spin, hasher);
    hash_optional_bob(block.animation.idle.bob, hasher);
    hash_optional_pulse(block.animation.idle.pulse, hasher);
    hash_phase_seed(block.animation.idle.phase_seed, hasher);
    hash_optional_effect(block.animation.on_consumed, hasher);
    hash_optional_effect(block.animation.on_break, hasher);
}

fn hash_axis(axis: BlockAnimationAxis, hasher: &mut DefaultHasher) {
    match axis {
        BlockAnimationAxis::X => 1_u8,
        BlockAnimationAxis::Y => 2_u8,
        BlockAnimationAxis::Z => 3_u8,
    }
    .hash(hasher);
}

fn hash_phase_seed(phase_seed: BlockAnimationPhaseSeed, hasher: &mut DefaultHasher) {
    match phase_seed {
        BlockAnimationPhaseSeed::None => 0_u8,
        BlockAnimationPhaseSeed::Position => 1_u8,
    }
    .hash(hasher);
}

fn hash_optional_spin(value: Option<BlockSpinAnimation>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(value) = value {
        hash_axis(value.axis, hasher);
        value.speed.to_bits().hash(hasher);
    }
}

fn hash_optional_bob(value: Option<BlockBobAnimation>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(value) = value {
        hash_axis(value.axis, hasher);
        value.amplitude.to_bits().hash(hasher);
        value.frequency.to_bits().hash(hasher);
    }
}

fn hash_optional_pulse(value: Option<BlockPulseAnimation>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(value) = value {
        value.amplitude.to_bits().hash(hasher);
        value.frequency.to_bits().hash(hasher);
    }
}

fn hash_optional_effect(value: Option<BlockEffect>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(value) = value {
        match value.preset {
            BlockEffectPreset::Shatter => 1_u8,
        }
        .hash(hasher);
        value.duration_seconds.to_bits().hash(hasher);
        value.shard_count.hash(hasher);
        value.speed_scale.to_bits().hash(hasher);
        value.gravity_scale.to_bits().hash(hasher);
        value.wobble_scale.to_bits().hash(hasher);
        match value.color_mode {
            BlockEffectColorMode::Tint => 1_u8,
            BlockEffectColorMode::Render => 2_u8,
        }
        .hash(hasher);
    }
}

fn render_profile_tag(profile: &BlockRenderProfile) -> u8 {
    match profile {
        BlockRenderProfile::Solid => 0,
        BlockRenderProfile::Liquid => 1,
        BlockRenderProfile::SpeedPortal => 3,
        BlockRenderProfile::Neon => 5,
        BlockRenderProfile::Gem => 7,
        BlockRenderProfile::TransformTrigger => 9,
        BlockRenderProfile::CameraTrigger => 11,
    }
}

#[cfg(test)]
mod tests {
    use eg_blocks::{
        BlockAnimationAxis, BlockBobAnimation, BlockCuboid, BlockDefinition, BlockEffect,
        BlockRenderProfile,
    };

    use super::{block_icon_signature, render_profile_tag};

    #[test]
    fn block_icon_signature_is_stable_for_same_visual_inputs() {
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block")
            .clone();
        let signature_a = block_icon_signature(&block);
        let signature_b = block_icon_signature(&block);
        assert_eq!(signature_a, signature_b);
    }

    #[test]
    fn block_icon_signature_changes_when_render_inputs_change() {
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block")
            .clone();
        let mut changed = block.clone();
        changed.render.noise += 0.123;
        assert_ne!(block_icon_signature(&block), block_icon_signature(&changed));
    }

    #[test]
    fn block_icon_signature_changes_when_profile_or_projection_changes() {
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block")
            .clone();

        let mut with_profile_change = block.clone();
        with_profile_change.render.profile = match block.render.profile {
            BlockRenderProfile::Solid => BlockRenderProfile::Liquid,
            _ => BlockRenderProfile::Solid,
        };
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_profile_change)
        );
    }

    #[test]
    fn render_profile_tag_maps_all_profiles() {
        assert_eq!(render_profile_tag(&BlockRenderProfile::Solid), 0);
        assert_eq!(render_profile_tag(&BlockRenderProfile::Liquid), 1);
        assert_eq!(render_profile_tag(&BlockRenderProfile::SpeedPortal), 3);
        assert_eq!(render_profile_tag(&BlockRenderProfile::Neon), 5);
        assert_eq!(render_profile_tag(&BlockRenderProfile::Gem), 7);
        assert_eq!(render_profile_tag(&BlockRenderProfile::TransformTrigger), 9);
        assert_eq!(render_profile_tag(&BlockRenderProfile::CameraTrigger), 11);
    }

    #[test]
    fn block_icon_signature_changes_when_geometry_or_animation_changes() {
        let block = BlockDefinition::all_placeable()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block")
            .clone();

        let mut with_geometry_change = block.clone();
        with_geometry_change.geometry.elements.push(BlockCuboid {
            from: [1.0, 1.0, 1.0],
            to: [2.0, 2.0, 2.0],
            color_tint: None,
        });
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_geometry_change)
        );

        let mut with_animation_change = block.clone();
        with_animation_change.animation.idle.bob = Some(BlockBobAnimation {
            axis: BlockAnimationAxis::Y,
            amplitude: 0.25,
            frequency: 1.5,
        });
        with_animation_change.animation.on_consumed = Some(BlockEffect::default());
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_animation_change)
        );
    }
}
