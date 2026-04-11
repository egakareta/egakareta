/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::super::{
    EditorBlockDrag, EditorDirtyFlags, EditorDragBlockStart, EditorSubsystem, PerfStage, State,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::{AppPhase, EditorInteractionChange, EditorMode};
use glam::{EulerRot, Mat3, Vec2, Vec3};

const MARQUEE_DRAG_THRESHOLD_PX: f64 = 4.0;
const CAMERA_TRIGGER_MARQUEE_RADIUS_PX: f32 = 16.0;

impl EditorSubsystem {
    fn marquee_has_dragged_far_enough(&self) -> bool {
        let Some(start) = self.ui.marquee_start_screen else {
            return false;
        };
        let current = self.ui.marquee_current_screen.unwrap_or(start);
        let dx = current[0] - start[0];
        let dy = current[1] - start[1];
        (dx * dx + dy * dy) >= MARQUEE_DRAG_THRESHOLD_PX * MARQUEE_DRAG_THRESHOLD_PX
    }

    pub(crate) fn marquee_selection_rect_screen(&self) -> Option<([f64; 2], [f64; 2], bool)> {
        let start = self.ui.marquee_start_screen?;
        let current = self.ui.marquee_current_screen.unwrap_or(start);
        Some((start, current, self.marquee_has_dragged_far_enough()))
    }

    pub(crate) fn begin_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        let allows_marquee =
            self.ui.mode.is_selection_mode() || self.ui.mode == EditorMode::Trigger;
        if phase != AppPhase::Editor || self.ui.right_dragging || !allows_marquee {
            return false;
        }
        self.ui.marquee_start_screen = Some([x, y]);
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    pub(crate) fn update_marquee_selection(&mut self, x: f64, y: f64, phase: AppPhase) -> bool {
        let allows_marquee =
            self.ui.mode.is_selection_mode() || self.ui.mode == EditorMode::Trigger;
        if phase != AppPhase::Editor || self.ui.right_dragging || !allows_marquee {
            return false;
        }
        if self.ui.marquee_start_screen.is_none() {
            return false;
        }
        self.ui.marquee_current_screen = Some([x, y]);
        true
    }

    pub(crate) fn marquee_overlapping_blocks(&self, viewport: Vec2) -> Vec<usize> {
        let Some(start) = self.ui.marquee_start_screen else {
            return Vec::new();
        };
        let current = self.ui.marquee_current_screen.unwrap_or(start);

        let rect_min = Vec2::new(
            start[0].min(current[0]) as f32,
            start[1].min(current[1]) as f32,
        );
        let rect_max = Vec2::new(
            start[0].max(current[0]) as f32,
            start[1].max(current[1]) as f32,
        );

        let mut hits = Vec::new();
        for index in 0..self.objects.len() {
            let Some((obj_min, obj_max)) = self.rotated_block_screen_bounds(index, viewport) else {
                continue;
            };
            let overlaps = obj_max.x >= rect_min.x
                && obj_min.x <= rect_max.x
                && obj_max.y >= rect_min.y
                && obj_min.y <= rect_max.y;
            if overlaps {
                hits.push(index);
            }
        }
        hits
    }

    fn rotated_block_screen_bounds(&self, index: usize, viewport: Vec2) -> Option<(Vec2, Vec2)> {
        let obj = self.objects.get(index)?;

        let center = Vec3::new(
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        );
        let half = Vec3::new(obj.size[0] * 0.5, obj.size[1] * 0.5, obj.size[2] * 0.5);
        let rotation = Mat3::from_euler(
            EulerRot::XYZ,
            obj.rotation_degrees[0].to_radians(),
            obj.rotation_degrees[1].to_radians(),
            obj.rotation_degrees[2].to_radians(),
        );

        let mut points = Vec::with_capacity(10);
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let local = Vec3::new(half.x * sx, half.y * sy, half.z * sz);
                    let world = center + rotation * local;
                    if let Some(screen) = self.world_to_screen_v(world, viewport) {
                        points.push(screen);
                    }
                }
            }
        }

        if let Some(center_screen) = self.world_to_screen_v(center, viewport) {
            points.push(center_screen);
        }

        if points.is_empty() {
            return None;
        }

        let mut min = points[0];
        let mut max = points[0];
        for p in points.into_iter().skip(1) {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }

        Some((min, max))
    }

    fn marquee_trigger_hit(&self, rect_min: Vec2, rect_max: Vec2, viewport: Vec2) -> Option<usize> {
        let rect_center = (rect_min + rect_max) * 0.5;
        let mut best_hit: Option<(usize, f32)> = None;

        for (trigger_index, camera_trigger) in self.camera_trigger_markers() {
            let eye = self.camera_trigger_marker_eye(&camera_trigger);
            let Some(screen) = self.world_to_screen_v(eye, viewport) else {
                continue;
            };

            let overlaps = screen.x + CAMERA_TRIGGER_MARQUEE_RADIUS_PX >= rect_min.x
                && screen.x - CAMERA_TRIGGER_MARQUEE_RADIUS_PX <= rect_max.x
                && screen.y + CAMERA_TRIGGER_MARQUEE_RADIUS_PX >= rect_min.y
                && screen.y - CAMERA_TRIGGER_MARQUEE_RADIUS_PX <= rect_max.y;

            if !overlaps {
                continue;
            }

            let distance_to_center = screen.distance(rect_center);
            match best_hit {
                Some((_, best_distance)) if distance_to_center >= best_distance => {}
                _ => best_hit = Some((trigger_index, distance_to_center)),
            }
        }

        best_hit.map(|(index, _)| index)
    }

    fn apply_marquee_selection(&mut self, viewport: Vec2, additive: bool) {
        let Some(start) = self.ui.marquee_start_screen else {
            return;
        };
        let current = self.ui.marquee_current_screen.unwrap_or(start);

        let rect_min = Vec2::new(
            start[0].min(current[0]) as f32,
            start[1].min(current[1]) as f32,
        );
        let rect_max = Vec2::new(
            start[0].max(current[0]) as f32,
            start[1].max(current[1]) as f32,
        );

        if self.ui.mode.is_selection_mode() {
            let hits = self.marquee_overlapping_blocks(viewport);

            if additive {
                for hit in hits {
                    if !self.ui.selected_block_indices.contains(&hit) {
                        self.ui.selected_block_indices.push(hit);
                    }
                }
            } else {
                self.ui.selected_block_indices = hits;
            }
        } else {
            self.ui.selected_block_indices.clear();
            self.ui.selected_block_index = None;
            self.ui.hovered_block_index = None;
        }

        let trigger_hit = self.marquee_trigger_hit(rect_min, rect_max, viewport);
        if additive {
            if let Some(index) = trigger_hit {
                self.set_trigger_selected(Some(index));
            }
        } else {
            self.set_trigger_selected(trigger_hit);
        }

        self.sync_primary_selection_from_indices();
        self.selected_mask_cache = None;
        self.runtime.interaction.gizmo_drag = None;
        self.runtime.interaction.block_drag = None;

        self.ui.hovered_block_index = self.ui.selected_block_index;

        if let Some(index) = self.ui.selected_block_index {
            if let Some(obj) = self.objects.get(index) {
                self.ui.cursor = [obj.position[0], obj.position[1], obj.position[2]];
            }
        }

        self.mark_dirty(EditorDirtyFlags {
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: self.ui.selected_block_index.is_some(),
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn finish_marquee_selection(
        &mut self,
        x: f64,
        y: f64,
        viewport: Vec2,
        phase: AppPhase,
    ) -> bool {
        let allows_marquee =
            self.ui.mode.is_selection_mode() || self.ui.mode == EditorMode::Trigger;
        if phase != AppPhase::Editor || !allows_marquee {
            self.ui.marquee_start_screen = None;
            self.ui.marquee_current_screen = None;
            return false;
        }

        let had_marquee = self.ui.marquee_start_screen.is_some();
        if !had_marquee {
            return false;
        }

        self.ui.marquee_current_screen = Some([x, y]);
        let additive = self.ui.shift_held;

        if self.marquee_has_dragged_far_enough() {
            self.apply_marquee_selection(viewport, additive);
            self.ui.marquee_start_screen = None;
            self.ui.marquee_current_screen = None;
            return true;
        }

        self.ui.marquee_start_screen = None;
        self.ui.marquee_current_screen = None;
        false
    }

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

        let right_world = Vec3::new(camera_right_xy.x, 0.0, camera_right_xy.y);
        let up_world = Vec3::new(camera_up_xy.x, 0.0, camera_up_xy.y);

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

        let snap_enabled = self.effective_snap_to_grid();
        let snap_step = self.config.snap_step.max(0.05);

        let mut first_cursor: Option<[f32; 3]> = None;
        for block in &drag.start_blocks {
            if let Some(obj) = self.objects.get_mut(block.index) {
                let mut next = [
                    block.position[0]
                        + world_right_factor * camera_right_xy.x
                        + world_up_factor * camera_up_xy.x,
                    block.position[1],
                    block.position[2]
                        + world_right_factor * camera_right_xy.y
                        + world_up_factor * camera_up_xy.y,
                ];

                if snap_enabled {
                    next[0] = (next[0] / snap_step).round() * snap_step;
                    next[1] = (next[1].max(0.0) / snap_step).round() * snap_step;
                    next[2] = (next[2] / snap_step).round() * snap_step;
                } else {
                    next[1] = next[1].max(0.0);
                }

                obj.position = next;
                if first_cursor.is_none() {
                    first_cursor = Some(next);
                }
            }
        }

        if let Some(next_position) = first_cursor {
            self.ui.cursor = [
                next_position[0],
                next_position[1].max(0.0),
                next_position[2],
            ];
        }

        true
    }

    pub(crate) fn begin_block_drag(&mut self, x: f64, y: f64, viewport_size: Vec2) -> bool {
        if !self.ui.mode.is_selection_mode() {
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
                        rotation_degrees: obj.rotation_degrees,
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
        if phase != AppPhase::Editor || self.ui.right_dragging || !self.ui.mode.is_selection_mode()
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
        let selection_total_started_at = PlatformInstant::now();
        if phase != AppPhase::Editor || self.ui.right_dragging {
            self.perf_record(PerfStage::SelectionClick, selection_total_started_at);
            return EditorInteractionChange::None;
        }

        let additive = self.ui.shift_held;

        let pick_started_at = PlatformInstant::now();
        let Some(pick) = self.pick_from_screen(x, y, viewport) else {
            self.perf_record(PerfStage::SelectionPick, pick_started_at);
            let apply_started_at = PlatformInstant::now();
            if !additive {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
                self.ui.hovered_block_index = None;
                self.set_trigger_selected(None);
            }
            self.runtime.interaction.gizmo_drag = None;
            self.runtime.interaction.block_drag = None;
            self.perf_record(PerfStage::SelectionApply, apply_started_at);

            let mark_dirty_started_at = PlatformInstant::now();
            self.mark_dirty(EditorDirtyFlags {
                rebuild_block_mesh: true,
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
            self.perf_record(PerfStage::SelectionMarkDirty, mark_dirty_started_at);
            self.perf_record(PerfStage::SelectionClick, selection_total_started_at);
            return EditorInteractionChange::None;
        };
        self.perf_record(PerfStage::SelectionPick, pick_started_at);

        let trigger_mode = self.ui.mode == EditorMode::Trigger;
        let mut changed = EditorInteractionChange::None;
        let apply_started_at = PlatformInstant::now();

        if let Some(hit_trigger_index) = pick.hit_trigger_index {
            if additive && self.selected_trigger_index() == Some(hit_trigger_index) {
                self.set_trigger_selected(None);
            } else {
                self.set_trigger_selected(Some(hit_trigger_index));
            }

            if !additive || trigger_mode {
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
            }
            self.ui.hovered_block_index = None;
            changed = EditorInteractionChange::Hover;
        } else if let Some(hit_index) = pick.hit_block_index {
            if trigger_mode {
                if !additive {
                    self.set_trigger_selected(None);
                }
                self.ui.selected_block_indices.clear();
                self.ui.selected_block_index = None;
                self.ui.hovered_block_index = None;
                changed = EditorInteractionChange::Hover;
            } else {
                if !additive {
                    self.set_trigger_selected(None);
                }
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
            }
        } else if !additive {
            self.ui.selected_block_indices.clear();
            self.ui.selected_block_index = None;
            self.ui.hovered_block_index = None;
            self.set_trigger_selected(None);
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

        self.perf_record(PerfStage::SelectionApply, apply_started_at);

        let mark_dirty_started_at = PlatformInstant::now();
        self.mark_dirty(EditorDirtyFlags {
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: matches!(changed, EditorInteractionChange::Cursor),
            ..EditorDirtyFlags::default()
        });
        self.perf_record(PerfStage::SelectionMarkDirty, mark_dirty_started_at);
        self.perf_record(PerfStage::SelectionClick, selection_total_started_at);

        changed
    }
}

impl State {
    pub(crate) fn begin_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let handled = self.editor.begin_marquee_selection(x, y, self.phase);
        if handled {
            self.rebuild_editor_hover_outline_vertices();
        }
        handled
    }

    pub(crate) fn update_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let handled = self.editor.update_marquee_selection(x, y, self.phase);
        if handled {
            self.rebuild_editor_hover_outline_vertices();
        }
        handled
    }

    pub(crate) fn finish_editor_marquee_selection(&mut self, x: f64, y: f64) -> bool {
        let viewport_size = Vec2::new(
            self.render.gpu.config.width as f32,
            self.render.gpu.config.height as f32,
        );
        let had_marquee = self.editor.ui.marquee_start_screen.is_some();
        if !had_marquee {
            return false;
        }

        let marquee_consumed =
            self.editor
                .finish_marquee_selection(x, y, viewport_size, self.phase);
        if marquee_consumed {
            self.rebuild_editor_hover_outline_vertices();
            return true;
        }

        self.rebuild_editor_hover_outline_vertices();

        let mode = self.editor.mode();
        if self.phase == AppPhase::Editor
            && (mode.is_selection_mode() || mode == EditorMode::Trigger)
        {
            self.editor
                .select_block_from_screen(x, y, viewport_size, self.phase);
            return true;
        }

        false
    }

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
}

#[cfg(test)]
mod tests {
    use super::super::super::State;
    use crate::types::{AppPhase, EditorMode, LevelObject};
    use glam::Vec2;

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
    fn marquee_threshold_activates_only_after_minimum_drag() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.marquee_start_screen = Some([100.0, 100.0]);
            state.editor.ui.marquee_current_screen = Some([102.0, 102.0]);
            let (_, _, active) = state
                .editor
                .marquee_selection_rect_screen()
                .expect("marquee rect");
            assert!(!active);

            state.editor.ui.marquee_current_screen = Some([120.0, 120.0]);
            let (_, _, active) = state
                .editor
                .marquee_selection_rect_screen()
                .expect("marquee rect");
            assert!(active);
        });
    }

    #[test]
    fn begin_marquee_requires_editor_phase_and_supported_mode() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Timing;
            assert!(!state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Editor));

            state.editor.ui.mode = EditorMode::Select;
            assert!(!state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Menu));

            assert!(state
                .editor
                .begin_marquee_selection(50.0, 50.0, AppPhase::Editor));
            assert!(state.editor.ui.marquee_start_screen.is_some());
            assert!(state.editor.ui.marquee_current_screen.is_some());
        });
    }

    #[test]
    fn finish_marquee_clears_state_when_not_dragged_far_enough() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.ui.mode = EditorMode::Select;
            assert!(state
                .editor
                .begin_marquee_selection(100.0, 100.0, AppPhase::Editor));
            assert!(state
                .editor
                .update_marquee_selection(101.0, 101.0, AppPhase::Editor));

            let changed = state.editor.finish_marquee_selection(
                101.0,
                101.0,
                Vec2::new(1280.0, 720.0),
                AppPhase::Editor,
            );
            assert!(!changed);
            assert!(state.editor.ui.marquee_start_screen.is_none());
            assert!(state.editor.ui.marquee_current_screen.is_none());
        });
    }

    #[test]
    fn finish_marquee_with_large_rect_selects_visible_block() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Select;
            state.editor.objects = vec![test_block([0.0, 0.0, 0.0])];

            assert!(state
                .editor
                .begin_marquee_selection(0.0, 0.0, AppPhase::Editor));
            assert!(state
                .editor
                .update_marquee_selection(2000.0, 2000.0, AppPhase::Editor));

            let changed = state.editor.finish_marquee_selection(
                2000.0,
                2000.0,
                Vec2::new(1280.0, 720.0),
                AppPhase::Editor,
            );

            assert!(changed);
            assert!(state.editor.ui.selected_block_indices.contains(&0));
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
        });
    }
}
