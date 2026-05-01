/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::{
    commands::AppCommand,
    state::{EditorUiViewModel, PerfStage},
};

const LANE_HEIGHT: f32 = 11.0;
const LANE_SPACING: f32 = 2.0;
const FRAME_ROW_PADDING: f32 = 6.0;
const AXIS_HEIGHT: f32 = 20.0;
const PERF_FRAME_BUDGET_60_FPS_MS: f32 = 16.7;
const FLAME_GRAPH_DRAG_FRAME_STEP_PX: f32 = 4.0;

#[derive(Clone, Copy)]
struct FlameSpanLayout {
    stage: PerfStage,
    start_ms: f32,
    end_ms: f32,
    lane: usize,
    frame_index: u64,
    frame_time_ms: f32,
    frame_dominant_stage: Option<PerfStage>,
    frame_stage_total_ms: f32,
    local_start_ms: f32,
    local_end_ms: f32,
}

#[derive(Clone, Copy)]
struct PendingFlameSpanLayout {
    stage: PerfStage,
    start_ms: f32,
    end_ms: f32,
    frame_index: u64,
    frame_time_ms: f32,
    frame_dominant_stage: Option<PerfStage>,
    frame_stage_total_ms: f32,
    local_start_ms: f32,
    local_end_ms: f32,
}

fn lerp_channel(start: u8, end: u8, t: f32) -> u8 {
    (start as f32 + (end as f32 - start as f32) * t).round() as u8
}

fn frame_total_color(frame_time_ms: f32) -> egui::Color32 {
    let ratio = (frame_time_ms / PERF_FRAME_BUDGET_60_FPS_MS).clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        lerp_channel(255, 214, ratio),
        lerp_channel(210, 84, ratio),
        lerp_channel(90, 84, ratio),
    )
}

