/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use glam::{Mat4, Vec3};

use super::State;
use super::runtime::BlockVisualEffect;
use eg_blocks::BlockEffectColorMode;
use eg_blocks::{BlockDefinition, BlockRenderProfile};
use eg_game::{
    TimelineSimulationRuntime, advance_simulation_time, trigger_transformed_objects_at_time,
};
use eg_mesh::{
    BlockEffectInstance, build_block_effect_vertices,
    build_block_instance_batches_from_refs_at_time, build_trail_vertices,
    build_trail_vertices_with_alpha,
};
use eg_types::{AppPhase, CameraUniform, ColorSpaceUniform, Direction, EditorMode, LevelObject};

const DEAD_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/dead.mp3"
));
const METRONOME_MAJOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_major.mp3"
));
const METRONOME_MINOR_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/metronome_minor.mp3"
));

fn is_play_world_render_object(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile != BlockRenderProfile::CameraTrigger
}

impl State {
    fn play_death_sfx_once(&mut self) {
        if self.gameplay.death_sfx_played {
            return;
        }

        self.gameplay.death_sfx_played = true;
        self.audio.state.runtime.play_sfx(DEAD_SFX);
    }

    fn push_block_effect_events(&mut self, events: Vec<eg_game::ConsumedObjectEvent>) {
        self.frame_runtime
            .player_render
            .gem_shatter_effects
            .extend(events.iter().filter_map(|event| {
                let block = BlockDefinition::get(&event.object.block_id);
                let effect = block.animation.on_consumed?;
                let color = match effect.color_mode {
                    BlockEffectColorMode::Tint => event.object.color_tint,
                    BlockEffectColorMode::Render => [
                        block.render.color_top[0],
                        block.render.color_top[1],
                        block.render.color_top[2],
                    ],
                };
                Some(BlockVisualEffect::from_object(&event.object, effect, color))
            }));
    }

