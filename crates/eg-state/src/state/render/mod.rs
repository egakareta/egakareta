/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub mod draw;
pub mod plugin;
pub mod resources;

#[cfg(test)]
use super::State;

#[cfg(test)]
impl Drop for State {
    fn drop(&mut self) {
        self.render_meshes_mut().destroy();
        self.render_gpu_mut().line_uniform_buffer.destroy();
        self.render_gpu_mut().camera_uniform_buffer.destroy();
        self.render_gpu_mut().color_space_uniform_buffer.destroy();
        self.render_gpu_mut().grid_uniform_buffer.destroy();
        let _ = self.render_gpu().device.poll(wgpu::PollType::Poll);
    }
}

#[cfg(test)]
mod tests {
    use crate::state::State;
    use eg_types::PhysicalSize;

    #[tokio::test]
    async fn test_apply_resize_zero_size() {
        let mut state = State::new_test().await;

        // Save old size and config
        let old_size = state.render_gpu().size;
        let old_config_width = state.render_gpu().config.width;
        let old_config_height = state.render_gpu().config.height;

        // Apply 0 size width
        state.render_gpu_mut().apply_resize(PhysicalSize {
            width: 0,
            height: 100,
        });

        // Assert it returned early and didn't change size
        assert_eq!(state.render_gpu().size.width, old_size.width);
        assert_eq!(state.render_gpu().config.width, old_config_width);

        // Apply 0 size height
        state.render_gpu_mut().apply_resize(PhysicalSize {
            width: 100,
            height: 0,
        });

        // Assert it returned early and didn't change size
        assert_eq!(state.render_gpu().size.height, old_size.height);
        assert_eq!(state.render_gpu().config.height, old_config_height);

        // Apply non-zero resize
        state.render_gpu_mut().apply_resize(PhysicalSize {
            width: 800,
            height: 600,
        });

        assert_eq!(state.render_gpu().size.width, 800);
        assert_eq!(state.render_gpu().config.width, 800);
    }
}
