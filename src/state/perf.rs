/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::VecDeque;

pub(crate) const PERF_FRAME_BUDGET_60_FPS_MS: f32 = 16.7;
const PERF_FRAME_HISTORY_CAPACITY: usize = 1200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    BlockMeshChunkBuild,
    BlockMeshChunkUpload,
    BlockMeshIncrementalAppend,
    BlockMeshUploadStatic,
    BlockMeshUploadSelected,
    BlockMeshSelectedOnly,
    BlockMeshSelectedOnlyBuild,
    BlockMeshSelectedOnlyUpload,
    TTapToggleTotal,
    TTapSolve,
    TimelineSeek,
    TimelineSeekPreview,
    TimelineSeekDirtyBlockMesh,
    TimelineSeekAudioResync,
    TimelineSeekAudioStop,
    TimelineSeekRuntimeBuild,
    TimelineSeekAudioStart,
}

pub(crate) const PERF_STAGE_COUNT: usize = 42;

#[derive(Clone)]
pub(crate) struct PerfFrameSnapshot {
    pub(crate) frame_index: u64,
    pub(crate) frame_time_ms: f32,
    pub(crate) stage_ms: [f32; PERF_STAGE_COUNT],
    pub(crate) dominant_stage: Option<PerfStage>,
}

#[derive(Clone)]
pub(crate) struct PerfFrameContributor {
    pub(crate) stage: PerfStage,
    pub(crate) name: &'static str,
    pub(crate) ms: f32,
    pub(crate) percent_of_frame: f32,
}

#[derive(Clone)]
pub(crate) struct PerfFrameStageEntry {
    pub(crate) stage: PerfStage,
    pub(crate) name: &'static str,
    pub(crate) ms: f32,
    pub(crate) percent_of_frame: f32,
    pub(crate) children: Vec<PerfFrameStageEntry>,
}

impl PerfStage {
    pub(crate) const fn as_index(self) -> usize {
        self as usize
    }

    pub(crate) const fn roots_without_frame_total() -> &'static [PerfStage] {
        &[
            Self::TimelinePlayback,
            Self::TimelineSeek,
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
                Self::BlockMeshChunkBuild,
                Self::BlockMeshChunkUpload,
                Self::BlockMeshIncrementalAppend,
                Self::BlockMeshUploadStatic,
                Self::BlockMeshUploadSelected,
                Self::BlockMeshSelectedOnly,
            ],
            Self::BlockMeshSelectedOnly => &[
                Self::BlockMeshSelectedOnlyBuild,
                Self::BlockMeshSelectedOnlyUpload,
            ],
            Self::TTapToggleTotal => &[Self::TTapSolve],
            Self::TimelineSeek => &[
                Self::TimelineSeekPreview,
                Self::TimelineSeekDirtyBlockMesh,
                Self::TimelineSeekAudioResync,
            ],
            Self::TimelineSeekAudioResync => &[
                Self::TimelineSeekAudioStop,
                Self::TimelineSeekRuntimeBuild,
                Self::TimelineSeekAudioStart,
            ],
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
            Self::BlockMeshChunkBuild => "BlockMeshChunkBuild",
            Self::BlockMeshChunkUpload => "BlockMeshChunkUpload",
            Self::BlockMeshIncrementalAppend => "BlockMeshIncrementalAppend",
            Self::BlockMeshUploadStatic => "BlockMeshUploadStatic",
            Self::BlockMeshUploadSelected => "BlockMeshUploadSelected",
            Self::BlockMeshSelectedOnly => "BlockMeshSelectedOnly",
            Self::BlockMeshSelectedOnlyBuild => "SelectedOnlyBuild",
            Self::BlockMeshSelectedOnlyUpload => "SelectedOnlyUpload",
            Self::TTapToggleTotal => "TKeyToggle",
            Self::TTapSolve => "TKeySolve",
            Self::TimelineSeek => "TimelineSeek",
            Self::TimelineSeekPreview => "SeekPreview",
            Self::TimelineSeekDirtyBlockMesh => "SeekDirtyBlockMesh",
            Self::TimelineSeekAudioResync => "SeekAudioResync",
            Self::TimelineSeekAudioStop => "SeekAudioStop",
            Self::TimelineSeekRuntimeBuild => "SeekRuntimeBuild",
            Self::TimelineSeekAudioStart => "SeekAudioStart",
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
    pub(crate) paused: bool,
    frame_history: VecDeque<PerfFrameSnapshot>,
    next_frame_index: u64,
    selected_frame_id: Option<u64>,
}

