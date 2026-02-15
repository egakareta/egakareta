use crate::commands::AppCommand;
use crate::State;

pub(crate) const MIN_TIMELINE_DURATION_SECONDS: f32 = 0.1;
pub(crate) const MAX_TIMELINE_DURATION_SECONDS: f32 = 600.0;

pub(crate) fn timeline_metrics(duration_seconds: f32) -> f32 {
    duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS)
}

pub(crate) fn show_timeline_bar(ui: &mut egui::Ui, state: &mut State, duration_seconds: f32) {
    let available_width = ui.available_width();
    let timeline_height = 18.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(available_width, timeline_height),
        egui::Sense::click(),
    );

    let painter = ui.painter();
    let center_y = rect.center().y;
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(160));
    painter.line_segment(
        [
            egui::pos2(rect.left(), center_y),
            egui::pos2(rect.right(), center_y),
        ],
        stroke,
    );

    // Draw beat lines from timing points
    let timing_points = state.editor_timing_points();
    for (tp_idx, tp) in timing_points.iter().enumerate() {
        if tp.bpm <= 0.0 {
            continue;
        }
        let beat_duration = 60.0 / tp.bpm;
        let end_time = if tp_idx + 1 < timing_points.len() {
            timing_points[tp_idx + 1].time_seconds
        } else {
            duration_seconds
        };

        let mut beat = 0u32;
        let mut time = tp.time_seconds;
        while time <= end_time {
            let t = (time / duration_seconds).clamp(0.0, 1.0);
            let x = rect.left() + rect.width() * t;
            let is_downbeat = beat.is_multiple_of(tp.time_signature_numerator);
            let (alpha, width) = if is_downbeat { (100, 1.5) } else { (50, 0.5) };
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(
                    width,
                    egui::Color32::from_rgba_premultiplied(180, 180, 255, alpha),
                ),
            );
            beat += 1;
            time += beat_duration;
        }
    }

    // Draw tap circles
    for tap_time in state.editor_tap_times() {
        let t = (*tap_time / duration_seconds).clamp(0.0, 1.0);
        let x = rect.left() + rect.width() * t;
        painter.circle_filled(
            egui::pos2(x, center_y),
            3.0,
            egui::Color32::from_rgb(255, 170, 64),
        );
    }

    // Draw timing point markers (red triangles)
    for tp in state.editor_timing_points() {
        let t = (tp.time_seconds / duration_seconds).clamp(0.0, 1.0);
        let x = rect.left() + rect.width() * t;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
        );
    }

    // Draw playhead
    let current_t = state.editor_timeline_time_seconds() / duration_seconds;
    let current_x = rect.left() + rect.width() * current_t.clamp(0.0, 1.0);
    painter.line_segment(
        [
            egui::pos2(current_x, rect.top()),
            egui::pos2(current_x, rect.bottom()),
        ],
        egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 200, 255)),
    );

    // Click to seek
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let time_seconds = t * duration_seconds;
            state.dispatch(AppCommand::EditorSetTimelineTime(time_seconds));
        }
    }
}

