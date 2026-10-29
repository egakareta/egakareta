/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_state::{commands::AppCommand, state::editor_command::EditorCommand};
use eg_types::{AppSettings, EditorMode};
use euphorium::WaveformView;

use crate::UiCommandSink;

pub(crate) const MIN_TIMELINE_DURATION_SECONDS: f32 = 0.1;
pub(crate) const MAX_TIMELINE_DURATION_SECONDS: f32 = 600.0;
pub(crate) const DEFAULT_TIMELINE_WINDOW_SECONDS: f32 = 20.0;
const TIMELINE_TAP_HIT_RADIUS_PIXELS: f32 = 8.0;
const TIMELINE_CURRENT_TAP_EPSILON_SECONDS: f32 = 0.0001;
const TIMELINE_CONTROL_HEIGHT_SCALE: f32 = 1.5;

/// Direct values needed only while rendering the timeline bar.
pub(crate) struct TimelineBarInputs<'a> {
    pub app_settings: &'a AppSettings,
    pub mode: EditorMode,
    pub last_mode: Option<EditorMode>,
    pub playing: bool,
    pub timeline_time_seconds: f32,
    pub tap_times: &'a [f32],
    pub timing_points: &'a [eg_types::TimingPoint],
    pub waveform_zoom: f32,
    pub waveform_scroll: f32,
    pub trigger_objects: &'a [eg_types::LevelObject],
    pub trigger_selected_index: Option<usize>,
    pub duration_seconds: f32,
}

/// Direct values needed only while rendering the timing waveform.
pub(crate) struct WaveformPanelInputs<'a> {
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub timing_points: &'a [eg_types::TimingPoint],
    pub waveform_zoom: f32,
}

pub(crate) fn show_shadowed_label(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    font_id: egui::FontId,
    text_color: egui::Color32,
    shadow_color: egui::Color32,
    shadow_offset: egui::Vec2,
    wrap_width: f32,
) -> egui::Response {
    let galley = ui
        .painter()
        .layout(text.into(), font_id, egui::Color32::PLACEHOLDER, wrap_width);
    let shadow_extent = egui::vec2(shadow_offset.x.abs(), shadow_offset.y.abs());
    let (rect, response) =
        ui.allocate_exact_size(galley.size() + shadow_extent, egui::Sense::hover());
    let text_pos = rect.min
        + egui::vec2(
            shadow_offset.x.min(0.0).abs(),
            shadow_offset.y.min(0.0).abs(),
        );

    ui.painter()
        .galley(text_pos + shadow_offset, galley.clone(), shadow_color);
    ui.painter().galley(text_pos, galley, text_color);

    response
}

fn timeline_visible_duration(duration_seconds: f32, zoom: f32) -> f32 {
    let duration = duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS);
    let zoom = zoom.clamp(0.1, 10.0);
    (DEFAULT_TIMELINE_WINDOW_SECONDS / zoom).clamp(MIN_TIMELINE_DURATION_SECONDS, duration)
}

fn centered_waveform_range(
    timeline_time_seconds: f32,
    visible_duration_seconds: f32,
) -> std::ops::Range<f64> {
    let visible_duration = f64::from(visible_duration_seconds);
    let view_start = f64::from(timeline_time_seconds) - visible_duration * 0.5;
    view_start..view_start + visible_duration
}

pub(crate) fn timeline_metrics(duration_seconds: f32) -> f32 {
    duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS)
}

fn timeline_bar_nearest_tap_index(
    tap_times: &[f32],
    pointer_x: f32,
    rect: egui::Rect,
    view_start: f32,
    visible_duration: f32,
) -> Option<usize> {
    tap_times
        .iter()
        .enumerate()
        .filter_map(|(index, tap_time)| {
            if !tap_time.is_finite()
                || *tap_time < view_start
                || *tap_time > view_start + visible_duration
            {
                return None;
            }

            let x = rect.left() + (*tap_time - view_start) / visible_duration * rect.width();
            let distance = (x - pointer_x).abs();
            (distance <= TIMELINE_TAP_HIT_RADIUS_PIXELS).then_some((index, distance))
        })
        .min_by(|(_, left_distance), (_, right_distance)| {
            f32::total_cmp(left_distance, right_distance)
        })
        .map(|(index, _)| index)
}

fn timeline_bar_is_tapping(mode: EditorMode, last_mode: Option<EditorMode>) -> bool {
    mode == EditorMode::Tapping
        || (mode == EditorMode::Null && last_mode == Some(EditorMode::Tapping))
}