impl EditorPerfProfiler {
    pub(crate) fn new() -> Self {
        Self {
            enabled: false,
            stats: [PerfStat::zero(); PERF_STAGE_COUNT],
            frame_stage_ms: [0.0; PERF_STAGE_COUNT],
            frame_spike_count: 0,
            last_spike_stage: None,
            paused: false,
            frame_history: VecDeque::with_capacity(PERF_FRAME_HISTORY_CAPACITY),
            next_frame_index: 0,
            selected_frame_id: None,
        }
    }

    pub(crate) fn observe(&mut self, stage: PerfStage, ms: f32) {
        self.stats[stage.as_index()].observe(ms);
        self.frame_stage_ms[stage.as_index()] += ms;
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stage_ms = [0.0; PERF_STAGE_COUNT];
    }

    pub(crate) fn finish_frame(&mut self, frame_time_ms: f32) {
        self.observe(PerfStage::FrameTotal, frame_time_ms);

        if frame_time_ms > PERF_FRAME_BUDGET_60_FPS_MS {
            self.frame_spike_count += 1;
            self.last_spike_stage = self
                .dominant_stage_this_frame()
                .or(Some(PerfStage::FrameTotal));
        }

        if self.paused {
            return;
        }

        let dominant_stage = self.dominant_stage_this_frame();
        let snapshot = PerfFrameSnapshot {
            frame_index: self.next_frame_index,
            frame_time_ms,
            stage_ms: self.frame_stage_ms,
            dominant_stage,
        };
        self.next_frame_index = self.next_frame_index.saturating_add(1);

        if self.frame_history.len() >= PERF_FRAME_HISTORY_CAPACITY {
            let dropped = self.frame_history.pop_front();
            if let (Some(selected_id), Some(dropped_snapshot)) = (self.selected_frame_id, dropped) {
                if selected_id == dropped_snapshot.frame_index {
                    self.selected_frame_id = None;
                }
            }
        }
        self.frame_history.push_back(snapshot);
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected_frame_id = None;
    }

    pub(crate) fn frame_history(&self) -> Vec<PerfFrameSnapshot> {
        self.frame_history.iter().cloned().collect()
    }

    pub(crate) fn selected_history_index(&self) -> Option<usize> {
        let selected_id = self.selected_frame_id?;
        self.frame_history
            .iter()
            .position(|snapshot| snapshot.frame_index == selected_id)
    }

    pub(crate) fn set_selected_history_index(&mut self, index: usize) {
        self.selected_frame_id = self.frame_history.get(index).map(|s| s.frame_index);
    }

    pub(crate) fn selected_frame(&self) -> Option<PerfFrameSnapshot> {
        let selected_id = self.selected_frame_id?;
        self.frame_history
            .iter()
            .find(|snapshot| snapshot.frame_index == selected_id)
            .cloned()
    }

    pub(crate) fn latest_frame(&self) -> Option<PerfFrameSnapshot> {
        self.frame_history.back().cloned()
    }

    pub(crate) fn selected_or_latest_frame(&self) -> Option<PerfFrameSnapshot> {
        self.selected_frame().or_else(|| self.latest_frame())
    }

    pub(crate) fn dominant_stage_this_frame(&self) -> Option<PerfStage> {
        Self::dominant_stage_from_stage_ms(&self.frame_stage_ms)
    }

    fn dominant_stage_from_stage_ms(stage_ms: &[f32; PERF_STAGE_COUNT]) -> Option<PerfStage> {
        let stages = PerfStage::roots_without_frame_total();

        let mut dominant: Option<(PerfStage, f32)> = None;
        for stage in stages {
            let value = stage_ms[stage.as_index()];
            dominant = match dominant {
                None => Some((*stage, value)),
                Some((_, best)) if value > best => Some((*stage, value)),
                current => current,
            };
        }

        dominant.map(|(stage, _)| stage)
    }

