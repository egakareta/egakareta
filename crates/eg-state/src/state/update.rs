/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(test)]
use super::State;
#[cfg(test)]
use eg_blocks::{BlockDefinition, BlockRenderProfile};
#[cfg(test)]
use eg_game::TimelineSimulationRuntime;
#[cfg(test)]
use eg_game::trigger_transformed_objects_at_time;
#[cfg(test)]
use eg_mesh::build_trail_vertices_with_alpha;
#[cfg(test)]
use eg_types::AppPhase;
#[cfg(test)]
use eg_types::{Direction, EditorMode, LevelObject};

#[cfg(test)]
fn is_play_world_render_object(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile != BlockRenderProfile::CameraTrigger
}

#[cfg(test)]
impl State {
    #[cfg(test)]
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

        Some(trigger_transformed_objects_at_time(
            base_objects,
            &self.editor.trigger_objects(),
            time_seconds.max(0.0),
        ))
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    /// Advances the application state by one frame.
    ///
    /// This method handles:
    /// - Starting auth refreshes for existing sessions.
    /// - Consuming the scheduled frame delta for the fixed-step simulation.
    /// - Updating the active subsystem (Menu, Editor, or Gameplay) logic.
    #[cfg(test)]
    pub fn update(&mut self) {
        puffin::profile_scope!("FrameTotal");
        #[cfg(test)]
        let frame_dt = self.frame_runtime.editor.delta_seconds;
        if self.phase == AppPhase::Menu {
            return;
        }

        if self.phase == AppPhase::Editor {
            puffin::profile_scope!("EditorUpdate");
            #[cfg(test)]
            {
                self.render_meshes_mut().trail.clear();

                if self.editor.timeline.playback.playing {
                    if !self.editor_is_effectively_timing_mode() {
                        self.update_editor_playback_trail_mesh();
                    } else {
                        self.render_meshes_mut().trail.clear();
                    }
                } else if self.editor.input.mode != EditorMode::Timing {
                    self.update_editor_scrub_trail_mesh();
                }
            }

            if (self.editor.runtime.interaction.gizmo_drag.is_some()
                || self.editor.runtime.interaction.block_drag.is_some())
                && let Some(pointer) = self.editor.input.pointer_screen
            {
                puffin::profile_scope!("DragSelection");
                self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
            }

            #[cfg(test)]
            {
                puffin::profile_scope!("DirtyProcess");
                self.process_editor_dirty(frame_dt);
            }
            return;
        }

        puffin::profile_scope!("PlayingUpdate");
        if self.is_game_paused() {
            return;
        }
        #[cfg(test)]
        {
            let trigger_render_objects =
                self.playing_trigger_objects_at_time(self.gameplay.state.elapsed_seconds);
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
                let animated_batches = eg_mesh::build_block_instance_batches_from_refs_at_time(
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use eg_audio::audio::runtime_asset_source_key;
    use eg_game::TimelineSimulationRuntime;
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
    async fn update_playing_phase_builds_trail_for_airborne_player() {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Playing;
        state.gameplay.state.started = false;
        state.gameplay.state.direction = eg_types::Direction::Right;
        state.gameplay.state.position = [0.0, 0.0, 0.0];
        state.gameplay.state.is_grounded = false;
        state.gameplay.state.trail_segments =
            vec![vec![[900.0, 0.0, 900.0]], vec![[0.0, 0.0, 0.0]]];

        state.update();

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
