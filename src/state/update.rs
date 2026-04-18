/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{Mat4, Vec3};

use super::{PerfFrameContributor, PerfFrameSnapshot, PerfFrameStageEntry, PerfStage, State};
use crate::game::{
    advance_simulation_time, trigger_transformed_objects_at_time, TimelineSimulationRuntime,
};
use crate::mesh::{
    build_block_vertices_with_phase, build_trail_vertices, build_trail_vertices_with_alpha,
};
use crate::platform::state_host::PlatformInstant;
use crate::types::{
    AppPhase, CameraUniform, ColorSpaceUniform, Direction, EditorMode, LevelObject,
};

impl State {
    fn recent_trail_segments<'a>(
        trail_segments: &'a [Vec<[f32; 3]>],
        max_points: usize,
    ) -> Vec<(usize, &'a [[f32; 3]])> {
        if max_points == 0 || trail_segments.is_empty() {
            return Vec::new();
        }

        let mut remaining = max_points;
        let mut selected: Vec<(usize, &'a [[f32; 3]])> = Vec::new();

        for (index, segment) in trail_segments.iter().enumerate().rev() {
            if segment.is_empty() {
                continue;
            }
            if remaining == 0 {
                break;
            }

            let take = segment.len().min(remaining);
            let start = segment.len() - take;
            selected.push((index, &segment[start..]));
            remaining -= take;
        }

        selected.reverse();
        selected
    }

    fn maybe_resync_editor_playback_from_pending_seek(&mut self, frame_dt: f32) {
        if self.phase != AppPhase::Editor || !self.editor.timeline.playback.playing {
            return;
        }

        let Some(target_time) = self.editor.timeline.playback.pending_seek_time_seconds else {
            self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
            return;
        };

        let cooldown =
            (self.editor.timeline.playback.seek_resync_cooldown_seconds - frame_dt).max(0.0);
        self.editor.timeline.playback.seek_resync_cooldown_seconds = cooldown;
        if cooldown > 0.0 {
            return;
        };

        self.editor.timeline.playback.pending_seek_time_seconds = None;
        self.editor.timeline.clock.time_seconds = target_time;
        let audio_resync_started_at = PlatformInstant::now();
        self.resync_editor_timeline_playback_audio();
        self.perf_record(PerfStage::TimelineSeekAudioResync, audio_resync_started_at);
        self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
    }

    fn playing_trigger_objects_at_time(&mut self, time_seconds: f32) -> Option<Vec<LevelObject>> {
        if self.phase != AppPhase::Playing || !self.editor.has_object_transform_triggers() {
            return None;
        }

        let base_objects = self
            .session
            .playing_trigger_base_objects
            .get_or_insert_with(|| self.gameplay.state.objects.clone());

        if !self.session.playing_trigger_hitboxes
            && base_objects.len() != self.gameplay.state.objects.len()
        {
            *base_objects = self.gameplay.state.objects.clone();
        }

        Some(trigger_transformed_objects_at_time(
            base_objects,
            self.editor.triggers(),
            time_seconds.max(0.0),
        ))
    }

    fn prune_playing_trigger_base_objects_from_consumed(&mut self) {
        if !self.session.playing_trigger_hitboxes {
            let _ = self.gameplay.state.take_consumed_object_indices();
            return;
        }

        let consumed_indices = self.gameplay.state.take_consumed_object_indices();
        if consumed_indices.is_empty() {
            return;
        }

        let Some(base_objects) = self.session.playing_trigger_base_objects.as_mut() else {
            return;
        };

        let mut indices = consumed_indices;
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            if index < base_objects.len() {
                base_objects.remove(index);
            }
        }
    }

    fn apply_playing_object_triggers(&mut self, time_seconds: f32) -> Option<Vec<LevelObject>> {
        let transformed = self.playing_trigger_objects_at_time(time_seconds)?;

        if self.session.playing_trigger_hitboxes {
            self.gameplay.state.objects = transformed.clone();
            self.gameplay.state.rebuild_behavior_cache();
        }

        Some(transformed)
    }

    fn target_playing_time(&self, frame_dt: f32) -> f32 {
        let elapsed = self.gameplay.state.elapsed_seconds.max(0.0);
        if !self.gameplay.state.started || self.gameplay.state.game_over {
            return elapsed;
        }

        let fallback_target = elapsed + frame_dt.max(0.0);
        let audio_target = self
            .audio
            .state
            .runtime
            .playback_time_seconds()
            .unwrap_or(fallback_target);

        let clamped_forward_target = audio_target.max(elapsed);
        clamped_forward_target.min(elapsed + 0.25)
    }

    fn advance_playing_state_to_time(
        &mut self,
        target_time: f32,
        simulation_dt: f32,
    ) -> Option<Vec<LevelObject>> {
        let mut trigger_render_objects: Option<Vec<LevelObject>> = None;
        let mut elapsed_seconds = self.gameplay.state.elapsed_seconds;

        advance_simulation_time(
            &mut elapsed_seconds,
            target_time,
            simulation_dt,
            |step_target, step_dt| {
                trigger_render_objects = self.apply_playing_object_triggers(step_target);
                self.gameplay.state.update(step_dt);
                self.prune_playing_trigger_base_objects_from_consumed();
                !self.gameplay.state.game_over
            },
        );
        self.gameplay.state.elapsed_seconds = elapsed_seconds;

        if self.editor.has_object_transform_triggers() {
            trigger_render_objects =
                self.apply_playing_object_triggers(self.gameplay.state.elapsed_seconds);
        }

        trigger_render_objects
    }

    fn build_editor_playback_trail_vertices(
        runtime: &TimelineSimulationRuntime,
    ) -> Vec<crate::types::Vertex> {
        const EDITOR_PLAYBACK_TRAIL_ALPHA: f32 = 0.45;
        const POSITION_EPSILON: f32 = 0.001;
        const MAX_RENDERED_EDITOR_TRAIL_POINTS: usize = 1024;

        let mut trail_vertices = Vec::new();
        let player_pos = runtime.position();
        let is_grounded = runtime.is_grounded();
        let is_game_over = runtime.game_over();
        let trail_segments = runtime.trail_segments();
        let recent_segments =
            Self::recent_trail_segments(trail_segments, MAX_RENDERED_EDITOR_TRAIL_POINTS);

        for (segment_index, segment) in recent_segments {
            if segment.is_empty() {
                continue;
            }

            let is_last_segment = segment_index + 1 == trail_segments.len();
            trail_vertices.extend(build_trail_vertices_with_alpha(
                segment,
                is_game_over,
                EDITOR_PLAYBACK_TRAIL_ALPHA,
            ));

            if !is_last_segment || !is_grounded {
                continue;
            }

            let Some(last_point) = segment.last() else {
                continue;
            };

            let dx = player_pos[0] - last_point[0];
            let dy = player_pos[1] - last_point[1];
            let dz = player_pos[2] - last_point[2];
            if dx.abs() > POSITION_EPSILON
                || dy.abs() > POSITION_EPSILON
                || dz.abs() > POSITION_EPSILON
            {
                trail_vertices.extend(build_trail_vertices_with_alpha(
                    &[*last_point, player_pos],
                    is_game_over,
                    EDITOR_PLAYBACK_TRAIL_ALPHA,
                ));
            }
        }

        if !is_grounded {
            let head_length = 0.22;
            let dir = match runtime.direction() {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                player_pos[0] - dir[0] * head_length,
                player_pos[1],
                player_pos[2] - dir[1] * head_length,
            ];
            let head_points = [head_start, player_pos];
            trail_vertices.extend(build_trail_vertices_with_alpha(
                &head_points,
                is_game_over,
                EDITOR_PLAYBACK_TRAIL_ALPHA,
            ));
        }

        trail_vertices
    }

    fn update_editor_playback_trail_mesh(&mut self) {
        let Some(runtime) = self.editor.timeline.playback.runtime.as_ref() else {
            self.render.meshes.trail.clear();
            return;
        };

        let trail_vertices = Self::build_editor_playback_trail_vertices(runtime);
        self.render
            .meshes
            .trail
            .write_streaming_vertices(&self.render.gpu.queue, &trail_vertices);
    }

    fn update_editor_scrub_trail_mesh(&mut self) {
        let target_time = self
            .editor
            .timeline
            .clock
            .time_seconds
            .clamp(0.0, self.editor.timeline.clock.duration_seconds.max(0.0));

        let needs_reset = match self.editor.timeline.scrub_runtime.as_ref() {
            Some(runtime) => {
                self.editor.timeline.scrub_runtime_revision
                    != self.editor.timeline.simulation_revision
                    || target_time + 1e-6 < runtime.elapsed_seconds()
            }
            None => true,
        };

        if needs_reset {
            self.editor.timeline.scrub_runtime =
                Some(TimelineSimulationRuntime::new_with_triggers(
                    self.editor.spawn.position,
                    self.editor.spawn.direction,
                    &self.editor.objects,
                    &self.editor.timeline.taps.tap_times,
                    self.editor.triggers(),
                    self.editor.simulate_trigger_hitboxes(),
                ));
            self.editor.timeline.scrub_runtime_revision = self.editor.timeline.simulation_revision;
        }

        let Some(runtime) = self.editor.timeline.scrub_runtime.as_mut() else {
            self.render.meshes.trail.clear();
            return;
        };

        runtime.advance_to(target_time);
        let trail_vertices = Self::build_editor_playback_trail_vertices(runtime);
        self.render
            .meshes
            .trail
            .write_streaming_vertices(&self.render.gpu.queue, &trail_vertices);
    }

    pub(crate) fn toggle_editor_perf_overlay(&mut self) {
        self.editor.perf.profiler.enabled = !self.editor.perf.profiler.enabled;
    }

    pub(crate) fn toggle_editor_perf_pause(&mut self) {
        self.editor.perf.profiler.toggle_pause();
    }

    pub(crate) fn editor_perf_paused(&self) -> bool {
        self.editor.perf.profiler.paused
    }

    pub(crate) fn clear_editor_perf_selection(&mut self) {
        self.editor.perf.profiler.clear_selection();
    }

    pub(crate) fn select_editor_perf_history_index(&mut self, index: usize) {
        self.editor.perf.profiler.set_selected_history_index(index);
    }

    pub(crate) fn editor_perf_selected_history_index(&self) -> Option<usize> {
        self.editor.perf.profiler.selected_history_index()
    }

    pub(crate) fn editor_perf_frame_history(&self) -> Vec<PerfFrameSnapshot> {
        if !self.editor.perf.profiler.enabled {
            return Vec::new();
        }

        self.editor.perf.profiler.frame_history()
    }

    pub(crate) fn editor_perf_selected_frame(&self) -> Option<PerfFrameSnapshot> {
        if !self.editor.perf.profiler.enabled {
            return None;
        }

        self.editor.perf.profiler.selected_or_latest_frame()
    }

    pub(crate) fn editor_perf_selected_top_contributors(
        &self,
        limit: usize,
    ) -> Vec<PerfFrameContributor> {
        if !self.editor.perf.profiler.enabled {
            return Vec::new();
        }

        self.editor
            .perf
            .profiler
            .selected_frame_top_contributors(limit)
    }

    pub(crate) fn editor_perf_selected_stage_tree(&self) -> Vec<PerfFrameStageEntry> {
        if !self.editor.perf.profiler.enabled {
            return Vec::new();
        }

        self.editor.perf.profiler.selected_frame_tree()
    }

    pub(crate) fn editor_perf_overlay_enabled(&self) -> bool {
        self.editor.perf.profiler.enabled
    }

    pub(crate) fn editor_perf_spike_count(&self) -> u64 {
        self.editor.perf.profiler.frame_spike_count
    }

    pub(crate) fn editor_perf_last_spike_stage_name(&self) -> &'static str {
        self.editor
            .perf
            .profiler
            .last_spike_stage
            .map(|stage| stage.name())
            .unwrap_or("none")
    }

    pub(super) fn perf_record(&mut self, stage: PerfStage, started_at: PlatformInstant) {
        self.editor.perf_record(stage, started_at);
    }

    /// Advances the application state by one frame.
    ///
    /// This method handles:
    /// - Audio and waveform loading updates.
    /// - Smoothing and recording FPS performance metrics.
    /// - Accumulating time for the fixed-step simulation.
    /// - Updating the active subsystem (Menu, Editor, or Gameplay) logic.
    /// - Managing input-driven camera movements.
    pub fn update(&mut self) {
        self.editor.perf.profiler.begin_frame();
        self.update_audio_imports();
        self.update_runtime_audio_preloads();
        self.update_waveform_loading();
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = PlatformInstant::now();
        let frame_dt = (now - self.frame_runtime.editor.last_frame).as_secs_f32();
        self.frame_runtime.editor.last_frame = now;
        let frame_dt_ms = frame_dt * 1000.0;
        let instant_fps = 1.0 / frame_dt.max(1e-4);
        if self.editor.perf.fps_smoothed <= 0.0 {
            self.editor.perf.fps_smoothed = instant_fps;
        } else {
            self.editor.perf.fps_smoothed = self.editor.perf.fps_smoothed * 0.9 + instant_fps * 0.1;
        }
        self.frame_runtime.editor.accumulator =
            (self.frame_runtime.editor.accumulator + frame_dt).min(0.25);
        self.frame_runtime.global_time_seconds += frame_dt.max(0.0);

        let color_space_uniform = ColorSpaceUniform {
            apply_gamma_correction: if self.render.gpu.apply_gamma_correction {
                1.0
            } else {
                0.0
            },
            time_seconds: self.frame_runtime.global_time_seconds,
            _pad: [0.0; 2],
        };
        self.render.gpu.queue.write_buffer(
            &self.render.gpu.color_space_uniform_buffer,
            0,
            bytemuck::bytes_of(&color_space_uniform),
        );

        if self.phase == AppPhase::Menu {
            self.frame_runtime.editor.accumulator = 0.0;
            self.update_menu_camera();
            self.editor.perf.profiler.finish_frame(frame_dt_ms);
            return;
        }

        if self.phase == AppPhase::Editor {
            self.frame_runtime.editor.accumulator = 0.0;
            self.render.meshes.trail.clear();

            if self.editor.timeline.playback.playing {
                self.maybe_resync_editor_playback_from_pending_seek(frame_dt);
                let timeline_playback_started_at = PlatformInstant::now();
                let audio_time = self
                    .audio
                    .state
                    .runtime
                    .playback_time_seconds()
                    .unwrap_or(self.editor.timeline.clock.time_seconds);
                let timeline_time_source = self
                    .editor
                    .timeline
                    .playback
                    .pending_seek_time_seconds
                    .unwrap_or(audio_time);
                let clamped_time =
                    timeline_time_source.min(self.editor.timeline.clock.duration_seconds);
                let simulate_preview = !self.editor_is_effectively_timing_mode();

                if !simulate_preview {
                    self.editor.timeline.playback.runtime = None;
                }

                if (clamped_time - self.editor.timeline.clock.time_seconds).abs() > 1e-4 {
                    let old_time = self.editor.timeline.clock.time_seconds;
                    self.editor.timeline.clock.time_seconds = clamped_time;

                    if simulate_preview && self.editor.has_object_transform_triggers() {
                        self.mark_editor_dirty(super::EditorDirtyFlags {
                            rebuild_block_mesh: true,
                            ..super::EditorDirtyFlags::default()
                        });
                    }

                    if simulate_preview {
                        let mut applied_runtime_state = false;
                        if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut() {
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
                            let mut runtime = TimelineSimulationRuntime::new_with_triggers(
                                self.editor.spawn.position,
                                self.editor.spawn.direction,
                                &self.editor.objects,
                                &self.editor.timeline.taps.tap_times,
                                self.editor.triggers(),
                                self.editor.simulate_trigger_hitboxes(),
                            );
                            runtime.advance_to(clamped_time);
                            let snapshot = runtime.snapshot();
                            self.apply_editor_timeline_preview_state(
                                snapshot.position,
                                snapshot.direction,
                            );
                            self.editor.timeline.playback.runtime = Some(runtime);
                        }
                    } else if clamped_time > old_time {
                        if let Some(tp) = self
                            .editor
                            .timing
                            .timing_points
                            .iter()
                            .rev()
                            .find(|tp| tp.time_seconds <= clamped_time)
                        {
                            if tp.bpm > 0.0 {
                                let beat_duration = 60.0 / tp.bpm;
                                let old_beat = if old_time < tp.time_seconds {
                                    -1
                                } else {
                                    ((old_time - tp.time_seconds) / beat_duration).floor() as i64
                                };
                                let new_beat = ((clamped_time - tp.time_seconds) / beat_duration)
                                    .floor() as i64;

                                if new_beat > old_beat {
                                    let is_downbeat =
                                        new_beat % (tp.time_signature_numerator.max(1) as i64) == 0;
                                    if is_downbeat {
                                        self.audio.state.runtime.play_sfx(include_bytes!(
                                            "../../assets/metronome_major.mp3"
                                        ));
                                    } else {
                                        self.audio.state.runtime.play_sfx(include_bytes!(
                                            "../../assets/metronome_minor.mp3"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                if simulate_preview {
                    self.update_editor_playback_trail_mesh();
                }

                if clamped_time >= self.editor.timeline.clock.duration_seconds
                    || !self.audio.state.runtime.is_playing()
                {
                    self.editor.timeline.playback.playing = false;
                    self.editor.timeline.playback.runtime = None;
                    self.editor.timeline.playback.pending_seek_time_seconds = None;
                    self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
                    if simulate_preview && self.editor.has_object_transform_triggers() {
                        self.mark_editor_dirty(super::EditorDirtyFlags {
                            rebuild_block_mesh: true,
                            ..super::EditorDirtyFlags::default()
                        });
                    }
                    self.stop_audio();
                }
                self.perf_record(PerfStage::TimelinePlayback, timeline_playback_started_at);
            } else if self.editor.ui.mode != EditorMode::Timing {
                self.update_editor_scrub_trail_mesh();
            }

            self.update_editor_pan_from_keys(frame_dt);
            self.update_editor_camera_transition(frame_dt);
            if self.editor.runtime.interaction.gizmo_drag.is_some()
                || self.editor.runtime.interaction.block_drag.is_some()
            {
                if let Some(pointer) = self.editor.ui.pointer_screen {
                    let drag_started_at = PlatformInstant::now();
                    self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
                    self.perf_record(PerfStage::DragSelection, drag_started_at);
                }
            }

            let camera_changed = (self.editor.camera.editor_pan[0]
                - self.editor.runtime.gizmo.last_pan[0])
                .abs()
                > 1e-4
                || (self.editor.camera.editor_pan[1] - self.editor.runtime.gizmo.last_pan[1]).abs()
                    > 1e-4
                || (self.editor.camera.editor_target_z - self.editor.runtime.gizmo.last_target_z)
                    .abs()
                    > 1e-4
                || (self.editor.camera.editor_rotation - self.editor.runtime.gizmo.last_rotation)
                    .abs()
                    > 1e-4
                || (self.editor.camera.editor_pitch - self.editor.runtime.gizmo.last_pitch).abs()
                    > 1e-4;

            let has_selection = self.editor.ui.selected_block_index.is_some()
                || !self.editor.ui.selected_block_indices.is_empty();
            let is_dragging = self.editor.runtime.interaction.gizmo_drag.is_some()
                || self.editor.runtime.interaction.block_drag.is_some();

            if has_selection && self.editor.ui.mode.shows_gizmo() {
                if is_dragging {
                    let gizmo_started_at = PlatformInstant::now();
                    self.rebuild_editor_gizmo_vertices();
                    self.perf_record(PerfStage::GizmoRebuild, gizmo_started_at);
                    self.editor.runtime.gizmo.rebuild_accumulator = 0.0;
                } else if camera_changed {
                    self.editor.runtime.gizmo.rebuild_accumulator += frame_dt;
                    if self.editor.runtime.gizmo.rebuild_accumulator >= (1.0 / 24.0) {
                        let gizmo_started_at = PlatformInstant::now();
                        self.rebuild_editor_gizmo_vertices();
                        self.perf_record(PerfStage::GizmoRebuild, gizmo_started_at);
                        self.editor.runtime.gizmo.rebuild_accumulator = 0.0;
                    }
                } else {
                    self.editor.runtime.gizmo.rebuild_accumulator = 0.0;
                }
            } else {
                self.editor.runtime.gizmo.rebuild_accumulator = 0.0;
            }

            self.editor.runtime.gizmo.last_pan = self.editor.camera.editor_pan;
            self.editor.runtime.gizmo.last_target_z = self.editor.camera.editor_target_z;
            self.editor.runtime.gizmo.last_rotation = self.editor.camera.editor_rotation;
            self.editor.runtime.gizmo.last_pitch = self.editor.camera.editor_pitch;
            let dirty_started_at = PlatformInstant::now();
            self.process_editor_dirty(frame_dt);
            self.perf_record(PerfStage::DirtyProcess, dirty_started_at);
            self.update_editor_camera();

            self.editor.perf.profiler.finish_frame(frame_dt_ms);
            return;
        }

        let target_time = self.target_playing_time(frame_dt);
        let trigger_render_objects = self.advance_playing_state_to_time(target_time, FIXED_DT);
        self.frame_runtime.editor.accumulator = 0.0;

        let render_objects = trigger_render_objects
            .as_deref()
            .unwrap_or(&self.gameplay.state.objects);
        if self.gameplay.state.has_animated_blocks() || trigger_render_objects.is_some() {
            let animated_vertices = build_block_vertices_with_phase(
                render_objects,
                self.gameplay.state.block_animation_phase_seconds(),
            );
            self.render.meshes.blocks.replace_with_vertices(
                &self.render.gpu.device,
                "Block Vertex Buffer",
                &animated_vertices,
            );
        }

        if self.gameplay.state.level_complete && self.gameplay.state.completion_hold_seconds <= 0.0
        {
            self.back_to_menu();
            return;
        }

        if self.gameplay.state.game_over {
            self.stop_audio();
        }

        let mut trail_vertices = Vec::new();
        let player_pos = self.gameplay.state.position;
        // Cull segments that are too far from the player to save on vertices.
        const CULL_DISTANCE_SQ: f32 = 120.0 * 120.0;
        const MAX_RENDERED_PLAYING_TRAIL_POINTS_PER_SEGMENT: usize = 1024;

        for (segment_index, segment) in self.gameplay.state.trail_segments.iter().enumerate() {
            if segment.is_empty() {
                continue;
            }

            let is_last_segment = segment_index + 1 == self.gameplay.state.trail_segments.len();

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

            if is_last_segment && self.gameplay.state.is_grounded {
                let start = segment
                    .len()
                    .saturating_sub(MAX_RENDERED_PLAYING_TRAIL_POINTS_PER_SEGMENT);
                let mut points = segment[start..].to_vec();
                points.push(self.gameplay.state.position);
                trail_vertices.extend(build_trail_vertices(&points, self.gameplay.state.game_over));
            } else {
                let start = segment
                    .len()
                    .saturating_sub(MAX_RENDERED_PLAYING_TRAIL_POINTS_PER_SEGMENT);
                trail_vertices.extend(build_trail_vertices(
                    &segment[start..],
                    self.gameplay.state.game_over,
                ));
            }
        }

        if !self.gameplay.state.is_grounded {
            let head_length = 0.22;
            let dir = match self.gameplay.state.direction {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                self.gameplay.state.position[0] - dir[0] * head_length,
                self.gameplay.state.position[1],
                self.gameplay.state.position[2] - dir[1] * head_length,
            ];
            let head_points = [head_start, self.gameplay.state.position];
            trail_vertices.extend(build_trail_vertices(
                &head_points,
                self.gameplay.state.game_over,
            ));
        }

        self.render
            .meshes
            .trail
            .write_streaming_vertices(&self.render.gpu.queue, &trail_vertices);

        self.frame_runtime.player_render.line_uniform.offset = [
            (self.gameplay.state.position[0] * 100.0).round() / 100.0,
            (self.gameplay.state.position[2] * 100.0).round() / 100.0,
        ];
        self.frame_runtime.player_render.line_uniform.rotation = match self.gameplay.state.direction
        {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        self.render.gpu.queue.write_buffer(
            &self.render.gpu.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.frame_runtime.player_render.line_uniform),
        );

        let aspect = self.render.gpu.config.width as f32 / self.render.gpu.config.height as f32;
        let (eye, target) = self.playing_camera_view();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.render.gpu.queue.write_buffer(
            &self.render.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );

        if self.phase == AppPhase::Playing {
            self.editor.perf.profiler.finish_frame(frame_dt_ms);
        }
    }

    fn update_menu_camera(&mut self) {
        let aspect = self.render.gpu.config.width as f32 / self.render.gpu.config.height as f32;
        let radius = 25.0;
        let angle = -25.0f32.to_radians();
        let eye = Vec3::new(radius * angle.cos(), 15.0, radius * angle.sin());
        let target = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.render.gpu.queue.write_buffer(
            &self.render.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    fn update_editor_camera(&mut self) {
        let aspect = self.render.gpu.config.width as f32 / self.render.gpu.config.height as f32;
        let (eye, target) = if self.editor_is_playing() {
            let (e, t) = self.editor_preview_camera_view();
            (Vec3::from_array(e), Vec3::from_array(t))
        } else {
            let target = Vec3::new(
                self.editor.camera.editor_pan[0],
                self.editor.camera.editor_target_z,
                self.editor.camera.editor_pan[1],
            );
            let offset = self.editor_camera_offset();
            (target + offset, target)
        };
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.render.gpu.queue.write_buffer(
            &self.render.gpu.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::game::TimelineSimulationRuntime;
    use crate::types::{
        AppPhase, EditorMode, LevelObject, SpawnDirection, TimedTrigger, TimedTriggerAction,
        TimedTriggerEasing, TimedTriggerTarget, TimingPoint,
    };

    fn sample_object() -> LevelObject {
        LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }
    }

    fn instant_move_trigger() -> TimedTrigger {
        TimedTrigger {
            time_seconds: 0.0,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Object { object_id: 0 },
            action: TimedTriggerAction::MoveTo {
                position: [2.0, 0.0, 0.0],
            },
        }
    }

    #[test]
    fn target_playing_time_respects_started_and_game_over_state() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.gameplay.state.elapsed_seconds = 1.25;
            state.gameplay.state.started = false;
            assert_eq!(state.target_playing_time(0.2), 1.25);

            state.gameplay.state.started = true;
            state.gameplay.state.game_over = false;
            assert_eq!(state.target_playing_time(0.2), 1.45);

            state.gameplay.state.game_over = true;
            assert_eq!(state.target_playing_time(0.2), 1.25);

            state.gameplay.state.game_over = false;
            assert_eq!(state.target_playing_time(10.0), 1.5);
            assert_eq!(state.target_playing_time(-1.0), 1.25);
        });
    }

    #[test]
    fn pending_seek_without_target_resets_cooldown_in_editor_playback() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.playback.pending_seek_time_seconds = None;
            state.editor.timeline.playback.seek_resync_cooldown_seconds = 0.75;

            state.maybe_resync_editor_playback_from_pending_seek(0.1);
            assert_eq!(
                state.editor.timeline.playback.seek_resync_cooldown_seconds,
                0.0
            );
        });
    }

    #[test]
    fn playing_object_triggers_transform_objects_and_optionally_hitboxes() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Playing;
            state.gameplay.state.objects = vec![sample_object()];
            state.gameplay.state.rebuild_behavior_cache();
            state.editor.set_triggers(vec![instant_move_trigger()]);

            state.session.playing_trigger_hitboxes = false;
            let transformed = state
                .playing_trigger_objects_at_time(0.5)
                .expect("expected transformed objects");
            assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
            assert_eq!(state.gameplay.state.objects[0].position, [0.0, 0.0, 0.0]);

            state.session.playing_trigger_hitboxes = true;
            let transformed = state
                .apply_playing_object_triggers(0.5)
                .expect("expected transformed objects");
            assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
            assert_eq!(state.gameplay.state.objects[0].position, [2.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn advance_playing_state_to_time_updates_elapsed_and_returns_render_objects() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Playing;
            state.gameplay.state.started = true;
            state.gameplay.state.objects = vec![sample_object()];
            state.gameplay.state.rebuild_behavior_cache();
            state.editor.set_triggers(vec![instant_move_trigger()]);

            let transformed = state
                .advance_playing_state_to_time(0.2, 1.0 / 120.0)
                .expect("expected trigger render objects");

            assert!(state.gameplay.state.elapsed_seconds <= 0.2 + 1e-6);
            assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
        });
    }

    #[test]
    fn playback_trail_vertices_include_airborne_head_segment() {
        let grounded_runtime =
            TimelineSimulationRuntime::new([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[]);
        assert!(grounded_runtime.is_grounded());
        let grounded_vertices = State::build_editor_playback_trail_vertices(&grounded_runtime);

        let support = [LevelObject {
            position: [0.0, 2.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }];
        let mut airborne_runtime =
            TimelineSimulationRuntime::new([0.0, 3.0, 0.0], SpawnDirection::Forward, &support, &[]);
        airborne_runtime.advance_to(1.0 / crate::game::BASE_PLAYER_SPEED + 0.01);
        assert!(!airborne_runtime.is_grounded());
        let airborne_vertices = State::build_editor_playback_trail_vertices(&airborne_runtime);

        assert!(grounded_vertices.is_empty());
        assert!(!airborne_vertices.is_empty());
    }

    #[test]
    fn recent_trail_segments_keeps_latest_points_with_bound() {
        let segments = vec![
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            vec![[3.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
            vec![[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [7.0, 0.0, 0.0]],
        ];

        let selected = State::recent_trail_segments(&segments, 4);
        let total_points: usize = selected.iter().map(|(_, points)| points.len()).sum();

        assert_eq!(total_points, 4);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].0, 1);
        assert_eq!(selected[1].0, 2);
        assert_eq!(selected[0].1, &[[4.0, 0.0, 0.0]]);
        assert_eq!(
            selected[1].1,
            &[[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [7.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn perf_overlay_toggle_controls_visibility() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            assert!(!state.editor_perf_overlay_enabled());
            assert!(state.editor_perf_frame_history().is_empty());
            assert!(state.editor_perf_selected_frame().is_none());

            state.toggle_editor_perf_overlay();
            assert!(state.editor_perf_overlay_enabled());
            assert_eq!(state.editor_perf_spike_count(), 0);
            assert_eq!(state.editor_perf_last_spike_stage_name(), "none");
        });
    }

    #[test]
    fn pending_seek_resync_waits_for_cooldown_then_applies_target_time() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.set_mode(EditorMode::Timing);
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.clock.time_seconds = 0.5;
            state.editor.timeline.playback.pending_seek_time_seconds = Some(2.0);
            state.editor.timeline.playback.seek_resync_cooldown_seconds = 0.2;

            state.maybe_resync_editor_playback_from_pending_seek(0.05);
            assert_eq!(
                state.editor.timeline.playback.pending_seek_time_seconds,
                Some(2.0)
            );
            assert!(state.editor.timeline.playback.seek_resync_cooldown_seconds > 0.0);
            assert_eq!(state.editor.timeline.clock.time_seconds, 0.5);

            state.maybe_resync_editor_playback_from_pending_seek(1.0);
            assert_eq!(
                state.editor.timeline.playback.pending_seek_time_seconds,
                None
            );
            assert_eq!(
                state.editor.timeline.playback.seek_resync_cooldown_seconds,
                0.0
            );
            assert_eq!(state.editor.timeline.clock.time_seconds, 2.0);
        });
    }

    #[test]
    fn pending_seek_resync_early_returns_when_not_editor_or_not_playing() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.editor.timeline.playback.pending_seek_time_seconds = Some(3.0);
            state.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;

            state.phase = AppPhase::Menu;
            state.editor.timeline.playback.playing = true;
            state.maybe_resync_editor_playback_from_pending_seek(1.0);
            assert_eq!(
                state.editor.timeline.playback.pending_seek_time_seconds,
                Some(3.0)
            );

            state.phase = AppPhase::Editor;
            state.editor.timeline.playback.playing = false;
            state.maybe_resync_editor_playback_from_pending_seek(1.0);
            assert_eq!(
                state.editor.timeline.playback.pending_seek_time_seconds,
                Some(3.0)
            );
        });
    }

    #[test]
    fn playing_trigger_objects_at_time_requires_playing_phase_and_transform_triggers() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.gameplay.state.objects = vec![sample_object()];
            state.gameplay.state.rebuild_behavior_cache();

            state.phase = AppPhase::Menu;
            state.editor.set_triggers(vec![instant_move_trigger()]);
            assert!(state.playing_trigger_objects_at_time(0.2).is_none());

            state.phase = AppPhase::Playing;
            state.editor.set_triggers(Vec::new());
            assert!(state.playing_trigger_objects_at_time(0.2).is_none());

            state.editor.set_triggers(vec![instant_move_trigger()]);
            let transformed = state
                .playing_trigger_objects_at_time(0.2)
                .expect("expected transformed objects");
            assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);

            state.session.playing_trigger_hitboxes = false;
            state.session.playing_trigger_base_objects =
                Some(vec![sample_object(), sample_object()]);
            let transformed = state
                .playing_trigger_objects_at_time(0.2)
                .expect("expected transformed objects");
            assert_eq!(transformed.len(), 1);
            assert_eq!(
                state
                    .session
                    .playing_trigger_base_objects
                    .as_ref()
                    .map(|objects| objects.len()),
                Some(1)
            );
        });
    }

    #[test]
    fn update_editor_scrub_trail_mesh_creates_and_resets_runtime_when_needed() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.timeline.clock.time_seconds = 3.0;
            state.editor.timeline.scrub_runtime = None;
            state.editor.timeline.simulation_revision = 2;

            state.update_editor_scrub_trail_mesh();
            assert!(state.editor.timeline.scrub_runtime.is_some());
            assert_eq!(state.editor.timeline.scrub_runtime_revision, 2);

            let mut runtime = TimelineSimulationRuntime::new_with_triggers(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.objects,
                &state.editor.timeline.taps.tap_times,
                state.editor.triggers(),
                state.editor.simulate_trigger_hitboxes(),
            );
            runtime.advance_to(6.0);
            state.editor.timeline.scrub_runtime = Some(runtime);
            state.editor.timeline.scrub_runtime_revision =
                state.editor.timeline.simulation_revision;
            state.editor.timeline.clock.time_seconds = 2.0;

            state.update_editor_scrub_trail_mesh();
            let elapsed = state
                .editor
                .timeline
                .scrub_runtime
                .as_ref()
                .map(|runtime| runtime.elapsed_seconds())
                .unwrap_or_default();
            assert!(elapsed <= 2.000_1);
        });
    }

    #[test]
    fn update_menu_branch_resets_accumulator_and_records_frame() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Menu;
            state.frame_runtime.editor.accumulator = 0.2;

            state.update();

            assert_eq!(state.frame_runtime.editor.accumulator, 0.0);
            assert!(
                state.editor.perf.profiler.stats[crate::state::PerfStage::FrameTotal.as_index()]
                    .calls
                    > 0
            );
        });
    }

    #[test]
    fn update_editor_playback_in_timing_mode_keeps_runtime_none_and_handles_pending_seek() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.editor.timeline.clock.duration_seconds = 4.0;
            state.editor.timeline.clock.time_seconds = 0.5;
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.playback.pending_seek_time_seconds = Some(1.5);
            state.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;

            state.update();

            assert!(state.editor.timeline.playback.runtime.is_none());
            assert!(state.editor.timeline.clock.time_seconds >= 0.5);
        });
    }

    #[test]
    fn update_editor_playback_stops_when_audio_not_playing() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Place;
            state.editor.timeline.clock.duration_seconds = 10.0;
            state.editor.timeline.clock.time_seconds = 2.0;
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.playback.runtime =
                Some(TimelineSimulationRuntime::new_with_triggers(
                    state.editor.spawn.position,
                    state.editor.spawn.direction,
                    &state.editor.objects,
                    &state.editor.timeline.taps.tap_times,
                    state.editor.triggers(),
                    state.editor.simulate_trigger_hitboxes(),
                ));

            // In tests, no active playback source means runtime reports not playing.
            state.update();

            assert!(!state.editor.timeline.playback.playing);
            assert!(state.editor.timeline.playback.runtime.is_none());
            assert!(state
                .editor
                .timeline
                .playback
                .pending_seek_time_seconds
                .is_none());
        });
    }

    #[test]
    fn update_playing_phase_level_complete_returns_to_menu() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Playing;
            state.gameplay.state.level_complete = true;
            state.gameplay.state.completion_hold_seconds = 0.0;

            state.update();

            assert_eq!(state.phase, AppPhase::Menu);
        });
    }

    #[test]
    fn update_playing_phase_game_over_keeps_phase_and_stops_audio_path() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Playing;
            state.gameplay.state.started = true;
            state.gameplay.state.game_over = true;
            state.gameplay.state.objects = vec![sample_object()];
            state.gameplay.state.rebuild_behavior_cache();

            state.update();

            assert_eq!(state.phase, AppPhase::Playing);
            assert!(state.gameplay.state.game_over);
        });
    }

    #[test]
    fn update_playing_phase_builds_trail_and_line_uniform_for_right_direction() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Playing;
            state.gameplay.state.started = false;
            state.gameplay.state.direction = crate::types::Direction::Right;
            state.gameplay.state.position = [0.0, 0.0, 0.0];
            state.gameplay.state.is_grounded = false;
            state.gameplay.state.trail_segments =
                vec![vec![[900.0, 0.0, 900.0]], vec![[0.0, 0.0, 0.0]]];

            state.update();

            assert_eq!(
                state.frame_runtime.player_render.line_uniform.rotation,
                -std::f32::consts::FRAC_PI_2
            );
            assert!(state.render.meshes.trail.draw_data().is_some());
        });
    }

    #[test]
    fn update_editor_timing_playback_advances_with_pending_seek_and_timing_points() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.editor.timeline.playback.playing = true;
            state.editor.timeline.clock.time_seconds = 0.1;
            state.editor.timeline.clock.duration_seconds = 8.0;
            state.editor.timeline.playback.pending_seek_time_seconds = Some(1.1);
            state.editor.timeline.playback.seek_resync_cooldown_seconds = 1.0;
            state.editor.timing.timing_points = vec![TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];

            state.update();

            assert!(state.editor.timeline.clock.time_seconds >= 1.0);
            assert!(!state.editor.timeline.playback.playing);
        });
    }
}
