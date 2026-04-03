/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::game::TimelineSimulationRuntime;
use crate::types::SpawnDirection;

pub(crate) struct EditorTimelineSnapshot {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
}

pub(crate) struct EditorTimelinePlaybackState {
    pub(crate) playing: bool,
    pub(crate) runtime: Option<TimelineSimulationRuntime>,
}

pub(crate) struct EditorTimelinePreviewState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
}

pub(crate) struct EditorTimelineClockState {
    pub(crate) time_seconds: f32,
    pub(crate) duration_seconds: f32,
}

pub(crate) struct EditorTimelineTapState {
    pub(crate) tap_times: Vec<f32>,
    pub(crate) tap_indicator_positions: Vec<[f32; 3]>,
}

pub(crate) struct EditorTimelineState {
    pub(crate) clock: EditorTimelineClockState,
    pub(crate) preview: EditorTimelinePreviewState,
    pub(crate) taps: EditorTimelineTapState,
    pub(crate) playback: EditorTimelinePlaybackState,
    pub(crate) simulation_revision: u64,
    pub(crate) snapshot_cache_revision: u64,
    pub(crate) snapshot_cache_step_seconds: f32,
    pub(crate) snapshot_cache: Vec<EditorTimelineSnapshot>,
    pub(crate) scrub_runtime_revision: u64,
    pub(crate) scrub_runtime: Option<TimelineSimulationRuntime>,
}

impl EditorTimelineState {
    pub(crate) fn new() -> Self {
        Self {
            clock: EditorTimelineClockState {
                time_seconds: 0.0,
                duration_seconds: 16.0,
            },
            preview: EditorTimelinePreviewState {
                position: [0.0, 0.0, 0.0],
                direction: SpawnDirection::Forward,
            },
            taps: EditorTimelineTapState {
                tap_times: Vec::new(),
                tap_indicator_positions: Vec::new(),
            },
            playback: EditorTimelinePlaybackState {
                playing: false,
                runtime: None,
            },
            simulation_revision: 1,
            snapshot_cache_revision: 0,
            snapshot_cache_step_seconds: 1.0 / 240.0,
            snapshot_cache: Vec::new(),
            scrub_runtime_revision: 0,
            scrub_runtime: None,
        }
    }
}
