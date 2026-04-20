/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::state::EditorUiViewModel;

const LANE_HEIGHT: f32 = 11.0;
const LANE_SPACING: f32 = 2.0;
const FRAME_ROW_PADDING: f32 = 6.0;
const AXIS_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy)]
struct FlameSpanLayout {
    stage: crate::state::PerfStage,
    start_ms: f32,
    end_ms: f32,
    lane: usize,
}

fn flame_stage_color(stage: crate::state::PerfStage) -> egui::Color32 {
    let name = stage.name();
    let mut hash = 0u64;
    for b in name.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*b as u64);
    }

    let h = (hash % 360) as f32 / 360.0;
    let s = 0.45 + (hash % 50) as f32 / 250.0; // 0.45 - 0.65
    let v = 0.65 + (hash % 50) as f32 / 250.0; // 0.65 - 0.85

    egui::ecolor::Hsva::new(h, s, v, 1.0).into()
}

fn flame_span_label(
    stage: crate::state::PerfStage,
    duration_ms: f32,
    available_width: f32,
) -> Option<String> {
    match stage {
        crate::state::PerfStage::FrameTotal => {
            if available_width < 34.0 {
                return None;
            }
            Some(format!("{duration_ms:.2}ms"))
        }
        _ => {
            if available_width >= 52.0 {
                Some(stage.name().to_owned())
            } else {
                None
            }
        }
    }
}

fn selected_zoom_range(view: &EditorUiViewModel<'_>, history_len: usize) -> Option<(usize, usize)> {
    if history_len == 0 {
        return None;
    }

    let last = history_len - 1;

    if let Some((start, end)) = view.perf_selected_history_range {
        return Some((start.min(last), end.min(last)));
    }

    if let Some(index) = view.perf_selected_history_index {
        let clamped = index.min(last);
        return Some((clamped, clamped));
    }

    let center = if view.perf_histogram_follow_latest {
        last
    } else {
        view.perf_histogram_focus_index.unwrap_or(last).min(last)
    };
    let chunk_size = 16.min(history_len).max(1);
    let mut start = center.saturating_sub(chunk_size / 2);
    if start + chunk_size > history_len {
        start = history_len.saturating_sub(chunk_size);
    }
    let end = start + chunk_size.saturating_sub(1);
    Some((start, end))
}

fn build_timeline_span_layout(
    selected_frames: &[crate::state::PerfFrameSnapshot],
) -> (Vec<FlameSpanLayout>, usize, Vec<(f32, u64)>, f32) {
    let mut frame_boundaries = Vec::with_capacity(selected_frames.len().saturating_add(1));
    let mut absolute_events: Vec<(crate::state::PerfStage, f32, f32)> = Vec::new();

    let mut timeline_cursor_ms = 0.0f32;
    for snapshot in selected_frames {
        frame_boundaries.push((timeline_cursor_ms, snapshot.frame_index));

        for event in snapshot
            .span_events
            .iter()
            .filter(|event| event.duration_ms > 0.01)
        {
            let start_ms = (timeline_cursor_ms + event.start_ms).max(timeline_cursor_ms);
            let end_ms = (timeline_cursor_ms + event.end_ms).max(start_ms + 0.001);
            absolute_events.push((event.stage, start_ms, end_ms));
        }

        timeline_cursor_ms += snapshot.frame_time_ms.max(0.01);
    }
    frame_boundaries.push((timeline_cursor_ms, 0));

    absolute_events.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| (right.2 - right.1).total_cmp(&(left.2 - left.1)))
            .then_with(|| left.2.total_cmp(&right.2))
    });

    let mut lane_ends: Vec<f32> = Vec::new();
    let mut spans = Vec::with_capacity(absolute_events.len());
    for (stage, start_ms, end_ms) in absolute_events {
        let lane = lane_ends
            .iter()
            .position(|lane_end| start_ms >= *lane_end)
            .unwrap_or_else(|| {
                lane_ends.push(0.0);
                lane_ends.len() - 1
            });

        lane_ends[lane] = end_ms;
        spans.push(FlameSpanLayout {
            stage,
            start_ms,
            end_ms,
            lane,
        });
    }

    (
        spans,
        lane_ends.len().max(1),
        frame_boundaries,
        timeline_cursor_ms.max(1.0),
    )
}

pub(crate) fn show_perf_flame_graph_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    viewport_width: f32,
    viewport_height: f32,
) {
    let history = &view.perf_frame_history;

    ui.separator();

    if history.is_empty() {
        ui.monospace("No frame history");
        return;
    }

    let history_len = history.len();

    if let Some((start, end)) = selected_zoom_range(view, history_len) {
        let selected_frames = &history[start..=end];
        let (span_layout, lane_count, frame_boundaries, total_selected_time_ms) =
            build_timeline_span_layout(selected_frames);
        let chart_height =
            lane_count as f32 * (LANE_HEIGHT + LANE_SPACING) + FRAME_ROW_PADDING * 2.0;

        egui::ScrollArea::vertical()
            .max_height((viewport_height * 0.34).max(180.0))
            .show(ui, |ui| {
                let (chart_rect, _) = ui.allocate_exact_size(
                    egui::vec2(viewport_width.max(280.0), chart_height.max(88.0)),
                    egui::Sense::hover(),
                );
                let chart_painter = ui.painter_at(chart_rect);

                let flame_rect = egui::Rect::from_min_max(
                    egui::pos2(chart_rect.left() + 4.0, chart_rect.top() + 2.0),
                    egui::pos2(chart_rect.right() - 4.0, chart_rect.bottom() - AXIS_HEIGHT),
                );

                for (boundary_ms, _frame_index) in frame_boundaries
                    .iter()
                    .skip(1)
                    .take(selected_frames.len().saturating_sub(1))
                {
                    let ratio = (*boundary_ms / total_selected_time_ms).clamp(0.0, 1.0);
                    let x = egui::lerp(flame_rect.left()..=flame_rect.right(), ratio);
                    chart_painter.line_segment(
                        [
                            egui::pos2(x, flame_rect.top()),
                            egui::pos2(x, flame_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(58)),
                    );
                }

                for span in span_layout {
                    let start_ratio = (span.start_ms / total_selected_time_ms).clamp(0.0, 1.0);
                    let end_ratio = (span.end_ms / total_selected_time_ms).clamp(0.0, 1.0);
                    let x_min = egui::lerp(flame_rect.left()..=flame_rect.right(), start_ratio);
                    let x_max = egui::lerp(flame_rect.left()..=flame_rect.right(), end_ratio)
                        .max(x_min + 1.0);
                    let y_min = flame_rect.top()
                        + FRAME_ROW_PADDING
                        + span.lane as f32 * (LANE_HEIGHT + LANE_SPACING);
                    let y_max = (y_min + LANE_HEIGHT).min(flame_rect.bottom() - 1.0);
                    if y_max <= y_min {
                        continue;
                    }

                    let span_rect = egui::Rect::from_min_max(
                        egui::pos2(x_min, y_min),
                        egui::pos2(x_max, y_max),
                    );
                    chart_painter.rect_filled(span_rect, 1.0, flame_stage_color(span.stage));

                    let duration_ms = (span.end_ms - span.start_ms).max(0.0);
                    if let Some(label) =
                        flame_span_label(span.stage, duration_ms, span_rect.width())
                    {
                        chart_painter.text(
                            egui::pos2(span_rect.left() + 2.0, span_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::monospace(8.0),
                            egui::Color32::from_rgb(8, 12, 16),
                        );
                    }
                }
            });
    }
}