fn timeline_bar_push_tap_click_commands(
    inputs: &TimelineBarInputs<'_>,
    pointer_x: f32,
    rect: egui::Rect,
    view_start: f32,
    visible_duration: f32,
    commands: &mut impl UiCommandSink,
) -> bool {
    let Some(index) = timeline_bar_nearest_tap_index(
        inputs.tap_times,
        pointer_x,
        rect,
        view_start,
        visible_duration,
    ) else {
        return false;
    };

    commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(
        inputs.tap_times[index],
    )));
    if timeline_bar_is_tapping(inputs.mode, inputs.last_mode) {
        commands.push(AppCommand::Editor(EditorCommand::SetSelectedTap(Some(
            index,
        ))));
    }
    true
}

fn timeline_bar_tap_is_current(tap_time: f32, timeline_time_seconds: f32) -> bool {
    (tap_time - timeline_time_seconds).abs() <= TIMELINE_CURRENT_TAP_EPSILON_SECONDS
}

fn format_timeline_time_seconds(time_seconds: f64) -> String {
    let total_milliseconds = (time_seconds.max(0.0) * 1000.0).round() as u64;
    let minutes = total_milliseconds / 60_000;
    let seconds = (total_milliseconds / 1000) % 60;
    let milliseconds = total_milliseconds % 1000;

    format!("{minutes:02}:{seconds:02}:{milliseconds:03}")
}

