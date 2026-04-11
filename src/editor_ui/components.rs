/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::EditorUiViewModel;

pub(crate) const MIN_TIMELINE_DURATION_SECONDS: f32 = 0.1;
pub(crate) const MAX_TIMELINE_DURATION_SECONDS: f32 = 600.0;
pub(crate) const DEFAULT_TIMELINE_WINDOW_SECONDS: f32 = 20.0;

fn timeline_visible_duration(duration_seconds: f32, zoom: f32) -> f32 {
    let duration = duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS);
    let zoom = zoom.clamp(0.1, 10.0);
    (DEFAULT_TIMELINE_WINDOW_SECONDS / zoom).clamp(MIN_TIMELINE_DURATION_SECONDS, duration)
}

pub(crate) fn timeline_metrics(duration_seconds: f32) -> f32 {
    duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS)
}

pub(crate) fn show_timeline_bar(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    let duration_seconds = duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS);
    let timeline_zoom = view.waveform_zoom;
    let visible_duration = timeline_visible_duration(duration_seconds, timeline_zoom);
    // Removed max_scroll/clamp to allow playhead to stay centered at start/end by going beyond 0..duration
    let centered_scroll = view.timeline_time_seconds - visible_duration * 0.5;
    if (centered_scroll - view.waveform_scroll).abs() > 0.0001 {
        commands.push(AppCommand::EditorSetWaveformScroll(centered_scroll));
    }
    let view_start = centered_scroll;
    let view_end = view_start + visible_duration;

    ui.horizontal(|ui| {
        ui.label(format!("{} Time:", egui_phosphor::regular::CLOCK));
        let mut time_seconds = view.timeline_time_seconds;
        let drag_value = egui::DragValue::new(&mut time_seconds)
            .speed(0.01)
            .range(0.0..=duration_seconds);
        if ui
            .add_sized([80.0, ui.spacing().interact_size.y], drag_value)
            .changed()
        {
            commands.push(AppCommand::EditorSetTimelineTime(time_seconds));
        }

        ui.add_space(4.0);
        let button_size = egui::vec2(22.0, ui.spacing().interact_size.y);

        let play_icon = if view.playing {
            egui_phosphor::regular::PAUSE
        } else {
            egui_phosphor::regular::PLAY
        };
        let play_tooltip = if view.playing { "Pause" } else { "Play" };
        if ui
            .add_sized(button_size, egui::Button::new(play_icon))
            .on_hover_text(format!("{} (Space)", play_tooltip))
            .clicked()
        {
            commands.push(AppCommand::EditorToggleTimelinePlayback);
        }
        ui.add_space(4.0);

        if ui
            .add_sized(
                button_size,
                egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_MINUS),
            )
            .on_hover_text("Zoom Out")
            .clicked()
        {
            let new_zoom = (timeline_zoom / 1.25).clamp(0.1, 10.0);
            commands.push(AppCommand::EditorSetWaveformZoom(new_zoom));
        }
        if ui
            .add_sized(
                button_size,
                egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_PLUS),
            )
            .on_hover_text("Zoom In")
            .clicked()
        {
            let new_zoom = (timeline_zoom * 1.25).clamp(0.1, 10.0);
            commands.push(AppCommand::EditorSetWaveformZoom(new_zoom));
        }
        ui.add_space(4.0);

        let available_width = ui.available_width();
        let timeline_height = 18.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, timeline_height),
            egui::Sense::click_and_drag(),
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
        let timing_points = view.timing_points;
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
                    let x = rect.left() + (time - view_start) / visible_duration * rect.width();
                    let is_downbeat = beat.is_multiple_of(tp.time_signature_numerator);
                    let (alpha, width) = if is_downbeat { (100, 1.5) } else { (50, 0.5) };
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        egui::Stroke::new(
                            width,
                            egui::Color32::from_rgba_premultiplied(180, 180, 255, alpha),
                        ),
                    );
                }
                beat += 1;
                time += beat_duration;
                if beat > 10000 {
                    break;
                }
            }
        }

        // Draw tap circles
        for tap_time in view.tap_times {
            if *tap_time >= view_start && *tap_time <= view_end {
                let x = rect.left() + (*tap_time - view_start) / visible_duration * rect.width();
                painter.circle_filled(
                    egui::pos2(x, center_y),
                    3.0,
                    egui::Color32::from_rgb(255, 170, 64),
                );
            }
        }

        // Draw timing point markers (red vertical lines)
        for tp in view.timing_points {
            if tp.time_seconds >= view_start && tp.time_seconds <= view_end {
                let x =
                    rect.left() + (tp.time_seconds - view_start) / visible_duration * rect.width();
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
                );
            }
        }

        // Draw trigger markers near the top edge.
        for (index, trigger) in view.triggers.iter().enumerate() {
            if trigger.time_seconds < view_start || trigger.time_seconds > view_end {
                continue;
            }

            let x =
                rect.left() + (trigger.time_seconds - view_start) / visible_duration * rect.width();
            let fill = match &trigger.action {
                crate::types::TimedTriggerAction::CameraFollow { .. } => {
                    egui::Color32::from_rgb(110, 210, 140)
                }
                crate::types::TimedTriggerAction::CameraPose { .. } => {
                    egui::Color32::from_rgb(255, 210, 90)
                }
                _ => egui::Color32::from_rgb(110, 140, 255),
            };
            let stroke = if view.trigger_selected_index == Some(index) {
                egui::Stroke::new(2.0, egui::Color32::WHITE)
            } else {
                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(20, 24, 30, 220))
            };
            let points = vec![
                egui::pos2(x, rect.top() + 2.0),
                egui::pos2(x - 5.0, rect.top() + 10.0),
                egui::pos2(x + 5.0, rect.top() + 10.0),
            ];
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }

        // Draw playhead
        if view.timeline_time_seconds >= view_start && view.timeline_time_seconds <= view_end {
            let current_x = rect.left()
                + (view.timeline_time_seconds - view_start) / visible_duration * rect.width();
            painter.line_segment(
                [
                    egui::pos2(current_x, rect.top()),
                    egui::pos2(current_x, rect.bottom()),
                ],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 200, 255)),
            );
        }

        // Drag primary to move timeline around the fixed playhead.
        if response.dragged_by(egui::PointerButton::Primary) {
            let pointer_delta_x = ui.input(|i| i.pointer.delta().x);
            if pointer_delta_x.abs() > 0.0 {
                let time_per_pixel = visible_duration / rect.width().max(1.0);
                let new_time = (view.timeline_time_seconds - pointer_delta_x * time_per_pixel)
                    .clamp(0.0, duration_seconds);
                commands.push(AppCommand::EditorSetTimelineTime(new_time));
                commands.push(AppCommand::EditorSetWaveformScroll(
                    new_time - visible_duration * 0.5,
                ));
            }
        }

        // Mouse wheel over the timeline bar adjusts window width.
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta);
            if scroll_delta.y.abs() > 0.0 {
                let zoom_factor = 1.0 + scroll_delta.y * 0.002;
                let new_zoom = (timeline_zoom * zoom_factor).clamp(0.1, 10.0);
                commands.push(AppCommand::EditorSetWaveformZoom(new_zoom));
            }
        }

        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let mut nearest_trigger_index = None;
                let mut nearest_distance = f32::INFINITY;

                for (index, trigger) in view.triggers.iter().enumerate() {
                    if trigger.time_seconds < view_start || trigger.time_seconds > view_end {
                        continue;
                    }

                    let x = rect.left()
                        + (trigger.time_seconds - view_start) / visible_duration * rect.width();
                    let distance = (x - pointer.x).abs();
                    if distance < nearest_distance {
                        nearest_distance = distance;
                        nearest_trigger_index = Some(index);
                    }
                }

                if let Some(index) = nearest_trigger_index.filter(|_| nearest_distance <= 8.0) {
                    commands.push(AppCommand::EditorSetTriggerSelected(Some(index)));
                    commands.push(AppCommand::EditorSetTimelineTime(
                        view.triggers[index]
                            .time_seconds
                            .clamp(0.0, duration_seconds),
                    ));
                } else {
                    let normalized_x = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    let clicked_time = view_start + normalized_x * visible_duration;
                    commands.push(AppCommand::EditorSetTriggerSelected(None));
                    commands.push(AppCommand::EditorSetTimelineTime(
                        clicked_time.clamp(0.0, duration_seconds),
                    ));
                }
            }
        }
    });
}