    fn update_block_effect_mesh(&mut self, frame_dt: f32) {
        puffin::profile_scope!("BlockEffectMesh");
        let effects = &mut self.frame_runtime.player_render.gem_shatter_effects;
        for effect in effects.iter_mut() {
            effect.age_seconds += frame_dt.max(0.0);
        }
        effects.retain(|effect| effect.age_seconds < effect.effect.duration_seconds);

        if effects.is_empty() {
            self.render_meshes_mut().gem_shatter_effects.clear();
            return;
        }

        let instances = effects
            .iter()
            .map(|effect| BlockEffectInstance {
                position: effect.position,
                size: effect.size,
                color: effect.color,
                effect: effect.effect,
                age_seconds: effect.age_seconds,
            })
            .collect::<Vec<_>>();
        let vertices = build_block_effect_vertices(&instances);
        self.with_render_meshes_and_device(|device, meshes| {
            meshes.gem_shatter_effects.replace_with_vertices(
                device,
                "Gem Shatter Vertex Buffer",
                &vertices,
            );
        });
    }

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
        puffin::profile_scope!("SeekAudioResync");
        self.resync_editor_timeline_playback_audio();
        self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
    }

    fn playing_trigger_objects_at_time(&mut self, time_seconds: f32) -> Option<Vec<LevelObject>> {
        puffin::profile_scope!("PlayingTriggerObjects");
        if self.phase != AppPhase::Playing || !self.editor.has_object_transform_triggers() {
            return None;
        }

        let base_objects = self
            .session
            .playing_trigger_base_objects
            .get_or_insert_with(|| self.gameplay.state.objects.clone());

        if base_objects.len() != self.gameplay.state.objects.len() {
            *base_objects = self.gameplay.state.objects.clone();
        }

        // Visual rendering always reflects every transform trigger applied to the
        // (consumed-pruned when simulating) base object set.
        Some(trigger_transformed_objects_at_time(
            base_objects,
            &self.editor.trigger_objects(),
            time_seconds.max(0.0),
        ))
    }

    fn prune_playing_trigger_base_objects_from_consumed(&mut self) {
        puffin::profile_scope!("PlayingTriggerPruneConsumed");
        let _ = self.gameplay.state.take_consumed_object_indices();
    }

    fn apply_playing_object_triggers(&mut self, time_seconds: f32) -> Option<Vec<LevelObject>> {
        // The visual set applies every transform trigger to the base objects.
        let visual = self.playing_trigger_objects_at_time(time_seconds)?;

        Some(visual)
    }

    fn apply_pending_gameplay_turns_up_to(&mut self, time_seconds: f32) {
        const INPUT_TIME_EPSILON: f32 = 1e-6;
        while self
            .gameplay
            .pending_turn_inputs
            .front()
            .is_some_and(|input| input.time_seconds <= time_seconds + INPUT_TIME_EPSILON)
        {
            let _ = self.gameplay.pending_turn_inputs.pop_front();
            self.gameplay.state.turn_right();
        }
    }

    fn target_playing_time(&self, frame_dt: f32) -> f32 {
        let elapsed = self.gameplay.state.elapsed_seconds.max(0.0);
        if !self.gameplay.state.started
            || self.gameplay.state.game_over
            || self.gameplay.state.level_complete
        {
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
        clamped_forward_target
            .min(elapsed + 0.25)
            .min(self.gameplay.state.level_duration_seconds)
    }

    fn advance_playing_state_segment_to_time(
        &mut self,
        elapsed_seconds: &mut f32,
        target_time: f32,
        simulation_dt: f32,
        trigger_render_objects: &mut Option<Vec<LevelObject>>,
    ) -> bool {
        let mut should_continue = true;
        advance_simulation_time(
            elapsed_seconds,
            target_time,
            simulation_dt,
            |step_target, step_dt| {
                *trigger_render_objects = self.apply_playing_object_triggers(step_target);
                self.gameplay.state.update(step_dt);
                let events = self.gameplay.state.take_consumed_object_events();
                if !events.is_empty() && trigger_render_objects.is_none() {
                    *trigger_render_objects = Some(self.gameplay.state.objects.clone());
                }
                self.push_block_effect_events(events);
                self.prune_playing_trigger_base_objects_from_consumed();
                should_continue =
                    !self.gameplay.state.game_over && !self.gameplay.state.level_complete;
                should_continue
            },
        );
        self.gameplay.state.elapsed_seconds = *elapsed_seconds;
        should_continue
    }

    fn advance_playing_state_to_time(
        &mut self,
        target_time: f32,
        simulation_dt: f32,
    ) -> Option<Vec<LevelObject>> {
        puffin::profile_scope!("PlayingAdvanceToTime");
        const INPUT_TIME_EPSILON: f32 = 1e-6;
        let mut trigger_render_objects: Option<Vec<LevelObject>> = None;
        let mut elapsed_seconds = self.gameplay.state.elapsed_seconds;

        self.apply_pending_gameplay_turns_up_to(elapsed_seconds);
        while elapsed_seconds + INPUT_TIME_EPSILON < target_time {
            let next_turn_time = self
                .gameplay
                .pending_turn_inputs
                .front()
                .map(|input| input.time_seconds);

            let Some(next_turn_time) = next_turn_time else {
                let _ = self.advance_playing_state_segment_to_time(
                    &mut elapsed_seconds,
                    target_time,
                    simulation_dt,
                    &mut trigger_render_objects,
                );
                break;
            };

            if next_turn_time > target_time + INPUT_TIME_EPSILON {
                let _ = self.advance_playing_state_segment_to_time(
                    &mut elapsed_seconds,
                    target_time,
                    simulation_dt,
                    &mut trigger_render_objects,
                );
                break;
            }

            let segment_target = next_turn_time.max(elapsed_seconds);
            if segment_target > elapsed_seconds + INPUT_TIME_EPSILON
                && !self.advance_playing_state_segment_to_time(
                    &mut elapsed_seconds,
                    segment_target,
                    simulation_dt,
                    &mut trigger_render_objects,
                )
            {
                break;
            }

            self.gameplay.state.elapsed_seconds = elapsed_seconds;
            self.apply_pending_gameplay_turns_up_to(elapsed_seconds);
            if self.gameplay.state.game_over {
                break;
            }
        }
        self.gameplay.state.elapsed_seconds = elapsed_seconds;

        if self.editor.has_object_transform_triggers() {
            trigger_render_objects =
                self.apply_playing_object_triggers(self.gameplay.state.elapsed_seconds);
        }

        trigger_render_objects
    }

    fn build_editor_playback_trail_vertices(
        runtime: &TimelineSimulationRuntime,
    ) -> Vec<eg_mesh::Vertex> {
        puffin::profile_scope!("EditorPlaybackTrailBuild");
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
        puffin::profile_scope!("EditorPlaybackTrailMesh");
        let Some(runtime) = self.editor.timeline.playback.runtime.as_ref() else {
            self.render_meshes_mut().trail.clear();
            return;
        };

        let trail_vertices = Self::build_editor_playback_trail_vertices(runtime);
        self.with_render_meshes_and_queue(|queue, meshes| {
            meshes
                .trail
                .write_streaming_vertices(queue, &trail_vertices);
        });
    }

    fn update_editor_scrub_trail_mesh(&mut self) {
        puffin::profile_scope!("EditorScrubTrailMesh");
        let target_time = self
            .editor
            .timeline
            .clock
            .time_seconds
            .clamp(0.0, self.editor.timeline.clock.duration_seconds.max(0.0));

        self.rebuild_editor_timeline_snapshot_cache_if_needed();
        let trail_vertices = self.build_editor_scrub_trail_vertices_from_cache(target_time);
        self.with_render_meshes_and_queue(|queue, meshes| {
            meshes
                .trail
                .write_streaming_vertices(queue, &trail_vertices);
        });
    }

    fn build_editor_scrub_trail_vertices_from_cache(
        &self,
        target_time: f32,
    ) -> Vec<eg_mesh::Vertex> {
        const EDITOR_SCRUB_TRAIL_ALPHA: f32 = 0.45;
        const POSITION_EPSILON: f32 = 0.001;
        const MAX_RENDERED_EDITOR_TRAIL_POINTS: usize = 1024;

        let cache = &self.editor.timeline.snapshot_cache;
        if cache.is_empty() {
            return Vec::new();
        }

        let step_seconds = self
            .editor
            .timeline
            .snapshot_cache_step_seconds
            .max(1.0 / 480.0);
        let max_index = cache.len().saturating_sub(1);
        let target_index = ((target_time.max(0.0) / step_seconds).floor() as usize).min(max_index);
        let snapshot = &cache[target_index];
        let trail_segments = self.cached_editor_trail_segments_for_snapshot(snapshot);
        let recent_segments =
            Self::recent_trail_segments(&trail_segments, MAX_RENDERED_EDITOR_TRAIL_POINTS);

        let mut trail_vertices = Vec::new();
        for (segment_index, segment) in recent_segments {
            if segment.is_empty() {
                continue;
            }

            let is_last_segment = segment_index + 1 == trail_segments.len();
            trail_vertices.extend(build_trail_vertices_with_alpha(
                segment,
                snapshot.game_over,
                EDITOR_SCRUB_TRAIL_ALPHA,
            ));

            if !is_last_segment || !snapshot.is_grounded {
                continue;
            }

            let Some(last_point) = segment.last() else {
                continue;
            };

            let dx = snapshot.position[0] - last_point[0];
            let dy = snapshot.position[1] - last_point[1];
            let dz = snapshot.position[2] - last_point[2];
            if dx.abs() > POSITION_EPSILON
                || dy.abs() > POSITION_EPSILON
                || dz.abs() > POSITION_EPSILON
            {
                trail_vertices.extend(build_trail_vertices_with_alpha(
                    &[*last_point, snapshot.position],
                    snapshot.game_over,
                    EDITOR_SCRUB_TRAIL_ALPHA,
                ));
            }
        }

        if !snapshot.is_grounded {
            let head_length = 0.22;
            let dir = match snapshot.direction {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                snapshot.position[0] - dir[0] * head_length,
                snapshot.position[1],
                snapshot.position[2] - dir[1] * head_length,
            ];
            let head_points = [head_start, snapshot.position];
            trail_vertices.extend(build_trail_vertices_with_alpha(
                &head_points,
                snapshot.game_over,
                EDITOR_SCRUB_TRAIL_ALPHA,
            ));
        }

        trail_vertices
    }

    fn cached_editor_trail_segments_for_snapshot(
        &self,
        snapshot: &crate::state::editor_timeline::EditorTimelineSnapshot,
    ) -> Vec<Vec<[f32; 3]>> {
        let mut segments = Vec::new();
        let segment_count = snapshot
            .trail_segment_count
            .min(self.editor.timeline.trail_segment_start_cache.len());
        let point_count = snapshot
            .trail_point_count
            .min(self.editor.timeline.trail_point_cache.len());

        for segment_index in 0..segment_count {
            let start = self.editor.timeline.trail_segment_start_cache[segment_index];
            let end = if segment_index + 1 < segment_count {
                self.editor.timeline.trail_segment_start_cache[segment_index + 1]
            } else {
                point_count
            }
            .min(point_count);

            if start < end {
                segments.push(self.editor.timeline.trail_point_cache[start..end].to_vec());
            }
        }

        segments
    }

    fn current_color_space_uniform(&self) -> ColorSpaceUniform {
        ColorSpaceUniform {
            apply_gamma_correction: if self.render_gpu().apply_gamma_correction {
                1.0
            } else {
                0.0
            },
            time_seconds: self.frame_runtime.global_time_seconds,
            viewport_size: [
                self.render_gpu().config.width.max(1) as f32,
                self.render_gpu().config.height.max(1) as f32,
            ],
        }
    }

    fn color_space_uniform_for_legacy_update(&mut self) -> ColorSpaceUniform {
        self.frame_runtime
            .scheduled_color_space_uniform
            .take()
            .unwrap_or_else(|| self.current_color_space_uniform())
    }

    fn upload_color_space_uniform(&self, color_space_uniform: ColorSpaceUniform) {
        puffin::profile_scope!("ColorSpaceUniformUpload");
        self.render_gpu().queue.write_buffer(
            &self.render_gpu().color_space_uniform_buffer,
            0,
            bytemuck::bytes_of(&color_space_uniform),
        );
    }

    /// Advances the application state by one frame.
    ///
    /// This method handles:
    /// - Starting auth refreshes for existing sessions.
    /// - Consuming the scheduled frame delta for the fixed-step simulation.
    /// - Updating the active subsystem (Menu, Editor, or Gameplay) logic.
    /// - Managing input-driven camera movements.
    pub fn update(&mut self) {
        puffin::profile_scope!("FrameTotal");
        const FIXED_DT: f32 = 1.0 / 120.0;

        let using_scheduled_frame_time = self.frame_runtime.editor.scheduled_delta_available;
        let frame_dt = self.frame_delta_for_legacy_update();

        let color_space_uniform = self.color_space_uniform_for_legacy_update();
        self.upload_color_space_uniform(color_space_uniform);

        if self.phase == AppPhase::Menu {
            puffin::profile_scope!("MenuUpdate");
            if !using_scheduled_frame_time {
                self.frame_runtime.editor.accumulator = 0.0;
            }
            self.frame_runtime.player_render.gem_shatter_effects.clear();
            self.render_meshes_mut().gem_shatter_effects.clear();
            self.refresh_menu_level_preview_if_needed();
            self.update_menu_camera();
            return;
        }

        if self.phase == AppPhase::Editor {
            puffin::profile_scope!("EditorUpdate");
            if !using_scheduled_frame_time {
                self.frame_runtime.editor.accumulator = 0.0;
            }
            self.render_meshes_mut().trail.clear();

            if self.editor.timeline.playback.playing {
                self.maybe_resync_editor_playback_from_pending_seek(frame_dt);
                puffin::profile_scope!("TimelinePlayback");
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

                    if simulate_preview
                        && (self.editor.has_object_transform_triggers()
                            || self.editor.has_camera_timeline_triggers())
                    {
                        self.mark_editor_dirty(
                            super::EditorDirtyFlags::timeline_trigger_preview_changed(),
                        );
                    }

                    if simulate_preview {
                        let mut applied_runtime_state = false;
                        if let Some(runtime) = self.editor.timeline.playback.runtime.as_mut()
                            && clamped_time + 1e-6 >= runtime.elapsed_seconds()
                        {
                            runtime.advance_to(clamped_time);
                            let events = runtime.take_consumed_object_events();
                            let consumed_gems = !events.is_empty();
                            let snapshot = runtime.snapshot();
                            self.push_block_effect_events(events);
                            if consumed_gems {
                                self.mark_editor_dirty(
                                    super::EditorDirtyFlags::block_mesh_and_hitboxes_changed(),
                                );
                            }
                            self.apply_editor_timeline_preview_state(
                                snapshot.position,
                                snapshot.direction,
                            );
                            applied_runtime_state = true;
                        }

                        if !applied_runtime_state {
                            let mut runtime = TimelineSimulationRuntime::new(
                                self.editor.spawn.position,
                                self.editor.spawn.direction,
                                &self.editor.objects,
                                &self.editor.timeline.taps.tap_times,
                            );
                            runtime.advance_to(clamped_time);
                            let events = runtime.take_consumed_object_events();
                            let consumed_gems = !events.is_empty();
                            self.push_block_effect_events(events);
                            let snapshot = runtime.snapshot();
                            if consumed_gems {
                                self.mark_editor_dirty(
                                    super::EditorDirtyFlags::block_mesh_and_hitboxes_changed(),
                                );
                            }
                            self.apply_editor_timeline_preview_state(
                                snapshot.position,
                                snapshot.direction,
                            );
                            self.editor.timeline.playback.runtime = Some(runtime);
                        }
                    } else if clamped_time > old_time
                        && let Some(tp) = self
                            .editor
                            .timing
                            .timing_points
                            .iter()
                            .rev()
                            .find(|tp| tp.time_seconds <= clamped_time)
                        && tp.bpm > 0.0
                    {
                        let beat_duration = 60.0 / tp.bpm;
                        let old_beat = if old_time < tp.time_seconds {
                            -1
                        } else {
                            ((old_time - tp.time_seconds) / beat_duration).floor() as i64
                        };
                        let new_beat =
                            ((clamped_time - tp.time_seconds) / beat_duration).floor() as i64;

                        if new_beat > old_beat {
                            let is_downbeat =
                                new_beat % (tp.time_signature_numerator.max(1) as i64) == 0;
                            if is_downbeat {
                                self.audio.state.runtime.play_sfx(METRONOME_MAJOR_SFX);
                            } else {
                                self.audio.state.runtime.play_sfx(METRONOME_MINOR_SFX);
                            }
                        }
                    }
                }

                if simulate_preview {
                    self.update_editor_playback_trail_mesh();
                }
                self.update_block_effect_mesh(frame_dt);

                if clamped_time >= self.editor.timeline.clock.duration_seconds
                    || !self.audio.state.runtime.is_playing()
                {
                    self.editor.timeline.playback.playing = false;
                    self.editor.timeline.playback.runtime = None;
                    self.editor.timeline.playback.pending_seek_time_seconds = None;
                    self.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;
                    if let Some(last_mode) = self.editor.runtime.interaction.last_mode.take() {
                        self.editor.set_mode(last_mode);
                    } else {
                        self.editor.set_mode(EditorMode::Place);
                    }
                    if simulate_preview {
                        self.mark_editor_dirty(
                            super::EditorDirtyFlags::block_mesh_and_hitboxes_changed(),
                        );
                    }
                    self.stop_audio();
                }
            } else if self.editor.input.mode != EditorMode::Timing {
                self.frame_runtime.player_render.gem_shatter_effects.clear();
                self.render_meshes_mut().gem_shatter_effects.clear();
                self.update_editor_scrub_trail_mesh();
            } else {
                self.frame_runtime.player_render.gem_shatter_effects.clear();
                self.render_meshes_mut().gem_shatter_effects.clear();
            }

            {
                puffin::profile_scope!("EditorCameraInputUpdate");
                if !self.editor_camera_pan_scheduled {
                    self.update_editor_pan_from_keys(frame_dt);
                }
                if !self.editor_camera_transition_scheduled {
                    self.update_editor_camera_transition(frame_dt);
                }
            }
            if (self.editor.runtime.interaction.gizmo_drag.is_some()
                || self.editor.runtime.interaction.block_drag.is_some())
                && let Some(pointer) = self.editor.input.pointer_screen
            {
                puffin::profile_scope!("DragSelection");
                self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
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
                || (self.editor.camera.editor_yaw - self.editor.runtime.gizmo.last_rotation).abs()
                    > 1e-4
                || (self.editor.camera.editor_pitch - self.editor.runtime.gizmo.last_pitch).abs()
                    > 1e-4;

            let has_selection = self.editor.input.selected_block_index.is_some()
                || !self.editor.input.selected_block_indices.is_empty();
            let is_dragging = self.editor.runtime.interaction.gizmo_drag.is_some()
                || self.editor.runtime.interaction.block_drag.is_some();

            if has_selection && self.editor.input.mode.shows_gizmo() {
                if is_dragging {
                    puffin::profile_scope!("GizmoRebuild");
                    self.rebuild_editor_gizmo_vertices();
                    self.editor.runtime.gizmo.rebuild_accumulator = 0.0;
                } else if camera_changed {
                    self.editor.runtime.gizmo.rebuild_accumulator += frame_dt;
                    if self.editor.runtime.gizmo.rebuild_accumulator >= (1.0 / 24.0) {
                        puffin::profile_scope!("GizmoRebuild");
                        self.rebuild_editor_gizmo_vertices();
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
            self.editor.runtime.gizmo.last_rotation = self.editor.camera.editor_yaw;
            self.editor.runtime.gizmo.last_pitch = self.editor.camera.editor_pitch;
            {
                puffin::profile_scope!("DirtyProcess");
                self.process_editor_dirty(frame_dt);
            }
            {
                puffin::profile_scope!("CameraArrowRebuild");
                self.rebuild_camera_arrow_vertices();
            }
            {
                puffin::profile_scope!("EditorCameraUniform");
                self.update_editor_camera();
            }
            self.editor_camera_pan_scheduled = false;
            self.editor_camera_transition_scheduled = false;
            return;
        }

        puffin::profile_scope!("PlayingUpdate");
        if self.is_game_paused() {
            self.frame_runtime.editor.accumulator = 0.0;
            self.update_playing_render_uniforms();
            return;
        }

        let trigger_render_objects = if self.gameplay_app_authoritative {
            None
        } else if self.gameplay.state.level_complete {
            self.gameplay.state.update(frame_dt);
            None
        } else {
            let target_time = self.target_playing_time(frame_dt);
            self.advance_playing_state_to_time(target_time, FIXED_DT)
        };
        self.frame_runtime.editor.accumulator = 0.0;
        self.update_block_effect_mesh(frame_dt);

        let render_objects = trigger_render_objects
            .as_deref()
            .unwrap_or(&self.gameplay.state.objects);
        if self.gameplay.state.has_animated_blocks()
            || trigger_render_objects.is_some()
            || self.editor.has_camera_timeline_triggers()
        {
            puffin::profile_scope!("PlayingAnimatedBlockMesh");
            let render_object_refs = render_objects
                .iter()
                .filter(|object| is_play_world_render_object(object))
                .collect::<Vec<_>>();
            let animated_batches = build_block_instance_batches_from_refs_at_time(
                &render_object_refs,
                Some(self.gameplay.state.elapsed_seconds),
            );
            self.with_render_meshes_and_device(|device, meshes| {
                meshes.blocks_instanced.replace_with_batches(
                    device,
                    "Block Instance Buffer",
                    &animated_batches,
                );
                meshes.blocks.replace_with_geometry(
                    device,
                    "Block Vertex Buffer",
                    &animated_batches.fallback_geometry,
                );
            });
            self.render_meshes_mut().blocks_chunked.clear();
        }

        if self.gameplay.state.level_complete && self.gameplay.state.completion_hold_seconds <= 0.0
        {
            self.record_current_level_progress();
            self.back_to_menu();
            return;
        }

        if self.gameplay.state.game_over {
            self.play_death_sfx_once();
            if !self.respawn_from_practice_checkpoint() {
                self.record_current_level_progress();
                self.stop_audio();
                self.clear_pending_gameplay_inputs();
            }
        }

        let mut trail_vertices = Vec::new();
        {
            puffin::profile_scope!("PlayingTrailBuild");
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
                    trail_vertices
                        .extend(build_trail_vertices(&points, self.gameplay.state.game_over));
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
        }

        self.with_render_meshes_and_queue(|queue, meshes| {
            meshes
                .trail
                .write_streaming_vertices(queue, &trail_vertices);
        });

        self.update_playing_render_uniforms();
    }

    fn update_playing_render_uniforms(&mut self) {
        self.frame_runtime.player_render.line_uniform.offset = [
            (self.gameplay.state.position[0] * 100.0).round() / 100.0,
            (self.gameplay.state.position[2] * 100.0).round() / 100.0,
        ];
        self.frame_runtime.player_render.line_uniform.rotation = match self.gameplay.state.direction
        {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        {
            puffin::profile_scope!("PlayingLineUniformUpload");
            self.render_gpu().queue.write_buffer(
                &self.render_gpu().line_uniform_buffer,
                0,
                bytemuck::bytes_of(&self.frame_runtime.player_render.line_uniform),
            );
        }

        let aspect = self.render_gpu().config.width as f32 / self.render_gpu().config.height as f32;
        let (eye, target) = self.playing_camera_view();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_position: [eye.x, eye.y, eye.z, 0.0],
        };

        {
            puffin::profile_scope!("PlayingCameraUniformUpload");
            self.render_gpu().queue.write_buffer(
                &self.render_gpu().camera_uniform_buffer,
                0,
                bytemuck::bytes_of(&camera_uniform),
            );
        }
    }

    fn update_menu_camera(&mut self) {
        puffin::profile_scope!("MenuCameraUniform");
        let aspect = self.render_gpu().config.width as f32 / self.render_gpu().config.height as f32;
        let eye = Vec3::from_array(self.menu.state.preview_camera_position);
        let target = Vec3::from_array(self.menu.state.preview_camera_target);
        let up = Vec3::new(0.0, 1.0, 0.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_position: [eye.x, eye.y, eye.z, 0.0],
        };

        self.render_gpu().queue.write_buffer(
            &self.render_gpu().camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    fn update_editor_camera(&mut self) {
        puffin::profile_scope!("EditorCameraUniformBuild");
        let aspect = self.render_gpu().config.width as f32 / self.render_gpu().config.height as f32;
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
            camera_position: [eye.x, eye.y, eye.z, 0.0],
        };

        self.render_gpu().queue.write_buffer(
            &self.render_gpu().camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use eg_audio::audio::runtime_asset_source_key;
    use eg_game::{GameState, TimelineSimulationRuntime};
    use eg_types::{
        AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraTrigger, Direction, EasingDirection, EditorMode,
        LevelObject, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TimingPoint, TriggerTarget,
    };

    fn sample_object() -> LevelObject {
        LevelObject {
            id: "object_0_0_0".to_string(),
            ..LevelObject::default()
        }
    }

    fn instant_move_trigger() -> LevelObject {
        LevelObject {
            position: [2.0, 0.0, 0.0],
            trigger: Some(TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: EasingDirection::Linear,
                target: TriggerTarget::Objects {
                    object_ids: vec!["object_0_0_0".to_string()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
        }
    }

    fn camera_trigger() -> LevelObject {
        LevelObject {
            id: "camera_trigger".to_string(),
            trigger: Some(TimedTrigger {
                target: TriggerTarget::Camera,
                camera: Some(CameraTrigger::default()),
                ..TimedTrigger::default()
            }),
            ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
        }
    }

    fn gameplay_floor() -> LevelObject {
        LevelObject {
            id: "floor".to_string(),
            position: [-10.0, -1.0, -10.0],
            size: [20.0, 1.0, 20.0],
            ..LevelObject::default()
        }
    }

    fn prepare_started_gameplay(state: &mut State, speed: f32) {
        state.phase = AppPhase::Playing;
        state.gameplay.state = GameState::new();
        state.gameplay.state.objects = vec![gameplay_floor()];
        state.gameplay.state.rebuild_behavior_cache();
        state
            .gameplay
            .state
            .apply_spawn_exact([0.0, 0.0, 0.0], Direction::Forward);
        state.gameplay.state.started = true;
        state.gameplay.state.speed = speed;
        state.clear_pending_gameplay_inputs();
    }

    #[tokio::test]
    async fn target_playing_time_respects_started_and_game_over_state() {
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
    }

    #[tokio::test]
    async fn queued_gameplay_turn_applies_at_its_timestamp() {
        let mut state = State::new_test().await;
        prepare_started_gameplay(&mut state, 12.0);

        state.queue_gameplay_turn_right_at(0.05);
        let _ = state.advance_playing_state_to_time(0.1, 1.0 / 120.0);

        assert!(matches!(state.gameplay.state.direction, Direction::Right));
        approx::assert_abs_diff_eq!(state.gameplay.state.position[2], 0.6, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(state.gameplay.state.position[0], 0.6, epsilon = 1e-4);
        assert!(state.gameplay.pending_turn_inputs.is_empty());
    }

    #[tokio::test]
    async fn active_gameplay_turn_right_queues_until_update() {
        let mut state = State::new_test().await;
        prepare_started_gameplay(&mut state, 10.0);

        state.turn_right();

        assert!(matches!(state.gameplay.state.direction, Direction::Forward));
        assert_eq!(state.gameplay.pending_turn_inputs.len(), 1);

        let _ = state.advance_playing_state_to_time(1.0 / 120.0, 1.0 / 120.0);

        assert!(matches!(state.gameplay.state.direction, Direction::Right));
        assert!(state.gameplay.pending_turn_inputs.is_empty());
    }

    #[tokio::test]
    async fn pending_seek_without_target_resets_cooldown_in_editor_playback() {
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
    }

    #[tokio::test]
    async fn playing_object_triggers_transform_objects_and_optionally_hitboxes() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Playing;
        state.gameplay.state.objects = vec![sample_object()];
        state.gameplay.state.rebuild_behavior_cache();

        state.editor.objects.push(instant_move_trigger());
        state.session.playing_trigger_base_objects = None;
        let transformed = state
            .playing_trigger_objects_at_time(0.5)
            .expect("expected transformed objects");
        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
        assert_eq!(state.gameplay.state.objects[0].position, [0.0, 0.0, 0.0]);
    }

    #[tokio::test]
    async fn update_playing_state_clears_chunked_mesh_when_trigger_render_objects_are_active() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Playing;
        state.gameplay.state.started = true;
        state.gameplay.state.objects = vec![sample_object()];
        state.gameplay.state.rebuild_behavior_cache();
        state.editor.objects.push(instant_move_trigger());

        state.rebuild_block_vertices();
        assert!(!state.render_meshes().blocks_chunked.is_empty());

        state.update();

        assert!(state.render_meshes().blocks_chunked.is_empty());
        assert!(!state.render_meshes().blocks_instanced.is_empty());
    }

    #[tokio::test]
    async fn update_playing_state_hides_camera_triggers_from_trigger_render_mesh() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Playing;
        state.gameplay.state.started = true;
        state.gameplay.state.objects = vec![sample_object(), camera_trigger()];
        state.gameplay.state.rebuild_behavior_cache();
        state.editor.objects.push(instant_move_trigger());

        state.update();

        assert!(state.gameplay.state.objects.iter().any(|object| {
            object
                .trigger
                .as_ref()
                .is_some_and(|trigger| trigger.camera.is_some())
        }));
        assert!(!state.render_meshes().blocks_instanced.is_empty());
        assert!(state.render_meshes().blocks.draw_data().is_none());
    }

    #[tokio::test]
    async fn advance_playing_state_to_time_updates_elapsed_and_returns_render_objects() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Playing;
        state.gameplay.state.started = true;
        state.gameplay.state.objects = vec![sample_object()];
        state.gameplay.state.rebuild_behavior_cache();
        state.editor.objects.push(instant_move_trigger());

        let transformed = state
            .advance_playing_state_to_time(0.2, 1.0 / 120.0)
            .expect("expected trigger render objects");

        assert!(state.gameplay.state.elapsed_seconds <= 0.2 + 1e-6);
        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);
    }

    #[tokio::test]
    async fn playback_trail_vertices_include_airborne_head_segment() {
        let grounded_runtime =
            TimelineSimulationRuntime::new([0.0, 0.0, 0.0], Direction::Forward, &[], &[]);
        assert!(grounded_runtime.is_grounded());
        let grounded_vertices = State::build_editor_playback_trail_vertices(&grounded_runtime);

        let support = [LevelObject {
            position: [0.0, 2.0, 0.0],
            ..LevelObject::default()
        }];
        let mut airborne_runtime =
            TimelineSimulationRuntime::new([0.0, 3.0, 0.0], Direction::Forward, &support, &[]);
        airborne_runtime.advance_to(1.0 / eg_game::BASE_PLAYER_SPEED + 0.01);
        assert!(!airborne_runtime.is_grounded());
        let airborne_vertices = State::build_editor_playback_trail_vertices(&airborne_runtime);

        assert!(grounded_vertices.is_empty());
        assert!(!airborne_vertices.is_empty());
    }

    #[tokio::test]
    async fn recent_trail_segments_keeps_latest_points_with_bound() {
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

    #[tokio::test]
    async fn scrub_trail_uses_sparse_trail_cache_not_dense_snapshots() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.timeline.snapshot_cache_step_seconds = 1.0 / 240.0;
        state.editor.timeline.trail_point_cache = vec![[0.0, 0.0, 0.0], [24.0, 0.0, 0.0]];
        state.editor.timeline.trail_segment_start_cache = vec![0];
        state.editor.timeline.snapshot_cache = (0..=2_000)
            .map(
                |index| crate::state::editor_timeline::EditorTimelineSnapshot {
                    position: [index as f32 * 0.1, 0.0, 0.0],
                    direction: Direction::Forward,
                    is_grounded: true,
                    game_over: false,
                    trail_segment_count: 1,
                    trail_point_count: 2,
                },
            )
            .collect();

        let vertices = state.build_editor_scrub_trail_vertices_from_cache(2_000.0 / 240.0);

        assert!(
            vertices.len() < 120,
            "dense timeline samples must not be emitted as trail geometry"
        );
    }

    #[tokio::test]
    async fn scrub_trail_draws_grounded_head_from_fresh_cache() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.objects.clear();
        state.editor.spawn.position = [0.0, 0.0, 0.0];
        state.editor.spawn.direction = Direction::Forward;
        state.editor.timeline.clock.duration_seconds = 2.0;
        state.editor.invalidate_samples();
        state.rebuild_editor_timeline_snapshot_cache_if_needed();

        let vertices = state.build_editor_scrub_trail_vertices_from_cache(0.5);

        assert!(
            !state.editor.timeline.trail_point_cache.is_empty(),
            "fresh cache should store the initial trail anchor"
        );
        assert!(
            !vertices.is_empty(),
            "grounded scrub trail should draw before a fall or turn creates a new segment"
        );
    }

    #[tokio::test]
    async fn perf_overlay_toggle_controls_visibility() {
        let mut state = State::new_test().await;

        assert!(!state.perf_overlay_enabled());

        state.toggle_perf_overlay();
        assert!(state.perf_overlay_enabled());

        state.toggle_perf_overlay();
        assert!(!state.perf_overlay_enabled());
    }

    #[tokio::test]
    async fn pending_seek_resync_waits_for_cooldown_then_applies_target_time() {
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
    }

    #[tokio::test]
    async fn pending_seek_resync_early_returns_when_not_editor_or_not_playing() {
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
    }

    #[tokio::test]
    async fn playing_trigger_objects_at_time_requires_playing_phase_and_transform_triggers() {
        let mut state = State::new_test().await;
        state.gameplay.state.objects = vec![sample_object()];
        state.gameplay.state.rebuild_behavior_cache();

        state.phase = AppPhase::Menu;
        state.editor.objects.push(instant_move_trigger());
        assert!(state.playing_trigger_objects_at_time(0.2).is_none());

        state.phase = AppPhase::Playing;
        state.editor.objects.clear();
        assert!(state.playing_trigger_objects_at_time(0.2).is_none());

        state.editor.objects.push(instant_move_trigger());
        let transformed = state
            .playing_trigger_objects_at_time(0.2)
            .expect("expected transformed objects");
        assert_eq!(transformed[0].position, [2.0, 0.0, 0.0]);

        state.session.playing_trigger_base_objects = Some(vec![sample_object(), sample_object()]);
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
    }

    #[tokio::test]
    async fn update_editor_scrub_trail_mesh_uses_snapshot_cache_without_runtime() {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Editor;
        state.editor.timeline.clock.duration_seconds = 8.0;
        state.editor.timeline.clock.time_seconds = 3.0;
        state.editor.timeline.scrub_runtime = None;
        state.editor.timeline.simulation_revision = 2;
        state.editor.invalidate_samples();

        state.update_editor_scrub_trail_mesh();
        assert!(state.editor.timeline.scrub_runtime.is_none());
        assert_eq!(
            state.editor.timeline.snapshot_cache_revision,
            state.editor.timeline.simulation_revision
        );

        state.editor.timeline.clock.time_seconds = 2.0;

        state.update_editor_scrub_trail_mesh();
        assert!(state.editor.timeline.scrub_runtime.is_none());
    }

    #[tokio::test]
    async fn update_menu_branch_resets_accumulator() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Menu;
        state.frame_runtime.editor.accumulator = 0.2;

        state.update();

        assert_eq!(state.frame_runtime.editor.accumulator, 0.0);
    }

    #[tokio::test]
    async fn update_does_not_drain_async_audio_import_channel() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.set_editor_level_name("ScheduledAsyncOnly".to_string());
        state
            .audio
            .state
            .editor
            .audio_import_channel
            .0
            .send(("audio.mp3".to_string(), vec![5, 6, 7]))
            .expect("audio import send should succeed");

        state.update();

        let source_key = runtime_asset_source_key("ScheduledAsyncOnly", "audio.mp3");
        assert_eq!(
            state.audio.state.editor.cache.bytes_for_source(&source_key),
            None,
            "State::update should leave async channel draining to StateSchedule"
        );

        state.update_audio_imports();
        assert_eq!(
            state.audio.state.editor.cache.bytes_for_source(&source_key),
            Some(&vec![5, 6, 7]),
            "the legacy helper still drains the queued result when called directly"
        );
    }

    #[tokio::test]
    async fn update_editor_playback_in_timing_mode_keeps_runtime_none_and_handles_pending_seek() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Timing;
        state.editor.timeline.clock.duration_seconds = 4.0;
        state.editor.timeline.clock.time_seconds = 0.5;
        state.editor.timeline.playback.playing = true;
        state.editor.timeline.playback.pending_seek_time_seconds = Some(1.5);
        state.editor.timeline.playback.seek_resync_cooldown_seconds = 0.0;

        state.update();

        assert!(state.editor.timeline.playback.runtime.is_none());
        assert!(state.editor.timeline.clock.time_seconds >= 0.5);
    }

    #[tokio::test]
    async fn update_editor_playback_stops_when_audio_not_playing() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Place;
        state.editor.timeline.clock.duration_seconds = 10.0;
        state.editor.timeline.clock.time_seconds = 2.0;
        state.editor.timeline.playback.playing = true;
        state.editor.timeline.playback.runtime = Some(TimelineSimulationRuntime::new(
            state.editor.spawn.position,
            state.editor.spawn.direction,
            &state.editor.objects,
            &state.editor.timeline.taps.tap_times,
        ));

        // In tests, no active playback source means runtime reports not playing.
        state.update();

        assert!(!state.editor.timeline.playback.playing);
        assert!(state.editor.timeline.playback.runtime.is_none());
        assert!(
            state
                .editor
                .timeline
                .playback
                .pending_seek_time_seconds
                .is_none()
        );
    }

    #[tokio::test]
    async fn update_playing_phase_level_complete_returns_to_menu() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Playing;
        state.gameplay.state.level_complete = true;
        state.gameplay.state.completion_hold_seconds = 0.0;

        state.update();

        assert_eq!(state.phase, AppPhase::Menu);
    }

    #[tokio::test]
    async fn update_playtest_level_complete_returns_to_editor_after_hold() {
        let mut state = State::new_test().await;
        state.enter_editor_phase("PlaytestComplete".to_string());
        state.editor.objects = vec![sample_object()];
        state.editor_playtest();
        state.gameplay.state.level_complete = true;
        state.gameplay.state.completion_hold_seconds = 0.0;

        state.update();

        assert_eq!(state.phase, AppPhase::Editor);
        assert!(!state.session.playtesting_editor);
        assert_eq!(state.gameplay.state.objects, state.editor.objects);
    }

    #[tokio::test]
    async fn update_playing_phase_game_over_keeps_phase_and_stops_audio_path() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Playing;
        state.gameplay.state.started = true;
        state.gameplay.state.game_over = true;
        state.gameplay.state.objects = vec![sample_object()];
        state.gameplay.state.rebuild_behavior_cache();

        assert!(!state.gameplay.death_sfx_played);
        state.update();

        assert_eq!(state.phase, AppPhase::Playing);
        assert!(state.gameplay.state.game_over);
        assert!(state.gameplay.death_sfx_played);

        state.update();

        assert!(state.gameplay.death_sfx_played);
    }

    #[tokio::test]
    async fn update_playing_phase_builds_trail_and_line_uniform_for_right_direction() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Playing;
        state.gameplay.state.started = false;
        state.gameplay.state.direction = eg_types::Direction::Right;
        state.gameplay.state.position = [0.0, 0.0, 0.0];
        state.gameplay.state.is_grounded = false;
        state.gameplay.state.trail_segments =
            vec![vec![[900.0, 0.0, 900.0]], vec![[0.0, 0.0, 0.0]]];

        state.update();

        assert_eq!(
            state.frame_runtime.player_render.line_uniform.rotation,
            -std::f32::consts::FRAC_PI_2
        );
        assert!(state.render_meshes().trail.draw_data().is_some());
    }

    #[tokio::test]
    async fn update_playing_phase_paused_does_not_advance_gameplay() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Playing;
        state.session.playtesting_editor = false;
        state.session.game_paused = true;
        state.gameplay.state.started = true;
        state.gameplay.state.elapsed_seconds = 1.0;
        state.gameplay.state.position = [2.0, 0.0, 3.0];

        state.update();

        assert_eq!(state.phase, AppPhase::Playing);
        assert_eq!(state.gameplay.state.elapsed_seconds, 1.0);
        assert_eq!(state.gameplay.state.position, [2.0, 0.0, 3.0]);
        assert_eq!(state.frame_runtime.editor.accumulator, 0.0);
    }

    #[tokio::test]
    async fn update_editor_timing_playback_advances_with_pending_seek_and_timing_points() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.input.mode = EditorMode::Timing;
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
    }
}
