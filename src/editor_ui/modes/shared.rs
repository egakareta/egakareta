/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::commands::AppCommand;
use crate::state::editor_command::EditorCommand;
use crate::state::EditorUiViewModel;
use crate::types::{EditorMode, SpawnDirection};

const PROPERTY_POPUP_MARGIN: f32 = 12.0;
const PROPERTY_POPUP_MIN_WIDTH: f32 = 220.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorPropertyPopup {
    id: egui::Id,
    anchor: egui::Align2,
    offset: egui::Vec2,
    order: egui::Order,
    min_width: f32,
}

impl EditorPropertyPopup {
    pub(crate) fn above_bottom_bar(
        id_source: impl std::hash::Hash,
        bottom_bar_height: f32,
    ) -> Self {
        Self {
            id: egui::Id::new(id_source),
            anchor: egui::Align2::LEFT_BOTTOM,
            offset: egui::Vec2::new(
                PROPERTY_POPUP_MARGIN,
                -PROPERTY_POPUP_MARGIN - bottom_bar_height,
            ),
            order: egui::Order::Foreground,
            min_width: PROPERTY_POPUP_MIN_WIDTH,
        }
    }
}

pub(crate) fn show_editor_property_popup(
    ctx: &egui::Context,
    popup: EditorPropertyPopup,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Area::new(popup.id)
        .anchor(popup.anchor, popup.offset)
        .order(popup.order)
        .show(ctx, |ui| {
            egui::Frame::popup(&ctx.global_style()).show(ui, |ui| {
                ui.set_min_width(popup.min_width);
                add_contents(ui);
            });
        });
}

pub(crate) fn show_mode_and_snap_controls(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal(|ui| {
        ui.label("Mode:");
        let mode = view.mode;
        if ui
            .selectable_label(
                mode == EditorMode::Select,
                format!("{} Select", egui_phosphor::regular::CURSOR_CLICK),
            )
            .on_hover_text(format!(
                "Select{}",
                view.app_settings.hotkey_hint("mode_select")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Select,
            )));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Move,
                format!("{} Move", egui_phosphor::regular::ARROWS_OUT),
            )
            .on_hover_text(format!(
                "Move{}",
                view.app_settings.hotkey_hint("mode_move")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetMode(EditorMode::Move)));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Scale,
                format!("{} Scale", egui_phosphor::regular::CORNERS_OUT),
            )
            .on_hover_text(format!(
                "Scale{}",
                view.app_settings.hotkey_hint("mode_scale")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Scale,
            )));
        }
        if ui
            .selectable_label(
                mode == EditorMode::Rotate,
                format!("{} Rotate", egui_phosphor::regular::ARROWS_CLOCKWISE),
            )
            .on_hover_text(format!(
                "Rotate{}",
                view.app_settings.hotkey_hint("mode_rotate")
            ))
            .clicked()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Rotate,
            )));
        }
        ui.separator();
        let mut snap = view.snap_to_grid;
        if ui
            .checkbox(
                &mut snap,
                format!("{} Grid Snap", egui_phosphor::regular::GRID_FOUR),
            )
            .changed()
        {
            commands.push(AppCommand::Editor(EditorCommand::SetSnapToGrid(snap)));
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
            commands.push(AppCommand::Editor(EditorCommand::SetSnapStep(snap_step)));
        }

        ui.separator();
        let mut snap_rotation = view.snap_rotation;
        if ui.checkbox(&mut snap_rotation, "Rotation Snap").changed() {
            commands.push(AppCommand::Editor(EditorCommand::SetSnapRotation(
                snap_rotation,
            )));
        }

        ui.label("Step:");
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
            commands.push(AppCommand::Editor(EditorCommand::SetSnapRotationStep(
                snap_rotation_step,
            )));
        }
    });
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
    });
}

#[cfg(test)]
mod tests {
    use super::{
        show_editor_property_popup, show_mode_and_snap_controls, show_player_camera_status_row,
        EditorPropertyPopup, PROPERTY_POPUP_MARGIN,
    };
    use crate::commands::AppCommand;
    use crate::state::editor_command::EditorCommand;
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
            show_place_window: false,
            show_settings: false,
            settings_section: SettingsSection::Backends,
            keybind_capture_action: None,
            music_metadata,
            creator_metadata: crate::types::LevelCreatorMetadata::default(),
            sky_color: crate::types::default_sky_color(),
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
            place_preview_position: [0.0, 0.0, 0.0],
            place_preview_size: [1.0, 1.0, 1.0],
            recent_block_ids: &[],
            selected_block: None,
            selected_block_count: 0,
            transform_trigger_capture_active: false,
            clipboard_block_count: 0,
            can_undo: false,
            can_redo: false,
            playing: false,
            timeline_time_seconds: 1.0,
            timeline_duration_seconds: 16.0,
            tap_times: &[],
            selected_tap: None,
            timeline_preview_position: [2.0, 1.0, 0.0],
            timeline_preview_direction: preview_direction,
            timing_points,
            playback_speed: 1.0,
            timing_selected_index: None,
            waveform_zoom: 1.0,
            waveform_scroll: 0.0,
            waveform_samples: &[],
            waveform_sample_rate: 0,
            waveform_window_size: crate::audio_service::WAVEFORM_WINDOW,
            waveform_loading: false,
            waveform_complete: false,
            bpm_tap_result: None,
            triggers: triggers.to_vec(),
            trigger_selected_index: None,
            simulate_trigger_hitboxes: false,
            camera_position: [9.0, 8.0, 7.0],
            camera_preview_position: [3.0, 2.0, 1.0],
            camera_preview_target: [0.0, 0.0, 0.0],
            camera_rotation: 0.0,
            camera_pitch: 0.0,
            fps: 90.0,
            marquee_selection_rect_screen: None,
            object_count: 0,
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
        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                show_mode_and_snap_controls(ui, &view, &mut commands);
            });
        });

        assert!(commands.is_empty());
    }

    #[test]
    fn property_popup_runs_content_closure_with_bottom_bar_offset() {
        let popup = EditorPropertyPopup::above_bottom_bar("test_property_popup", 48.0);
        assert_eq!(popup.anchor, egui::Align2::LEFT_BOTTOM);
        assert_eq!(popup.offset.x, PROPERTY_POPUP_MARGIN);
        assert_eq!(popup.offset.y, -PROPERTY_POPUP_MARGIN - 48.0);

        let ctx = egui::Context::default();
        let mut rendered = false;
        let _ = ctx.run_ui(egui::RawInput::default(), |_root_ui| {
            show_editor_property_popup(&ctx, popup, |ui| {
                rendered = true;
                ui.label("Property popup");
            });
        });

        assert!(rendered);
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
        let _ = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                show_player_camera_status_row(ui, &forward_view);
                show_player_camera_status_row(ui, &right_view);
            });
        });
    }
}
