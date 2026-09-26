/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Shared App-world/editor timeline snapshot-cache rebuild.

use bevy_ecs::prelude::*;
use bevy_state::prelude::State;
use eg_editor_domain::{TapDivisionPreviewRange, derive_timing_division_tap_previews};
use eg_game::TimelineSimulationRuntime;
use eg_types::{AppPhase, EditorMode};
#[cfg(test)]
use eg_types::{LevelObject, SpawnMetadata};

use super::{
    EditorRuntimeRes, EditorSimulationInputsRes, EditorSpawnRes, EditorTimelineRes,
    EditorTimingRes,
    editor_objects::EditorObjectRevision,
    editor_timeline::{EditorTimelineSnapshot, EditorTimelineState},
    state_resources::EditorSimulationInputs,
};

const MAX_SNAPSHOT_SAMPLES_PER_FRAME: usize = 4;

pub(crate) fn sync_editor_simulation_inputs_system(
    runtime: Res<EditorRuntimeRes>,
    revision: Res<EditorObjectRevision>,
    timeline: Res<EditorTimelineRes>,
    spawn: Res<EditorSpawnRes>,
    timing: Res<EditorTimingRes>,
    authored_objects: Query<
        (
            &super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::editor_objects::EditorAuthoredObject>,
    >,
    mut inputs: ResMut<EditorSimulationInputsRes>,
) {
    if runtime.0.interaction.block_drag.is_some() || runtime.0.interaction.gizmo_drag.is_some() {
        return;
    }

    let object_changed = inputs.0.object_revision != revision.0;
    let timeline_inputs_changed = inputs.0.spawn_position != spawn.0.position
        || inputs.0.spawn_direction != spawn.0.direction
        || inputs.0.tap_times != timeline.0.taps.tap_times
        || inputs.0.duration_seconds != timeline.0.clock.duration_seconds
        || inputs.0.snapshot_cache_step_seconds != timeline.0.snapshot_cache_step_seconds
        || inputs.0.timing_points != timing.0.timing_points;
    if !object_changed && !timeline_inputs_changed {
        return;
    }

    if !object_changed {
        inputs.0.spawn_position = spawn.0.position;
        inputs.0.spawn_direction = spawn.0.direction;
        inputs.0.tap_times = timeline.0.taps.tap_times.clone();
        inputs.0.duration_seconds = timeline.0.clock.duration_seconds;
        inputs.0.snapshot_cache_step_seconds = timeline.0.snapshot_cache_step_seconds;
        inputs.0.timing_points = timing.0.timing_points.clone();
        return;
    }

    let objects = super::editor_objects::collect_editor_object_data(&authored_objects);
    let next_inputs = EditorSimulationInputs {
        spawn_position: spawn.0.position,
        spawn_direction: spawn.0.direction,
        objects,
        tap_times: timeline.0.taps.tap_times.clone(),
        duration_seconds: timeline.0.clock.duration_seconds,
        snapshot_cache_step_seconds: timeline.0.snapshot_cache_step_seconds,
        timing_points: timing.0.timing_points.clone(),
        object_revision: revision.0,
    };

    // A revision change means the authored set actually mutated, so the fresh
    // collect necessarily differs: assign directly instead of comparing two
    // 19k vectors first (the `!=` walk alone costs milliseconds per edit).
    if object_changed || inputs.0 != next_inputs {
        inputs.0 = next_inputs;
    }
}

pub(crate) fn reset_editor_timeline_derived_caches_system(
    inputs: Res<EditorSimulationInputsRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    if inputs.is_changed() {
        timeline.0.clear_derived_caches();
    }
}

pub(crate) fn rebuild_editor_timeline_tap_preview_cache_system(
    phase: Res<State<AppPhase>>,
    input: Res<super::EditorInputRes>,
    inputs: Res<EditorSimulationInputsRes>,
    mut timeline: ResMut<EditorTimelineRes>,
    mut was_tapping: Local<bool>,
) {
    let is_tapping = *phase.get() == AppPhase::Editor && input.0.mode == EditorMode::Tapping;
    let entered_tapping = is_tapping && !*was_tapping;
    *was_tapping = is_tapping;

    if !is_tapping || (!inputs.is_changed() && !entered_tapping) {
        return;
    }

    let duration_seconds = inputs.0.duration_seconds.max(0.0);
    timeline.0.tap_division_preview_cache = derive_timing_division_tap_previews(
        inputs.0.spawn_position,
        inputs.0.spawn_direction,
        &inputs.0.tap_times,
        &inputs.0.timing_points,
        duration_seconds,
        TapDivisionPreviewRange {
            start_seconds: 0.0,
            end_seconds: duration_seconds,
        },
        &inputs.0.objects,
    );
}

/// Keep the App-owned timeline cache current before render extraction.
pub(crate) fn rebuild_editor_timeline_snapshot_cache_system(
    phase: Res<State<AppPhase>>,
    input: Res<super::EditorInputRes>,
    inputs: Res<EditorSimulationInputsRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    if *phase.get() != AppPhase::Editor || input.0.mode != EditorMode::Tapping {
        return;
    }

    rebuild_editor_timeline_snapshot_cache_chunk(&mut timeline.0, &inputs.0);
}

fn rebuild_editor_timeline_snapshot_cache_chunk(
    timeline: &mut EditorTimelineState,
    inputs: &EditorSimulationInputs,
) {
    if timeline.snapshot_cache_complete {
        return;
    }

    let duration_seconds = inputs.duration_seconds.max(0.0);
    let step_seconds = inputs.snapshot_cache_step_seconds.max(1.0 / 480.0);
    let total_sample_count = ((duration_seconds / step_seconds).ceil() as usize).saturating_add(1);
    let mut runtime = timeline.snapshot_cache_runtime.take().unwrap_or_else(|| {
        timeline.snapshot_cache.clear();
        timeline.trail_point_cache.clear();
        timeline.trail_segment_start_cache.clear();
        timeline.snapshot_cache_next_index = 0;
        TimelineSimulationRuntime::new(
            inputs.spawn_position,
            inputs.spawn_direction,
            &inputs.objects,
            &inputs.tap_times,
        )
    });
    let start_index = timeline.snapshot_cache_next_index.min(total_sample_count);
    let end_index = start_index
        .saturating_add(MAX_SNAPSHOT_SAMPLES_PER_FRAME)
        .min(total_sample_count.max(1));
    let mut trail_segment_lengths = cached_trail_segment_lengths(timeline);
    timeline
        .snapshot_cache
        .reserve(end_index.saturating_sub(start_index));

    puffin::profile_scope!("TimelineSamplesChunk");
    for index in start_index..end_index {
        let sample_time = (index as f32 * step_seconds).min(duration_seconds);
        runtime.advance_to(sample_time);
        append_trail_cache(
            timeline,
            runtime.trail_segments(),
            &mut trail_segment_lengths,
        );
        let snapshot = runtime.snapshot();
        timeline.snapshot_cache.push(EditorTimelineSnapshot {
            position: snapshot.position,
            direction: snapshot.direction,
            is_grounded: snapshot.is_grounded,
            game_over: runtime.game_over(),
            trail_segment_count: timeline.trail_segment_start_cache.len(),
            trail_point_count: timeline.trail_point_cache.len(),
        });
    }

    timeline.snapshot_cache_next_index = end_index;
    if end_index >= total_sample_count {
        timeline.snapshot_cache_complete = true;
        timeline.snapshot_cache_next_index = 0;
        timeline.scrub_runtime = None;
    } else {
        timeline.snapshot_cache_runtime = Some(runtime);
    }
}

/// Rebuild the timeline snapshot/trail cache when the authored simulation
/// revision changes. The App schedule and test helpers use this function so
/// the render extraction path sees current editor data.
#[cfg(test)]
pub(crate) fn rebuild_editor_timeline_snapshot_cache_if_needed(
    timeline: &mut EditorTimelineState,
    objects: &[LevelObject],
    spawn: &SpawnMetadata,
) {
    if timeline.snapshot_cache_complete {
        return;
    }

    puffin::profile_scope!("TimelineSamples");
    let duration_seconds = timeline.clock.duration_seconds.max(0.0);
    let step_seconds = timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
    let total_sample_count = ((duration_seconds / step_seconds).ceil() as usize).saturating_add(1);

    let existing_count = timeline.snapshot_cache.len();
    let resume_index = if existing_count > 0 {
        existing_count
    } else {
        timeline.snapshot_cache.clear();
        timeline.trail_point_cache.clear();
        timeline.trail_segment_start_cache.clear();
        0
    };

    let mut runtime = {
        puffin::profile_scope!("TimelineSamplesRuntimeBuild");
        TimelineSimulationRuntime::new(
            spawn.position,
            spawn.direction,
            objects,
            &timeline.taps.tap_times,
        )
    };

    if resume_index > 0 {
        let resume_time = ((resume_index - 1) as f32 * step_seconds).min(duration_seconds);
        puffin::profile_scope!("TimelineSamplesResumeCatchUp");
        runtime.advance_to(resume_time);
    }

    let mut trail_segment_lengths = cached_trail_segment_lengths(timeline);
    timeline
        .snapshot_cache
        .reserve(total_sample_count.saturating_sub(existing_count));

    puffin::profile_scope!("TimelineSamplesSampleLoop");
    for index in resume_index..total_sample_count.max(1) {
        let sample_time = (index as f32 * step_seconds).min(duration_seconds);
        runtime.advance_to(sample_time);
        append_trail_cache(
            timeline,
            runtime.trail_segments(),
            &mut trail_segment_lengths,
        );
        let snapshot = runtime.snapshot();
        timeline.snapshot_cache.push(EditorTimelineSnapshot {
            position: snapshot.position,
            direction: snapshot.direction,
            is_grounded: snapshot.is_grounded,
            game_over: runtime.game_over(),
            trail_segment_count: timeline.trail_segment_start_cache.len(),
            trail_point_count: timeline.trail_point_cache.len(),
        });
    }

    timeline.snapshot_cache_complete = true;
    timeline.scrub_runtime = None;
}

fn append_trail_cache(
    timeline: &mut EditorTimelineState,
    trail_segments: &[Vec<[f32; 3]>],
    cached_segment_lengths: &mut Vec<usize>,
) {
    for (segment_index, segment) in trail_segments.iter().enumerate() {
        if segment_index >= cached_segment_lengths.len() {
            timeline
                .trail_segment_start_cache
                .push(timeline.trail_point_cache.len());
            timeline.trail_point_cache.extend(segment.iter().copied());
            cached_segment_lengths.push(segment.len());
            continue;
        }

        let cached_len = cached_segment_lengths[segment_index];
        if segment.len() > cached_len {
            timeline
                .trail_point_cache
                .extend(segment[cached_len..].iter().copied());
            cached_segment_lengths[segment_index] = segment.len();
        }
    }
}

fn cached_trail_segment_lengths(timeline: &EditorTimelineState) -> Vec<usize> {
    let point_count = timeline.trail_point_cache.len();
    timeline
        .trail_segment_start_cache
        .iter()
        .enumerate()
        .map(|(segment_index, start)| {
            let end = timeline
                .trail_segment_start_cache
                .get(segment_index + 1)
                .copied()
                .unwrap_or(point_count)
                .min(point_count);
            end.saturating_sub((*start).min(point_count))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{schedule::Schedule, world::World};
    use bevy_state::prelude::State;
    use eg_types::{AppPhase, EditorMode, TimingPoint};

    use super::rebuild_editor_timeline_tap_preview_cache_system;
    use crate::state::{
        EditorInputRes, EditorSimulationInputsRes, EditorTimelineRes,
        state_resources::EditorSimulationInputs,
    };

    #[test]
    fn timing_division_preview_rebuild_waits_until_tapping_mode() {
        let mut world = World::new();
        world.insert_resource(State::new(AppPhase::Editor));
        world.insert_resource(EditorInputRes::default());
        world.insert_resource(EditorSimulationInputsRes(EditorSimulationInputs {
            duration_seconds: 0.25,
            timing_points: vec![TimingPoint {
                time_seconds: 0.0,
                bpm: 600.0,
                ..Default::default()
            }],
            ..Default::default()
        }));
        world.insert_resource(EditorTimelineRes::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(rebuild_editor_timeline_tap_preview_cache_system);

        schedule.run(&mut world);
        assert!(
            world
                .resource::<EditorTimelineRes>()
                .0
                .tap_division_preview_cache
                .is_empty()
        );

        world.resource_mut::<EditorInputRes>().0.mode = EditorMode::Tapping;
        schedule.run(&mut world);
        assert!(
            !world
                .resource::<EditorTimelineRes>()
                .0
                .tap_division_preview_cache
                .is_empty()
        );
    }
}
