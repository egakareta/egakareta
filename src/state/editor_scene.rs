use super::*;

impl State {
    pub(super) fn tap_indicator_position_from_world(&self, position: [f32; 3]) -> [f32; 3] {
        let step = if self.editor_snap_to_grid {
            self.editor_snap_step.max(0.05)
        } else {
            1.0
        };
        [
            ((position[0] - 0.5) / step).round() * step,
            ((position[1] - 0.5) / step).round() * step,
            (position[2] / step).round() * step,
        ]
    }

    pub(super) fn invalidate_editor_timeline_samples(&mut self) {
        self.editor_timeline_samples_dirty = true;
        self.editor_timeline_samples_rebuild_from_seconds = None;
    }

    pub(super) fn invalidate_editor_timeline_samples_from(&mut self, from_seconds: f32) {
        self.editor_timeline_samples_dirty = true;
        let clamped = from_seconds.max(0.0);
        self.editor_timeline_samples_rebuild_from_seconds = Some(
            self.editor_timeline_samples_rebuild_from_seconds
                .map_or(clamped, |existing| existing.min(clamped)),
        );
    }

    pub(super) fn ensure_editor_timeline_samples(&mut self) {
        if !self.editor_timeline_samples_dirty {
            return;
        }
        let perf_started_at = PlatformInstant::now();

        let duration = self.editor_timeline_duration_seconds.max(0.0);
        if duration <= 0.0 {
            self.editor_timeline_samples.clear();
            self.editor_timeline_samples.push(EditorTimelineSample {
                time_seconds: 0.0,
                position: self.editor_spawn.position,
            });
            self.editor_timeline_samples_dirty = false;
            self.editor_timeline_samples_rebuild_from_seconds = None;
            return;
        }

        let sample_count = ((duration * 24.0).clamp(120.0, 1024.0)) as usize;
        let time_step = (duration / sample_count as f32).max(1e-4);
        let simulation_dt = time_step.clamp(1.0 / 120.0, 1.0 / 45.0);

        let expected_len = sample_count + 1;
        let last_time_matches_duration = self
            .editor_timeline_samples
            .last()
            .is_some_and(|sample| (sample.time_seconds - duration).abs() <= 1e-3);

        let can_incremental_rebuild = self.editor_timeline_samples_rebuild_from_seconds.is_some()
            && self.editor_timeline_samples.len() == expected_len
            && last_time_matches_duration;

        if !can_incremental_rebuild {
            self.editor_timeline_samples.clear();
        }

        let mut runtime = TimelineSimulationRuntime::new_with_dt(
            self.editor_spawn.position,
            self.editor_spawn.direction,
            &self.editor_objects,
            &self.editor_tap_times,
            simulation_dt,
        );

        let rebuild_from_index = if can_incremental_rebuild {
            let rebuild_from_time = self
                .editor_timeline_samples_rebuild_from_seconds
                .unwrap_or(0.0)
                .clamp(0.0, duration);

            self.editor_timeline_samples
                .iter()
                .position(|sample| sample.time_seconds >= rebuild_from_time)
                .unwrap_or(sample_count)
        } else {
            0
        };

        if rebuild_from_index > 0 {
            let rebuild_start_time = self.editor_timeline_samples[rebuild_from_index].time_seconds;
            runtime.advance_to(rebuild_start_time);
            self.editor_timeline_samples.truncate(rebuild_from_index);
        }

        for index in rebuild_from_index..=sample_count {
            let t = (index as f32 * time_step).min(duration);
            runtime.advance_to(t);
            let snapshot = runtime.snapshot();
            if index < self.editor_timeline_samples.len() {
                self.editor_timeline_samples[index] = EditorTimelineSample {
                    time_seconds: t,
                    position: snapshot.position,
                };
            } else {
                self.editor_timeline_samples.push(EditorTimelineSample {
                    time_seconds: t,
                    position: snapshot.position,
                });
            }
        }

        self.editor_timeline_samples_dirty = false;
        self.editor_timeline_samples_rebuild_from_seconds = None;
        self.perf_record(PerfStage::TimelineSampleRebuild, perf_started_at);
    }

