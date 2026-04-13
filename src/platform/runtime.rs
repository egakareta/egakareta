/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::load_menu_favicon_texture;
use crate::platform::pipeline::FramePipeline;
use crate::State;

pub struct Runtime {
    pub state: State,
    pub pipeline: FramePipeline,
}

impl Runtime {
    pub fn new(state: State) -> Self {
        let egui_ctx = egui::Context::default();

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "sora".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/Sora.ttf"
            ))),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "sora".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "sora".to_owned());

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        egui_ctx.set_fonts(fonts);

        let egui_renderer = state.create_egui_renderer();
        let menu_favicon = load_menu_favicon_texture(&egui_ctx);
        let pipeline = FramePipeline::new(egui_ctx, egui_renderer, menu_favicon);

        Self { state, pipeline }
    }

    pub fn run_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        self.pipeline.run_frame(&mut self.state, raw_input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;

    #[test]
    fn test_runtime_new() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let runtime = Runtime::new(state);

            // Verify the egui context was initialized
            let ctx = runtime.pipeline.ctx();
            assert!(ctx.pixels_per_point() > 0.0);

            // Verify the state was correctly moved into the runtime
            assert!(runtime.state.is_menu());
        });
    }

    #[test]
    fn test_runtime_run_frame() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let mut runtime = Runtime::new(state);

            let mut raw_input = egui::RawInput::default();
            raw_input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            ));

            let output = runtime.run_frame(raw_input);

            // Verify we get valid pixels_per_point and shapes were generated (menu UI)
            assert!(output.pixels_per_point > 0.0);
            assert!(!output.shapes.is_empty());
        });
    }
}
