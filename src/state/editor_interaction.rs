use glam::{Vec2, Vec3};

use super::{EditorDirtyFlags, EditorInteractionChange, EditorSubsystem, GizmoDragKind, State};
use crate::types::{AppPhase, EditorMode};

impl EditorSubsystem {
    pub(crate) fn drag_camera_by_pixels(
        &mut self,
        dx: f64,
        dy: f64,
        phase: AppPhase,
        is_game_active: bool,
    ) {
        if !self.ui.right_dragging {
            return;
        }

        const ROTATE_SPEED: f32 = 0.008;
        const PITCH_SPEED: f32 = 0.006;

        if phase == AppPhase::Editor {
            self.camera.editor_rotation -= dx as f32 * ROTATE_SPEED;
            self.camera.editor_pitch = (self.camera.editor_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        } else if phase == AppPhase::Playing && !is_game_active {
            self.camera.playing_rotation -= dx as f32 * ROTATE_SPEED;
            self.camera.playing_pitch = (self.camera.playing_pitch + dy as f32 * PITCH_SPEED)
                .clamp(10.0f32.to_radians(), 85.0f32.to_radians());
        }
    }

    pub(crate) fn drag_gizmo_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.drag_gizmo(x, y, viewport) {
            self.sync_objects_for_drag();
            if self.runtime.interaction.gizmo_drag.as_ref().map(|d| d.kind)
                == Some(GizmoDragKind::Move)
            {
                self.mark_dirty(EditorDirtyFlags {
                    rebuild_cursor: true,
                    ..EditorDirtyFlags::default()
                });
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn drag_selection_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor || self.ui.right_dragging || self.ui.mode != EditorMode::Select
        {
            return false;
        }

        if self.drag_selection(x, y, viewport) {
            self.sync_objects_for_drag();
            self.mark_dirty(EditorDirtyFlags {
                rebuild_cursor: true,
                ..EditorDirtyFlags::default()
            });
            true
        } else {
            false
        }
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

    pub(crate) fn begin_gizmo_drag_ext(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.begin_gizmo_drag(x, y, viewport) {
            self.record_history_state();
            true
        } else {
            false
        }
    }

    pub(crate) fn begin_selected_block_drag_ext(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        if phase != AppPhase::Editor {
            return false;
        }

        if self.begin_block_drag(x, y, viewport) {
            self.record_history_state();
            true
        } else {
            false
        }
    }

    pub(crate) fn select_block_from_screen(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> EditorInteractionChange {
        if phase != AppPhase::Editor || self.ui.right_dragging {
            return EditorInteractionChange::None;
        }

        let additive = self.ui.shift_held;

        let Some(pick) = self.pick_from_screen(x, y, viewport) else {
            if !additive {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
                self.ui.hovered_block_index = None;
            }
            self.runtime.interaction.gizmo_drag = None;
            self.runtime.interaction.block_drag = None;
            self.mark_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
            return EditorInteractionChange::None;
        };

        let mut changed = EditorInteractionChange::None;

        if let Some(hit_index) = pick.hit_block_index {
            if additive {
                if let Some(existing) = self
                    .ui
                    .selected_block_indices
                    .iter()
                    .position(|idx| *idx == hit_index)
                {
                    self.ui.selected_block_indices.remove(existing);
                    if self.ui.selected_block_indices.is_empty() {
                        self.ui.selected_block_index = None;
                        self.ui.hovered_block_index = None;
                    }
                } else {
                    self.ui.selected_block_indices.push(hit_index);
                }
            } else {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_indices.push(hit_index);
            }
            self.ui.hovered_block_index = Some(hit_index);
            changed = EditorInteractionChange::Hover;
        } else if !additive {
            self.ui.selected_block_indices.clear();
            self.ui.selected_block_index = None;
            self.ui.hovered_block_index = None;
            changed = EditorInteractionChange::Hover;
        }

        self.sync_primary_selection_from_indices();
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        if let Some(index) = self.ui.selected_block_index {
            if let Some(obj) = self.objects.get(index) {
                self.ui.cursor = [obj.position[0], obj.position[1], obj.position[2]];
                changed = EditorInteractionChange::Cursor;
            }
        } else if pick.cursor != self.ui.cursor {
            self.ui.cursor = pick.cursor;
            changed = EditorInteractionChange::Cursor;
        }

        self.mark_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            rebuild_cursor: matches!(changed, EditorInteractionChange::Cursor),
            ..EditorDirtyFlags::default()
        });

        changed
    }
}

impl State {
    pub fn drag_editor_gizmo_from_screen(&mut self, x: f64, y: f64) -> bool {
        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .drag_gizmo_from_screen(x, y, viewport, self.phase)
    }

    pub fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .drag_selection_from_screen(x, y, viewport, self.phase)
    }

    pub fn update_editor_cursor_from_screen(&mut self, x: f64, y: f64) {
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

    pub(super) fn begin_editor_gizmo_drag(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .begin_gizmo_drag_ext(x, y, viewport_size, self.phase)
    }

    pub(super) fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .begin_selected_block_drag_ext(x, y, viewport_size, self.phase)
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
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .select_block_from_screen(x, y, viewport_size, self.phase);
    }

    pub fn drag_editor_camera_by_pixels(&mut self, dx: f64, dy: f64) {
        let is_game_active = self.gameplay.state.started && !self.gameplay.state.game_over;
        self.editor
            .drag_camera_by_pixels(dx, dy, self.phase, is_game_active);
    }
}