    pub(crate) fn top_contributors_for_snapshot(
        &self,
        snapshot: &PerfFrameSnapshot,
        limit: usize,
    ) -> Vec<PerfFrameContributor> {
        let mut contributors: Vec<_> = PerfStage::roots_without_frame_total()
            .iter()
            .map(|stage| {
                let ms = snapshot.stage_ms[stage.as_index()];
                let percent_of_frame = if snapshot.frame_time_ms > 1e-4 {
                    (ms / snapshot.frame_time_ms) * 100.0
                } else {
                    0.0
                };

                PerfFrameContributor {
                    stage: *stage,
                    name: stage.name(),
                    ms,
                    percent_of_frame,
                }
            })
            .filter(|entry| entry.ms > 0.01)
            .collect();

        contributors.sort_by(|left, right| right.ms.total_cmp(&left.ms));
        contributors.truncate(limit);
        contributors
    }

    pub(crate) fn selected_frame_top_contributors(
        &self,
        limit: usize,
    ) -> Vec<PerfFrameContributor> {
        self.selected_or_latest_frame()
            .map(|snapshot| self.top_contributors_for_snapshot(&snapshot, limit))
            .unwrap_or_default()
    }

    pub(crate) fn frame_tree_for_snapshot(
        &self,
        snapshot: &PerfFrameSnapshot,
    ) -> Vec<PerfFrameStageEntry> {
        PerfStage::roots_without_frame_total()
            .iter()
            .filter_map(|stage| {
                Self::frame_tree_entry(*stage, snapshot.frame_time_ms, &snapshot.stage_ms)
            })
            .collect()
    }

    pub(crate) fn selected_frame_tree(&self) -> Vec<PerfFrameStageEntry> {
        self.selected_or_latest_frame()
            .map(|snapshot| self.frame_tree_for_snapshot(&snapshot))
            .unwrap_or_default()
    }

