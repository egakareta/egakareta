/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::TapDivisionPreview;
use eg_game::TimelineSimulationRuntime;
use eg_types::Direction;

pub struct EditorTimelineSnapshot {
    pub position: [f32; 3],
    pub direction: Direction,
    pub is_grounded: bool,
    pub game_over: bool,
    pub trail_segment_count: usize,
    pub trail_point_count: usize,
}

pub struct EditorTimelinePlaybackState {
    pub playing: bool,
    pub runtime: Option<TimelineSimulationRuntime>,
    pub pending_seek_time_seconds: Option<f32>,
    pub seek_resync_cooldown_seconds: f32,
}

pub struct EditorTimelinePreviewState {
    pub position: [f32; 3],
    pub direction: Direction,
}

pub struct EditorTimelineClockState {
    pub time_seconds: f32,
    pub duration_seconds: f32,
}

pub struct EditorTimelineTapState {
    pub tap_times: Vec<f32>,
    pub tap_indicator_positions: Vec<[f32; 3]>,
    pub selected_index: Option<usize>,
}

pub struct EditorTimelineState {
    pub clock: EditorTimelineClockState,
    pub preview: EditorTimelinePreviewState,
    pub taps: EditorTimelineTapState,
    pub playback: EditorTimelinePlaybackState,
    pub simulation_revision: u64,
    pub snapshot_cache_revision: u64,
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
    pub fn new() -> Self {
        Self {
            clock: EditorTimelineClockState {
                time_seconds: 0.0,
                duration_seconds: 16.0,
            },
            preview: EditorTimelinePreviewState {
                position: [0.0, 0.0, 0.0],
                direction: Direction::Forward,
            },
            taps: EditorTimelineTapState {
                tap_times: Vec::new(),
                tap_indicator_positions: Vec::new(),
                selected_index: None,
            },
            playback: EditorTimelinePlaybackState {
                playing: false,
                runtime: None,
                pending_seek_time_seconds: None,
                seek_resync_cooldown_seconds: 0.0,
            },
            simulation_revision: 1,
            snapshot_cache_revision: 0,
            snapshot_cache_step_seconds: 1.0 / 240.0,
            snapshot_cache: Vec::new(),
            trail_point_cache: Vec::new(),
            trail_segment_start_cache: Vec::new(),
            scrub_runtime_revision: 0,
            scrub_runtime: None,
            tap_division_preview_cache_revision: 0,
            tap_division_preview_cache_timing_revision: 0,
            tap_division_preview_cache_duration_seconds: 0.0,
            tap_division_preview_cache_window: (0.0, 0.0),
            tap_division_preview_cache: Vec::new(),
        }
    }
}

impl Default for EditorTimelineState {
    fn default() -> Self {
        Self::new()
    }
}
