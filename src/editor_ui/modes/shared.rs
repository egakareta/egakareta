/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::EditorUiViewModel;
use crate::types::{EditorMode, SpawnDirection};

pub(crate) fn show_mode_and_snap_controls(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) -> EditorMode {
    ui.horizontal(|ui| {
        ui.label("Mode:");
        let mut mode = view.mode;
        if ui
            .selectable_label(
                mode == EditorMode::Select,
                format!("{} Select", egui_phosphor::regular::CURSOR_CLICK),
            )
            .clicked()
        {
            mode = EditorMode::Select;
            commands.push(AppCommand::EditorSetMode(EditorMode::Select));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Move,
                format!("{} Move", egui_phosphor::regular::ARROWS_OUT),
            )
            .clicked()
        {
            mode = EditorMode::Move;
            commands.push(AppCommand::EditorSetMode(EditorMode::Move));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Scale,
                format!("{} Scale", egui_phosphor::regular::CORNERS_OUT),
            )
            .clicked()
        {
            mode = EditorMode::Scale;
            commands.push(AppCommand::EditorSetMode(EditorMode::Scale));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Rotate,
                format!("{} Rotate", egui_phosphor::regular::ARROWS_CLOCKWISE),
            )
            .clicked()
        {
            mode = EditorMode::Rotate;
            commands.push(AppCommand::EditorSetMode(EditorMode::Rotate));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Place,
                format!("{} Place", egui_phosphor::regular::CUBE),
            )
            .clicked()
        {
            mode = EditorMode::Place;
            commands.push(AppCommand::EditorSetMode(EditorMode::Place));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Trigger,
                format!("{} Trigger", egui_phosphor::regular::LIGHTNING),
            )
            .clicked()
        {
            mode = EditorMode::Trigger;
            commands.push(AppCommand::EditorSetMode(EditorMode::Trigger));
        }

        ui.separator();
        let mut snap = view.snap_to_grid;
        if ui
            .checkbox(
                &mut snap,
                format!("{} Snap to Grid", egui_phosphor::regular::GRID_FOUR),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapToGrid(snap));
        }

        ui.label("Step:");
        let mut snap_step = view.snap_step;
        if ui
            .add(
                egui::DragValue::new(&mut snap_step)
                    .speed(0.05)
                    .range(0.05..=100.0),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapStep(snap_step));
        }

        ui.separator();
        let mut snap_rotation = view.snap_rotation;
        if ui.checkbox(&mut snap_rotation, "Snap Rotation").changed() {
            commands.push(AppCommand::EditorSetSnapRotation(snap_rotation));
        }

        ui.label("Rot Step:");
        let mut snap_rotation_step = view.snap_rotation_step_degrees;
        if ui
            .add(
                egui::DragValue::new(&mut snap_rotation_step)
                    .speed(0.5)
                    .range(1.0..=180.0)
                    .suffix("°"),
            )
            .changed()
        {
            commands.push(AppCommand::EditorSetSnapRotationStep(snap_rotation_step));
        }

        // Ask egui for an immediate second pass when mode changes so panel sizing
        // responds in-frame instead of showing one-frame stale geometry.
        if mode != view.mode {
            ui.ctx().request_discard("editor mode changed");
        }

        mode
    })
    .inner
}

pub(crate) fn show_player_camera_status_row(ui: &mut egui::Ui, view: &EditorUiViewModel<'_>) {
    let position = view.timeline_preview_position;
    let direction = view.timeline_preview_direction;
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
        ui.label(format!(
            "Player Camera: ({:.1}, {:.1}, {:.1}) -> ({:.1}, {:.1}, {:.1})",
            view.camera_preview_position[0],
            view.camera_preview_position[1],
            view.camera_preview_position[2],
            view.camera_preview_target[0],
            view.camera_preview_target[1],
            view.camera_preview_target[2],
        ));
        ui.separator();
        ui.label(format!(
            "Editor Camera: ({:.1}, {:.1}, {:.1})",
            view.camera_position[0], view.camera_position[1], view.camera_position[2]
        ));
        ui.separator();
        ui.label(format!("FPS: {:.0}", view.fps));
    });
}

#[cfg(test)]
mod tests {
    use super::{show_mode_and_snap_controls, show_player_camera_status_row};
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
        preview_direction: SpawnDirection,
        mode: EditorMode,
    ) -> EditorUiViewModel<'a> {
        EditorUiViewModel {
            mode,
            last_mode: Some(mode),
            available_levels: &[],
            level_name: Some("Shared Test"),
            show_metadata: false,
            show_import: false,
            show_settings: false,
            settings_section: SettingsSection::Backends,
            keybind_capture_action: None,
            import_text: "",
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
            timeline_time_seconds: 1.0,
            timeline_duration_seconds: 16.0,
            tap_times: &[],
            timeline_preview_position: [2.0, 1.0, 0.0],
            timeline_preview_direction: preview_direction,
            timing_points,
            playback_speed: 1.0,
            timing_selected_index: None,
            waveform_zoom: 1.0,
            waveform_scroll: 0.0,
            waveform_samples: &[],
            waveform_sample_rate: 0,
            bpm_tap_result: None,
            triggers,
            trigger_selected_index: None,
            simulate_trigger_hitboxes: false,
            camera_position: [9.0, 8.0, 7.0],
            camera_preview_position: [3.0, 2.0, 1.0],
            camera_preview_target: [0.0, 0.0, 0.0],
            camera_rotation: 0.0,
            camera_pitch: 0.0,
            fps: 90.0,
            graphics_backend: "WGPU".to_string(),
            audio_backend: "Default".to_string(),
            perf_overlay_enabled: false,
            perf_overlay_lines: Vec::new(),
            perf_overlay_entries: Vec::new(),
            marquee_selection_rect_screen: None,
        }
    }

    #[test]
    fn show_mode_and_snap_controls_without_interaction_does_not_emit_commands() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = make_view(
            &app_settings,
            &music_metadata,
            &[],
            &[],
            SpawnDirection::Forward,
            EditorMode::Move,
        );
        let mut commands = Vec::<AppCommand>::new();

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_mode_and_snap_controls(ui, &view, &mut commands);
            });
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn show_player_camera_status_row_renders_for_forward_and_right_directions() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let forward_view = make_view(
            &app_settings,
            &music_metadata,
            &[],
            &[],
            SpawnDirection::Forward,
            EditorMode::Select,
        );
        let right_view = make_view(
            &app_settings,
            &music_metadata,
            &[],
            &[],
            SpawnDirection::Right,
            EditorMode::Select,
        );

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_player_camera_status_row(ui, &forward_view);
                show_player_camera_status_row(ui, &right_view);
            });
        });
    }
}
