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
                    .filter(|index| !self.selection_contains(*index))
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

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use crate::types::{EditorInteractionChange, EditorMode, LevelObject};
    use glam::{Vec2, Vec3};

    fn test_block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            position,
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn selected_block_hover_is_cleared_and_stays_stable() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.hovered_block_index = Some(0);

            let viewport = Vec2::new(1280.0, 720.0);
            let center_screen = state
                .editor
                .world_to_screen_v(Vec3::new(0.5, 0.5, 0.5), viewport)
                .expect("block center should project to screen");

            let first = state.editor.update_cursor_from_screen(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport,
            );
            assert!(matches!(first, EditorInteractionChange::Hover));
            assert_eq!(state.editor.ui.hovered_block_index, None);

            let second = state.editor.update_cursor_from_screen(
                center_screen.x as f64,
                center_screen.y as f64,
                viewport,
            );
            assert!(matches!(second, EditorInteractionChange::None));
            assert_eq!(state.editor.ui.hovered_block_index, None);
        });
    }
}
