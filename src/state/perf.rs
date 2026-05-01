/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{
    collections::{vec_deque, VecDeque},
    ops::RangeInclusive,
};

use crate::platform::state_host::PlatformInstant;

pub(crate) const PERF_FRAME_BUDGET_60_FPS_MS: f32 = 16.7;
const PERF_FRAME_HISTORY_CAPACITY: usize = 10_000;

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
    #[allow(dead_code)]
    PreviewMeshBuild,
    DirtyRebuildCursor,
    TimelineSampleRebuild,
    TapIndicatorMeshRebuild,
    BlockMeshRebuild,
    BlockMeshMaskBuild,
    BlockMeshSplitStatic,
    BlockMeshSplitSelected,
    #[allow(dead_code)]
    BlockMeshChunkBuild,
    #[allow(dead_code)]
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
    pub(crate) span_events: Vec<PerfFrameSpanEvent>,
}

#[derive(Clone, Copy)]
pub(crate) enum PerfFrameHistoryRef<'a> {
    Empty,
    Frames(&'a VecDeque<PerfFrameSnapshot>),
}

impl<'a> PerfFrameHistoryRef<'a> {
    pub(crate) const fn empty() -> Self {
        Self::Empty
    }

    pub(crate) fn len(self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Frames(history) => history.len(),
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub(crate) fn range(self, range: RangeInclusive<usize>) -> PerfFrameHistoryRange<'a> {
        match self {
            Self::Empty => PerfFrameHistoryRange::Empty,
            Self::Frames(history) => PerfFrameHistoryRange::Frames(history.range(range)),
        }
    }
}

pub(crate) enum PerfFrameHistoryRange<'a> {
    Empty,
    Frames(vec_deque::Iter<'a, PerfFrameSnapshot>),
}

