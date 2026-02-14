use crate::block_repository::all_placeable_blocks;
use crate::types::{EditorMode, SpawnDirection};
use crate::State;

const MIN_TIMELINE_DURATION_SECONDS: f32 = 0.1;
const MAX_TIMELINE_DURATION_SECONDS: f32 = 600.0;
const MENU_WORDMARK_PNG: &[u8] = include_bytes!("../assets/wordmark.png");

pub fn load_menu_wordmark_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let decoded = image::load_from_memory(MENU_WORDMARK_PNG).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    Some(ctx.load_texture("menu_wordmark", color_image, egui::TextureOptions::LINEAR))
}

pub fn show_menu_wordmark_ui(ctx: &egui::Context, state: &State, wordmark: &egui::TextureHandle) {
    if !state.is_menu() {
        return;
    }

    let texture_size = wordmark.size_vec2();
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return;
    }

    let max_width = (ctx.screen_rect().width() * 0.68).max(240.0);
    let scale = (max_width / texture_size.x).min(1.0);
    let display_size = texture_size * scale;

    egui::Area::new("menu_wordmark_area".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.add(egui::Image::new((wordmark.id(), display_size)));
        });
}

fn timeline_metrics(duration_seconds: f32) -> f32 {
    duration_seconds.max(MIN_TIMELINE_DURATION_SECONDS)
}

pub fn show_editor_ui(ctx: &egui::Context, state: &mut State) {
    if !state.is_editor() {
        return;
    }

    egui::TopBottomPanel::top("editor_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Top-level tabs: Compose / Timing
            let mode = state.editor_mode();
            let is_compose = mode == EditorMode::Select || mode == EditorMode::Place;
            if ui.selectable_label(is_compose, "Compose").clicked() && !is_compose {
                state.set_editor_mode(EditorMode::Place);
            }
            if ui
                .selectable_label(mode == EditorMode::Timing, "Timing")
                .clicked()
                && mode != EditorMode::Timing
            {
                state.set_editor_mode(EditorMode::Timing);
                state.load_waveform_for_current_audio();
            }

            ui.separator();

            ui.label("Level:");

            let levels = state.available_levels().to_vec();
            let selected = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());

            egui::ComboBox::from_id_salt("level_select")
                .selected_text(&selected)
                .show_ui(ui, |ui| {
                    for level in levels {
                        if ui.selectable_label(selected == level, &level).clicked() {
                            state.load_builtin_level_into_editor(&level);
                        }
                    }
                });

            ui.separator();

            ui.label("Name:");
            let mut name = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());
            if ui.text_edit_singleline(&mut name).changed() {
                state.set_editor_level_name(name);
            }

            ui.separator();

            if ui.button("Export .ldz").clicked() {
                state.trigger_level_export();
            }

            if ui.button("Import .ldz/JSON").clicked() {
                state.set_editor_show_import(true);
            }

            if ui.button("Metadata").clicked() {
                state.set_editor_show_metadata(true);
            }
        });
    });

    if state.editor_show_metadata() {
        egui::Window::new("Level Metadata").show(ctx, |ui| {
            ui.label("Level Name:");
            let mut name = state
                .editor_level_name()
                .unwrap_or_else(|| "Untitled".to_string());
            if ui.text_edit_singleline(&mut name).changed() {
                state.set_editor_level_name(name);
            }

            ui.separator();
            ui.heading("Music");

            let mut music = state.editor_music_metadata().clone();
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Source:");
                if ui.text_edit_singleline(&mut music.source).changed() {
                    changed = true;
                }
                if ui.button("Import External Audio").clicked() {
                    state.trigger_audio_import();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Title:");
                let mut title = music.title.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut title).changed() {
                    music.title = Some(title);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Author:");
                let mut author = music.author.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut author).changed() {
                    music.author = Some(author);
                    changed = true;
                }
            });

            if changed {
                state.set_editor_music_metadata(music);
            }

            if ui.button("Close").clicked() {
                state.set_editor_show_metadata(false);
            }
        });
    }

    if state.editor_show_import() {
        egui::Window::new("Import Level").show(ctx, |ui| {
            ui.label("Paste level JSON or Base64 LDZ below:");
            let mut text = state.editor_import_text().to_string();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut text)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                )
                .changed()
            {
                state.set_editor_import_text(text);
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    state.complete_import();
                }
                if ui.button("Cancel").clicked() {
                    state.set_editor_show_import(false);
                }
            });
        });
    }

    egui::TopBottomPanel::bottom("block_selection_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let duration_seconds = timeline_metrics(state.editor_timeline_duration_seconds());

                if state.editor_mode() == EditorMode::Timing {
                    show_timing_mode_bottom_panel(ui, state, duration_seconds);
                } else {
                    show_compose_mode_bottom_panel(ui, state, duration_seconds);
                }

                // Shared timeline bar with beat lines
                show_timeline_bar(ui, state, duration_seconds);
            });
        });

    // Waveform visualization central panel (Timing mode only)
    if state.editor_mode() == EditorMode::Timing {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style()).fill(egui::Color32::from_rgb(15, 20, 28)),
            )
            .show(ctx, |ui| {
                show_waveform_panel(ui, state);
            });
    }

    if state.editor_perf_overlay_enabled() {
        egui::Area::new("editor_perf_overlay".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_black_alpha(210))
                    .show(ui, |ui| {
                        for line in state.editor_perf_overlay_lines() {
                            ui.monospace(line);
                        }
                    });
            });
    }
}

