/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Shared App-world/editor compatibility timeline snapshot-cache rebuild.

use bevy_ecs::prelude::*;
use bevy_state::prelude::State;
use eg_game::TimelineSimulationRuntime;
use eg_types::{AppPhase, LevelObject, SpawnMetadata};

use super::{
    EditorSpawnRes, EditorTimelineRes,
    editor_timeline::{EditorTimelineSnapshot, EditorTimelineState},
};

/// Keep the App-owned timeline cache current before render extraction.
pub(crate) fn rebuild_editor_timeline_snapshot_cache_system(
    phase: Res<State<AppPhase>>,
    authored_objects: Query<
        (
            &super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::editor_objects::EditorAuthoredObject>,
    >,
    spawn: Res<EditorSpawnRes>,
    mut timeline: ResMut<EditorTimelineRes>,
) {
    if *phase.get() != AppPhase::Editor {
        return;
    }

    let objects = super::editor_objects::collect_editor_object_data(&authored_objects);
    rebuild_editor_timeline_snapshot_cache_if_needed(&mut timeline.0, &objects, &spawn.0);
}

/// Rebuild the timeline snapshot/trail cache when the authored simulation
/// revision changes. The App schedule and test helpers use this function so
/// the render extraction path sees current editor data.
pub(crate) fn rebuild_editor_timeline_snapshot_cache_if_needed(
    timeline: &mut EditorTimelineState,
    objects: &[LevelObject],
    spawn: &SpawnMetadata,
) {
    if timeline.snapshot_cache_revision == timeline.simulation_revision
        && !timeline.snapshot_cache.is_empty()
    {
        return;
    }

    puffin::profile_scope!("TimelineSamples");
    let duration_seconds = timeline.clock.duration_seconds.max(0.0);
    let step_seconds = timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
    let total_sample_count = ((duration_seconds / step_seconds).ceil() as usize).saturating_add(1);

    let existing_count = timeline.snapshot_cache.len();
    let resume_index = if existing_count > 0 && timeline.snapshot_cache_revision == 0 {
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

    timeline.snapshot_cache_revision = timeline.simulation_revision;
    timeline.scrub_runtime = None;
    timeline.scrub_runtime_revision = 0;
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
