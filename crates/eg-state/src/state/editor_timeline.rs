/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::TapDivisionPreview;
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
    #[default = 1]
    pub simulation_revision: u64,
    #[default = 0]
    pub snapshot_cache_revision: u64,
    #[default(1.0 / 240.0)]
    pub snapshot_cache_step_seconds: f32,
    pub snapshot_cache: Vec<EditorTimelineSnapshot>,
    pub trail_point_cache: Vec<[f32; 3]>,
    pub trail_segment_start_cache: Vec<usize>,
    pub scrub_runtime_revision: u64,
    pub scrub_runtime: Option<TimelineSimulationRuntime>,
    pub tap_division_preview_cache_revision: u64,
    pub tap_division_preview_cache_timing_revision: u64,
    pub tap_division_preview_cache_duration_seconds: f32,
    pub tap_division_preview_cache_window: (f32, f32),
    pub tap_division_preview_cache: Vec<TapDivisionPreview>,
}

impl EditorTimelineState {
    pub fn invalidate_samples(&mut self) {
        self.simulation_revision = self.simulation_revision.wrapping_add(1);
        self.snapshot_cache_revision = 0;
        self.snapshot_cache.clear();
        self.trail_point_cache.clear();
        self.trail_segment_start_cache.clear();
        self.scrub_runtime = None;
        self.scrub_runtime_revision = 0;
    }

    pub fn invalidate_samples_from(&mut self, from_seconds: f32) {
        self.simulation_revision = self.simulation_revision.wrapping_add(1);

        let step_seconds = self.snapshot_cache_step_seconds.max(1.0 / 480.0);
        if !self.snapshot_cache.is_empty() && from_seconds > step_seconds {
            let keep_count = (from_seconds / step_seconds).floor() as usize;
            let keep_count = keep_count.min(self.snapshot_cache.len());
            if keep_count > 0 {
                self.snapshot_cache.truncate(keep_count);
                self.truncate_cached_timeline_trail_to_snapshot_prefix();
                self.snapshot_cache_revision = 0;
            } else {
                self.clear_cached_timeline_samples();
            }
        } else {
            self.clear_cached_timeline_samples();
        }

        if self
            .scrub_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.elapsed_seconds() >= from_seconds)
        {
            self.scrub_runtime = None;
            self.scrub_runtime_revision = 0;
        } else if self.scrub_runtime.is_none() {
            self.scrub_runtime_revision = 0;
        }
    }

    fn clear_cached_timeline_samples(&mut self) {
        self.snapshot_cache.clear();
        self.trail_point_cache.clear();
        self.trail_segment_start_cache.clear();
        self.snapshot_cache_revision = 0;
    }

    fn truncate_cached_timeline_trail_to_snapshot_prefix(&mut self) {
        let Some(snapshot) = self.snapshot_cache.last() else {
            self.trail_point_cache.clear();
            self.trail_segment_start_cache.clear();
            return;
        };

        self.trail_point_cache.truncate(snapshot.trail_point_count);
        self.trail_segment_start_cache
            .truncate(snapshot.trail_segment_count);
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
            simulation_revision: self.simulation_revision,
            snapshot_cache_revision: self.snapshot_cache_revision,
            snapshot_cache_step_seconds: self.snapshot_cache_step_seconds,
            snapshot_cache: self.snapshot_cache.clone(),
            trail_point_cache: self.trail_point_cache.clone(),
            trail_segment_start_cache: self.trail_segment_start_cache.clone(),
            scrub_runtime_revision: self.scrub_runtime_revision,
            scrub_runtime: None,
            tap_division_preview_cache_revision: self.tap_division_preview_cache_revision,
            tap_division_preview_cache_timing_revision: self
                .tap_division_preview_cache_timing_revision,
            tap_division_preview_cache_duration_seconds: self
                .tap_division_preview_cache_duration_seconds,
            tap_division_preview_cache_window: self.tap_division_preview_cache_window,
            tap_division_preview_cache: self.tap_division_preview_cache.clone(),
        }
    }
}
