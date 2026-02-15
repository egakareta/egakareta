use super::*;

impl State {
    pub(crate) fn toggle_editor_perf_overlay(&mut self) {
        self.editor_perf.profiler.enabled = !self.editor_perf.profiler.enabled;
    }

    pub(crate) fn editor_perf_overlay_enabled(&self) -> bool {
        self.editor_perf.profiler.enabled
    }

    pub(crate) fn editor_perf_overlay_lines(&self) -> Vec<String> {
        if !self.editor_perf.profiler.enabled {
            return Vec::new();
        }

        let mut lines = Vec::new();
        lines.push("Perf Overlay (Ctrl+Shift+Alt+F12)".to_string());
        lines.push(format!(
            "Spikes(>16.7ms): {} | Last Spike: {}",
            self.editor_perf.profiler.frame_spike_count,
            self.editor_perf
                .profiler
                .last_spike_stage
                .map(|stage| stage.name())
                .unwrap_or("none")
        ));

        for stage in [
            PerfStage::FrameTotal,
            PerfStage::TimelinePlayback,
            PerfStage::DragSelection,
            PerfStage::GizmoRebuild,
            PerfStage::DirtyProcess,
            PerfStage::TimelineSampleRebuild,
            PerfStage::TapIndicatorMeshRebuild,
            PerfStage::BlockMeshRebuild,
            PerfStage::TTapToggleTotal,
            PerfStage::TTapSolve,
        ] {
            let stat = self.editor_perf.profiler.stats[stage.as_index()];
            lines.push(format!(
                "{:<18} last {:>6.2}ms | avg {:>6.2}ms | max {:>6.2}ms | n {}",
                stage.name(),
                stat.last_ms,
                stat.ema_ms,
                stat.max_ms,
                stat.calls
            ));
        }

        lines
    }

    pub(super) fn perf_record(&mut self, stage: PerfStage, started_at: PlatformInstant) {
        let elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
        self.editor_perf.profiler.observe(stage, elapsed_ms);
    }