pub(crate) fn show_waveform_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    let duration_seconds = view
        .timeline_duration_seconds
        .max(MIN_TIMELINE_DURATION_SECONDS);
    let waveform_samples = view.waveform_samples;
    let sample_rate = view.waveform_sample_rate;
    let zoom = view.waveform_zoom;

    let available_size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(15, 20, 28));

    // Visible time window (centered on playhead to prevent drift)
    let visible_duration = timeline_visible_duration(duration_seconds, zoom);
    let view_start = view.timeline_time_seconds - visible_duration * 0.5;
    let view_end = view_start + visible_duration;

    if !waveform_samples.is_empty() && sample_rate > 0 {
        // Waveform drawing
        const WAVEFORM_WINDOW: usize = 256;
        let samples_per_second = sample_rate as f32 / WAVEFORM_WINDOW as f32;
        let pixels_per_second = rect.width() / visible_duration;

        let start_sample = (view_start * samples_per_second).floor().max(0.0) as usize;
        let end_sample = (view_end * samples_per_second).ceil().max(0.0) as usize;
        let end_sample = end_sample.min(waveform_samples.len());

        if end_sample > start_sample {
            let waveform_color = egui::Color32::from_rgba_premultiplied(100, 160, 255, 120);
            let center_y = rect.center().y;
            let half_height = rect.height() * 0.4;

            let pixel_per_sample = pixels_per_second / samples_per_second;

            for (idx, &amplitude) in waveform_samples[start_sample..end_sample]
                .iter()
                .enumerate()
            {
                let sample_idx = start_sample + idx;
                let sample_time = sample_idx as f32 / samples_per_second;
                let x = rect.left() + (sample_time - view_start) * pixels_per_second;

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
    let timing_points = view.timing_points;
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
                let x = rect.left() + (time - view_start) / visible_duration * rect.width();
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
            let x = rect.left() + (tp.time_seconds - view_start) / visible_duration * rect.width();
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
    let current_time = view.timeline_time_seconds;
    if current_time >= view_start && current_time <= view_end {
        let x = rect.left() + (current_time - view_start) / visible_duration * rect.width();
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 200, 255)),
        );

        // Add timing point button
        let button_size = egui::vec2(20.0, 20.0);
        let button_rect = egui::Rect::from_center_size(egui::pos2(x, rect.center().y), button_size);
        if ui
            .put(button_rect, egui::Button::new(egui_phosphor::regular::PLUS))
            .on_hover_text("Add timing point at playhead")
            .clicked()
        {
            commands.push(AppCommand::EditorAddTimingPoint {
                time_seconds: current_time,
                bpm: 120.0,
            });
        }
    }

    // Interaction: drag primary to move timeline around the fixed playhead (relative scrubbing)
    if response.dragged_by(egui::PointerButton::Primary) {
        let pointer_delta_x = ui.input(|i| i.pointer.delta().x);
        if pointer_delta_x.abs() > 0.0 {
            let time_per_pixel = visible_duration / rect.width().max(1.0);
            let new_time = (view.timeline_time_seconds - pointer_delta_x * time_per_pixel)
                .clamp(0.0, duration_seconds);
            commands.push(AppCommand::EditorSetTimelineTime(new_time));
            commands.push(AppCommand::EditorSetWaveformScroll(
                new_time - visible_duration * 0.5,
            ));
        }
    }

    // Scroll with mouse wheel to zoom
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);
        if scroll_delta.y.abs() > 0.0 {
            let zoom_factor = 1.0 + scroll_delta.y * 0.002;
            let new_zoom = (zoom * zoom_factor).clamp(0.1, 10.0);
            commands.push(AppCommand::EditorSetWaveformZoom(new_zoom));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_approx_eq as approx_eq;

    #[test]
    fn test_timeline_visible_duration() {
        // Default zoom 1.0 should give default window
        approx_eq(
            timeline_visible_duration(100.0, 1.0),
            DEFAULT_TIMELINE_WINDOW_SECONDS,
            0.001,
        );

        // Zoom in (2.0) should give smaller window (10.0s)
        approx_eq(timeline_visible_duration(100.0, 2.0), 10.0, 0.001);

        // Zoom out (0.5) should give larger window (40.0s)
        approx_eq(timeline_visible_duration(100.0, 0.5), 40.0, 0.001);

        // Should be clamped to duration
        approx_eq(timeline_visible_duration(10.0, 1.0), 10.0, 0.001);

        // Should be clamped to max zoom (10.0), giving a 2.0s window with current limits
        approx_eq(timeline_visible_duration(100.0, 1000.0), 2.0, 0.001);
    }

    #[test]
    fn test_zoom_scaling_logic() {
        let initial_zoom: f32 = 1.0;

        // Zoom in logic
        let zoom_in_1 = (initial_zoom * 1.25).clamp(0.1, 10.0);
        approx_eq(zoom_in_1, 1.25, 0.001);

        let zoom_in_2 = (zoom_in_1 * 1.25).clamp(0.1, 10.0);
        approx_eq(zoom_in_2, 1.5625, 0.001);

        // Zoom out logic
        let zoom_out_1 = (initial_zoom / 1.25).clamp(0.1, 10.0);
        approx_eq(zoom_out_1, 0.8, 0.001);

        // Clamping logic
        let max_zoom = (9.0_f32 * 1.25).clamp(0.1, 10.0);
        approx_eq(max_zoom, 10.0, 0.001);

        let min_zoom = (0.12_f32 * 0.8).clamp(0.1, 10.0);
        approx_eq(min_zoom, 0.1, 0.001);
    }

    #[test]
    fn test_waveform_tick_alignment() {
        // Setup constants similar to show_waveform_panel
        let rect_width = 1000.0;
        let rect_left = 100.0;
        let duration_seconds = 60.0;
        let zoom = 1.0;
        let playhead_time = 15.0; // Playhead at 15s
        let sample_rate = 44100;
        let waveform_window = 256;

        let visible_duration = timeline_visible_duration(duration_seconds, zoom);
        let view_start = playhead_time - visible_duration * 0.5;

        let samples_per_second = sample_rate as f32 / waveform_window as f32;
        let pixels_per_second = rect_width / visible_duration;

        // Pick a time and calculate its x-position as a tick would
        let test_time = 7.5;
        let _tick_x = rect_left + (test_time - view_start) / visible_duration * rect_width;

        // Calculate the same time's x-position as a waveform sample would
        // A sample at test_time would have this index:
        let sample_idx = (test_time * samples_per_second) as usize;
        let sample_time = sample_idx as f32 / samples_per_second;
        let waveform_x = rect_left + (sample_time - view_start) * pixels_per_second;

        // The tick_x and waveform_x should be aligned for the same time
        let tick_x_at_sample_time =
            rect_left + (sample_time - view_start) / visible_duration * rect_width;
        approx_eq(waveform_x, tick_x_at_sample_time, 0.001);
    }
}