fn parse_timeline_time_seconds(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text
        .strip_prefix(egui_phosphor::regular::CLOCK)
        .unwrap_or(text);
    let text = text.trim();
    let parts: Vec<&str> = text.split(':').collect();

    match parts.as_slice() {
        [minutes, seconds, milliseconds] => {
            let minutes = minutes.trim().parse::<u64>().ok()?;
            let seconds = seconds.trim().parse::<u64>().ok()?;
            let milliseconds = milliseconds.trim().parse::<u64>().ok()?;
            if seconds >= 60 || milliseconds >= 1000 {
                return None;
            }
            Some(minutes as f64 * 60.0 + seconds as f64 + milliseconds as f64 / 1000.0)
        }
        [seconds] => seconds.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn show_timeline_bar(
    ui: &mut egui::Ui,
    inputs: &TimelineBarInputs<'_>,
    commands: &mut impl UiCommandSink,
) {
    let app_settings = inputs.app_settings;
    let playing = inputs.playing;
    let timeline_time_seconds = inputs.timeline_time_seconds;
    let tap_times = inputs.tap_times;
    let timing_points = inputs.timing_points;
    let waveform_zoom = inputs.waveform_zoom;
    let waveform_scroll = inputs.waveform_scroll;
    let trigger_objects = inputs.trigger_objects;
    let trigger_selected_index = inputs.trigger_selected_index;
    let duration_seconds = inputs.duration_seconds;
    puffin::profile_scope!("UiTimelineBar");
    let duration_seconds = duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS);
    let timeline_zoom = waveform_zoom;
    let visible_duration = timeline_visible_duration(duration_seconds, timeline_zoom);
    // Removed max_scroll/clamp to allow playhead to stay centered at start/end by going beyond 0..duration
    let centered_scroll = timeline_time_seconds - visible_duration * 0.5;
    if (centered_scroll - waveform_scroll).abs() > 0.0001 {
        commands.push(AppCommand::Editor(EditorCommand::SetWaveformScroll(
            centered_scroll,
        )));
    }
    let view_start = centered_scroll;
    let view_end = view_start + visible_duration;

    ui.horizontal(|ui| {
        let timeline_control_height = ui.spacing().interact_size.y * TIMELINE_CONTROL_HEIGHT_SCALE;
        let mut time_seconds = timeline_time_seconds;
        let drag_value = egui::DragValue::new(&mut time_seconds)
            .speed(0.01)
            .range(0.0..=duration_seconds)
            .prefix(format!("{} ", egui_phosphor::regular::CLOCK))
            .custom_formatter(|value, _| format_timeline_time_seconds(value))
            .custom_parser(parse_timeline_time_seconds);
        if ui
            .scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
                ui.add_sized([112.0, timeline_control_height], drag_value)
            })
            .inner
            .changed()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(
                time_seconds,
            )));
        }

        let button_size = egui::vec2(timeline_control_height.max(22.0), timeline_control_height);

        let play_icon = if playing {
            egui_phosphor::regular::PAUSE
        } else {
            egui_phosphor::regular::PLAY
        };
        let play_tooltip = if playing { "Pause" } else { "Play" };
        if ui
            .add_sized(button_size, egui::Button::new(play_icon).frame(false))
            .on_hover_text(format!(
                "{}{}",
                play_tooltip,
                app_settings.hotkey_hint("toggle_timeline_playback")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback));
        }

        if ui
            .add_sized(
                button_size,
                egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_MINUS).frame(false),
            )
            .on_hover_text(format!(
                "Zoom Out{}",
                app_settings.hotkey_hint_or("zoom_out", "-")
            ))
            .clicked()
        {
            let new_zoom = (timeline_zoom / 1.25).clamp(0.1, 10.0);
            commands.push(AppCommand::Editor(EditorCommand::SetWaveformZoom(new_zoom)));
        }
        if ui
            .add_sized(
                button_size,
                egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_PLUS).frame(false),
            )
            .on_hover_text(format!(
                "Zoom In{}",
                app_settings.hotkey_hint_or("zoom_in", "+")
            ))
            .clicked()
        {
            let new_zoom = (timeline_zoom * 1.25).clamp(0.1, 10.0);
            commands.push(AppCommand::Editor(EditorCommand::SetWaveformZoom(new_zoom)));
        }

        let playtest_button_width = 72.0;
        let available_width =
            (ui.available_width() - playtest_button_width - ui.spacing().item_spacing.x).max(0.0);
        let timeline_height = timeline_control_height;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, timeline_height),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter();
        let center_y = rect.center().y;
        let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(160));
        // Clip the horizontal line to the duration end point if visible
        let line_end_x = if duration_seconds >= view_start && duration_seconds <= view_end {
            rect.left() + (duration_seconds - view_start) / visible_duration * rect.width()
        } else {
            rect.right()
        };
        painter.line_segment(
            [
                egui::pos2(rect.left(), center_y),
                egui::pos2(line_end_x, center_y),
            ],
            stroke,
        );

        // Draw beat lines from timing points
        {
            puffin::profile_scope!("UiTimelineBeatLines");
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
        }

        // Draw tap circles
        for tap_time in tap_times {
            if *tap_time >= view_start && *tap_time <= view_end {
                let x = rect.left() + (*tap_time - view_start) / visible_duration * rect.width();
                let center = egui::pos2(x, center_y);
                if timeline_bar_tap_is_current(*tap_time, timeline_time_seconds) {
                    painter.circle_stroke(
                        center,
                        3.0,
                        egui::Stroke::new(
                            2.0,
                            egui::Color32::from_rgba_premultiplied(255, 245, 210, 230),
                        ),
                    );
                }
                painter.circle_filled(center, 3.0, egui::Color32::from_rgb(255, 170, 64));
            }
        }

        // Draw timing point markers (red vertical lines)
        for tp in timing_points {
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
        for (index, trigger_object) in trigger_objects.iter().enumerate() {
            let Some(trigger) = trigger_object.trigger.as_ref() else {
                continue;
            };
            if trigger.time_seconds < view_start || trigger.time_seconds > view_end {
                continue;
            }

            let x =
                rect.left() + (trigger.time_seconds - view_start) / visible_duration * rect.width();
            let fill = match trigger.camera.as_ref().map(|camera| camera.mode) {
                Some(eg_types::CameraMode::Follow) => egui::Color32::from_rgb(110, 210, 140),
                Some(eg_types::CameraMode::Static) => egui::Color32::from_rgb(255, 210, 90),
                None => egui::Color32::from_rgb(110, 140, 255),
            };
            let stroke = if trigger_selected_index == Some(index) {
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
        if timeline_time_seconds >= view_start && timeline_time_seconds <= view_end {
            let current_x = rect.left()
                + (timeline_time_seconds - view_start) / visible_duration * rect.width();
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
                let new_time = (timeline_time_seconds - pointer_delta_x * time_per_pixel)
                    .clamp(0.0, duration_seconds);
                commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(new_time)));
                commands.push(AppCommand::Editor(EditorCommand::SetWaveformScroll(
                    new_time - visible_duration * 0.5,
                )));
            }
        }

        // Mouse wheel over the timeline bar adjusts window width.
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta());
            if scroll_delta.y.abs() > 0.0 {
                let zoom_factor = 1.0 + scroll_delta.y * 0.002;
                let new_zoom = (timeline_zoom * zoom_factor).clamp(0.1, 10.0);
                commands.push(AppCommand::Editor(EditorCommand::SetWaveformZoom(new_zoom)));
            }
        }

        if response.clicked_by(egui::PointerButton::Secondary)
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(index) = timeline_bar_nearest_tap_index(
                tap_times,
                pointer.x,
                rect,
                view_start,
                visible_duration,
            )
        {
            commands.push(AppCommand::Editor(EditorCommand::RemoveTapAt(
                tap_times[index],
            )));
        }

        if response.clicked_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && !timeline_bar_push_tap_click_commands(
                inputs,
                pointer.x,
                rect,
                view_start,
                visible_duration,
                commands,
            )
        {
            let mut nearest_trigger_index = None;
            let mut nearest_distance = f32::INFINITY;

            for (index, trigger_object) in trigger_objects.iter().enumerate() {
                let Some(trigger) = trigger_object.trigger.as_ref() else {
                    continue;
                };
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
                commands.push(AppCommand::Editor(EditorCommand::SetTriggerSelected(Some(
                    index,
                ))));
                commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(
                    trigger_objects[index]
                        .trigger
                        .as_ref()
                        .map(|trigger| trigger.time_seconds)
                        .unwrap_or(0.0)
                        .clamp(0.0, duration_seconds),
                )));
            } else {
                let normalized_x = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let clicked_time = view_start + normalized_x * visible_duration;
                commands.push(AppCommand::Editor(EditorCommand::SetTriggerSelected(None)));
                commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(
                    clicked_time.clamp(0.0, duration_seconds),
                )));
            }
        }

        ui.add_space(4.0);
        if ui
            .add_sized(
                [playtest_button_width, timeline_control_height],
                egui::Button::new(format!("{} Test", egui_phosphor::regular::GAME_CONTROLLER))
                    .frame(false),
            )
            .on_hover_text(format!(
                "Start playtesting{}",
                app_settings.hotkey_hint_or("playtest", "Enter")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::Playtest));
        }
    });
}

