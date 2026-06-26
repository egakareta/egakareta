/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub mod block_icon;
pub mod draw;
pub mod mirror;
pub mod plugin;
pub mod resources;

use eg_render::{GpuContext, MeshSlot, SceneMeshes};

use eg_platform::state_host::SurfaceHost;
use eg_types::PhysicalSize;

use super::State;

impl State {
    pub fn surface_width(&self) -> u32 {
        self.render.gpu.surface_width()
    }

    pub fn surface_height(&self) -> u32 {
        self.render.gpu.surface_height()
    }

    pub fn handle_surface_lost(&mut self) {
        let size = self.render.current_size();
        self.resize_surface(size);
    }
}

pub struct RenderSubsystem {
    pub surface_host: Option<SurfaceHost>,
    pub gpu: GpuContext,
    pub meshes: SceneMeshes,
}

impl RenderSubsystem {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> Option<&eg_platform::state_host::NativeWindow> {
        match &self.surface_host {
            Some(SurfaceHost::Window(w)) => Some(w),
            _ => None,
        }
    }

    pub fn current_size(&self) -> PhysicalSize<u32> {
        self.surface_host
            .as_ref()
            .map(|h| h.current_size())
            .unwrap_or(self.gpu.size)
    }
}

#[cfg(test)]
impl Drop for RenderSubsystem {
    fn drop(&mut self) {
        self.meshes.destroy();
        self.gpu.line_uniform_buffer.destroy();
        self.gpu.camera_uniform_buffer.destroy();
        self.gpu.color_space_uniform_buffer.destroy();
        self.gpu.grid_uniform_buffer.destroy();
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
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
        let old_size = state.render.gpu.size;
        let old_config_width = state.render.gpu.config.width;
        let old_config_height = state.render.gpu.config.height;

        // Apply 0 size width
        state.render.gpu.apply_resize(PhysicalSize {
            width: 0,
            height: 100,
        });

        // Assert it returned early and didn't change size
        assert_eq!(state.render.gpu.size.width, old_size.width);
        assert_eq!(state.render.gpu.config.width, old_config_width);

        // Apply 0 size height
        state.render.gpu.apply_resize(PhysicalSize {
            width: 100,
            height: 0,
        });

        // Assert it returned early and didn't change size
        assert_eq!(state.render.gpu.size.height, old_size.height);
        assert_eq!(state.render.gpu.config.height, old_config_height);

        // Apply non-zero resize
        state.render.gpu.apply_resize(PhysicalSize {
            width: 800,
            height: 600,
        });

        assert_eq!(state.render.gpu.size.width, 800);
        assert_eq!(state.render.gpu.config.width, 800);
    }
}
