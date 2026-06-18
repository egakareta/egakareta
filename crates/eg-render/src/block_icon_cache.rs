/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use eg_blocks::{all_placeable_blocks, resolve_block_texture_layers};
use eg_types::{BlockDefinition, BlockRenderProfile};
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

        for block in all_placeable_blocks()
            .iter()
            .filter(|block| block.placeable)
        {
            seen_ids.insert(block.id.clone());
            let signature = block_icon_signature(block);
            let should_refresh = self
                .entries
                .get(block.id.as_str())
                .is_none_or(|cached| cached.signature != signature);

            if !should_refresh {
                continue;
            }

            let Some(texture) = gpu.render_block_icon_snapshot(block.id.as_str(), BLOCK_ICON_SIZE)
            else {
                if let Some(removed) = self.entries.remove(block.id.as_str()) {
                    egui_renderer.free_texture(&removed.texture_id);
                }
                continue;
            };

            self.upsert_block_icon(gpu, egui_renderer, block.id.as_str(), texture, signature);
        }

        let stale_ids: Vec<String> = self
            .entries
            .keys()
            .filter(|id| !seen_ids.contains(*id))
            .cloned()
            .collect();
        for id in stale_ids {
            if let Some(removed) = self.entries.remove(id.as_str()) {
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
    let layers = resolve_block_texture_layers(block.id.as_str());
    layers.top.hash(&mut hasher);
    layers.side.hash(&mut hasher);
    layers.bottom.hash(&mut hasher);
    block.assets.mesh.hash(&mut hasher);
    block.assets.icon.hash(&mut hasher);
    render_profile_tag(&block.render.profile).hash(&mut hasher);
    block.render.icon_dimetric_projection.hash(&mut hasher);
    hash_optional_f32_array(block.render.icon_camera.position, &mut hasher);
    hash_optional_f32_array(block.render.icon_camera.rotation, &mut hasher);
    hash_optional_f32(
        block.render.icon_camera.orthographic_half_extent,
        &mut hasher,
    );
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

fn hash_optional_f32_array(value: Option<[f32; 3]>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(values) = value {
        for value in values {
            value.to_bits().hash(hasher);
        }
    }
}

fn hash_optional_f32(value: Option<f32>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some(value) = value {
        value.to_bits().hash(hasher);
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
    use eg_blocks::all_placeable_blocks;
    use eg_types::BlockRenderProfile;

    use super::{block_icon_signature, render_profile_tag};

    #[test]
    fn block_icon_signature_is_stable_for_same_visual_inputs() {
        let block = all_placeable_blocks()
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
        let block = all_placeable_blocks()
            .iter()
            .find(|block| block.placeable)
            .expect("expected at least one placeable block")
            .clone();
        let mut changed = block.clone();
        changed.render.noise += 0.123;
        assert_ne!(block_icon_signature(&block), block_icon_signature(&changed));
    }

    #[test]
    fn block_icon_signature_changes_when_profile_or_mesh_changes() {
        let block = all_placeable_blocks()
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

        let mut with_mesh_change = block.clone();
        with_mesh_change.assets.mesh = Some("custom_debug_mesh".to_string());
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_mesh_change)
        );

        let mut with_projection_change = block.clone();
        with_projection_change.render.icon_dimetric_projection =
            !block.render.icon_dimetric_projection;
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_projection_change)
        );

        let mut with_camera_change = block.clone();
        with_camera_change.render.icon_camera.position = Some([1.0, 2.0, 3.0]);
        assert_ne!(
            block_icon_signature(&block),
            block_icon_signature(&with_camera_change)
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
}
