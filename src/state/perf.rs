/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[derive(Clone, Copy)]
pub(crate) enum PerfStage {
    FrameTotal = 0,
    TimelinePlayback,
    DragSelection,
    SelectionClick,
    SelectionPick,
    SelectionApply,
    SelectionMarkDirty,
    PickUnproject,
    PickRaycast,
    GizmoRebuild,
    DirtyProcess,
    DirtySyncGameObjects,
    DirtyRebuildBlockMesh,
    DirtyRebuildSelectionOverlays,
    DirtyRebuildTapIndicators,
    DirtyRebuildPreviewPlayer,
    PreviewSolveTimeline,
    PreviewMeshBuild,
    DirtyRebuildCursor,
    TimelineSampleRebuild,
    TapIndicatorMeshRebuild,
    BlockMeshRebuild,
    BlockMeshMaskBuild,
    BlockMeshSplitStatic,
    BlockMeshSplitSelected,
    BlockMeshUploadStatic,
    BlockMeshUploadSelected,
    BlockMeshSelectedOnly,
    BlockMeshSelectedOnlyBuild,
    BlockMeshSelectedOnlyUpload,
    TTapToggleTotal,
    TTapSolve,
}

pub(crate) const PERF_STAGE_COUNT: usize = 32;

#[derive(Clone)]
pub(crate) struct PerfOverlayEntry {
    pub(crate) name: &'static str,
    pub(crate) last_ms: f32,
    pub(crate) avg_ms: f32,
    pub(crate) max_ms: f32,
    pub(crate) calls: u64,
    pub(crate) children: Vec<PerfOverlayEntry>,
}

impl PerfStage {
    pub(crate) const fn as_index(self) -> usize {
        self as usize
    }

    pub(crate) const fn roots() -> &'static [PerfStage] {
        &[
            Self::FrameTotal,
            Self::TimelinePlayback,
            Self::DragSelection,
            Self::SelectionClick,
            Self::GizmoRebuild,
            Self::DirtyProcess,
            Self::TimelineSampleRebuild,
            Self::TapIndicatorMeshRebuild,
            Self::BlockMeshRebuild,
            Self::TTapToggleTotal,
        ]
    }

    pub(crate) const fn children(self) -> &'static [PerfStage] {
        match self {
            Self::SelectionClick => &[
                Self::SelectionPick,
                Self::SelectionApply,
                Self::SelectionMarkDirty,
            ],
            Self::SelectionPick => &[Self::PickUnproject, Self::PickRaycast],
            Self::DirtyProcess => &[
                Self::DirtySyncGameObjects,
                Self::DirtyRebuildBlockMesh,
                Self::DirtyRebuildSelectionOverlays,
                Self::DirtyRebuildTapIndicators,
                Self::DirtyRebuildPreviewPlayer,
                Self::DirtyRebuildCursor,
            ],
            Self::DirtyRebuildPreviewPlayer => {
                &[Self::PreviewSolveTimeline, Self::PreviewMeshBuild]
            }
            Self::BlockMeshRebuild => &[
                Self::BlockMeshMaskBuild,
                Self::BlockMeshSplitStatic,
                Self::BlockMeshSplitSelected,
                Self::BlockMeshUploadStatic,
                Self::BlockMeshUploadSelected,
                Self::BlockMeshSelectedOnly,
            ],
            Self::BlockMeshSelectedOnly => &[
                Self::BlockMeshSelectedOnlyBuild,
                Self::BlockMeshSelectedOnlyUpload,
            ],
            Self::TTapToggleTotal => &[Self::TTapSolve],
            _ => &[],
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::FrameTotal => "FrameTotal",
            Self::TimelinePlayback => "TimelinePlayback",
            Self::DragSelection => "DragSelection",
            Self::SelectionClick => "SelectClick",
            Self::SelectionPick => "SelectPick",
            Self::SelectionApply => "SelectApply",
            Self::SelectionMarkDirty => "SelectMarkDirty",
            Self::PickUnproject => "PickUnproject",
            Self::PickRaycast => "PickRaycast",
            Self::GizmoRebuild => "GizmoRebuild",
            Self::DirtyProcess => "DirtyProcess",
            Self::DirtySyncGameObjects => "DirtySyncObjects",
            Self::DirtyRebuildBlockMesh => "DirtyBlockMesh",
            Self::DirtyRebuildSelectionOverlays => "DirtySelectionOverlays",
            Self::DirtyRebuildTapIndicators => "DirtyTapIndicators",
            Self::DirtyRebuildPreviewPlayer => "DirtyPreviewPlayer",
            Self::PreviewSolveTimeline => "PreviewSolveTimeline",
            Self::PreviewMeshBuild => "PreviewMeshBuild",
            Self::DirtyRebuildCursor => "DirtyCursor",
            Self::TimelineSampleRebuild => "TimelineSamples",
            Self::TapIndicatorMeshRebuild => "TapIndicatorMesh",
            Self::BlockMeshRebuild => "BlockMeshRebuild",
            Self::BlockMeshMaskBuild => "BlockMaskBuild",
            Self::BlockMeshSplitStatic => "BlockMeshSplitStatic",
            Self::BlockMeshSplitSelected => "BlockMeshSplitSelected",
            Self::BlockMeshUploadStatic => "BlockMeshUploadStatic",
            Self::BlockMeshUploadSelected => "BlockMeshUploadSelected",
            Self::BlockMeshSelectedOnly => "BlockMeshSelectedOnly",
            Self::BlockMeshSelectedOnlyBuild => "SelectedOnlyBuild",
            Self::BlockMeshSelectedOnlyUpload => "SelectedOnlyUpload",
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

    fn overlay_entry_for_stage(&self, stage: PerfStage) -> PerfOverlayEntry {
        let stat = self.stats[stage.as_index()];
        let children = stage
            .children()
            .iter()
            .map(|child| self.overlay_entry_for_stage(*child))
            .collect();

        PerfOverlayEntry {
            name: stage.name(),
            last_ms: stat.last_ms,
            avg_ms: stat.ema_ms,
            max_ms: stat.max_ms,
            calls: stat.calls,
            children,
        }
    }

    pub(crate) fn overlay_entries(&self) -> Vec<PerfOverlayEntry> {
        PerfStage::roots()
            .iter()
            .map(|stage| self.overlay_entry_for_stage(*stage))
            .collect()
    }

    pub(crate) fn dominant_stage_this_frame(&self) -> Option<PerfStage> {
        let stages = [
            PerfStage::TimelinePlayback,
            PerfStage::DragSelection,
            PerfStage::SelectionClick,
            PerfStage::GizmoRebuild,
            PerfStage::DirtyProcess,
            PerfStage::TimelineSampleRebuild,
            PerfStage::TapIndicatorMeshRebuild,
            PerfStage::BlockMeshRebuild,
            PerfStage::TTapToggleTotal,
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