    fn frame_tree_entry(
        stage: PerfStage,
        frame_time_ms: f32,
        stage_ms: &[f32; PERF_STAGE_COUNT],
    ) -> Option<PerfFrameStageEntry> {
        let children: Vec<_> = stage
            .children()
            .iter()
            .filter_map(|child| Self::frame_tree_entry(*child, frame_time_ms, stage_ms))
            .collect();
        let ms = stage_ms[stage.as_index()];

        if ms <= 0.01 && children.is_empty() {
            return None;
        }

        let percent_of_frame = if frame_time_ms > 1e-4 {
            (ms / frame_time_ms) * 100.0
        } else {
            0.0
        };

        Some(PerfFrameStageEntry {
            stage,
            name: stage.name(),
            ms,
            percent_of_frame,
            children,
        })
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

#[cfg(test)]
mod tests {
    use super::{EditorPerfProfiler, PerfStage, PerfStat, PERF_FRAME_BUDGET_60_FPS_MS};

    fn approx_eq(left: f32, right: f32, eps: f32) {
        assert!((left - right).abs() <= eps, "left={left}, right={right}");
    }

    #[test]
    fn perf_stat_observe_tracks_last_ema_max_and_calls() {
        let mut stat = PerfStat::zero();
        stat.observe(10.0);

        approx_eq(stat.last_ms, 10.0, 1e-6);
        approx_eq(stat.ema_ms, 10.0, 1e-6);
        approx_eq(stat.max_ms, 10.0, 1e-6);
        assert_eq!(stat.calls, 1);

        stat.observe(20.0);
        approx_eq(stat.last_ms, 20.0, 1e-6);
        approx_eq(stat.ema_ms, 11.0, 1e-6);
        approx_eq(stat.max_ms, 20.0, 1e-6);
        assert_eq!(stat.calls, 2);
    }

    #[test]
    fn profiler_tracks_frame_and_dominant_stage() {
        let mut profiler = EditorPerfProfiler::new();

        profiler.observe(PerfStage::TimelinePlayback, 3.0);
        profiler.observe(PerfStage::TimelineSeek, 1.25);
        profiler.observe(PerfStage::TimelinePlayback, 2.0);

        approx_eq(
            profiler.frame_stage_ms[PerfStage::TimelinePlayback.as_index()],
            5.0,
            1e-6,
        );
        approx_eq(
            profiler.frame_stage_ms[PerfStage::TimelineSeek.as_index()],
            1.25,
            1e-6,
        );
        assert!(matches!(
            profiler.dominant_stage_this_frame(),
            Some(PerfStage::TimelinePlayback)
        ));

        profiler.begin_frame();
        approx_eq(
            profiler.frame_stage_ms[PerfStage::TimelinePlayback.as_index()],
            0.0,
            1e-6,
        );
        approx_eq(
            profiler.frame_stage_ms[PerfStage::TimelineSeek.as_index()],
            0.0,
            1e-6,
        );
    }

    #[test]
    fn profiler_finish_frame_records_snapshot_and_spike() {
        let mut profiler = EditorPerfProfiler::new();
        profiler.begin_frame();
        profiler.observe(PerfStage::DirtyProcess, 8.0);
        profiler.observe(PerfStage::BlockMeshRebuild, 10.0);
        profiler.finish_frame(PERF_FRAME_BUDGET_60_FPS_MS + 4.0);

        assert_eq!(profiler.frame_spike_count, 1);
        assert!(matches!(
            profiler.last_spike_stage,
            Some(PerfStage::BlockMeshRebuild)
        ));

        let history = profiler.frame_history();
        assert_eq!(history.len(), 1);
        approx_eq(
            history[0].frame_time_ms,
            PERF_FRAME_BUDGET_60_FPS_MS + 4.0,
            1e-6,
        );
        assert!(matches!(
            history[0].dominant_stage,
            Some(PerfStage::BlockMeshRebuild)
        ));
    }

    #[test]
    fn pause_freezes_history_but_keeps_cumulative_stats() {
        let mut profiler = EditorPerfProfiler::new();

        profiler.begin_frame();
        profiler.observe(PerfStage::TimelinePlayback, 2.0);
        profiler.finish_frame(5.0);
        assert_eq!(profiler.frame_history().len(), 1);

        profiler.toggle_pause();
        profiler.begin_frame();
        profiler.observe(PerfStage::TimelinePlayback, 4.0);
        profiler.finish_frame(7.0);

        assert_eq!(profiler.frame_history().len(), 1);
        assert_eq!(
            profiler.stats[PerfStage::TimelinePlayback.as_index()].calls,
            2
        );
        approx_eq(
            profiler.stats[PerfStage::TimelinePlayback.as_index()].last_ms,
            4.0,
            1e-6,
        );
    }

    #[test]
    fn selected_frame_top_contributors_are_sorted() {
        let mut profiler = EditorPerfProfiler::new();
        profiler.begin_frame();
        profiler.observe(PerfStage::DirtyProcess, 4.0);
        profiler.observe(PerfStage::SelectionClick, 2.0);
        profiler.observe(PerfStage::TimelinePlayback, 1.0);
        profiler.finish_frame(10.0);

        let top = profiler.selected_frame_top_contributors(3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].stage, PerfStage::DirtyProcess);
        assert_eq!(top[1].stage, PerfStage::SelectionClick);
        assert_eq!(top[2].stage, PerfStage::TimelinePlayback);
        approx_eq(top[0].percent_of_frame, 40.0, 1e-6);
    }

    #[test]
    fn selected_history_index_round_trips() {
        let mut profiler = EditorPerfProfiler::new();
        for frame in 0..4 {
            profiler.begin_frame();
            profiler.observe(PerfStage::TimelinePlayback, frame as f32 + 1.0);
            profiler.finish_frame(10.0 + frame as f32);
        }

        profiler.set_selected_history_index(1);
        assert_eq!(profiler.selected_history_index(), Some(1));

        let selected = profiler
            .selected_frame()
            .expect("selected frame should exist");
        assert_eq!(selected.frame_index, 1);

        profiler.clear_selection();
        assert_eq!(profiler.selected_history_index(), None);
    }
}
