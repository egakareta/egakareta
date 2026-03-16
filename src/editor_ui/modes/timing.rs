use crate::commands::AppCommand;
use crate::editor_ui::components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS};
use crate::state::EditorUiViewModel;

pub(crate) fn show_timing_mode_bottom_panel(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    _duration_seconds: f32,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal(|ui| {
        // Playback speed control
        ui.label(format!("{} Speed:", egui_phosphor::regular::GAUGE));
        let speed = view.playback_speed;
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
                        commands.push(AppCommand::EditorSetPlaybackSpeed(s));
                    }
                }
            });

        ui.separator();

        let mut duration = view.timeline_duration_seconds;
        ui.label(format!(
            "{} Duration (s):",
            egui_phosphor::regular::HOURGLASS
        ));
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetTimelineDuration(duration));
        }
    });

    ui.separator();

    ui.horizontal(|ui| {
        // Timing points list
        ui.vertical(|ui| {
            ui.label(format!(
                "{} Timing Points:",
                egui_phosphor::regular::METRONOME
            ));
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{} Add at playhead",
                        egui_phosphor::regular::PLUS_CIRCLE
                    ))
                    .clicked()
                {
                    let time = view.timeline_time_seconds;
                    commands.push(crate::commands::AppCommand::EditorAddTimingPoint {
                        time_seconds: time,
                        bpm: 120.0,
                    });
                }
                if let Some(selected_idx) = view.timing_selected_index {
                    if ui
                        .button(format!("{} Remove", egui_phosphor::regular::TRASH))
                        .clicked()
                    {
                        commands.push(crate::commands::AppCommand::EditorRemoveTimingPoint(
                            selected_idx,
                        ));
                    }
                    if ui
                        .button(format!(
                            "{} Use current time",
                            egui_phosphor::regular::CLOCK_AFTERNOON
                        ))
                        .clicked()
                    {
                        let time = view.timeline_time_seconds;
                        commands.push(crate::commands::AppCommand::EditorSetTimingPointTime(
                            selected_idx,
                            time,
                        ));
                    }
                }
            });

            let timing_points = view.timing_points;
            let selected_idx = view.timing_selected_index;
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
                            commands.push(AppCommand::EditorSetTimingSelected(Some(i)));
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
            let timing_points = view.timing_points;
            if let Some(idx) = view.timing_selected_index {
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
                            commands.push(AppCommand::EditorSetTimingPointTime(idx, time));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("BPM:");
                        let mut bpm = tp.bpm;
                        if ui
                            .add(egui::DragValue::new(&mut bpm).speed(0.1).range(1.0..=999.0))
                            .changed()
                        {
                            commands.push(AppCommand::EditorSetTimingPointBpm(idx, bpm));
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
                            commands.push(AppCommand::EditorSetTimingPointTimeSignature(
                                idx,
                                num.max(1) as u32,
                                den.max(1) as u32,
                            ));
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
                commands.push(AppCommand::EditorBpmTap);
            }
            if let Some(bpm) = view.bpm_tap_result {
                ui.label(format!("Detected: {:.1} BPM", bpm));
                if let Some(idx) = view.timing_selected_index {
                    if ui.button("Apply to selected").clicked() {
                        commands.push(AppCommand::EditorSetTimingPointBpm(idx, bpm));
                    }
                }
            }
            if ui.button("Reset taps").clicked() {
                commands.push(AppCommand::EditorBpmTapReset);
            }
        });
    });
}
