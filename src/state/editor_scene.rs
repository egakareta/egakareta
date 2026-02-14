use super::*;

impl State {
    pub(super) fn place_editor_block(&mut self) {
        self.record_editor_history_state();
        self.editor_objects.push(create_block_at_cursor(
            self.editor.cursor,
            &self.editor_selected_block_id,
        ));
        self.editor_selected_block_index = None;
        self.editor_selected_block_indices.clear();
        self.editor_hovered_block_index = None;
        self.sync_editor_objects();
        self.rebuild_editor_cursor_vertices();
    }

    pub(super) fn sync_editor_objects(&mut self) {
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.editor_selected_block_index {
            if index >= self.editor_objects.len() {
                self.editor_selected_block_index = None;
            }
        }
        self.editor_selected_block_indices
            .retain(|index| *index < self.editor_objects.len());
        self.sync_primary_selection_from_indices();
        if let Some(index) = self.editor_hovered_block_index {
            if index >= self.editor_objects.len() {
                self.editor_hovered_block_index = None;
            }
        }
        self.game.objects = self.editor_objects.clone();
        self.rebuild_block_vertices();
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
        self.rebuild_tap_indicator_vertices();
        self.rebuild_editor_preview_player_vertices();
    }

    pub(super) fn topmost_block_index_at_cursor(&self, cursor: [i32; 3]) -> Option<usize> {
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.editor_objects.iter().enumerate() {
            let occupies_x = cursor[0] as f32 + 0.5 >= obj.position[0]
                && cursor[0] as f32 + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_y = cursor[1] as f32 + 0.5 >= obj.position[1]
                && cursor[1] as f32 + 0.5 <= obj.position[1] + obj.size[1];
            if occupies_x && occupies_y {
                let top = obj.position[2] + obj.size[2];
                if top > top_height {
                    top_height = top;
                    top_index = Some(index);
                }
            }
        }

        top_index
    }

    pub(super) fn apply_spawn_to_game(&mut self, position: [f32; 3], direction: SpawnDirection) {
        self.game.apply_spawn(position, direction);
    }

    pub(super) fn editor_timeline_position(&self, time_seconds: f32) -> ([f32; 3], SpawnDirection) {
        derive_timeline_position(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_tap_times,
            time_seconds,
            &self.editor_objects,
        )
    }

    pub(super) fn editor_timeline_elapsed_seconds(&self, time_seconds: f32) -> f32 {
        derive_timeline_elapsed_seconds(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_tap_times,
            time_seconds,
            &self.editor_objects,
        )
    }

    pub(super) fn apply_editor_timeline_preview_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        let bounds = self.editor.bounds;
        self.editor.cursor = [
            position[0].round() as i32,
            position[1].round() as i32,
            position[2].round() as i32,
        ];
        self.editor.cursor[0] = self.editor.cursor[0].clamp(-bounds, bounds);
        self.editor.cursor[1] = self.editor.cursor[1].clamp(-bounds, bounds);
        self.editor.cursor[2] = self.editor.cursor[2].max(0);

