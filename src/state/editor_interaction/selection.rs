use super::super::{
    EditorBlockDrag, EditorDirtyFlags, EditorDragBlockStart, EditorInteractionChange,
    EditorSubsystem, State,
};
use crate::types::{AppPhase, EditorMode};
use glam::{Vec2, Vec3};

impl EditorSubsystem {
    pub(crate) fn drag_selection(&mut self, x: f64, y: f64, viewport: Vec2) -> bool {
        self.ui.pointer_screen = Some([x, y]);

        let Some(drag) = self.runtime.interaction.block_drag.clone() else {
            return false;
        };
        let mouse_delta = Vec2::new(
            (x - drag.start_mouse[0]) as f32,
            (y - drag.start_mouse[1]) as f32,
        );

        if mouse_delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let (camera_right_xy, camera_up_xy) = self.camera_axes_xy();
        let center = Vec3::new(
            drag.start_center_world[0],
            drag.start_center_world[1],
            drag.start_center_world[2],
        );

        let Some(origin_screen) = self.world_to_screen_v(center, viewport) else {
            return true;
        };

        let camera_shift =
            Vec2::new(drag.start_center_screen[0], drag.start_center_screen[1]) - origin_screen;
        let effective_mouse_delta = mouse_delta + camera_shift;

        let right_world = Vec3::new(camera_right_xy.x, camera_right_xy.y, 0.0);
        let up_world = Vec3::new(camera_up_xy.x, camera_up_xy.y, 0.0);

        let Some(right_screen) = self.world_to_screen_v(center + right_world, viewport) else {
            return true;
        };
        let Some(up_screen) = self.world_to_screen_v(center + up_world, viewport) else {
            return true;
        };

        let right_screen_delta = right_screen - origin_screen;
        let up_screen_delta = up_screen - origin_screen;

        let det =
            right_screen_delta.x * up_screen_delta.y - right_screen_delta.y * up_screen_delta.x;
        if det.abs() <= f32::EPSILON {
            return true;
        }

        let inv_det = 1.0 / det;
        let world_right_factor = (effective_mouse_delta.x * up_screen_delta.y
            - effective_mouse_delta.y * up_screen_delta.x)
            * inv_det;
        let world_up_factor = (right_screen_delta.x * effective_mouse_delta.y
            - right_screen_delta.y * effective_mouse_delta.x)
            * inv_det;

        let snap_enabled = self.config.snap_to_grid;
        let snap_step = self.config.snap_step.max(0.05);

        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.objects.get_mut(block.index) {
                let mut next = [
                    block.position[0]
                        + world_right_factor * camera_right_xy.x
                        + world_up_factor * camera_up_xy.x,
                    block.position[1]
                        + world_right_factor * camera_right_xy.y
                        + world_up_factor * camera_up_xy.y,
                    block.position[2],
                ];

                if snap_enabled {
                    next[0] = (next[0] / snap_step).round() * snap_step;
                    next[1] = (next[1] / snap_step).round() * snap_step;
                    next[2] = (next[2].max(0.0) / snap_step).round() * snap_step;
                } else {
                    next[2] = next[2].max(0.0);
                }

                obj.position = next;
                if first_cursor.is_none() {
                    first_cursor = Some(next);
                }
            }
        }

        if let Some(next_position) = first_cursor {
            let bounds = self.ui.bounds as f32;
            self.ui.cursor = [
                next_position[0].clamp(-bounds, bounds),
                next_position[1].clamp(-bounds, bounds),
                next_position[2].max(0.0),
            ];
        }

        true
    }

    pub(crate) fn begin_block_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if self.ui.mode != EditorMode::Select {
            return false;
        }

        let selected_indices = self.selected_indices_normalized();
        if selected_indices.is_empty() {
            return false;
        }

        let Some(pick) = self.pick_from_screen(x, y, viewport_size) else {
            return false;
        };

        if pick
            .hit_block_index
            .is_some_and(|i| self.selection_contains(i))
        {
            let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
                return false;
            };
            let center = Vec3::new(
                bounds_position[0] + bounds_size[0] * 0.5,
                bounds_position[1] + bounds_size[1] * 0.5,
                bounds_position[2] + bounds_size[2] * 0.5,
            );
            let Some(center_screen) = self.world_to_screen_v(center, viewport_size) else {
                return false;
            };

            let mut start_blocks = Vec::with_capacity(selected_indices.len());
            for index in selected_indices {
                if let Some(obj) = self.objects.get(index) {
                    start_blocks.push(EditorDragBlockStart {
                        index,
                        position: obj.position,
                        size: obj.size,
                    });
                }
            }

            self.runtime.interaction.block_drag = Some(EditorBlockDrag {
                start_mouse: [x, y],
                start_center_screen: [center_screen.x, center_screen.y],
                start_center_world: [center.x, center.y, center.z],
                start_blocks,
            });
            return true;
        }

        false
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
                rebuild_block_mesh: true,
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
        self.selected_mask_cache = None;
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
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: matches!(changed, EditorInteractionChange::Cursor),
            ..EditorDirtyFlags::default()
        });

        changed
    }
}

impl State {
    pub(crate) fn drag_editor_selection_from_screen(&mut self, x: f64, y: f64) -> bool {
        let viewport = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .drag_selection_from_screen(x, y, viewport, self.phase)
    }

    pub(crate) fn begin_editor_selected_block_drag(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .begin_selected_block_drag_ext(x, y, viewport_size, self.phase)
    }

    pub(crate) fn select_editor_block_from_screen(&mut self, x: f64, y: f64) {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        self.editor
            .select_block_from_screen(x, y, viewport_size, self.phase);
    }
}
