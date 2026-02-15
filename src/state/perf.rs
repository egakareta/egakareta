#[derive(Clone, Copy)]
pub(crate) enum PerfStage {
    FrameTotal = 0,
    TimelinePlayback,
    DragSelection,
    GizmoRebuild,
    DirtyProcess,
    TimelineSampleRebuild,
    TapIndicatorMeshRebuild,
    BlockMeshRebuild,
    TTapToggleTotal,
    TTapSolve,
}

pub(crate) const PERF_STAGE_COUNT: usize = 10;

impl PerfStage {
    pub(crate) const fn as_index(self) -> usize {
        self as usize
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::FrameTotal => "FrameTotal",
            Self::TimelinePlayback => "TimelinePlayback",
            Self::DragSelection => "DragSelection",
            Self::GizmoRebuild => "GizmoRebuild",
            Self::DirtyProcess => "DirtyProcess",
            Self::TimelineSampleRebuild => "TimelineSamples",
            Self::TapIndicatorMeshRebuild => "TapIndicatorMesh",
            Self::BlockMeshRebuild => "BlockMeshRebuild",
            Self::TTapToggleTotal => "TKeyToggle",
            Self::TTapSolve => "TKeySolve",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PerfStat {
    pub(crate) last_ms: f32,
    pub(crate) ema_ms: f32,
    pub(crate) max_ms: f32,
    pub(crate) calls: u64,
}

impl PerfStat {
    pub(crate) const fn zero() -> Self {
        Self {
            last_ms: 0.0,
            ema_ms: 0.0,
            max_ms: 0.0,
            calls: 0,
        }
    }

    pub(crate) fn observe(&mut self, ms: f32) {
        self.last_ms = ms;
        if self.calls == 0 {
            self.ema_ms = ms;
        } else {
            self.ema_ms = self.ema_ms * 0.9 + ms * 0.1;
        }
        self.max_ms = self.max_ms.max(ms);
        self.calls += 1;
    }
}

pub(crate) struct EditorPerfProfiler {
    pub(crate) enabled: bool,
    pub(crate) stats: [PerfStat; PERF_STAGE_COUNT],
    pub(crate) frame_stage_ms: [f32; PERF_STAGE_COUNT],
    pub(crate) frame_spike_count: u64,
    pub(crate) last_spike_stage: Option<PerfStage>,
}

impl EditorPerfProfiler {
    pub(crate) fn new() -> Self {
        Self {
            enabled: false,
            stats: [PerfStat::zero(); PERF_STAGE_COUNT],
            frame_stage_ms: [0.0; PERF_STAGE_COUNT],
            frame_spike_count: 0,
            last_spike_stage: None,
        }
    }

    pub(crate) fn observe(&mut self, stage: PerfStage, ms: f32) {
        self.stats[stage.as_index()].observe(ms);
        self.frame_stage_ms[stage.as_index()] += ms;
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stage_ms = [0.0; PERF_STAGE_COUNT];
    }

    pub(crate) fn dominant_stage_this_frame(&self) -> Option<PerfStage> {
        let stages = [
            PerfStage::TimelinePlayback,
            PerfStage::DragSelection,
            PerfStage::GizmoRebuild,
            PerfStage::DirtyProcess,
            PerfStage::TimelineSampleRebuild,
            PerfStage::TapIndicatorMeshRebuild,
            PerfStage::BlockMeshRebuild,
            PerfStage::TTapToggleTotal,
            PerfStage::TTapSolve,
        ];

        let mut dominant: Option<(PerfStage, f32)> = None;
        for stage in stages {
            let value = self.frame_stage_ms[stage.as_index()];
            dominant = match dominant {
                None => Some((stage, value)),
                Some((_, best)) if value > best => Some((stage, value)),
                current => current,
            };
        }

        dominant.map(|(stage, _)| stage)
    }
}

pub(crate) struct EditorPerfState {
    pub(crate) profiler: EditorPerfProfiler,
    pub(crate) fps_smoothed: f32,
}

impl EditorPerfState {
    pub(crate) fn new() -> Self {
        Self {
            profiler: EditorPerfProfiler::new(),
            fps_smoothed: 0.0,
        }
    }
}