        let max_pan = bounds as f32;
        self.editor_camera_pan[0] = (position[0] + 0.5).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] = (position[1] + 0.5).clamp(-max_pan, max_pan);

        self.rebuild_editor_cursor_vertices();
        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    pub(super) fn refresh_editor_timeline_position(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let (position, direction) =
            self.editor_timeline_position(self.editor_timeline_time_seconds);
        self.apply_editor_timeline_preview_state(position, direction);
    }

    pub(super) fn rebuild_editor_cursor_vertices(&mut self) {
        let vertices = build_editor_cursor_vertices(self.editor.cursor);
        self.editor_cursor_mesh.replace_with_vertices(
            &self.device,
            "Editor Cursor Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_hover_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_hover_outline_mesh.clear();
            return;
        }

        let Some(index) = self
            .editor_hovered_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            self.editor_hover_outline_mesh.clear();
            return;
        };

        if self.selection_contains(index) {
            self.editor_hover_outline_mesh.clear();
            return;
        }

        let obj = &self.editor_objects[index];
        let vertices = build_editor_hover_outline_vertices(obj.position, obj.size);
        self.editor_hover_outline_mesh.replace_with_vertices(
            &self.device,
            "Editor Hover Outline Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_gizmo_mesh.clear();
            return;
        }

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            self.editor_gizmo_mesh.clear();
            return;
        };

        let center = Vec3::new(
            bounds_position[0] + bounds_size[0] * 0.5,
            bounds_position[1] + bounds_size[1] * 0.5,
            bounds_position[2] + bounds_size[2] * 0.5,
        );
        let axis_lengths = self.editor_gizmo_axis_lengths_world(center, 50.0);
        let axis_width = self.editor_gizmo_axis_width_world(center, 3.0);

        let active_part = if let Some(drag) = &self.editor_gizmo_drag {
            match (drag.axis, drag.kind) {
                (GizmoAxis::X, GizmoDragKind::Move) => Some(GizmoPart::MoveX),
                (GizmoAxis::Y, GizmoDragKind::Move) => Some(GizmoPart::MoveY),
                (GizmoAxis::Z, GizmoDragKind::Move) => Some(GizmoPart::MoveZ),
                (GizmoAxis::XNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveXNeg),
                (GizmoAxis::YNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveYNeg),
                (GizmoAxis::ZNeg, GizmoDragKind::Move) => Some(GizmoPart::MoveZNeg),
                (GizmoAxis::X, GizmoDragKind::Resize) => Some(GizmoPart::ResizeX),
                (GizmoAxis::Y, GizmoDragKind::Resize) => Some(GizmoPart::ResizeY),
                (GizmoAxis::Z, GizmoDragKind::Resize) => Some(GizmoPart::ResizeZ),
                (GizmoAxis::XNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeXNeg),
                (GizmoAxis::YNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeYNeg),
                (GizmoAxis::ZNeg, GizmoDragKind::Resize) => Some(GizmoPart::ResizeZNeg),
            }
        } else {
            None
        };

        let vertices = build_editor_gizmo_vertices(
            bounds_position,
            bounds_size,
            axis_lengths,
            axis_width,
            active_part,
        );
        self.editor_gizmo_mesh.replace_with_vertices(
            &self.device,
            "Editor Gizmo Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.editor_selection_outline_mesh.clear();
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            self.editor_selection_outline_mesh.clear();
            return;
        }

        let mut vertices = Vec::new();
        for index in selected_indices {
            if let Some(obj) = self.editor_objects.get(index) {
                vertices.extend(build_editor_selection_outline_vertices(
                    obj.position,
                    obj.size,
                ));
            }
        }
        self.editor_selection_outline_mesh.replace_with_vertices(
            &self.device,
            "Editor Selection Outline Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_spawn_marker_vertices(&mut self) {
        let vertices = build_spawn_marker_vertices(
            self.editor_spawn.position,
            matches!(self.editor_spawn.direction, SpawnDirection::Right),
        );
        self.spawn_marker_mesh.replace_with_vertices(
            &self.device,
            "Spawn Marker Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_block_vertices(&mut self) {
        let vertices = build_block_vertices(&self.game.objects);

        self.block_mesh
            .replace_with_vertices(&self.device, "Block Vertex Buffer", &vertices);
    }

    pub(super) fn rebuild_tap_indicator_vertices(&mut self) {
        if self.phase != AppPhase::Editor {
            self.tap_indicator_mesh.clear();
            return;
        }

        let mut positions = Vec::new();
        for &time_seconds in &self.editor_tap_times {
            let (pos, _) = derive_timeline_position(
                self.editor_spawn.position,
                self.editor_spawn.direction,
                &self.editor_tap_times,
                time_seconds,
                &self.editor_objects,
            );
            positions.push([
                (pos[0] - 0.5).round() as i32,
                (pos[1] - 0.5).round() as i32,
                pos[2].round() as i32,
            ]);
        }

        positions.sort_unstable();
        positions.dedup();

        let vertices = build_tap_indicator_vertices(&positions);
        self.tap_indicator_mesh.replace_with_vertices(
            &self.device,
            "Tap Indicator Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_preview_player_vertices(&mut self) {
        if self.phase != AppPhase::Editor {
            self.editor_preview_player_mesh.clear();
            return;
        }

        let (position, direction) =
            self.editor_timeline_position(self.editor_timeline_time_seconds);
        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    fn rebuild_editor_preview_player_vertices_for_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        let is_tapping = self
            .editor_tap_times
            .iter()
            .any(|tap| (tap - self.editor_timeline_time_seconds).abs() <= 0.01);
        let preview_origin = [position[0] - 0.5, position[1] - 0.5, position[2]];
        let vertices = build_editor_preview_player_vertices(preview_origin, direction, is_tapping);
        self.editor_preview_player_mesh.replace_with_vertices(
            &self.device,
            "Editor Preview Player Vertex Buffer",
            &vertices,
        );
    }
}
