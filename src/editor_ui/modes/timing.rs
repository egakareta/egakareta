/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
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

#[cfg(test)]
mod tests {
    use super::show_timing_mode_bottom_panel;
    use crate::commands::AppCommand;
    use crate::state::EditorUiViewModel;
    use crate::types::{
        AppSettings, EditorMode, MusicMetadata, SettingsSection, SpawnDirection, TimedTrigger,
        TimingPoint,
    };

    fn make_view<'a>(
        app_settings: &'a AppSettings,
        music_metadata: &'a MusicMetadata,
        timing_points: &'a [TimingPoint],
        triggers: &'a [TimedTrigger],
        timing_selected_index: Option<usize>,
        bpm_tap_result: Option<f32>,
    ) -> EditorUiViewModel<'a> {
        EditorUiViewModel {
            mode: EditorMode::Timing,
            last_mode: Some(EditorMode::Timing),
            available_levels: &[],
            level_name: Some("Test Level"),
            show_metadata: false,
            show_settings: false,
            settings_section: SettingsSection::Backends,
            keybind_capture_action: None,
            music_metadata,
            app_settings,
            configured_graphics_backend: "Auto",
            configured_audio_backend: "Default",
            graphics_backend_options: &[],
            audio_backend_options: &[],
            settings_restart_required: false,
            snap_to_grid: true,
            snap_step: 1.0,
            snap_rotation: true,
            snap_rotation_step_degrees: 15.0,
            selected_block_id: "core/stone",
            selected_block: None,
            playing: false,
            timeline_time_seconds: 2.5,
            timeline_duration_seconds: 16.0,
            tap_times: &[],
            timeline_preview_position: [1.0, 2.0, 3.0],
            timeline_preview_direction: SpawnDirection::Forward,
            timing_points,
            playback_speed: 1.0,
            timing_selected_index,
            waveform_zoom: 1.0,
            waveform_scroll: 0.0,
            waveform_samples: &[],
            waveform_sample_rate: 0,
            bpm_tap_result,
            triggers,
            trigger_selected_index: None,
            simulate_trigger_hitboxes: false,
            camera_position: [0.0, 0.0, 0.0],
            camera_preview_position: [0.0, 1.0, 2.0],
            camera_preview_target: [0.0, 0.0, 0.0],
            camera_rotation: 0.0,
            camera_pitch: 0.0,
            fps: 120.0,
            graphics_backend: "WGPU".to_string(),
            audio_backend: "Default".to_string(),
            perf_overlay_enabled: false,
            perf_overlay_lines: Vec::new(),
            perf_overlay_entries: Vec::new(),
            marquee_selection_rect_screen: None,
        }
    }

    #[test]
    fn show_timing_mode_bottom_panel_handles_selected_timing_point_state() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let timing_points = vec![TimingPoint {
            time_seconds: 1.0,
            bpm: 128.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        }];
        let view = make_view(
            &app_settings,
            &music_metadata,
            &timing_points,
            &[],
            Some(0),
            Some(127.5),
        );
        let mut commands = Vec::<AppCommand>::new();

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_timing_mode_bottom_panel(ui, &view, 16.0, &mut commands);
            });
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn show_timing_mode_bottom_panel_handles_empty_state() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = make_view(&app_settings, &music_metadata, &[], &[], None, None);
        let mut commands = Vec::<AppCommand>::new();

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_timing_mode_bottom_panel(ui, &view, 8.0, &mut commands);
            });
        });

        assert!(commands.is_empty());
    }
}
