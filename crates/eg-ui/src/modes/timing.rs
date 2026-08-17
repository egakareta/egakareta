/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::TimingPoint;

use crate::{
    UiCommandSink,
    components::{MAX_TIMELINE_DURATION_SECONDS, MIN_TIMELINE_DURATION_SECONDS},
};

/// Direct values needed only while rendering timing-mode controls.
pub(crate) struct TimingModePanelInputs<'a> {
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub timing_points: &'a [TimingPoint],
    pub playback_speed: f32,
    pub timing_selected_index: Option<usize>,
    pub bpm_tap_result: Option<f32>,
}

pub(crate) fn show_timing_mode_bottom_panel(
    ui: &mut egui::Ui,
    inputs: &TimingModePanelInputs<'_>,
    commands: &mut impl UiCommandSink,
) {
    let timeline_time_seconds = inputs.timeline_time_seconds;
    let timeline_duration_seconds = inputs.timeline_duration_seconds;
    let timing_points = inputs.timing_points;
    let playback_speed = inputs.playback_speed;
    let timing_selected_index = inputs.timing_selected_index;
    let bpm_tap_result = inputs.bpm_tap_result;
    ui.horizontal(|ui| {
        // Playback speed control
        ui.label(format!("{} Speed:", egui_phosphor::regular::GAUGE));
        let speed = playback_speed;
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
                        commands.set_playback_speed(s);
                    }
                }
            });

        ui.separator();

        let mut duration = timeline_duration_seconds;
        ui.label(format!("{} Duration:", egui_phosphor::regular::HOURGLASS));
        if ui
            .add(
                egui::DragValue::new(&mut duration)
                    .speed(0.1)
                    .range(MIN_TIMELINE_DURATION_SECONDS..=MAX_TIMELINE_DURATION_SECONDS)
                    .suffix("s"),
            )
            .changed()
        {
            commands.set_timeline_duration(duration);
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
                if let Some(selected_idx) = timing_selected_index {
                    if ui
                        .button(format!("{} Remove", egui_phosphor::regular::TRASH))
                        .clicked()
                    {
                        commands.remove_timing_point(selected_idx);
                    }
                    if ui
                        .button(format!(
                            "{} Use current time",
                            egui_phosphor::regular::CLOCK_AFTERNOON
                        ))
                        .clicked()
                    {
                        let time = timeline_time_seconds;
                        commands.set_timing_point_time(selected_idx, time);
                    }
                }
            });

            let selected_idx = timing_selected_index;
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
                            commands.set_timing_selected(Some(i));
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
            if let Some(idx) = timing_selected_index {
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
                            commands.set_timing_point_time(idx, time);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("BPM:");
                        let mut bpm = tp.bpm;
                        if ui
                            .add(egui::DragValue::new(&mut bpm).speed(0.1).range(1.0..=999.0))
                            .changed()
                        {
                            commands.set_timing_point_bpm(idx, bpm);
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
                            commands.set_timing_point_time_signature(
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
                commands.bpm_tap();
            }
            if let Some(bpm) = bpm_tap_result {
                ui.label(format!("Detected: {:.1} BPM", bpm));
                if let Some(idx) = timing_selected_index
                    && ui.button("Apply to selected").clicked()
                {
                    commands.set_timing_point_bpm(idx, bpm);
                }
            }
            if ui.button("Reset taps").clicked() {
                commands.bpm_tap_reset();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use eg_types::{AppSettings, LevelObject, MusicMetadata, TimingPoint};

    use super::{TimingModePanelInputs, show_timing_mode_bottom_panel};
    use crate::test_utils::{editor_ui_view, with_world_command_sink};

    fn make_view<'a>(
        app_settings: &'a AppSettings,
        music_metadata: &'a MusicMetadata,
        timing_points: &'a [TimingPoint],
        trigger_objects: &'a [LevelObject],
        timing_selected_index: Option<usize>,
        bpm_tap_result: Option<f32>,
    ) -> crate::test_utils::EditorUiTestInputs {
        let mut view = editor_ui_view(app_settings, music_metadata);
        view.timeline_time_seconds = 2.5;
        view.timeline_preview_position = [1.0, 2.0, 3.0];
        view.timing_points = timing_points.to_vec();
        view.timing_selected_index = timing_selected_index;
        view.bpm_tap_result = bpm_tap_result;
        view.trigger_objects = trigger_objects.to_vec();
        view
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
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    show_timing_mode_bottom_panel(
                        ui,
                        &TimingModePanelInputs {
                            timeline_time_seconds: view.timeline_time_seconds,
                            timeline_duration_seconds: view.timeline_duration_seconds,
                            timing_points: &view.timing_points,
                            playback_speed: view.playback_speed,
                            timing_selected_index: view.timing_selected_index,
                            bpm_tap_result: view.bpm_tap_result,
                        },
                        commands,
                    );
                });
            })
        });
    }

    #[test]
    fn show_timing_mode_bottom_panel_handles_empty_state() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = make_view(&app_settings, &music_metadata, &[], &[], None, None);
        let ctx = egui::Context::default();
        let (_world, _) = with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    show_timing_mode_bottom_panel(
                        ui,
                        &TimingModePanelInputs {
                            timeline_time_seconds: view.timeline_time_seconds,
                            timeline_duration_seconds: view.timeline_duration_seconds,
                            timing_points: &view.timing_points,
                            playback_speed: view.playback_speed,
                            timing_selected_index: view.timing_selected_index,
                            bpm_tap_result: view.bpm_tap_result,
                        },
                        commands,
                    );
                });
            })
        });
    }
}
