use super::super::{EditorSubsystem, State};
use crate::types::AppPhase;
use crate::types::EditorInteractionChange;
use glam::Vec2;

impl EditorSubsystem {
    pub(crate) fn update_cursor_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport_size: Vec2,
    ) -> EditorInteractionChange {
        self.ui.pointer_screen = Some([x, y]);

        let Some(pick) = self.pick_from_screen(x, y, viewport_size) else {
            if self.ui.mode.is_selection_mode() && self.ui.hovered_block_index.is_some() {
                self.ui.hovered_block_index = None;
                return EditorInteractionChange::Hover;
            }
            return EditorInteractionChange::None;
        };

        if self.ui.mode.is_selection_mode() {
            let next_hover = if self.runtime.interaction.hovered_gizmo.is_some() {
                None
            } else {
                pick.hit_block_index
            };

            if self.ui.hovered_block_index != next_hover {
                self.ui.hovered_block_index = next_hover;
                return EditorInteractionChange::Hover;
            }
            return EditorInteractionChange::None;
        }

        if pick.cursor != self.ui.cursor {
            self.ui.cursor = pick.cursor;
            return EditorInteractionChange::Cursor;
        }

        EditorInteractionChange::None
    }

    pub(crate) fn update_cursor_from_screen_ext(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> EditorInteractionChange {
        if phase != AppPhase::Editor || self.ui.right_dragging {
            return EditorInteractionChange::None;
        }

        self.update_cursor_from_screen(x, y, viewport)
    }
}

impl State {
    pub(crate) fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        match self
            .editor
            .update_cursor_from_screen_ext(x, y, viewport_size, self.phase)
        {
            EditorInteractionChange::Hover => self.rebuild_editor_hover_outline_vertices(),
            EditorInteractionChange::Cursor => self.rebuild_editor_cursor_vertices(),
            EditorInteractionChange::None => {}
        }
    }
}