fn show_compose_mode_bottom_panel(ui: &mut egui::Ui, state: &mut State, duration_seconds: f32) {
    ui.horizontal(|ui| {
        ui.label("Mode:");
        let mode = state.editor_mode();
        if ui
            .selectable_label(mode == EditorMode::Select, "Select")
            .clicked()
        {
            state.set_editor_mode(EditorMode::Select);
        }
        if ui
            .selectable_label(mode == EditorMode::Place, "Place")
            .clicked()
        {
            state.set_editor_mode(EditorMode::Place);
        }

        ui.separator();
        let mut snap = state.editor_snap_to_grid();
        if ui.checkbox(&mut snap, "Snap to Grid").changed() {
            state.set_editor_snap_to_grid(snap);
        }

        ui.label("Step:");
        let mut snap_step = state.editor_snap_step();
        if ui
            .add(
                egui::DragValue::new(&mut snap_step)
                    .speed(0.05)
                    .range(0.05..=100.0),
            )
            .changed()
        {
            state.set_editor_snap_step(snap_step);
        }
    });

    match state.editor_mode() {
        EditorMode::Place => {
            ui.horizontal(|ui| {
                ui.label("Block:");

                let current = state.editor_selected_block_id().to_string();
                for block in all_placeable_blocks() {
                    if !block.placeable {
                        continue;
                    }
                    if ui
                        .selectable_label(current == block.id, &block.display_name)
                        .clicked()
                    {
                        state.set_editor_block_id(block.id.clone());
                    }
                }
            });
        }
        EditorMode::Select => {
            ui.label("Tip: Shift+Click to select multiple blocks.");
            if let Some(mut selected) = state.editor_selected_block() {
                ui.horizontal_wrapped(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Move:");
                        let mut changed = false;
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[0]).prefix("X "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[1]).prefix("Y "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.position[2]).prefix("Z "))
                            .changed();
                        if changed {
                            state.set_editor_selected_block_position(selected.position);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Resize:");
                        let mut changed = false;
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[0]).prefix("W "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[1]).prefix("D "))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut selected.size[2]).prefix("H "))
                            .changed();
                        if changed {
                            state.set_editor_selected_block_size(selected.size);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Angle:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut selected.rotation_degrees)
                                    .speed(0.5)
                                    .suffix("°"),
                            )
                            .changed()
                        {
                            state.set_editor_selected_block_rotation(selected.rotation_degrees);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Round:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut selected.roundness)
                                    .speed(0.01)
                                    .range(0.0..=10.0),
                            )
                            .changed()
                        {
                            state.set_editor_selected_block_roundness(selected.roundness);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Color:");
                    for block in all_placeable_blocks() {
                        if !block.placeable {
                            continue;
                        }
                        if ui
                            .selectable_label(selected.block_id == block.id, &block.display_name)
                            .clicked()
                        {
                            state.set_editor_selected_block_id(block.id.clone());
                        }
                    }
                });
            } else {
                ui.label("Select mode: click a block to edit it.");
            }
        }
        EditorMode::Timing => {} // handled separately
    }

    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Timeline:");

        let mut time_seconds = state.editor_timeline_time_seconds();
        let slider = egui::Slider::new(&mut time_seconds, 0.0..=duration_seconds)
            .text("Time")
            .show_value(true);
        if ui.add(slider).changed() {
            state.set_editor_timeline_time_seconds(time_seconds);
        }

        let mut duration = state.editor_timeline_duration_seconds();
        ui.label("Duration (s):");
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS),
            )
            .changed()
        {
            state.set_editor_timeline_duration_seconds(duration);
        }

        if ui.button("Add tap").clicked() {
            state.editor_add_tap();
        }
        if ui.button("Remove tap").clicked() {
            state.editor_remove_tap();
        }
        if ui.button("Clear taps").clicked() {
            state.editor_clear_taps();
        }
    });

    let (position, direction) = state.editor_timeline_preview();
    let direction_label = match direction {
        SpawnDirection::Forward => "Forward",
        SpawnDirection::Right => "Right",
    };
    ui.horizontal(|ui| {
        ui.label(format!(
            "Player: ({:.1}, {:.1}, {:.1}) | {}",
            position[0], position[1], position[2], direction_label
        ));
        ui.separator();
        ui.label(format!("FPS: {:.0}", state.editor_fps()));
    });
}

