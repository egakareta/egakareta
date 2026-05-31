/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
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
            self.runtime.interaction.hovered_tap_index = None;
            self.runtime.interaction.hovered_tap_division = None;
            if self.ui.mode.is_selection_mode() && self.ui.hovered_block_index.is_some() {
                self.ui.hovered_block_index = None;
                return EditorInteractionChange::Hover;
            }
            return EditorInteractionChange::None;
        };

        if self.ui.mode.is_selection_mode() {
            self.runtime.interaction.hovered_tap_index = None;
            self.runtime.interaction.hovered_tap_division = None;
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

        self.runtime.interaction.hovered_tap_index = (self.ui.mode
            == crate::types::EditorMode::Tapping)
            .then_some(pick.hit_tap_index)
            .flatten();
        self.runtime.interaction.hovered_tap_division = (self.ui.mode
            == crate::types::EditorMode::Tapping)
            .then_some(pick.hit_tap_division)
            .flatten();

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
        let previous_tap_division_hover = self.editor.runtime.interaction.hovered_tap_division;
        let previous_tap_hover = self.editor.runtime.interaction.hovered_tap_index;
        match self
            .editor
            .update_cursor_from_screen_ext(x, y, viewport_size, self.phase)
        {
            EditorInteractionChange::Hover => self.rebuild_editor_hover_outline_vertices(),
            EditorInteractionChange::Cursor => self.rebuild_editor_cursor_vertices(),
            EditorInteractionChange::None => {}
        }
        if self.editor.runtime.interaction.hovered_tap_division != previous_tap_division_hover
            || self.editor.runtime.interaction.hovered_tap_index != previous_tap_hover
        {
            self.rebuild_tap_indicator_vertices();
            self.rebuild_editor_cursor_vertices();
        }
    }
}