    pub(super) fn nearest_editor_timeline_sample_time_for_target(
        &self,
        target: [f32; 3],
    ) -> Option<f32> {
        self.editor_timeline_samples
            .iter()
            .min_by(|a, b| {
                let distance_sq = |sample: &EditorTimelineSample| {
                    let dx = sample.position[0] - target[0];
                    let dy = sample.position[1] - target[1];
                    let dz = sample.position[2] - target[2];
                    dx * dx + dy * dy + dz * dz
                };

                f32::total_cmp(&distance_sq(a), &distance_sq(b))
            })
            .map(|sample| sample.time_seconds)
    }

    pub(super) fn mark_editor_dirty(&mut self, dirty: EditorDirtyFlags) {
        self.editor_dirty.merge(dirty);
    }

    pub(super) fn process_editor_dirty(&mut self) {
        let dirty = self.editor_dirty;
        if !dirty.any() {
            return;
        }

        self.editor_dirty = EditorDirtyFlags::default();

        if dirty.sync_game_objects {
            self.game.objects = self.editor_objects.clone();
        }

        if dirty.rebuild_block_mesh {
            self.rebuild_block_vertices();
        }

        if dirty.rebuild_selection_overlays {
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_hover_outline_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }

        if dirty.rebuild_tap_indicators {
            self.rebuild_tap_indicator_vertices();
        }

        if dirty.rebuild_preview_player {
            self.rebuild_editor_preview_player_vertices();
        }
    }

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
        self.invalidate_editor_timeline_samples();
        self.mark_editor_dirty(EditorDirtyFlags::from_object_sync());
    }

    pub(super) fn sync_editor_objects_for_drag(&mut self) {
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
        self.invalidate_editor_timeline_samples();
        self.mark_editor_dirty(EditorDirtyFlags {
            sync_game_objects: true,
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(super) fn topmost_block_index_at_cursor(&self, cursor: [f32; 3]) -> Option<usize> {
        let mut top_index: Option<usize> = None;
        let mut top_height = f32::NEG_INFINITY;

        for (index, obj) in self.editor_objects.iter().enumerate() {
            let occupies_x = cursor[0] + 0.5 >= obj.position[0]
                && cursor[0] + 0.5 <= obj.position[0] + obj.size[0];
            let occupies_y = cursor[1] + 0.5 >= obj.position[1]
                && cursor[1] + 0.5 <= obj.position[1] + obj.size[1];
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
        self.editor_timeline_preview_position = position;
        self.editor_timeline_preview_direction = direction;

        let bounds = self.editor.bounds as f32;
        if !self.editor_timeline_playing {
            self.editor.cursor = [
                position[0].round(),
                position[1].round(),
                position[2].round(),
            ];
            self.editor.cursor[0] = self.editor.cursor[0].clamp(-bounds, bounds);
            self.editor.cursor[1] = self.editor.cursor[1].clamp(-bounds, bounds);
            self.editor.cursor[2] = self.editor.cursor[2].max(0.0);

            self.rebuild_editor_cursor_vertices();
        }

        let max_pan = bounds;
        self.editor_camera_pan[0] = (position[0] + 0.5).clamp(-max_pan, max_pan);
        self.editor_camera_pan[1] = (position[1] + 0.5).clamp(-max_pan, max_pan);

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
        self.meshes.editor_cursor.replace_with_vertices(
            &self.gpu.device,
            "Editor Cursor Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_hover_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.meshes.editor_hover_outline.clear();
            return;
        }

        let Some(index) = self
            .editor_hovered_block_index
            .filter(|index| *index < self.editor_objects.len())
        else {
            self.meshes.editor_hover_outline.clear();
            return;
        };

        if self.selection_contains(index) {
            self.meshes.editor_hover_outline.clear();
            return;
        }

        let obj = &self.editor_objects[index];
        let vertices = build_editor_hover_outline_vertices(obj.position, obj.size);
        self.meshes.editor_hover_outline.replace_with_vertices(
            &self.gpu.device,
            "Editor Hover Outline Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_gizmo_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.meshes.editor_gizmo.clear();
            return;
        }

        let Some((bounds_position, bounds_size)) = self.selected_group_bounds() else {
            self.meshes.editor_gizmo.clear();
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
        self.meshes.editor_gizmo.replace_with_vertices(
            &self.gpu.device,
            "Editor Gizmo Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_editor_selection_outline_vertices(&mut self) {
        if self.phase != AppPhase::Editor || self.editor_mode != EditorMode::Select {
            self.meshes.editor_selection_outline.clear();
            return;
        }

        let selected_indices = self.selected_block_indices_normalized();
        if selected_indices.is_empty() {
            self.meshes.editor_selection_outline.clear();
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
        self.meshes.editor_selection_outline.replace_with_vertices(
            &self.gpu.device,
            "Editor Selection Outline Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_spawn_marker_vertices(&mut self) {
        let vertices = build_spawn_marker_vertices(
            self.editor_spawn.position,
            matches!(self.editor_spawn.direction, SpawnDirection::Right),
        );
        self.meshes.spawn_marker.replace_with_vertices(
            &self.gpu.device,
            "Spawn Marker Vertex Buffer",
            &vertices,
        );
    }

    pub(super) fn rebuild_block_vertices(&mut self) {
        let perf_started_at = PlatformInstant::now();
        let vertices = build_block_vertices(&self.game.objects);

        self.meshes.blocks.replace_with_vertices(
            &self.gpu.device,
            "Block Vertex Buffer",
            &vertices,
        );
        self.perf_record(PerfStage::BlockMeshRebuild, perf_started_at);
    }

    pub(super) fn rebuild_tap_indicator_vertices(&mut self) {
        let perf_started_at = PlatformInstant::now();
        if self.phase != AppPhase::Editor {
            self.meshes.tap_indicators.clear();
            self.perf_record(PerfStage::TapIndicatorMeshRebuild, perf_started_at);
            return;
        }

        let mut positions = self.editor_tap_indicator_positions.clone();
        positions.sort_unstable_by(|a, b| {
            a[0].total_cmp(&b[0])
                .then(a[1].total_cmp(&b[1]))
                .then(a[2].total_cmp(&b[2]))
        });
        positions.dedup();

        let vertices = build_tap_indicator_vertices(&positions);
        self.meshes.tap_indicators.replace_with_vertices(
            &self.gpu.device,
            "Tap Indicator Vertex Buffer",
            &vertices,
        );
        self.perf_record(PerfStage::TapIndicatorMeshRebuild, perf_started_at);
    }

    pub(super) fn rebuild_editor_preview_player_vertices(&mut self) {
        if self.phase != AppPhase::Editor {
            self.meshes.editor_preview_player.clear();
            return;
        }

        let (position, direction) =
            self.editor_timeline_position(self.editor_timeline_time_seconds);
        self.rebuild_editor_preview_player_vertices_for_state(position, direction);
    }

    pub(super) fn rebuild_editor_preview_player_vertices_for_state(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
    ) {
        self.editor_timeline_preview_position = position;
        self.editor_timeline_preview_direction = direction;

        let is_tapping = self
            .editor_tap_times
            .iter()
            .any(|tap| (tap - self.editor_timeline_time_seconds).abs() <= 0.01);
        let preview_origin = [position[0] - 0.5, position[1] - 0.5, position[2]];
        let vertices = build_editor_preview_player_vertices(preview_origin, direction, is_tapping);
        self.meshes.editor_preview_player.replace_with_vertices(
            &self.gpu.device,
            "Editor Preview Player Vertex Buffer",
            &vertices,
        );
    }
}