    pub fn update(&mut self) {
        self.editor_perf.profiler.begin_frame();
        self.update_audio_imports();
        self.update_waveform_loading();
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = PlatformInstant::now();
        let frame_dt = (now - self.editor_frame.last_frame).as_secs_f32();
        self.editor_frame.last_frame = now;
        let frame_dt_ms = frame_dt * 1000.0;
        let instant_fps = 1.0 / frame_dt.max(1e-4);
        if self.editor_perf.fps_smoothed <= 0.0 {
            self.editor_perf.fps_smoothed = instant_fps;
        } else {
            self.editor_perf.fps_smoothed = self.editor_perf.fps_smoothed * 0.9 + instant_fps * 0.1;
        }
        self.editor_frame.accumulator = (self.editor_frame.accumulator + frame_dt).min(0.25);

        if self.phase == AppPhase::Menu {
            self.editor_frame.accumulator = 0.0;
            self.update_menu_camera();
            self.editor_perf
                .profiler
                .observe(PerfStage::FrameTotal, frame_dt_ms);
            if frame_dt_ms > 16.7 {
                self.editor_perf.profiler.frame_spike_count += 1;
                self.editor_perf.profiler.last_spike_stage = self
                    .editor_perf
                    .profiler
                    .dominant_stage_this_frame()
                    .or(Some(PerfStage::FrameTotal));
            }
            return;
        }

        if self.phase == AppPhase::Editor {
            self.editor_frame.accumulator = 0.0;
            self.meshes.trail.clear();

            if self.editor_timeline.playback.playing {
                let timeline_playback_started_at = PlatformInstant::now();
                let audio_time = self
                    .audio_state
                    .runtime
                    .playback_time_seconds()
                    .unwrap_or(self.editor_timeline.clock.time_seconds);
                let clamped_time = audio_time.min(self.editor_timeline.clock.duration_seconds);

                if (clamped_time - self.editor_timeline.clock.time_seconds).abs() > 1e-4 {
                    self.editor_timeline.clock.time_seconds = clamped_time;

                    let mut applied_runtime_state = false;
                    if let Some(runtime) = self.editor_timeline.playback.runtime.as_mut() {
                        if clamped_time + 1e-6 >= runtime.elapsed_seconds() {
                            runtime.advance_to(clamped_time);
                            let snapshot = runtime.snapshot();
                            self.apply_editor_timeline_preview_state(
                                snapshot.position,
                                snapshot.direction,
                            );
                            applied_runtime_state = true;
                        }
                    }

                    if !applied_runtime_state {
                        let mut runtime = TimelineSimulationRuntime::new(
                            self.editor_spawn.position,
                            self.editor_spawn.direction,
                            &self.editor_objects,
                            &self.editor_timeline.taps.tap_times,
                        );
                        runtime.advance_to(clamped_time);
                        let snapshot = runtime.snapshot();
                        self.apply_editor_timeline_preview_state(
                            snapshot.position,
                            snapshot.direction,
                        );
                        self.editor_timeline.playback.runtime = Some(runtime);
                    }
                }

                if clamped_time >= self.editor_timeline.clock.duration_seconds
                    || !self.audio_state.runtime.is_playing()
                {
                    self.editor_timeline.playback.playing = false;
                    self.editor_timeline.playback.runtime = None;
                    self.stop_audio();
                    self.refresh_editor_timeline_position();
                }
                self.perf_record(PerfStage::TimelinePlayback, timeline_playback_started_at);
            }

            self.update_editor_pan_from_keys(frame_dt);
            if self.editor_runtime.interaction.gizmo_drag.is_some()
                || self.editor_runtime.interaction.block_drag.is_some()
            {
                if let Some(pointer) = self.editor.pointer_screen {
                    let drag_started_at = PlatformInstant::now();
                    self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
                    self.perf_record(PerfStage::DragSelection, drag_started_at);
                }
            }

            let camera_changed = (self.editor_camera.editor_pan[0]
                - self.editor_runtime.gizmo.last_pan[0])
                .abs()
                > 1e-4
                || (self.editor_camera.editor_pan[1] - self.editor_runtime.gizmo.last_pan[1]).abs()
                    > 1e-4
                || (self.editor_camera.editor_rotation - self.editor_runtime.gizmo.last_rotation)
                    .abs()
                    > 1e-4
                || (self.editor_camera.editor_pitch - self.editor_runtime.gizmo.last_pitch).abs()
                    > 1e-4
                || (self.editor_camera.editor_zoom - self.editor_runtime.gizmo.last_zoom).abs()
                    > 1e-4;

            let has_selection = self.editor.selected_block_index.is_some()
                || !self.editor.selected_block_indices.is_empty();
            let is_dragging = self.editor_runtime.interaction.gizmo_drag.is_some()
                || self.editor_runtime.interaction.block_drag.is_some();

            if has_selection && self.editor.mode == EditorMode::Select {
                if is_dragging {
                    let gizmo_started_at = PlatformInstant::now();
                    self.rebuild_editor_gizmo_vertices();
                    self.perf_record(PerfStage::GizmoRebuild, gizmo_started_at);
                    self.editor_runtime.gizmo.rebuild_accumulator = 0.0;
                } else if camera_changed {
                    self.editor_runtime.gizmo.rebuild_accumulator += frame_dt;
                    if self.editor_runtime.gizmo.rebuild_accumulator >= (1.0 / 24.0) {
                        let gizmo_started_at = PlatformInstant::now();
                        self.rebuild_editor_gizmo_vertices();
                        self.perf_record(PerfStage::GizmoRebuild, gizmo_started_at);
                        self.editor_runtime.gizmo.rebuild_accumulator = 0.0;
                    }
                } else {
                    self.editor_runtime.gizmo.rebuild_accumulator = 0.0;
                }
            } else {
                self.editor_runtime.gizmo.rebuild_accumulator = 0.0;
            }

            self.editor_runtime.gizmo.last_pan = self.editor_camera.editor_pan;
            self.editor_runtime.gizmo.last_rotation = self.editor_camera.editor_rotation;
            self.editor_runtime.gizmo.last_pitch = self.editor_camera.editor_pitch;
            self.editor_runtime.gizmo.last_zoom = self.editor_camera.editor_zoom;
            let dirty_started_at = PlatformInstant::now();
            self.process_editor_dirty();
            self.perf_record(PerfStage::DirtyProcess, dirty_started_at);
            self.update_editor_camera();

            self.editor_perf
                .profiler
                .observe(PerfStage::FrameTotal, frame_dt_ms);
            if frame_dt_ms > 16.7 {
                self.editor_perf.profiler.frame_spike_count += 1;
                self.editor_perf.profiler.last_spike_stage = self
                    .editor_perf
                    .profiler
                    .dominant_stage_this_frame()
                    .or(Some(PerfStage::FrameTotal));
            }
            return;
        }

        while self.editor_frame.accumulator >= FIXED_DT {
            self.game.update(FIXED_DT);
            self.editor_frame.accumulator -= FIXED_DT;
        }

        if self.game.game_over {
            self.stop_audio();
        }

        let mut trail_vertices = Vec::new();
        let player_pos = self.game.position;
        // Cull segments that are too far from the player to save on vertices.
        const CULL_DISTANCE_SQ: f32 = 120.0 * 120.0;

        for (segment_index, segment) in self.game.trail_segments.iter().enumerate() {
            if segment.is_empty() {
                continue;
            }

            let is_last_segment = segment_index + 1 == self.game.trail_segments.len();

            // For older segments, check if they are still potentially visible.
            if !is_last_segment {
                let last_point = segment.last().unwrap();
                let dx = last_point[0] - player_pos[0];
                let dy = last_point[1] - player_pos[1];
                let dz = last_point[2] - player_pos[2];
                if dx * dx + dy * dy + dz * dz > CULL_DISTANCE_SQ {
                    continue;
                }
            }

            if is_last_segment && self.game.is_grounded {
                let mut points = segment.clone();
                points.push(self.game.position);
                trail_vertices.extend(build_trail_vertices(&points, self.game.game_over));
            } else {
                trail_vertices.extend(build_trail_vertices(segment, self.game.game_over));
            }
        }

        if !self.game.is_grounded {
            let head_length = 0.22;
            let dir = match self.game.direction {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                self.game.position[0] - dir[0] * head_length,
                self.game.position[1] - dir[1] * head_length,
                self.game.position[2],
            ];
            let head_points = [head_start, self.game.position];
            trail_vertices.extend(build_trail_vertices(&head_points, self.game.game_over));
        }

        self.meshes
            .trail
            .write_streaming_vertices(&self.gpu.queue, &trail_vertices);

        self.player_render.line_uniform.offset = [
            (self.game.position[0] * 100.0).round() / 100.0,
            (self.game.position[1] * 100.0).round() / 100.0,
        ];
        self.player_render.line_uniform.rotation = match self.game.direction {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        self.gpu.queue.write_buffer(
            &self.gpu.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.player_render.line_uniform),
        );

        let aspect = self.gpu.config.width as f32 / self.gpu.config.height as f32;
        let pos_3d = Vec3::new(
            self.game.position[0],
            self.game.position[1],
            self.game.position[2],
        );
        let target = pos_3d;
        let offset = self.playing_camera_offset();
        let eye = pos_3d + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.gpu.queue.write_buffer(
            &self.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );

        if self.phase == AppPhase::Playing {
            self.editor_perf
                .profiler
                .observe(PerfStage::FrameTotal, frame_dt_ms);
            if frame_dt_ms > 16.7 {
                self.editor_perf.profiler.frame_spike_count += 1;
                self.editor_perf.profiler.last_spike_stage = self
                    .editor_perf
                    .profiler
                    .dominant_stage_this_frame()
                    .or(Some(PerfStage::FrameTotal));
            }
        }
    }

    fn update_menu_camera(&mut self) {
        let aspect = self.gpu.config.width as f32 / self.gpu.config.height as f32;
        let radius = 25.0;
        let angle = -25.0f32.to_radians();
        let eye = Vec3::new(radius * angle.cos(), radius * angle.sin(), 15.0);
        let target = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.gpu.queue.write_buffer(
            &self.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    fn update_editor_camera(&mut self) {
        let aspect = self.gpu.config.width as f32 / self.gpu.config.height as f32;
        let target = Vec3::new(
            self.editor_camera.editor_pan[0],
            self.editor_camera.editor_pan[1],
            0.0,
        );
        let offset = self.editor_camera_offset();
        let eye = target + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.gpu.queue.write_buffer(
            &self.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }
}
