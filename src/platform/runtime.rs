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

        // Load custom font: Sora
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "sora".to_owned(),
            egui::FontData::from_static(include_bytes!("../../assets/Sora.ttf")).into(),
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
            .push("sora".to_owned());
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
