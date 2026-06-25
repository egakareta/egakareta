/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_editor_domain::TapDivisionPreview;
use eg_game::TimelineSimulationRuntime;
use eg_types::Direction;
use smart_default::SmartDefault;

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
