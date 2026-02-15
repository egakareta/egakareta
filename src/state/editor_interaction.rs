use glam::{Vec2, Vec3};

use super::{EditorInteractionChange, GizmoDragKind, State};
use crate::types::{AppPhase, EditorMode};

impl State {
    pub fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor || self.editor.ui.right_dragging {
            return false;
        }

        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );

        if self.editor.drag_gizmo(x, y, viewport) {
            self.sync_editor_objects_for_drag();
            if self
                .editor
                .runtime
                .interaction
                .gizmo_drag
                .as_ref()
                .map(|d| d.kind)
                == Some(GizmoDragKind::Move)
            {
                self.rebuild_editor_cursor_vertices();
            }
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
            true
        } else {
            false
        }
    }

    pub fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        if self.drag_editor_gizmo_from_screen(x, y) {
            return true;
        }

        if self.phase != AppPhase::Editor
            || self.editor.ui.right_dragging
            || self.editor.ui.mode != EditorMode::Select
        {
            return false;
        }

        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );

        if self.editor.drag_selection(x, y, viewport) {
            self.sync_editor_objects_for_drag();
            self.rebuild_editor_cursor_vertices();
            true
        } else {
            false
        }
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor.ui.right_dragging {
            return;
        }

        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        match self.editor.update_cursor_from_screen(x, y, viewport_size) {
            EditorInteractionChange::Hover => self.rebuild_editor_hover_outline_vertices(),
            EditorInteractionChange::Cursor => self.rebuild_editor_cursor_vertices(),
            EditorInteractionChange::None => {}
        }
    }

    pub(super) fn begin_editor_gizmo_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor {
            return false;
        }

        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        if self.editor.begin_gizmo_drag(x, y, viewport_size) {
            self.record_editor_history_state();
            true
        } else {
            false
        }
    }

    pub(super) fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        if self.phase != AppPhase::Editor {
            return false;
        }

        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        if self.editor.begin_block_drag(x, y, viewport_size) {
            self.record_editor_history_state();
            true
        } else {
            false
        }
    }

    pub(super) fn editor_gizmo_axis_lengths_world(
        &self,
        center: Vec3,
        target_pixels: f32,
    ) -> [f32; 3] {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .gizmo_axis_lengths_world(center, target_pixels, viewport_size)
    }

    pub(super) fn editor_gizmo_axis_width_world(&self, center: Vec3, target_pixels: f32) -> f32 {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .gizmo_axis_width_world(center, target_pixels, viewport_size)
    }

    pub(super) fn select_editor_block_from_screen(&mut self, x: f64, y: f64) {
        if self.phase != AppPhase::Editor || self.editor.ui.right_dragging {
            return;
        }

        let additive = self.editor.ui.shift_held;

        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        let Some(pick) = self.editor.pick_from_screen(x, y, viewport_size) else {
            if !additive {
                self.editor.ui.selected_block_indices.clear();
                self.editor.ui.selected_block_index = None;
                self.editor.ui.hovered_block_index = None;
            }
            self.editor.runtime.interaction.gizmo_drag = None;
            self.editor.runtime.interaction.block_drag = None;
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_hover_outline_vertices();
            self.rebuild_editor_selection_outline_vertices();
            return;
        };

        if let Some(hit_index) = pick.hit_block_index {
            if additive {
                if let Some(existing) = self
                    .editor
                    .ui
                    .selected_block_indices
                    .iter()
                    .position(|idx| *idx == hit_index)
                {
                    self.editor.ui.selected_block_indices.remove(existing);
                    if self.editor.ui.selected_block_indices.is_empty() {
                        self.editor.ui.selected_block_index = None;
                        self.editor.ui.hovered_block_index = None;
                    }
                } else {
                    self.editor.ui.selected_block_indices.push(hit_index);
                }
            } else {
                self.editor.ui.selected_block_indices.clear();
                self.editor.ui.selected_block_indices.push(hit_index);
            }
            self.editor.ui.hovered_block_index = Some(hit_index);
        } else if !additive {
            self.editor.ui.selected_block_indices.clear();
            self.editor.ui.selected_block_index = None;
            self.editor.ui.hovered_block_index = None;
        }

        self.sync_primary_selection_from_indices();
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;

        if let Some(index) = self.editor.ui.selected_block_index {
            if let Some(obj) = self.editor.objects.get(index) {
                self.editor.ui.cursor = [obj.position[0], obj.position[1], obj.position[2]];
                self.rebuild_editor_cursor_vertices();
            }
        } else if pick.cursor != self.editor.ui.cursor {
            self.editor.ui.cursor = pick.cursor;
            self.rebuild_editor_cursor_vertices();
        }

        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        if !self.editor.ui.right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;

        if self.phase == AppPhase::Editor {
            self.editor.camera.editor_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor.camera.editor_pitch = (self.editor.camera.editor_pitch
                + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        } else if self.phase == AppPhase::Playing
            && (self.gameplay.state.game_over || !self.gameplay.state.started)
        {
            self.editor.camera.playing_rotation -= dx as f32 * ROTATE_SPEED;
            self.editor.camera.playing_pitch = (self.editor.camera.playing_pitch
                + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        }
    }
}