pub(crate) fn show_waveform_panel(ui: &mut egui::Ui, state: &mut State) {
    let duration_seconds = state
        .editor_timeline_duration_seconds()
        .max(MIN_TIMELINE_DURATION_SECONDS);
    let waveform_samples = state.editor_waveform_samples();
    let sample_rate = state.editor_waveform_sample_rate();
    let zoom = state.editor_waveform_zoom();
    let scroll = state.editor_waveform_scroll();

    let available_size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(15, 20, 28));

    // Visible time window
    let visible_duration = duration_seconds / zoom;
    let view_start = scroll;
    let view_end = (scroll + visible_duration).min(duration_seconds);

    if !waveform_samples.is_empty() && sample_rate > 0 {
        // Waveform drawing
        const WAVEFORM_WINDOW: usize = 256;
        let samples_per_second = sample_rate as f32 / WAVEFORM_WINDOW as f32;
        let start_sample = (view_start * samples_per_second) as usize;
        let end_sample = ((view_end * samples_per_second) as usize).min(waveform_samples.len());

        if end_sample > start_sample {
            let waveform_color = egui::Color32::from_rgba_premultiplied(100, 160, 255, 120);
            let center_y = rect.center().y;
            let half_height = rect.height() * 0.4;

            let pixel_per_sample = rect.width() / (end_sample - start_sample).max(1) as f32;

            for (idx, &amplitude) in waveform_samples[start_sample..end_sample]
                .iter()
                .enumerate()
            {
                let x = rect.left() + idx as f32 * pixel_per_sample;
                let amplitude = amplitude.clamp(0.0, 1.0);
                let bar_height = amplitude * half_height;

                if bar_height > 0.5 {
                    painter.line_segment(
                        [
                            egui::pos2(x, center_y - bar_height),
                            egui::pos2(x, center_y + bar_height),
                        ],
                        egui::Stroke::new(pixel_per_sample.max(1.0), waveform_color),
                    );
                }
            }
        }
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No waveform data. Import audio to view waveform.",
            egui::FontId::proportional(16.0),
            egui::Color32::from_gray(120),
        );
    }

    // Draw beat grid lines from timing points
    let timing_points = state.editor_timing_points();
    for (tp_idx, tp) in timing_points.iter().enumerate() {
        if tp.bpm <= 0.0 {
            continue;
        }
        let beat_duration = 60.0 / tp.bpm;
        let end_time = if tp_idx + 1 < timing_points.len() {
            timing_points[tp_idx + 1].time_seconds
        } else {
            duration_seconds
        };

        let mut beat = 0u32;
        let mut time = tp.time_seconds;
        while time <= end_time {
            if time >= view_start && time <= view_end {
                let x = rect.left() + (time - view_start) / (view_end - view_start) * rect.width();
                let is_downbeat = beat.is_multiple_of(tp.time_signature_numerator);
                let (color, width) = if is_downbeat {
                    (
                        egui::Color32::from_rgba_premultiplied(255, 255, 255, 80),
                        1.5,
                    )
                } else {
                    (
                        egui::Color32::from_rgba_premultiplied(255, 255, 255, 30),
                        0.5,
                    )
                };
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    egui::Stroke::new(width, color),
                );
                // Label downbeats
                if is_downbeat && rect.width() > 200.0 {
                    painter.text(
                        egui::pos2(x + 3.0, rect.top() + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:.1}s", time),
                        egui::FontId::proportional(9.0),
                        egui::Color32::from_gray(140),
                    );
                }
            }
            beat += 1;
            time += beat_duration;
            if beat > 10000 {
                break; // safety limit
            }
        }
    }

    // Draw timing point markers (red vertical lines with BPM labels)
    for tp in timing_points {
        if tp.time_seconds >= view_start && tp.time_seconds <= view_end {
            let x = rect.left()
                + (tp.time_seconds - view_start) / (view_end - view_start) * rect.width();
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 80, 80)),
            );
            painter.text(
                egui::pos2(x + 4.0, rect.top() + 14.0),
                egui::Align2::LEFT_TOP,
                format!("{:.1} BPM", tp.bpm),
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(255, 120, 120),
            );
        }
    }

    // Draw playhead
    let current_time = state.editor_timeline_time_seconds();
    if current_time >= view_start && current_time <= view_end {
        let x = rect.left() + (current_time - view_start) / (view_end - view_start) * rect.width();
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 200, 255)),
        );
    }

    // Interaction: click to seek
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let time_seconds = view_start + t * (view_end - view_start);
            state.dispatch(AppCommand::EditorSetTimelineTime(time_seconds));
        }
    }

    // Scroll with mouse wheel to zoom, drag to pan (via scroll offset)
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    if scroll_delta.y.abs() > 0.0 {
        let zoom_factor = 1.0 + scroll_delta.y * 0.002;
        let new_zoom = (zoom * zoom_factor).clamp(0.1, 100.0);
        state.dispatch(AppCommand::EditorSetWaveformZoom(new_zoom));
    }
    if response.dragged_by(egui::PointerButton::Middle)
        || response.dragged_by(egui::PointerButton::Secondary)
    {
        let drag_delta = response.drag_delta();
        let time_per_pixel = (view_end - view_start) / rect.width();
        let new_scroll = (scroll - drag_delta.x * time_per_pixel).max(0.0);
        state.dispatch(AppCommand::EditorSetWaveformScroll(new_scroll));
    }
}