impl<'a> Iterator for PerfFrameHistoryRange<'a> {
    type Item = &'a PerfFrameSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Frames(iter) => iter.next(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PerfFrameSpanEvent {
    pub(crate) stage: PerfStage,
    pub(crate) start_ms: f32,
    pub(crate) duration_ms: f32,
    pub(crate) end_ms: f32,
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct PerfFrameStageEntry {
    pub(crate) stage: PerfStage,
    pub(crate) name: &'static str,
    pub(crate) ms: f32,
    pub(crate) percent_of_frame: f32,
    pub(crate) children: Vec<PerfFrameStageEntry>,
}

#[derive(Clone)]
pub(crate) struct PerfFrameRangeSummary {
    pub(crate) start_frame_index: u64,
    pub(crate) end_frame_index: u64,
    pub(crate) frame_count: usize,
    pub(crate) average_frame_time_ms: f32,
    pub(crate) worst_frame_time_ms: f32,
    pub(crate) average_fps: f32,
    pub(crate) dominant_stage: Option<PerfStage>,
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
    frame_started_at: Option<PlatformInstant>,
    frame_span_events: Vec<PerfFrameSpanEvent>,
    next_frame_index: u64,
    selected_history_index: Option<usize>,
    selected_history_range: Option<(usize, usize)>,
    histogram_follow_latest: bool,
    histogram_focus_index: Option<usize>,
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
            frame_started_at: None,
            frame_span_events: Vec::with_capacity(256),
            next_frame_index: 0,
            selected_history_index: None,
            selected_history_range: None,
            histogram_follow_latest: true,
            histogram_focus_index: None,
        }
    }

    pub(crate) fn observe(&mut self, stage: PerfStage, ms: f32) {
        self.stats[stage.as_index()].observe(ms);
        self.frame_stage_ms[stage.as_index()] += ms;
    }

    pub(crate) fn observe_span(
        &mut self,
        stage: PerfStage,
        started_at: PlatformInstant,
        ended_at: PlatformInstant,
    ) {
        let duration_ms = ended_at
            .checked_duration_since(started_at)
            .map(|duration| duration.as_secs_f32() * 1000.0)
            .unwrap_or(0.0);
        self.observe(stage, duration_ms);

        if !self.enabled {
            return;
        }

        let Some(frame_started_at) = self.frame_started_at else {
            return;
        };

        let start_ms = started_at
            .checked_duration_since(frame_started_at)
            .map(|duration| duration.as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
            .max(0.0);
        let end_ms = ended_at
            .checked_duration_since(frame_started_at)
            .map(|duration| duration.as_secs_f32() * 1000.0)
            .unwrap_or(start_ms)
            .max(start_ms);

        self.frame_span_events.push(PerfFrameSpanEvent {
            stage,
            start_ms,
            duration_ms: duration_ms.max(0.0),
            end_ms,
        });
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stage_ms = [0.0; PERF_STAGE_COUNT];
        self.frame_span_events.clear();
        self.frame_started_at = self.enabled.then(PlatformInstant::now);
    }

    pub(crate) fn finish_frame(&mut self, frame_time_ms: f32) {
        self.observe(PerfStage::FrameTotal, frame_time_ms);

        if frame_time_ms > PERF_FRAME_BUDGET_60_FPS_MS {
            self.frame_spike_count += 1;
            self.last_spike_stage = self
                .dominant_stage_this_frame()
                .or(Some(PerfStage::FrameTotal));
        }

        if !self.enabled || self.paused {
            self.frame_started_at = None;
            self.frame_span_events.clear();
            return;
        }

        self.frame_span_events.push(PerfFrameSpanEvent {
            stage: PerfStage::FrameTotal,
            start_ms: 0.0,
            duration_ms: frame_time_ms.max(0.0),
            end_ms: frame_time_ms.max(0.0),
        });

        let dominant_stage = self.dominant_stage_this_frame();
        let snapshot = PerfFrameSnapshot {
            frame_index: self.next_frame_index,
            frame_time_ms,
            stage_ms: self.frame_stage_ms,
            dominant_stage,
            span_events: std::mem::take(&mut self.frame_span_events),
        };
        self.frame_started_at = None;
        self.next_frame_index = self.next_frame_index.saturating_add(1);

        if self.frame_history.len() >= PERF_FRAME_HISTORY_CAPACITY
            && self.frame_history.pop_front().is_some()
        {
            self.selected_history_index = self
                .selected_history_index
                .and_then(|index| index.checked_sub(1));

            if let Some((start, end)) = self.selected_history_range {
                if end == 0 {
                    self.selected_history_range = None;
                } else {
                    self.selected_history_range = Some((start.saturating_sub(1), end - 1));
                }
            }

            self.histogram_focus_index = self
                .histogram_focus_index
                .map(|index| index.saturating_sub(1));
        }
        self.frame_history.push_back(snapshot);
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            self.clear_selection();
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected_history_index = None;
        self.selected_history_range = None;
        self.histogram_follow_latest = true;
        self.histogram_focus_index = None;
    }

    #[cfg(test)]
    pub(crate) fn frame_history(&self) -> Vec<PerfFrameSnapshot> {
        self.frame_history.iter().cloned().collect()
    }

    pub(crate) fn frame_history_ref(&self) -> PerfFrameHistoryRef<'_> {
        PerfFrameHistoryRef::Frames(&self.frame_history)
    }

    pub(crate) fn selected_history_index(&self) -> Option<usize> {
        self.selected_history_index
            .filter(|index| *index < self.frame_history.len())
    }

    pub(crate) fn selected_history_range_indices(&self) -> Option<(usize, usize)> {
        if self.selected_history_index().is_some() {
            return None;
        }

        if let Some((start, end)) = self.selected_history_range {
            let last_index = self.frame_history.len().checked_sub(1)?;
            let clamped_start = start.min(last_index);
            let clamped_end = end.min(last_index);
            return Some((
                clamped_start.min(clamped_end),
                clamped_start.max(clamped_end),
            ));
        }

        let current_index = self.histogram_focus_history_index()?;
        let chunk_size = 16.min(self.frame_history.len()).max(1);
        let mut start = current_index.saturating_sub(chunk_size / 2);
        if start + chunk_size > self.frame_history.len() {
            start = self.frame_history.len().saturating_sub(chunk_size);
        }
        let end = start + chunk_size.saturating_sub(1);

        Some((start, end))
    }

    pub(crate) fn set_selected_history_index(&mut self, index: usize) {
        if index >= self.frame_history.len() {
            self.selected_history_index = None;
            self.selected_history_range = None;
            return;
        }

        self.selected_history_index = Some(index);
        self.selected_history_range = None;
        self.histogram_follow_latest = false;
        self.histogram_focus_index = Some(index);
    }

    pub(crate) fn set_selected_history_range(&mut self, start: usize, end: usize) {
        let Some(last_index) = self.frame_history.len().checked_sub(1) else {
            return;
        };

        let lo = start.min(end);
        let hi = start.max(end);
        let clamped_lo = lo.min(last_index);
        let clamped_hi = hi.min(last_index);

        self.selected_history_range = Some((clamped_lo, clamped_hi));
        self.selected_history_index = None;
        self.histogram_follow_latest = false;
        let center = clamped_lo + (clamped_hi - clamped_lo) / 2;
        self.histogram_focus_index = Some(center);
    }

    pub(crate) fn histogram_follow_latest(&self) -> bool {
        self.histogram_follow_latest
    }

    pub(crate) fn set_histogram_follow_latest(&mut self, follow_latest: bool) {
        self.histogram_follow_latest = follow_latest;
        if follow_latest {
            self.histogram_focus_index = None;
        } else if self.histogram_focus_index.is_none() {
            self.histogram_focus_index = self.frame_history.len().checked_sub(1);
        }
    }

    pub(crate) fn focus_histogram_index(&mut self, index: usize) {
        let Some(last_index) = self.frame_history.len().checked_sub(1) else {
            return;
        };

        self.histogram_follow_latest = false;
        self.histogram_focus_index = Some(index.min(last_index));
    }

    pub(crate) fn pan_histogram(&mut self, delta: i32) {
        let Some(current_index) = self.histogram_focus_history_index() else {
            return;
        };
        let Some(last_index) = self.frame_history.len().checked_sub(1) else {
            return;
        };
        let next_index = (current_index as i64 + delta as i64).clamp(0, last_index as i64) as usize;

        // Preserve explicit range width while panning so zoom level does not snap back
        // to the synthetic default window size.
        if let Some((start, end)) = self.selected_history_range_indices() {
            let width = end.saturating_sub(start) + 1;
            let left_count = (width - 1) / 2;
            let right_count = width - 1 - left_count;

            let mut next_start = next_index.saturating_sub(left_count);
            let mut next_end = next_index.saturating_add(right_count).min(last_index);

            let clamped_width = next_end.saturating_sub(next_start) + 1;
            if clamped_width < width {
                if next_start == 0 {
                    next_end = (width - 1).min(last_index);
                } else {
                    next_start = last_index.saturating_add(1).saturating_sub(width);
                    next_end = last_index;
                }
            }

            self.selected_history_index = None;
            self.selected_history_range = Some((next_start, next_end));
        } else {
            // No explicit range: keep old behavior and allow synthetic window tracking.
            self.selected_history_index = None;
            self.selected_history_range = None;
        }

        self.focus_histogram_index(next_index);
    }

    pub(crate) fn histogram_focus_history_index(&self) -> Option<usize> {
        let last_index = self.frame_history.len().checked_sub(1)?;
        if self.histogram_follow_latest {
            return Some(last_index);
        }

        self.histogram_focus_index
            .map(|index| index.min(last_index))
            .or(Some(last_index))
    }

    pub(crate) fn selected_frame(&self) -> Option<PerfFrameSnapshot> {
        let selected_index = self.selected_history_index()?;
        self.frame_history.get(selected_index).cloned()
    }

    pub(crate) fn latest_frame(&self) -> Option<PerfFrameSnapshot> {
        self.frame_history.back().cloned()
    }

    pub(crate) fn selected_or_latest_frame(&self) -> Option<PerfFrameSnapshot> {
        self.selected_frame().or_else(|| self.latest_frame())
    }

    pub(crate) fn active_range_summary(&self) -> Option<PerfFrameRangeSummary> {
        let (start, end) = self.active_history_range_indices()?;
        let start_frame = self.frame_history.get(start)?;
        let end_frame = self.frame_history.get(end)?;

        let mut frame_count = 0usize;
        let mut total_frame_time_ms = 0.0f32;
        let mut worst_frame_time_ms = 0.0f32;
        let mut stage_totals = [0.0f32; PERF_STAGE_COUNT];

        for frame in self.frame_history.range(start..=end) {
            frame_count += 1;
            total_frame_time_ms += frame.frame_time_ms;
            worst_frame_time_ms = worst_frame_time_ms.max(frame.frame_time_ms);
            for (stage_index, value) in frame.stage_ms.iter().enumerate() {
                stage_totals[stage_index] += *value;
            }
        }

        if frame_count == 0 {
            return None;
        }

        let average_frame_time_ms = total_frame_time_ms / frame_count as f32;
        let average_fps = if average_frame_time_ms > 1e-4 {
            1000.0 / average_frame_time_ms
        } else {
            0.0
        };

        let mut stage_avg = [0.0f32; PERF_STAGE_COUNT];
        for (stage_index, value) in stage_totals.iter().enumerate() {
            stage_avg[stage_index] = *value / frame_count as f32;
        }

        Some(PerfFrameRangeSummary {
            start_frame_index: start_frame.frame_index,
            end_frame_index: end_frame.frame_index,
            frame_count,
            average_frame_time_ms,
            worst_frame_time_ms,
            average_fps,
            dominant_stage: Self::dominant_stage_from_stage_ms(&stage_avg),
        })
    }

    pub(crate) fn dominant_stage_this_frame(&self) -> Option<PerfStage> {
        Self::dominant_stage_from_stage_ms(&self.frame_stage_ms)
    }

    fn dominant_stage_from_stage_ms(stage_ms: &[f32; PERF_STAGE_COUNT]) -> Option<PerfStage> {
        let stages = PerfStage::roots_without_frame_total();

        let mut dominant: Option<(PerfStage, f32)> = None;
        for stage in stages {
            let value = stage_ms[stage.as_index()];
            if value <= f32::EPSILON {
                continue;
            }
            dominant = match dominant {
                None => Some((*stage, value)),
                Some((_, best)) if value > best => Some((*stage, value)),
                current => current,
            };
        }

        dominant.map(|(stage, _)| stage)
    }

    fn active_history_range_indices(&self) -> Option<(usize, usize)> {
        if let Some((start, end)) = self.selected_history_range_indices() {
            return Some((start, end));
        }

        if let Some(index) = self.selected_history_index() {
            return Some((index, index));
        }

        self.frame_history
            .len()
            .checked_sub(1)
            .map(|last| (last, last))
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
    use web_time::Duration;

    use crate::platform::state_host::PlatformInstant;

    use super::{
        EditorPerfProfiler, PerfStage, PerfStat, PERF_FRAME_BUDGET_60_FPS_MS,
        PERF_FRAME_HISTORY_CAPACITY,
    };

    fn approx_eq(left: f32, right: f32, eps: f32) {
        assert!((left - right).abs() <= eps, "left={left}, right={right}");
    }

    fn enabled_profiler() -> EditorPerfProfiler {
        let mut profiler = EditorPerfProfiler::new();
        profiler.enabled = true;
        profiler
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
        let mut profiler = enabled_profiler();
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
    fn observe_span_captures_frame_event_offsets() {
        let mut profiler = enabled_profiler();
        profiler.begin_frame();

        let started_at = PlatformInstant::now();
        let ended_at = started_at + Duration::from_millis(2);
        profiler.observe_span(PerfStage::TimelineSeek, started_at, ended_at);
        profiler.finish_frame(12.0);

        let history = profiler.frame_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].span_events.len(), 2);

        let seek_event = history[0]
            .span_events
            .iter()
            .find(|event| event.stage == PerfStage::TimelineSeek)
            .expect("timeline seek span event should be recorded");
        assert!(seek_event.duration_ms >= 1.95);
        assert!(seek_event.end_ms >= seek_event.start_ms);

        let frame_total_event = history[0]
            .span_events
            .iter()
            .find(|event| event.stage == PerfStage::FrameTotal)
            .expect("frame total span event should be recorded");
        approx_eq(frame_total_event.start_ms, 0.0, 1e-6);
        approx_eq(frame_total_event.duration_ms, 12.0, 1e-6);
        approx_eq(frame_total_event.end_ms, 12.0, 1e-6);
    }

    #[test]
    fn pause_freezes_history_but_keeps_cumulative_stats() {
        let mut profiler = enabled_profiler();

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
    fn resuming_from_pause_follows_latest_and_clears_selection() {
        let mut profiler = enabled_profiler();
        profiler.begin_frame();
        profiler.finish_frame(10.0);
        profiler.begin_frame();
        profiler.finish_frame(11.0);

        profiler.toggle_pause();
        assert!(profiler.paused);

        profiler.set_selected_history_index(0);
        assert_eq!(profiler.selected_history_index(), Some(0));
        assert!(!profiler.histogram_follow_latest());

        profiler.toggle_pause();
        assert!(!profiler.paused);
        assert!(profiler.histogram_follow_latest());
        assert_eq!(profiler.selected_history_index(), None);
    }

    #[test]
    fn selected_history_index_round_trips() {
        let mut profiler = enabled_profiler();
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

    #[test]
    fn history_rotation_rebases_selection_and_focus_indices() {
        let mut profiler = enabled_profiler();
        for frame in 0..PERF_FRAME_HISTORY_CAPACITY {
            profiler.begin_frame();
            profiler.observe(PerfStage::TimelinePlayback, frame as f32 + 1.0);
            profiler.finish_frame(10.0);
        }

        profiler.set_selected_history_index(100);
        profiler.focus_histogram_index(120);

        profiler.begin_frame();
        profiler.observe(PerfStage::TimelinePlayback, 1.0);
        profiler.finish_frame(10.0);

        assert_eq!(profiler.selected_history_index(), Some(99));
        assert_eq!(profiler.histogram_focus_history_index(), Some(119));
        let selected = profiler
            .selected_frame()
            .expect("selected frame should be preserved after history rotation");
        assert_eq!(selected.frame_index, 100);

        profiler.clear_selection();
        profiler.set_selected_history_range(100, 110);

        profiler.begin_frame();
        profiler.observe(PerfStage::TimelinePlayback, 1.0);
        profiler.finish_frame(10.0);

        assert_eq!(profiler.selected_history_range_indices(), Some((99, 109)));
    }

    #[test]
    fn selected_history_range_and_chunk_summary_work() {
        let mut profiler = enabled_profiler();
        for frame in 0..6 {
            profiler.begin_frame();
            profiler.observe(PerfStage::TimelinePlayback, frame as f32 + 2.0);
            profiler.observe(PerfStage::DirtyProcess, 1.0);
            profiler.finish_frame(12.0 + frame as f32);
        }

        profiler.set_selected_history_range(1, 4);
        assert_eq!(profiler.selected_history_range_indices(), Some((1, 4)));

        let summary = profiler
            .active_range_summary()
            .expect("range summary should exist");
        assert_eq!(summary.frame_count, 4);
        assert_eq!(summary.start_frame_index, 1);
        assert_eq!(summary.end_frame_index, 4);
        approx_eq(summary.average_frame_time_ms, 14.5, 1e-6);
        approx_eq(summary.worst_frame_time_ms, 16.0, 1e-6);
    }

    #[test]
    fn histogram_pan_and_follow_latest_can_be_disabled() {
        let mut profiler = enabled_profiler();
        for frame in 0..5 {
            profiler.begin_frame();
            profiler.observe(PerfStage::TimelinePlayback, frame as f32 + 1.0);
            profiler.finish_frame(10.0);
        }

        assert!(profiler.histogram_follow_latest());
        profiler.pan_histogram(-2);
        assert!(!profiler.histogram_follow_latest());
        assert_eq!(profiler.histogram_focus_history_index(), Some(2));

        profiler.pan_histogram(-500);
        assert_eq!(profiler.histogram_focus_history_index(), Some(0));
        profiler.pan_histogram(500);
        assert_eq!(profiler.histogram_focus_history_index(), Some(4));
    }

    #[test]
    fn disabled_profiler_keeps_stats_without_capturing_history_or_spans() {
        let mut profiler = EditorPerfProfiler::new();

        profiler.begin_frame();
        let started_at = PlatformInstant::now();
        profiler.observe_span(
            PerfStage::TimelineSeek,
            started_at,
            started_at + Duration::from_millis(3),
        );
        profiler.finish_frame(PERF_FRAME_BUDGET_60_FPS_MS + 2.0);

        assert!(profiler.frame_history().is_empty());
        assert!(profiler.frame_span_events.is_empty());
        assert_eq!(profiler.frame_started_at, None);
        assert_eq!(profiler.frame_spike_count, 1);
        assert_eq!(profiler.stats[PerfStage::TimelineSeek.as_index()].calls, 1);
        assert_eq!(profiler.stats[PerfStage::FrameTotal.as_index()].calls, 1);
    }
}