pub(crate) fn show_waveform_panel(
    ui: &mut egui::Ui,
    inputs: &WaveformPanelInputs<'_>,
    waveform_view: &mut WaveformView,
    commands: &mut impl UiCommandSink,
) {
    let timeline_time_seconds = inputs.timeline_time_seconds;
    let timeline_duration_seconds = inputs.timeline_duration_seconds;
    let timing_points = inputs.timing_points;
    let waveform_zoom = inputs.waveform_zoom;
    puffin::profile_scope!("UiWaveformPanel");
    let duration_seconds = timeline_duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS);
    let zoom = waveform_zoom;

    let available_size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(15, 20, 28));

    // Euphorium owns incremental decoding and multiresolution peaks. Keep the
    // viewport centered on the playhead, including at the track boundaries,
    // to preserve the editor's fixed-cursor behavior.
    let requested_visible_duration = timeline_visible_duration(duration_seconds, zoom);
    waveform_view.set_visible_secs(requested_visible_duration as f64);
    let poll = waveform_view
        .poll_visible(timeline_time_seconds as f64)
        .ok();
    if poll.as_ref().is_some_and(|poll| !poll.finished) {
        ui.ctx().request_repaint();
    }
    let visible = centered_waveform_range(timeline_time_seconds, requested_visible_duration);
    let visible_duration = (visible.end - visible.start).max(f64::EPSILON) as f32;
    let view_start = visible.start as f32;
    let view_end = visible.end as f32;

    if waveform_view.is_available() {
        puffin::profile_scope!("UiWaveformDrawPeaks");
        let max_peaks = (rect.width().ceil() as usize).saturating_mul(2).max(1);
        let waveform_color = egui::Color32::from_rgba_premultiplied(100, 160, 255, 255);
        let waveform_color_past = egui::Color32::from_rgba_premultiplied(80, 140, 235, 255);
        let center_y = rect.center().y;
        let half_height = rect.height() * 0.4;
        let mut drew_peak = false;

        if let Some(slice) = waveform_view.slice(visible.clone(), max_peaks) {
            for (index, peak) in slice.peaks().iter().enumerate() {
                if !peak.is_available() {
                    continue;
                }
                let Some(peak_range) = slice.peak_range(index) else {
                    continue;
                };
                let start_time = peak_range.start.as_secs_f64();
                let end_time = peak_range.end.as_secs_f64();
                let x = rect.left()
                    + ((start_time + end_time) * 0.5 - visible.start) as f32 / visible_duration
                        * rect.width();
                let y_top = center_y - peak.max.clamp(-1.0, 1.0) * half_height;
                let y_bottom = center_y - peak.min.clamp(-1.0, 1.0) * half_height;
                let pixel_width = ((end_time - start_time) / f64::from(visible_duration)
                    * f64::from(rect.width()))
                .max(1.0) as f32;
                let color = if end_time <= f64::from(timeline_time_seconds) {
                    waveform_color_past
                } else {
                    waveform_color
                };
                painter.line_segment(
                    [egui::pos2(x, y_top), egui::pos2(x, y_bottom)],
                    egui::Stroke::new(pixel_width, color),
                );
                drew_peak = true;
            }
        }
        if !drew_peak {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if waveform_view.is_finished() {
                    "Waveform decoded with no visible peaks."
                } else {
                    "Loading waveform..."
                },
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(120),
            );
        }
    } else {
        let message = if waveform_view.is_finished() {
            "No waveform data. Import audio to view waveform."
        } else {
            "Loading waveform..."
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(16.0),
            egui::Color32::from_gray(120),
        );
    }

    // Draw beat grid lines from timing points
    {
        puffin::profile_scope!("UiWaveformBeatGrid");
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
    let current_time = timeline_time_seconds;
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
            commands.push(AppCommand::Editor(EditorCommand::AddTimingPoint {
                time_seconds: current_time,
                bpm: 120.0,
            }));
        }
    }

    // Interaction: drag primary to move timeline around the fixed playhead (relative scrubbing)
    if response.dragged_by(egui::PointerButton::Primary) {
        let pointer_delta_x = ui.input(|i| i.pointer.delta().x);
        if pointer_delta_x.abs() > 0.0 {
            let time_per_pixel = visible_duration / rect.width().max(1.0);
            let new_time = (timeline_time_seconds - pointer_delta_x * time_per_pixel)
                .clamp(0.0, duration_seconds);
            commands.push(AppCommand::Editor(EditorCommand::SetTimelineTime(new_time)));
            commands.push(AppCommand::Editor(EditorCommand::SetWaveformScroll(
                new_time - visible_duration * 0.5,
            )));
        }
    }

    // Scroll with mouse wheel to zoom
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta());
        if scroll_delta.y.abs() > 0.0 {
            let zoom_factor = 1.0 + scroll_delta.y * 0.002;
            let new_zoom = (zoom * zoom_factor).clamp(0.1, 10.0);
            commands.push(AppCommand::Editor(EditorCommand::SetWaveformZoom(new_zoom)));
        }
    }
}
#[cfg(test)]
mod tests {
    use eg_types::{
        AppSettings, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection,
        EditorMode, LevelObject, MusicMetadata, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger,
        TimingPoint, TriggerTarget,
    };

