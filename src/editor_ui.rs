use crate::{BlockKind, State};

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Block:");

                let current = state.editor_selected_block_kind();
                for (name, kind) in [
                    ("Standard", BlockKind::Standard),
                    ("Grass", BlockKind::Grass),
                    ("Dirt", BlockKind::Dirt),
                ] {
                    if ui.selectable_label(current == kind, name).clicked() {
                        state.set_editor_block_kind(kind);
                    }
                }
            });
        });
}
