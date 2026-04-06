/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::block_repository::{
    all_placeable_blocks, resolve_block_texture_layers, BlockDefinition, BlockRenderProfile,
};
use crate::State;
use egui::TextureId;
use egui_wgpu::Renderer as EguiRenderer;

const BLOCK_ICON_SIZE: u32 = 96;

struct CachedBlockIcon {
    texture: wgpu::Texture,
    texture_id: TextureId,
    signature: u64,
}

/// Caches offscreen-rendered block icons and exposes `egui` texture ids for UI use.
pub(crate) struct BlockIconCache {
    entries: HashMap<String, CachedBlockIcon>,
}

impl BlockIconCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn refresh_icons(&mut self, state: &State, egui_renderer: &mut EguiRenderer) {
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

            let Some(texture) =
                state.render_block_icon_snapshot(block.id.as_str(), BLOCK_ICON_SIZE)
            else {
                if let Some(removed) = self.entries.remove(block.id.as_str()) {
                    egui_renderer.free_texture(&removed.texture_id);
                }
                continue;
            };

            self.upsert_block_icon(state, egui_renderer, block.id.as_str(), texture, signature);
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

    pub(crate) fn texture_ids(&self) -> HashMap<String, TextureId> {
        self.entries
            .iter()
            .map(|(id, cached)| (id.clone(), cached.texture_id))
            .collect()
    }

    fn upsert_block_icon(
        &mut self,
        state: &State,
        egui_renderer: &mut EguiRenderer,
        block_id: &str,
        texture: wgpu::Texture,
        signature: u64,
    ) {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if let Some(existing) = self.entries.get_mut(block_id) {
            egui_renderer.update_egui_texture_from_wgpu_texture(
                state.device(),
                &view,
                wgpu::FilterMode::Linear,
                existing.texture_id,
            );
            existing.texture = texture;
            existing.signature = signature;
            return;
        }

        let texture_id =
            egui_renderer.register_native_texture(state.device(), &view, wgpu::FilterMode::Linear);
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

fn block_icon_signature(block: &BlockDefinition) -> u64 {
    let mut hasher = DefaultHasher::new();
    let layers = resolve_block_texture_layers(block.id.as_str());
    layers.top.hash(&mut hasher);
    layers.side.hash(&mut hasher);
    layers.bottom.hash(&mut hasher);
    block.assets.mesh.hash(&mut hasher);
    block.assets.icon.hash(&mut hasher);
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

fn render_profile_tag(profile: &BlockRenderProfile) -> u8 {
    match profile {
        BlockRenderProfile::Solid => 0,
        BlockRenderProfile::VoidFrame => 1,
        BlockRenderProfile::SpeedPortal => 2,
        BlockRenderProfile::FinishRing => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::block_icon_signature;
    use crate::block_repository::all_placeable_blocks;

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
}
