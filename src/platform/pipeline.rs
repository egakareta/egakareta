/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Frame pipeline for rendering and UI.
//!
//! The `FramePipeline` manages the rendering loop, integrating egui UI with the game state.
//! It handles UI updates, tessellation, texture management, and delegates rendering to the state.

use crate::platform::block_icon_cache::BlockIconCache;
use crate::{show_editor_ui, show_menu_favicon_ui, show_menu_topbar, State};
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::SurfaceError;

/// The main frame pipeline that orchestrates UI rendering and game updates.
///
/// This struct holds the egui context, renderer, and menu favicon texture.
/// It runs each frame by updating the UI, tessellating shapes, updating textures,
/// running game logic, and rendering everything to the surface.
pub struct FramePipeline {
    egui_ctx: egui::Context,
    egui_renderer: EguiRenderer,
    menu_favicon: Option<egui::TextureHandle>,
    block_icon_cache: BlockIconCache,
}

impl FramePipeline {
    /// Creates a new frame pipeline with the given egui context and renderer.
    pub fn new(
        egui_ctx: egui::Context,
        egui_renderer: EguiRenderer,
        menu_favicon: Option<egui::TextureHandle>,
    ) -> Self {
        Self {
            egui_ctx,
            egui_renderer,
            menu_favicon,
            block_icon_cache: BlockIconCache::new(),
        }
    }

    /// Runs a single frame of the application.
    ///
    /// This method:
    /// 1. Runs the egui UI logic with the provided raw input
    /// 2. Tessellates the UI shapes for rendering
    /// 3. Updates egui textures on the GPU
    /// 4. Updates the game state
    /// 5. Renders the UI to the surface
    /// 6. Handles surface errors if they occur
    /// 7. Frees unused textures
    ///
    /// Returns the full egui output for further processing.
    pub fn run_frame(&mut self, state: &mut State, raw_input: egui::RawInput) -> egui::FullOutput {
        self.block_icon_cache
            .refresh_icons(state, &mut self.egui_renderer);
        let block_icon_texture_ids = self.block_icon_cache.texture_ids();

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            show_editor_ui(ctx, state, &block_icon_texture_ids);
            show_menu_topbar(ctx, state);
            if let Some(favicon) = &self.menu_favicon {
                show_menu_favicon_ui(ctx, state, favicon);
            }
        });

        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes.clone(), full_output.pixels_per_point);

        let window_size = [state.surface_width(), state.surface_height()];
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: window_size,
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(state.device(), state.queue(), *id, image_delta);
        }

        state.update();

        match state.render_egui(&mut self.egui_renderer, &paint_jobs, &screen_descriptor) {
            Ok(_) => {}
            Err(SurfaceError::Lost) | Err(SurfaceError::Outdated) => {
                state.handle_surface_lost();
            }
            Err(SurfaceError::OutOfMemory) => {
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("OutOfMemory error in render pipeline");
                #[cfg(target_arch = "wasm32")]
                gloo_console::error!("OutOfMemory error in render pipeline");
            }
            Err(err) => {
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("Render error: {:?}", err);
                #[cfg(target_arch = "wasm32")]
                gloo_console::error!(format!("Render error: {:?}", err));
            }
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        full_output
    }

    /// Returns a reference to the egui context for UI interactions.
    pub fn ctx(&self) -> &egui::Context {
        &self.egui_ctx
    }
}

#[cfg(test)]
mod tests {
    use super::FramePipeline;
    use crate::State;

    #[test]
    fn run_frame_executes_without_surface_and_returns_output() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let egui_ctx = egui::Context::default();
            let renderer = state.create_egui_renderer();
            let mut pipeline = FramePipeline::new(egui_ctx, renderer, None);

            let output = pipeline.run_frame(&mut state, egui::RawInput::default());

            assert!(output.pixels_per_point > 0.0);
            let _ctx = pipeline.ctx();
        });
    }

    #[test]
    fn run_frame_executes_favicon_ui_branch_when_texture_is_present() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let egui_ctx = egui::Context::default();
            let renderer = state.create_egui_renderer();
            let favicon = egui_ctx.load_texture(
                "test-favicon",
                egui::ColorImage::new([2, 2], vec![egui::Color32::WHITE; 4]),
                egui::TextureOptions::LINEAR,
            );
            let mut pipeline = FramePipeline::new(egui_ctx, renderer, Some(favicon));

            let output = pipeline.run_frame(&mut state, egui::RawInput::default());

            assert!(output.pixels_per_point > 0.0);
        });
    }
}
