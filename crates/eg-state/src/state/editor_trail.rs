/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Editor trail vertex extraction from App-owned timeline state.

use eg_game::TimelineSimulationRuntime;
use eg_mesh::{Vertex, build_trail_vertices_with_alpha};
use eg_types::Direction;

use super::editor_timeline::{EditorTimelineSnapshot, EditorTimelineState};

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

/// Build the translucent trail for active editor playback.
pub(crate) fn build_editor_playback_trail_vertices(
    runtime: &TimelineSimulationRuntime,
) -> Vec<Vertex> {
    puffin::profile_scope!("EditorPlaybackTrailBuild");
    const EDITOR_PLAYBACK_TRAIL_ALPHA: f32 = 0.45;
    const POSITION_EPSILON: f32 = 0.001;
    const MAX_RENDERED_EDITOR_TRAIL_POINTS: usize = 1024;

    let mut trail_vertices = Vec::new();
    let player_pos = runtime.position();
    let is_grounded = runtime.is_grounded();
    let is_game_over = runtime.game_over();
    let trail_segments = runtime.trail_segments();
    let recent_segments = recent_trail_segments(trail_segments, MAX_RENDERED_EDITOR_TRAIL_POINTS);

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
        if dx.abs() > POSITION_EPSILON || dy.abs() > POSITION_EPSILON || dz.abs() > POSITION_EPSILON
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
        trail_vertices.extend(build_trail_vertices_with_alpha(
            &[head_start, player_pos],
            is_game_over,
            EDITOR_PLAYBACK_TRAIL_ALPHA,
        ));
    }

    trail_vertices
}

/// Build the translucent trail for a non-playing editor timeline position.
pub(crate) fn build_editor_scrub_trail_vertices(
    timeline: &EditorTimelineState,
    target_time: f32,
) -> Vec<Vertex> {
    const EDITOR_SCRUB_TRAIL_ALPHA: f32 = 0.45;
    const POSITION_EPSILON: f32 = 0.001;
    const MAX_RENDERED_EDITOR_TRAIL_POINTS: usize = 1024;

    if timeline.snapshot_cache.is_empty() {
        return Vec::new();
    }

    let step_seconds = timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
    let max_index = timeline.snapshot_cache.len().saturating_sub(1);
    let target_index = ((target_time.max(0.0) / step_seconds).floor() as usize).min(max_index);
    let snapshot = &timeline.snapshot_cache[target_index];
    let trail_segments = cached_editor_trail_segments(timeline, snapshot);
    let recent_segments = recent_trail_segments(&trail_segments, MAX_RENDERED_EDITOR_TRAIL_POINTS);

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
        if dx.abs() > POSITION_EPSILON || dy.abs() > POSITION_EPSILON || dz.abs() > POSITION_EPSILON
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
        trail_vertices.extend(build_trail_vertices_with_alpha(
            &[head_start, snapshot.position],
            snapshot.game_over,
            EDITOR_SCRUB_TRAIL_ALPHA,
        ));
    }

    trail_vertices
}

fn cached_editor_trail_segments(
    timeline: &EditorTimelineState,
    snapshot: &EditorTimelineSnapshot,
) -> Vec<Vec<[f32; 3]>> {
    let mut segments = Vec::new();
    let segment_count = snapshot
        .trail_segment_count
        .min(timeline.trail_segment_start_cache.len());
    let point_count = snapshot
        .trail_point_count
        .min(timeline.trail_point_cache.len());

    for segment_index in 0..segment_count {
        let start = timeline.trail_segment_start_cache[segment_index];
        let end = if segment_index + 1 < segment_count {
            timeline.trail_segment_start_cache[segment_index + 1]
        } else {
            point_count
        }
        .min(point_count);

        if start < end {
            segments.push(timeline.trail_point_cache[start..end].to_vec());
        }
    }

    segments
}