fn show_timing_mode_bottom_panel(ui: &mut egui::Ui, state: &mut State, duration_seconds: f32) {
    ui.horizontal(|ui| {
        // Playback speed control
        ui.label("Speed:");
        let speed = state.editor_playback_speed();
        let speeds = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
        egui::ComboBox::from_id_salt("playback_speed")
            .selected_text(format!("{:.2}x", speed))
            .width(70.0)
            .show_ui(ui, |ui| {
                for &s in &speeds {
                    if ui
                        .selectable_label((speed - s).abs() < 0.01, format!("{:.2}x", s))
                        .clicked()
                    {
                        state.set_editor_playback_speed(s);
                    }
                }
            });

        ui.separator();

        // Timeline controls (shared with compose)
        ui.label("Timeline:");
        let mut time_seconds = state.editor_timeline_time_seconds();
        let slider = egui::Slider::new(&mut time_seconds, 0.0..=duration_seconds)
            .text("Time")
            .show_value(true);
        if ui.add(slider).changed() {
            state.set_editor_timeline_time_seconds(time_seconds);
        }

        let mut duration = state.editor_timeline_duration_seconds();
        ui.label("Duration (s):");
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS),
            )
            .changed()
        {
            state.set_editor_timeline_duration_seconds(duration);
        }
    });

    ui.separator();

    ui.horizontal(|ui| {
        // Timing points list
        ui.vertical(|ui| {
            ui.label("Timing Points:");
            ui.horizontal(|ui| {
                if ui.button("Add at playhead").clicked() {
                    let time = state.editor_timeline_time_seconds();
                    state.editor_add_timing_point(time, 120.0);
                }
                if let Some(selected_idx) = state.editor_timing_selected_index() {
                    if ui.button("Remove").clicked() {
                        state.editor_remove_timing_point(selected_idx);
                    }
                    if ui.button("Use current time").clicked() {
                        let time = state.editor_timeline_time_seconds();
                        state.editor_update_timing_point_time(selected_idx, time);
                    }
                }
            });

            let timing_points = state.editor_timing_points().to_vec();
            let selected_idx = state.editor_timing_selected_index();
            egui::ScrollArea::vertical()
                .max_height(80.0)
                .show(ui, |ui| {
                    for (i, tp) in timing_points.iter().enumerate() {
                        let label = format!(
                            "#{}: {:.2}s | {:.1} BPM | {}/{}",
                            i + 1,
                            tp.time_seconds,
                            tp.bpm,
                            tp.time_signature_numerator,
                            tp.time_signature_denominator,
                        );
                        if ui
                            .selectable_label(selected_idx == Some(i), label)
                            .clicked()
                        {
                            state.set_editor_timing_selected_index(Some(i));
                        }
                    }
                    if timing_points.is_empty() {
                        ui.label("No timing points. Add one to get started.");
                    }
                });
        });

        ui.separator();

        // Selected timing point editor
        ui.vertical(|ui| {
            let timing_points = state.editor_timing_points().to_vec();
            if let Some(idx) = state.editor_timing_selected_index() {
                if let Some(tp) = timing_points.get(idx) {
                    ui.label(format!("Editing Point #{}", idx + 1));

                    ui.horizontal(|ui| {
                        ui.label("Offset (s):");
                        let mut time = tp.time_seconds;
                        if ui
                            .add(
                                egui::DragValue::new(&mut time)
                                    .speed(0.01)
                                    .range(0.0..=f32::MAX),
                            )
                            .changed()
                        {
                            state.editor_update_timing_point_time(idx, time);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("BPM:");
                        let mut bpm = tp.bpm;
                        if ui
                            .add(egui::DragValue::new(&mut bpm).speed(0.1).range(1.0..=999.0))
                            .changed()
                        {
                            state.editor_update_timing_point_bpm(idx, bpm);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Time Sig:");
                        let mut num = tp.time_signature_numerator as i32;
                        let mut den = tp.time_signature_denominator as i32;
                        let n_changed = ui
                            .add(egui::DragValue::new(&mut num).range(1..=32))
                            .changed();
                        ui.label("/");
                        let d_changed = ui
                            .add(egui::DragValue::new(&mut den).range(1..=32))
                            .changed();
                        if n_changed || d_changed {
                            state.editor_update_timing_point_time_signature(
                                idx,
                                num.max(1) as u32,
                                den.max(1) as u32,
                            );
                        }
                    });
                }
            } else {
                ui.label("Select a timing point to edit.");
            }
        });

        ui.separator();

        // BPM Tap helper
        ui.vertical(|ui| {
            ui.label("Tap for BPM:");
            if ui.button("Tap (click repeatedly)").clicked() {
                state.editor_bpm_tap();
            }
            if let Some(bpm) = state.editor_bpm_tap_result() {
                ui.label(format!("Detected: {:.1} BPM", bpm));
                if let Some(idx) = state.editor_timing_selected_index() {
                    if ui.button("Apply to selected").clicked() {
                        state.editor_update_timing_point_bpm(idx, bpm);
                    }
                }
            }
            if ui.button("Reset taps").clicked() {
                state.editor_bpm_tap_reset();
            }
        });
    });
}

fn show_timeline_bar(ui: &mut egui::Ui, state: &mut State, duration_seconds: f32) {
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
            state.set_editor_timeline_time_seconds(time_seconds);
        }
    }
}

fn show_waveform_panel(ui: &mut egui::Ui, state: &mut State) {
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
            state.set_editor_timeline_time_seconds(time_seconds);
        }
    }

    // Scroll with mouse wheel to zoom, drag to pan (via scroll offset)
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    if scroll_delta.y.abs() > 0.0 {
        let zoom_factor = 1.0 + scroll_delta.y * 0.002;
        let new_zoom = (zoom * zoom_factor).clamp(0.1, 100.0);
        state.set_editor_waveform_zoom(new_zoom);
    }
    if response.dragged_by(egui::PointerButton::Middle)
        || response.dragged_by(egui::PointerButton::Secondary)
    {
        let drag_delta = response.drag_delta();
        let time_per_pixel = (view_end - view_start) / rect.width();
        let new_scroll = (scroll - drag_delta.x * time_per_pixel).max(0.0);
        state.set_editor_waveform_scroll(new_scroll);
    }
}
