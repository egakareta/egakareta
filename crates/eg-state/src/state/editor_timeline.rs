/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::{TapDivisionPreview, interpolate_timeline_sample_positions};
use eg_game::TimelineSimulationRuntime;
use eg_types::Direction;
use smart_default::SmartDefault;

#[derive(Clone, Copy)]
pub struct EditorTimelineSnapshot {
    pub position: [f32; 3],
    pub direction: Direction,
    pub is_grounded: bool,
    pub game_over: bool,
    pub trail_segment_count: usize,
    pub trail_point_count: usize,
}

#[derive(Default)]
pub struct EditorTimelinePlaybackState {
    pub playing: bool,
    pub runtime: Option<TimelineSimulationRuntime>,
    pub pending_seek_time_seconds: Option<f32>,
    pub seek_resync_cooldown_seconds: f32,
}

#[derive(Default)]
pub struct EditorTimelinePreviewState {
    pub position: [f32; 3],
    pub direction: Direction,
}

#[derive(SmartDefault)]
pub struct EditorTimelineClockState {
    pub time_seconds: f32,
    #[default = 16.0]
    pub duration_seconds: f32,
}

#[derive(Default)]
pub struct EditorTimelineTapState {
    pub tap_times: Vec<f32>,
    pub tap_indicator_positions: Vec<[f32; 3]>,
    pub selected_index: Option<usize>,
}

#[derive(SmartDefault)]
pub struct EditorTimelineState {
    pub clock: EditorTimelineClockState,
    pub preview: EditorTimelinePreviewState,
    pub taps: EditorTimelineTapState,
    pub tap_cursor_override: Option<[f32; 3]>,
    pub playback: EditorTimelinePlaybackState,
    #[default(1.0 / 240.0)]
    pub snapshot_cache_step_seconds: f32,
    pub snapshot_cache_complete: bool,
    pub snapshot_cache: Vec<EditorTimelineSnapshot>,
    pub(crate) snapshot_cache_runtime: Option<TimelineSimulationRuntime>,
    pub(crate) snapshot_cache_next_index: usize,
    pub trail_point_cache: Vec<[f32; 3]>,
    pub trail_segment_start_cache: Vec<usize>,
    pub scrub_runtime: Option<TimelineSimulationRuntime>,
    pub tap_division_preview_cache: Vec<TapDivisionPreview>,
}

impl EditorTimelineState {
    pub(crate) fn clear_derived_caches(&mut self) {
        self.snapshot_cache_complete = false;
        self.snapshot_cache.clear();
        self.snapshot_cache_runtime = None;
        self.snapshot_cache_next_index = 0;
        self.trail_point_cache.clear();
        self.trail_segment_start_cache.clear();
        self.scrub_runtime = None;
        self.tap_division_preview_cache.clear();
    }

    pub(crate) fn cached_state_at_seconds(
        &self,
        time_seconds: f32,
    ) -> Option<([f32; 3], Direction)> {
        if self.snapshot_cache.is_empty() || !self.snapshot_cache_complete {
            return None;
        }

        let step_seconds = self.snapshot_cache_step_seconds.max(1.0 / 480.0);
        let target_time = time_seconds.clamp(0.0, self.clock.duration_seconds);
        let max_index = self.snapshot_cache.len().saturating_sub(1);
        let sample_position = (target_time / step_seconds).clamp(0.0, max_index as f32);
        let lower_index = sample_position.floor() as usize;
        let upper_index = (lower_index + 1).min(max_index);
        let alpha = (sample_position - lower_index as f32).clamp(0.0, 1.0);
        let lower = &self.snapshot_cache[lower_index];
        let upper = &self.snapshot_cache[upper_index];
        Some((
            interpolate_timeline_sample_positions(
                lower.position,
                lower.direction,
                upper.position,
                upper.direction,
                alpha,
            ),
            if alpha >= 1.0 - f32::EPSILON {
                upper.direction
            } else {
                lower.direction
            },
        ))
    }

    /// Copy timeline data needed by editor pointer picking without cloning the
    /// private simulation runtime owned by the active playback path.
    pub fn clone_for_interaction(&self) -> Self {
        Self {
            clock: EditorTimelineClockState {
                time_seconds: self.clock.time_seconds,
                duration_seconds: self.clock.duration_seconds,
            },
            preview: EditorTimelinePreviewState {
                position: self.preview.position,
                direction: self.preview.direction,
            },
            taps: EditorTimelineTapState {
                tap_times: self.taps.tap_times.clone(),
                tap_indicator_positions: self.taps.tap_indicator_positions.clone(),
                selected_index: self.taps.selected_index,
            },
            tap_cursor_override: self.tap_cursor_override,
            playback: EditorTimelinePlaybackState {
                playing: self.playback.playing,
                runtime: None,
                pending_seek_time_seconds: self.playback.pending_seek_time_seconds,
                seek_resync_cooldown_seconds: self.playback.seek_resync_cooldown_seconds,
            },
            snapshot_cache_step_seconds: self.snapshot_cache_step_seconds,
            snapshot_cache_complete: self.snapshot_cache_complete,
            snapshot_cache: self.snapshot_cache.clone(),
            snapshot_cache_runtime: None,
            snapshot_cache_next_index: 0,
            trail_point_cache: self.trail_point_cache.clone(),
            trail_segment_start_cache: self.trail_segment_start_cache.clone(),
            scrub_runtime: None,
            tap_division_preview_cache: self.tap_division_preview_cache.clone(),
        }
    }
}
