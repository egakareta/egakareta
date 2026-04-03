/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::load_menu_wordmark_texture;
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
        let menu_wordmark = load_menu_wordmark_texture(&egui_ctx);
        let pipeline = FramePipeline::new(egui_ctx, egui_renderer, menu_wordmark);

        Self { state, pipeline }
    }

    pub fn run_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        self.pipeline.run_frame(&mut self.state, raw_input)
    }
}
