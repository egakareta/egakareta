/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::State;

impl State {
    /// Renders a single block to an offscreen texture for use as an editor icon.
    pub fn render_block_icon_snapshot(&self, block_id: &str, size: u32) -> Option<wgpu::Texture> {
        self.render.gpu.render_block_icon_snapshot(block_id, size)
    }
}

#[cfg(test)]
mod tests {
    use super::super::State;

    #[tokio::test]
    async fn render_block_icon_snapshot_returns_texture_for_known_block() {
        let state = State::new_test().await;
        let texture = state
            .render_block_icon_snapshot("core/stone", 96)
            .expect("expected icon snapshot texture");
        let size = texture.size();
        assert_eq!(size.width, 96);
        assert_eq!(size.height, 96);
    }

    #[tokio::test]
    async fn render_block_icon_snapshot_clamps_zero_size_and_supports_unknown_block_ids() {
        let state = State::new_test().await;
        let texture = state
            .render_block_icon_snapshot("missing/block-id", 0)
            .expect("expected fallback block icon snapshot texture");
        let size = texture.size();
        assert_eq!(size.width, 1);
        assert_eq!(size.height, 1);
    }
}