    use super::*;
    use crate::test_utils::editor_ui_view;

    struct TimelineViewOptions {
        timeline_time_seconds: f32,
        timeline_duration_seconds: f32,
        waveform_zoom: f32,
        waveform_scroll: f32,
        playing: bool,
        trigger_selected_index: Option<usize>,
    }

    fn make_view<'a>(
        app_settings: &'a AppSettings,
        music_metadata: &'a MusicMetadata,
        timing_points: &'a [TimingPoint],
        trigger_objects: &'a [LevelObject],
        tap_times: &'a [f32],
        waveform_samples: &'a [f32],
        options: TimelineViewOptions,
    ) -> crate::test_utils::EditorUiTestInputs {
        let mut view = editor_ui_view(app_settings, music_metadata);
        view.playing = options.playing;
        view.timeline_time_seconds = options.timeline_time_seconds;
        view.timeline_duration_seconds = options.timeline_duration_seconds;
        view.tap_times = tap_times.to_vec();
        view.timing_points = timing_points.to_vec();
        view.waveform_zoom = options.waveform_zoom;
        view.waveform_scroll = options.waveform_scroll;
        view.waveform_samples = waveform_samples.to_vec();
        view.waveform_sample_rate = 44_100;
        view.waveform_complete = true;
        view.trigger_objects = trigger_objects.to_vec();
        view.trigger_selected_index = options.trigger_selected_index;
        view
    }

    fn mono_wav_bytes(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_bytes = samples.len() as u32 * 2;
        let mut wav = Vec::with_capacity(44 + data_bytes as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn test_timeline_visible_duration() {
        // Default zoom 1.0 should give default window
        approx::assert_abs_diff_eq!(
            timeline_visible_duration(100.0, 1.0),
            DEFAULT_TIMELINE_WINDOW_SECONDS,
            epsilon = 0.001,
        );

        // Zoom in (2.0) should give smaller window (10.0s)
        approx::assert_abs_diff_eq!(timeline_visible_duration(100.0, 2.0), 10.0, epsilon = 0.001);

        // Zoom out (0.5) should give larger window (40.0s)
        approx::assert_abs_diff_eq!(timeline_visible_duration(100.0, 0.5), 40.0, epsilon = 0.001);

        // Should be clamped to duration
        approx::assert_abs_diff_eq!(timeline_visible_duration(10.0, 1.0), 10.0, epsilon = 0.001);

        // Should be clamped to max zoom (10.0), giving a 2.0s window with current limits
        approx::assert_abs_diff_eq!(
            timeline_visible_duration(100.0, 1000.0),
            2.0,
            epsilon = 0.001
        );
    }

    #[test]
    fn waveform_range_keeps_playhead_centered_at_track_boundaries() {
        for timeline_time_seconds in [0.0, 30.0, 60.0] {
            let range = centered_waveform_range(timeline_time_seconds, 20.0);
            let center = (range.start + range.end) * 0.5;
            assert!((center - f64::from(timeline_time_seconds)).abs() < 1e-9);
        }

        let start_range = centered_waveform_range(0.0, 20.0);
        let end_range = centered_waveform_range(60.0, 20.0);
        assert!(start_range.start < 0.0);
        assert!(end_range.end > 60.0);
    }

    #[test]
    fn test_zoom_scaling_logic() {
        let initial_zoom: f32 = 1.0;

        // Zoom in logic
        let zoom_in_1 = (initial_zoom * 1.25).clamp(0.1, 10.0);
        approx::assert_abs_diff_eq!(zoom_in_1, 1.25, epsilon = 0.001);

        let zoom_in_2 = (zoom_in_1 * 1.25).clamp(0.1, 10.0);
        approx::assert_abs_diff_eq!(zoom_in_2, 1.5625, epsilon = 0.001);

        // Zoom out logic
        let zoom_out_1 = (initial_zoom / 1.25).clamp(0.1, 10.0);
        approx::assert_abs_diff_eq!(zoom_out_1, 0.8, epsilon = 0.001);

        // Clamping logic
        let max_zoom = (9.0_f32 * 1.25).clamp(0.1, 10.0);
        approx::assert_abs_diff_eq!(max_zoom, 10.0, epsilon = 0.001);

        let min_zoom = (0.12_f32 * 0.8).clamp(0.1, 10.0);
        approx::assert_abs_diff_eq!(min_zoom, 0.1, epsilon = 0.001);
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
        approx::assert_abs_diff_eq!(waveform_x, tick_x_at_sample_time, epsilon = 0.001);
    }

    #[test]
    fn timeline_bar_nearest_tap_index_finds_nearest_visible_tap() {
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(200.0, 20.0));
        let tap_times = [1.0, 5.0, 5.2, 12.0];

        assert_eq!(
            timeline_bar_nearest_tap_index(&tap_times, 198.0, rect, 0.0, 10.0),
            Some(1)
        );
        assert_eq!(
            timeline_bar_nearest_tap_index(&tap_times, 304.0, rect, 0.0, 10.0),
            None
        );
    }

    #[test]
    fn timeline_bar_tap_click_seeks_to_tap_time() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = Vec::new();
        let tap_times = [1.0, 5.0, 5.2, 12.0];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            &tap_times,
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 0.0,
                timeline_duration_seconds: 20.0,
                waveform_zoom: 1.0,
                waveform_scroll: 0.0,
                playing: false,
                trigger_selected_index: None,
            },
        );
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(200.0, 20.0));
        let mut commands = Vec::new();

        assert!(timeline_bar_push_tap_click_commands(
            &TimelineBarInputs {
                app_settings: &app_settings,
                mode: EditorMode::Timing,
                last_mode: Some(EditorMode::Timing),
                playing: view.playing,
                timeline_time_seconds: view.timeline_time_seconds,
                tap_times: &view.tap_times,
                timing_points: &view.timing_points,
                waveform_zoom: view.waveform_zoom,
                waveform_scroll: view.waveform_scroll,
                trigger_objects: &view.trigger_objects,
                trigger_selected_index: view.trigger_selected_index,
                duration_seconds: view.timeline_duration_seconds,
            },
            198.0,
            rect,
            0.0,
            10.0,
            &mut commands,
        ));

        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AppCommand::Editor(EditorCommand::SetTimelineTime(value)) => {
                approx::assert_abs_diff_eq!(*value, 5.0, epsilon = 0.001)
            }
            other => panic!("expected timeline seek command, got {other:?}"),
        }
    }

    #[test]
    fn timeline_bar_tap_click_selects_tap_in_tapping_mode() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = Vec::new();
        let tap_times = [1.0, 5.0, 5.2, 12.0];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            &tap_times,
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 0.0,
                timeline_duration_seconds: 20.0,
                waveform_zoom: 1.0,
                waveform_scroll: 0.0,
                playing: false,
                trigger_selected_index: None,
            },
        );
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(200.0, 20.0));
        let mut commands = Vec::new();

        assert!(timeline_bar_push_tap_click_commands(
            &TimelineBarInputs {
                app_settings: &app_settings,
                mode: EditorMode::Tapping,
                last_mode: Some(EditorMode::Tapping),
                playing: view.playing,
                timeline_time_seconds: view.timeline_time_seconds,
                tap_times: &view.tap_times,
                timing_points: &view.timing_points,
                waveform_zoom: view.waveform_zoom,
                waveform_scroll: view.waveform_scroll,
                trigger_objects: &view.trigger_objects,
                trigger_selected_index: view.trigger_selected_index,
                duration_seconds: view.timeline_duration_seconds,
            },
            198.0,
            rect,
            0.0,
            10.0,
            &mut commands,
        ));

        assert_eq!(commands.len(), 2);
        match &commands[0] {
            AppCommand::Editor(EditorCommand::SetTimelineTime(value)) => {
                approx::assert_abs_diff_eq!(*value, 5.0, epsilon = 0.001)
            }
            other => panic!("expected timeline seek command, got {other:?}"),
        }
        assert_eq!(
            commands[1],
            AppCommand::Editor(EditorCommand::SetSelectedTap(Some(1))),
        );
    }

    #[test]
    fn timeline_bar_tap_is_current_uses_tight_epsilon() {
        assert!(timeline_bar_tap_is_current(5.0, 5.0));
        assert!(timeline_bar_tap_is_current(5.0, 5.00005));
        assert!(!timeline_bar_tap_is_current(5.0, 5.001));
    }

    #[test]
    fn timeline_time_formatter_uses_minutes_seconds_milliseconds() {
        assert_eq!(format_timeline_time_seconds(0.0), "00:00:000");
        assert_eq!(format_timeline_time_seconds(62.345), "01:02:345");
        assert_eq!(format_timeline_time_seconds(599.9996), "10:00:000");
    }

    #[test]
    fn timeline_time_parser_accepts_formatted_and_seconds_input() {
        let formatted = parse_timeline_time_seconds("01:02:345").expect("formatted time");
        assert!((formatted - 62.345).abs() <= 0.001);
        let seconds = parse_timeline_time_seconds("12.5").expect("seconds fallback");
        assert!((seconds - 12.5).abs() <= 0.001);
        assert!(parse_timeline_time_seconds("00:60:000").is_none());
        assert!(parse_timeline_time_seconds("00:00:1000").is_none());
    }

    #[test]
    fn show_timeline_bar_emits_recentering_scroll_command() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = vec![TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            &[],
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 30.0,
                timeline_duration_seconds: 60.0,
                waveform_zoom: 1.0,
                waveform_scroll: 0.0,
                playing: false,
                trigger_selected_index: None,
            },
        );

        let mut commands = Vec::<AppCommand>::new();
        let ctx = egui::Context::default();
        crate::run_test_ui(&ctx, egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                show_timeline_bar(
                    ui,
                    &TimelineBarInputs {
                        app_settings: &app_settings,
                        mode: EditorMode::Timing,
                        last_mode: Some(EditorMode::Timing),
                        playing: view.playing,
                        timeline_time_seconds: view.timeline_time_seconds,
                        tap_times: &view.tap_times,
                        timing_points: &view.timing_points,
                        waveform_zoom: view.waveform_zoom,
                        waveform_scroll: view.waveform_scroll,
                        trigger_objects: &view.trigger_objects,
                        trigger_selected_index: view.trigger_selected_index,
                        duration_seconds: 60.0,
                    },
                    &mut commands,
                );
            });
        });

        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AppCommand::Editor(EditorCommand::SetWaveformScroll(value)) => {
                approx::assert_abs_diff_eq!(*value, 20.0, epsilon = 0.001)
            }
            other => panic!("expected scroll recenter command, got {other:?}"),
        }
    }

    #[test]
    fn show_timeline_bar_with_centered_scroll_and_trigger_variants_has_no_commands() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = vec![TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];
        let trigger_objects = vec![
            LevelObject {
                trigger: Some(TimedTrigger {
                    time_seconds: 1.0,
                    duration_seconds: 0.0,
                    easing: EasingDirection::Linear,
                    target: TriggerTarget::Camera,
                    transform: false,
                    camera: Some(CameraTrigger {
                        mode: CameraMode::Follow,
                        ..CameraTrigger::default()
                    }),
                }),
                ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
            },
            LevelObject {
                trigger: Some(TimedTrigger {
                    time_seconds: 2.0,
                    duration_seconds: 0.0,
                    easing: EasingDirection::Linear,
                    target: TriggerTarget::Camera,
                    transform: false,
                    camera: Some(CameraTrigger {
                        mode: CameraMode::Static,
                        ..CameraTrigger::default()
                    }),
                }),
                ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
            },
            LevelObject {
                position: [1.0, 2.0, 3.0],
                trigger: Some(TimedTrigger {
                    time_seconds: 3.0,
                    duration_seconds: 0.0,
                    easing: EasingDirection::Linear,
                    target: TriggerTarget::Objects {
                        object_ids: vec!["target".to_string()],
                    },
                    transform: true,
                    camera: None,
                }),
                ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
            },
        ];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &trigger_objects,
            &[0.5, 2.5, 4.5],
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 5.0,
                timeline_duration_seconds: 20.0,
                waveform_zoom: 1.0,
                waveform_scroll: -5.0,
                playing: true,
                trigger_selected_index: Some(1),
            },
        );

        let mut commands = Vec::<AppCommand>::new();
        let ctx = egui::Context::default();
        crate::run_test_ui(&ctx, egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                show_timeline_bar(
                    ui,
                    &TimelineBarInputs {
                        app_settings: &app_settings,
                        mode: EditorMode::Timing,
                        last_mode: Some(EditorMode::Timing),
                        playing: view.playing,
                        timeline_time_seconds: view.timeline_time_seconds,
                        tap_times: &view.tap_times,
                        timing_points: &view.timing_points,
                        waveform_zoom: view.waveform_zoom,
                        waveform_scroll: view.waveform_scroll,
                        trigger_objects: &view.trigger_objects,
                        trigger_selected_index: view.trigger_selected_index,
                        duration_seconds: 20.0,
                    },
                    &mut commands,
                );
            });
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn show_waveform_panel_handles_empty_waveform_without_commands() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = vec![TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            &[],
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 10.0,
                timeline_duration_seconds: 32.0,
                waveform_zoom: 1.0,
                waveform_scroll: 0.0,
                playing: false,
                trigger_selected_index: None,
            },
        );

        let mut commands = Vec::<AppCommand>::new();
        let ctx = egui::Context::default();
        crate::run_test_ui(&ctx, egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                show_waveform_panel(
                    ui,
                    &WaveformPanelInputs {
                        timeline_time_seconds: view.timeline_time_seconds,
                        timeline_duration_seconds: view.timeline_duration_seconds,
                        timing_points: &view.timing_points,
                        waveform_zoom: view.waveform_zoom,
                    },
                    &mut euphorium::WaveformView::new(),
                    &mut commands,
                );
            });
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn show_waveform_panel_renders_waveform_and_beat_grid_without_commands() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = vec![
            TimingPoint {
                time_seconds: 0.0,
                bpm: 128.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            },
            TimingPoint {
                time_seconds: 8.0,
                bpm: 0.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            },
            TimingPoint {
                time_seconds: 12.0,
                bpm: 96.0,
                time_signature_numerator: 3,
                time_signature_denominator: 4,
            },
        ];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            &[],
            &[],
            TimelineViewOptions {
                timeline_time_seconds: 10.0,
                timeline_duration_seconds: 60.0,
                waveform_zoom: 1.0,
                waveform_scroll: 0.0,
                playing: false,
                trigger_selected_index: None,
            },
        );

        let mut commands = Vec::<AppCommand>::new();
        let mut waveform_view = euphorium::WaveformView::new();
        waveform_view
            .set_shared_bytes(mono_wav_bytes(8_000, &[16_384; 8_000]))
            .expect("test WAV should load");
        let ctx = egui::Context::default();
        crate::run_test_ui(&ctx, egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                show_waveform_panel(
                    ui,
                    &WaveformPanelInputs {
                        timeline_time_seconds: view.timeline_time_seconds,
                        timeline_duration_seconds: view.timeline_duration_seconds,
                        timing_points: &view.timing_points,
                        waveform_zoom: view.waveform_zoom,
                    },
                    &mut waveform_view,
                    &mut commands,
                );
            });
        });

        assert!(commands.is_empty());
    }
}