fn flame_stage_color(stage: PerfStage) -> egui::Color32 {
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

fn show_flame_span_tooltip(
    ui: &mut egui::Ui,
    span: FlameSpanLayout,
    selected_frame_count: usize,
    total_selected_time_ms: f32,
) {
    let duration_ms = (span.end_ms - span.start_ms).max(0.0);
    let selection_share = duration_ms / total_selected_time_ms.max(0.001) * 100.0;
    let frame_share = duration_ms / span.frame_time_ms.max(0.001) * 100.0;
    let stage_frame_share = span.frame_stage_total_ms / span.frame_time_ms.max(0.001) * 100.0;

    ui.monospace(span.stage.name());
    ui.separator();
    ui.monospace(format!("Frame #{}", span.frame_index));
    ui.monospace(format!("Duration: {:.3}ms", duration_ms));
    ui.monospace(format!("Frame share: {:.1}%", frame_share));

    if selected_frame_count > 1 {
        ui.separator();
        ui.monospace(format!(
            "Selection time: {:.3}ms .. {:.3}ms",
            span.start_ms, span.end_ms
        ));
        ui.monospace(format!("Selection share: {:.1}%", selection_share));
    }

    ui.separator();
    ui.monospace(format!(
        "Frame local: {:.3}ms .. {:.3}ms",
        span.local_start_ms, span.local_end_ms
    ));
    ui.monospace(format!("Frame time: {:.3}ms", span.frame_time_ms));
    ui.monospace(format!(
        "Stage total in frame: {:.3}ms ({:.1}%)",
        span.frame_stage_total_ms, stage_frame_share
    ));
    ui.monospace(format!("Lane: {}", span.lane + 1));

    if let Some(dominant_stage) = span.frame_dominant_stage {
        ui.monospace(format!("Frame dominant: {}", dominant_stage.name()));
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

fn push_flame_graph_zoom_commands(
    view: &EditorUiViewModel<'_>,
    history_len: usize,
    scroll_y: f32,
    commands: &mut Vec<AppCommand>,
) {
    if history_len == 0 || scroll_y.abs() <= 0.0 {
        return;
    }

    let last = history_len - 1;
    let (current_start, current_end) = if let Some(range) = view.perf_selected_history_range {
        (range.0.min(last), range.1.min(last))
    } else if let Some(index) = view.perf_selected_history_index {
        let clamped = index.min(last);
        (clamped, clamped)
    } else {
        let center = if view.perf_histogram_follow_latest {
            last
        } else {
            view.perf_histogram_focus_index.unwrap_or(last).min(last)
        };
        let chunk_size = 16.min(history_len).max(1);
        let start = center.saturating_sub(chunk_size / 2);
        let end = (start + chunk_size).saturating_sub(1).min(last);
        (start, end)
    };

    let current_width = current_end.saturating_sub(current_start) + 1;
    let anchor_index = if let Some(index) = view.perf_selected_history_index {
        index.min(last)
    } else if let Some(index) = view.perf_histogram_focus_index {
        index.min(last)
    } else {
        (current_start + current_end) / 2
    };

    let zoom_factor = if scroll_y > 0.0 {
        1.0 + scroll_y * 0.005
    } else {
        1.0 / (1.0 - scroll_y * 0.005)
    };
    let next_width = (current_width as f32 / zoom_factor).clamp(1.0, 1024.0);
    let next_width_i = if scroll_y > 0.0 {
        next_width.floor() as usize
    } else {
        next_width.ceil() as usize
    }
    .clamp(1, 1024);

    if next_width_i == current_width {
        return;
    }

    let left_count = (next_width_i - 1) / 2;
    let right_count = next_width_i - 1 - left_count;

    let mut clamped_start = anchor_index.saturating_sub(left_count);
    let mut clamped_end = anchor_index.saturating_add(right_count).min(last);

    let clamped_width = clamped_end.saturating_sub(clamped_start) + 1;
    if clamped_width < next_width_i {
        if clamped_start == 0 {
            clamped_end = (next_width_i - 1).min(last);
        } else {
            clamped_start = last.saturating_add(1).saturating_sub(next_width_i);
            clamped_end = last;
        }
    }

    commands.push(AppCommand::EditorSetPerfFollowLatest(false));
    commands.push(AppCommand::EditorSelectPerfHistoryRange {
        start: clamped_start,
        end: clamped_end,
    });
    commands.push(AppCommand::EditorFocusPerfHistogramIndex(anchor_index));
}

fn build_timeline_span_layout<'a>(
    selected_frames: impl Iterator<Item = &'a crate::state::PerfFrameSnapshot>,
    selected_frame_count: usize,
) -> (Vec<FlameSpanLayout>, usize, Vec<(f32, u64)>, f32) {
    let mut frame_boundaries = Vec::with_capacity(selected_frame_count.saturating_add(1));
    let mut absolute_events: Vec<PendingFlameSpanLayout> = Vec::new();

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
            absolute_events.push(PendingFlameSpanLayout {
                stage: event.stage,
                start_ms,
                end_ms,
                frame_index: snapshot.frame_index,
                frame_time_ms: snapshot.frame_time_ms,
                frame_dominant_stage: snapshot.dominant_stage,
                frame_stage_total_ms: snapshot.stage_ms[event.stage.as_index()],
                local_start_ms: event.start_ms,
                local_end_ms: event.end_ms,
            });
        }

        timeline_cursor_ms += snapshot.frame_time_ms.max(0.01);
    }
    frame_boundaries.push((timeline_cursor_ms, 0));

    absolute_events.sort_by(|left, right| {
        left.start_ms
            .total_cmp(&right.start_ms)
            .then_with(|| (right.end_ms - right.start_ms).total_cmp(&(left.end_ms - left.start_ms)))
            .then_with(|| left.end_ms.total_cmp(&right.end_ms))
    });

    let mut lane_ends: Vec<f32> = Vec::new();
    let mut spans = Vec::with_capacity(absolute_events.len());
    for event in absolute_events {
        let lane = lane_ends
            .iter()
            .position(|lane_end| event.start_ms >= *lane_end)
            .unwrap_or_else(|| {
                lane_ends.push(0.0);
                lane_ends.len() - 1
            });

        lane_ends[lane] = event.end_ms;
        spans.push(FlameSpanLayout {
            stage: event.stage,
            start_ms: event.start_ms,
            end_ms: event.end_ms,
            lane,
            frame_index: event.frame_index,
            frame_time_ms: event.frame_time_ms,
            frame_dominant_stage: event.frame_dominant_stage,
            frame_stage_total_ms: event.frame_stage_total_ms,
            local_start_ms: event.local_start_ms,
            local_end_ms: event.local_end_ms,
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
    commands: &mut Vec<AppCommand>,
) {
    let history = &view.perf_frame_history;

    ui.separator();

    if history.is_empty() {
        ui.monospace("No frame history");
        return;
    }

    let history_len = history.len();

    if let Some((start, end)) = selected_zoom_range(view, history_len) {
        let selected_frame_count = end.saturating_sub(start) + 1;
        let (span_layout, lane_count, frame_boundaries, total_selected_time_ms) =
            build_timeline_span_layout(history.range(start..=end), selected_frame_count);
        let chart_height =
            lane_count as f32 * (LANE_HEIGHT + LANE_SPACING) + FRAME_ROW_PADDING * 2.0;

        egui::ScrollArea::vertical()
            .max_height((viewport_height * 0.34).max(180.0))
            .show(ui, |ui| {
                let (chart_rect, response) = ui.allocate_exact_size(
                    egui::vec2(viewport_width.max(280.0), chart_height.max(88.0)),
                    egui::Sense::click_and_drag(),
                );
                let chart_painter = ui.painter_at(chart_rect);

                let flame_rect = egui::Rect::from_min_max(
                    egui::pos2(chart_rect.left() + 4.0, chart_rect.top() + 2.0),
                    egui::pos2(chart_rect.right() - 4.0, chart_rect.bottom() - AXIS_HEIGHT),
                );

                if response.dragged() {
                    let delta_x = ui.input(|input| input.pointer.delta().x);
                    let frames_to_pan = (-delta_x / FLAME_GRAPH_DRAG_FRAME_STEP_PX).trunc() as i32;
                    if frames_to_pan != 0 {
                        commands.push(AppCommand::EditorSetPerfFollowLatest(false));
                        commands.push(AppCommand::EditorPanPerfHistogram(frames_to_pan));
                    }
                }

                if response.hovered() {
                    let scroll_y = ui.input(|input| input.raw_scroll_delta.y);
                    push_flame_graph_zoom_commands(view, history_len, scroll_y, commands);
                }

                for (boundary_ms, _frame_index) in frame_boundaries
                    .iter()
                    .skip(1)
                    .take(selected_frame_count.saturating_sub(1))
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
                    let duration_ms = (span.end_ms - span.start_ms).max(0.0);
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
                    let color = if span.stage == PerfStage::FrameTotal {
                        frame_total_color(duration_ms)
                    } else {
                        flame_stage_color(span.stage)
                    };
                    chart_painter.rect_filled(span_rect, 1.0, color);

                    let tooltip_id = ui.id().with((
                        "perf_flame_span",
                        span.frame_index,
                        span.stage.as_index(),
                        span.start_ms.to_bits(),
                        span.end_ms.to_bits(),
                    ));
                    let span_response = ui.interact(span_rect, tooltip_id, egui::Sense::hover());
                    if span_response.hovered() {
                        egui::Tooltip::always_open(
                            ui.ctx().clone(),
                            ui.layer_id(),
                            tooltip_id,
                            egui::PopupAnchor::Pointer,
                        )
                        .gap(12.0)
                        .show(|ui| {
                            show_flame_span_tooltip(
                                ui,
                                span,
                                selected_frame_count,
                                total_selected_time_ms,
                            );
                        });
                    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_total_color_moves_from_yellow_to_red_as_frame_time_increases() {
        assert_eq!(
            frame_total_color(0.0),
            egui::Color32::from_rgb(255, 210, 90)
        );
        assert_eq!(
            frame_total_color(PERF_FRAME_BUDGET_60_FPS_MS),
            egui::Color32::from_rgb(214, 84, 84)
        );
        assert_eq!(
            frame_total_color(PERF_FRAME_BUDGET_60_FPS_MS * 2.0),
            egui::Color32::from_rgb(214, 84, 84)
        );
    }
}
