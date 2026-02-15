use crate::{show_editor_ui, show_menu_wordmark_ui, State};
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::SurfaceError;

pub struct FramePipeline {
    egui_ctx: egui::Context,
    egui_renderer: EguiRenderer,
    menu_wordmark: Option<egui::TextureHandle>,
}

impl FramePipeline {
    pub fn new(
        egui_ctx: egui::Context,
        egui_renderer: EguiRenderer,
        menu_wordmark: Option<egui::TextureHandle>,
    ) -> Self {
        Self {
            egui_ctx,
            egui_renderer,
            menu_wordmark,
        }
    }

    pub fn run_frame(&mut self, state: &mut State, raw_input: egui::RawInput) -> egui::FullOutput {
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            show_editor_ui(ctx, state);
            if let Some(wordmark) = &self.menu_wordmark {
                show_menu_wordmark_ui(ctx, state, wordmark);
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
                web_sys::console::error_1(&"OutOfMemory error in render pipeline".into());
            }
            Err(err) => {
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("Render error: {:?}", err);
                #[cfg(target_arch = "wasm32")]
                web_sys::console::error_1(&format!("Render error: {:?}", err).into());
            }
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        full_output
    }

    pub fn ctx(&self) -> &egui::Context {
        &self.egui_ctx
    }
}
