use crate::game::TimelineSimulationRuntime;
use crate::types::SpawnDirection;

#[derive(Clone, Copy)]
pub(crate) struct EditorTimelineSample {
    pub(crate) time_seconds: f32,
    pub(crate) position: [f32; 3],
}

pub(crate) struct EditorTimelineSampleCache {
    pub(crate) samples: Vec<EditorTimelineSample>,
    pub(crate) dirty: bool,
    pub(crate) rebuild_from_seconds: Option<f32>,
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
    pub(crate) cache: EditorTimelineSampleCache,
    pub(crate) playback: EditorTimelinePlaybackState,
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
            cache: EditorTimelineSampleCache {
                samples: Vec::new(),
                dirty: true,
                rebuild_from_seconds: None,
            },
            playback: EditorTimelinePlaybackState {
                playing: false,
                runtime: None,
            },
        }
    }
}
