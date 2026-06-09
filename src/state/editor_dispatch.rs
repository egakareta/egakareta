/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::runtime::EditorDirtyFlags;
use super::State;
use crate::state::editor_command::EditorCommand;
use crate::types::{AppPhase, EditorMode};

impl State {
    /// Route `EditorCommand` variants to their handler methods.
    ///
    /// This is the editor-owned dispatch path. Prefer adding new editor commands
    /// to `EditorCommand` instead of `AppCommand` so editor features stay within
    /// the `state/` module.
    pub(crate) fn dispatch_editor(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::SetMode(mode) => {
                let old_mode = self.editor_effective_mode_for_playback();
                if self.editor.timeline.playback.playing {
                    self.set_editor_playback_effective_mode(mode);
                } else {
                    self.set_editor_mode(mode);
                }
                if mode == EditorMode::Tapping && old_mode != EditorMode::Tapping {
                    self.refresh_editor_tapping_preview_on_mode_entry();
                }
                if mode == EditorMode::Timing && old_mode != EditorMode::Timing {
                    self.load_waveform_for_current_audio();
                }
            }
            EditorCommand::SetBlockId(id) => self.set_editor_block_id(id),
            EditorCommand::SelectRecentBlock(index) => self.select_recent_block(index),
            EditorCommand::PickSelectedBlock => {
                self.editor_pick_selected_block_for_place();
            }
            EditorCommand::PickBlockAt { x, y } => {
                self.editor_pick_block_at_screen(x, y);
            }

            EditorCommand::SetSnapToGrid(snap) => self.set_editor_snap_to_grid(snap),
            EditorCommand::SetSnapStep(step) => self.set_editor_snap_step(step),
            EditorCommand::SetSnapRotation(snap) => self.set_editor_snap_rotation(snap),
            EditorCommand::SetSnapRotationStep(step) => {
                self.set_editor_snap_rotation_step(step);
            }

            EditorCommand::RemoveBlock => self.editor_remove_block(),
            EditorCommand::DuplicateBlock => self.editor_duplicate_selected_block_in_place(),
            EditorCommand::CopyBlock => self.editor_copy_block(),
            EditorCommand::PasteBlock => self.editor_paste_block(),
            EditorCommand::UpdateSelectedBlock(obj) => {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
                self.set_editor_selected_block_id(obj.block_id);
                self.set_editor_selected_block_rotation(obj.rotation_degrees);
                self.set_editor_selected_block_color_tint(obj.color_tint);
            }

            EditorCommand::NudgeSelected { dx, dy } => {
                self.editor_nudge_selected_blocks(dx, dy);
            }
            EditorCommand::SnapSelectionToGrid => {
                self.editor_snap_selection_to_grid();
            }
            EditorCommand::FocusCameraTarget => {
                self.editor_focus_camera_target();
            }
            EditorCommand::BeginTransformTriggerCapture => {
                self.begin_editor_transform_trigger_capture();
            }
            EditorCommand::CommitTransformTriggerCapture => {
                self.commit_editor_transform_trigger_capture();
            }
            EditorCommand::CancelTransformTriggerCapture => {
                self.cancel_editor_transform_trigger_capture();
            }

            EditorCommand::ToggleTimelinePlayback => self.toggle_editor_timeline_playback(),
            EditorCommand::ShiftTimeline(delta) => self.editor_shift_timeline_time(delta),
            EditorCommand::SetTimelineTime(time) => self.set_editor_timeline_time_seconds(time),
            EditorCommand::SetTimelineDuration(duration) => {
                self.set_editor_timeline_duration_seconds(duration);
            }
            EditorCommand::AddTap => self.editor_add_tap(),
            EditorCommand::RemoveTap => self.editor_remove_tap(),
            EditorCommand::RemoveTapAt(time) => self.editor_remove_tap_at(time),
            EditorCommand::SetSelectedTap(index) => {
                self.editor_set_selected_tap_index(index);
            }
            EditorCommand::SetSelectedTapTime(time) => {
                self.editor_set_selected_tap_time(time);
            }
            EditorCommand::ClearTaps => self.editor_clear_taps(),
            EditorCommand::SetPlaybackSpeed(speed) => self.set_editor_playback_speed(speed),
            EditorCommand::SetWaveformZoom(zoom) => self.set_editor_waveform_zoom(zoom),
            EditorCommand::SetWaveformScroll(scroll) => self.set_editor_waveform_scroll(scroll),
            EditorCommand::Playtest => self.editor_playtest(),

            EditorCommand::AddTimingPoint { time_seconds, bpm } => {
                self.editor_add_timing_point(time_seconds, bpm);
            }
            EditorCommand::RemoveTimingPoint(idx) => self.editor_remove_timing_point(idx),
            EditorCommand::SetTimingPointTime(idx, time) => {
                self.editor_update_timing_point_time(idx, time);
            }
            EditorCommand::SetTimingPointBpm(idx, bpm) => {
                self.editor_update_timing_point_bpm(idx, bpm);
            }
            EditorCommand::SetTimingPointTimeSignature(idx, num, den) => {
                self.editor_update_timing_point_time_signature(idx, num, den);
            }
            EditorCommand::SetTimingSelected(selected) => {
                self.set_editor_timing_selected_index(selected);
            }

            EditorCommand::BpmTap => self.editor_bpm_tap(),
            EditorCommand::BpmTapReset => self.editor_bpm_tap_reset(),

            EditorCommand::SetSpawnHere => {
                self.force_editor_cursor_from_pointer();
                self.editor_set_spawn_here();
            }
            EditorCommand::RotateSpawnDirection => self.editor_rotate_spawn_direction(),
            EditorCommand::RotatePlacementPreview => self.editor_rotate_placement_preview(),

            EditorCommand::Undo => self.editor_undo(),
            EditorCommand::Redo => self.editor_redo(),

            EditorCommand::AdjustZoom(delta) => self.adjust_editor_zoom(delta),
            EditorCommand::SetCameraOrientation {
                rotation,
                pitch,
                transition_seconds,
            } => self.set_editor_camera_orientation(rotation, pitch, transition_seconds),
            EditorCommand::AddCameraTrigger => self.editor_add_camera_trigger(),
            EditorCommand::SetTriggerSelected(selected) => {
                self.set_editor_trigger_selected(selected)
            }
            EditorCommand::SetSimulateTriggerHitboxes(enabled) => {
                self.set_editor_simulate_trigger_hitboxes(enabled);
            }

            EditorCommand::ToggleHitboxVisualization => self.toggle_editor_hitbox_visualization(),
            EditorCommand::TogglePerfOverlay => self.toggle_perf_overlay(),
            EditorCommand::ExportBlockObj => self.trigger_selected_block_obj_export(),

            EditorCommand::LoadLevel(name) => self.load_builtin_level_into_editor(&name),
            EditorCommand::RenameLevel(name) => self.set_editor_level_name(name),
            EditorCommand::ExportLevel => self.trigger_level_export(),
            EditorCommand::SetShowMetadata(show) => self.set_editor_show_metadata(show),
            EditorCommand::ToggleSettings => {
                self.set_editor_show_settings(!self.editor_show_settings());
            }
            EditorCommand::SetShowSettings(show) => self.set_editor_show_settings(show),
            EditorCommand::SetSettingsSection(section) => self.set_editor_settings_section(section),
            EditorCommand::SetGraphicsBackend(backend) => {
                self.set_preferred_graphics_backend(backend);
            }
            EditorCommand::SetAudioBackend(backend) => self.set_preferred_audio_backend(backend),
            EditorCommand::SetUiScaleMultiplier(multiplier) => {
                self.set_ui_scale_multiplier(multiplier)
            }
            EditorCommand::SetKeybindCapture(action) => {
                self.set_editor_keybind_capture_action(action);
            }
            EditorCommand::SetKeybind {
                action,
                slot,
                chord,
            } => self.set_keybind_for_action(action, slot, chord),
            EditorCommand::ClearKeybindSlot { action, slot } => {
                self.clear_keybind_slot_for_action(&action, slot);
            }
            EditorCommand::ResetKeybind(action) => self.reset_keybind_for_action(&action),
            EditorCommand::ResetKeybinds => self.reset_essential_keybinds(),
            EditorCommand::CompleteImport => self.complete_import(),
            EditorCommand::UpdateMusic(metadata) => self.set_editor_music_metadata(metadata),
            EditorCommand::UpdateCreatorMetadata(metadata) => {
                self.set_editor_creator_metadata(metadata);
            }
            EditorCommand::UpdateSkyColor(color) => self.set_editor_sky_color(color),
            EditorCommand::TriggerAudioImport => self.trigger_audio_import(),
            EditorCommand::CaptureMenuPreviewCamera => self.editor_capture_menu_preview_camera(),
            EditorCommand::UseAutoMenuPreviewCamera => self.editor_use_auto_menu_preview_camera(),

            EditorCommand::SetShiftHeld(held) => self.set_editor_shift_held(held),
            EditorCommand::SetCtrlHeld(held) => self.set_editor_ctrl_held(held),
            EditorCommand::SetAltHeld(held) => self.set_editor_alt_held(held),
            EditorCommand::SetPanUpHeld(held) => self.set_editor_pan_up_held(held),
            EditorCommand::SetPanDownHeld(held) => self.set_editor_pan_down_held(held),
            EditorCommand::SetPanLeftHeld(held) => self.set_editor_pan_left_held(held),
            EditorCommand::SetPanRightHeld(held) => self.set_editor_pan_right_held(held),

            EditorCommand::MouseButton { button, pressed } => {
                if button == 0 && pressed {
                    if let Some(pos) = self.editor.ui.pointer_screen {
                        self.handle_primary_click(pos[0], pos[1]);
                    } else {
                        self.handle_mouse_button(button, pressed);
                    }
                } else if button == 1 && pressed {
                    if let Some(pos) = self.editor.ui.pointer_screen {
                        self.editor_pick_block_at_screen(pos[0], pos[1]);
                    }
                } else {
                    self.handle_mouse_button(button, pressed);
                }
            }
            EditorCommand::PrimaryClick { x, y } => self.handle_primary_click(x, y),
            EditorCommand::PointerMoved { x, y } => self.handle_pointer_moved(x, y),
            EditorCommand::UpdateCursorFromScreen { x, y } => {
                self.force_editor_cursor_from_screen(x, y);
            }
            EditorCommand::CameraDrag { dx, dy } => self.drag_editor_camera_by_pixels(dx, dy),

            EditorCommand::TogglePlaceWindow => {
                self.set_editor_show_place_window(!self.editor_show_place_window());
            }
            EditorCommand::Escape => self.handle_editor_escape(),
        }
    }

    fn handle_editor_escape(&mut self) {
        if self.phase == AppPhase::Playing && !self.session.playtesting_editor {
            self.toggle_game_pause();
            return;
        }

        if !self.is_editor() {
            self.back_to_menu();
            return;
        }

        if self.cancel_editor_transform_trigger_capture() {
            return;
        }

        if self.clear_editor_selection_for_escape() {
            return;
        }

        if self.editor.timeline.playback.playing {
            self.editor.timeline.playback.playing = false;
            self.editor.timeline.playback.runtime = None;
            self.stop_audio();
        } else if self.editor.timeline.clock.time_seconds > 0.001 {
            self.set_editor_timeline_time_seconds(0.0);
        } else {
            self.back_to_menu();
        }
    }

    fn clear_editor_selection_for_escape(&mut self) -> bool {
        let had_block_selection = self.has_block_selection();
        let had_tap_selection = self.editor.selected_tap().is_some();

        if had_block_selection {
            self.editor.clear_block_selection();
            self.editor.mark_dirty(EditorDirtyFlags {
                rebuild_selection_overlays: true,
                ..EditorDirtyFlags::default()
            });
        }

        if had_tap_selection {
            self.editor.set_selected_tap_index(None);
            self.rebuild_tap_indicator_vertices();
        }

        had_block_selection || had_tap_selection
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::state::editor_command::EditorCommand;
    use crate::test_utils::stone;
    use crate::types::{AppPhase, EditorMode, SettingsSection};

    async fn new_editor_state() -> State {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("EditorDispatchCoverage");
        state
    }

    #[test]
    fn dispatch_editor_routes_ui_session_and_input_commands() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch_editor(EditorCommand::TogglePlaceWindow);
            assert!(state.editor_show_place_window());
            state.dispatch_editor(EditorCommand::TogglePlaceWindow);
            assert!(!state.editor_show_place_window());

            state.dispatch_editor(EditorCommand::SetShowSettings(true));
            state.dispatch_editor(EditorCommand::ToggleSettings);
            assert!(!state.editor_show_settings());

            state.dispatch_editor(EditorCommand::SetSettingsSection(SettingsSection::Keybinds));
            assert_eq!(state.editor_settings_section(), SettingsSection::Keybinds);

            state.dispatch_editor(EditorCommand::SetShiftHeld(true));
            state.dispatch_editor(EditorCommand::SetCtrlHeld(true));
            state.dispatch_editor(EditorCommand::SetAltHeld(true));
            state.dispatch_editor(EditorCommand::SetPanUpHeld(true));
            state.dispatch_editor(EditorCommand::SetPanDownHeld(true));
            state.dispatch_editor(EditorCommand::SetPanLeftHeld(true));
            state.dispatch_editor(EditorCommand::SetPanRightHeld(true));

            assert!(state.editor.ui.shift_held);
            assert!(state.editor.ui.ctrl_held);
            assert!(state.editor.ui.alt_held);
            assert!(state.editor.ui.pan_up_held);
            assert!(state.editor.ui.pan_down_held);
            assert!(state.editor.ui.pan_left_held);
            assert!(state.editor.ui.pan_right_held);

            state.dispatch_editor(EditorCommand::MouseButton {
                button: 0,
                pressed: false,
            });
            state.dispatch_editor(EditorCommand::PrimaryClick { x: 10.0, y: 20.0 });
            state.dispatch_editor(EditorCommand::PointerMoved { x: 11.0, y: 21.0 });
            state.dispatch_editor(EditorCommand::CameraDrag { dx: 2.0, dy: -3.0 });
        });
    }

    #[test]
    fn dispatch_editor_routes_selection_timeline_and_escape_commands() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects = vec![stone(0.0, 0.0, 0.0)];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.dispatch_editor(EditorCommand::CopyBlock);
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            state.dispatch_editor(EditorCommand::UpdateSelectedBlock(
                crate::test_utils::sized("core/grass", 2.0, 3.0, 4.0, 5.0, 6.0, 7.0),
            ));
            assert_eq!(state.editor.objects[0].block_id, "core/grass");
            assert_eq!(state.editor.objects[0].position, [2.0, 3.0, 4.0]);
            assert_eq!(state.editor.objects[0].size, [5.0, 6.0, 7.0]);

            state.dispatch_editor(EditorCommand::NudgeSelected { dx: 1, dy: 0 });
            state.dispatch_editor(EditorCommand::SnapSelectionToGrid);
            state.dispatch_editor(EditorCommand::FocusCameraTarget);
            state.dispatch_editor(EditorCommand::SetTimelineDuration(8.0));
            state.dispatch_editor(EditorCommand::SetTimelineTime(1.25));
            state.dispatch_editor(EditorCommand::AddTap);
            state.dispatch_editor(EditorCommand::SetSelectedTap(Some(0)));
            state.dispatch_editor(EditorCommand::SetSelectedTapTime(1.5));
            state.dispatch_editor(EditorCommand::RemoveTapAt(1.5));
            state.dispatch_editor(EditorCommand::AddTimingPoint {
                time_seconds: 0.5,
                bpm: 120.0,
            });
            state.dispatch_editor(EditorCommand::SetTimingSelected(Some(0)));
            state.dispatch_editor(EditorCommand::SetTimingPointTime(0, 0.75));
            state.dispatch_editor(EditorCommand::SetTimingPointBpm(0, 140.0));
            state.dispatch_editor(EditorCommand::SetTimingPointTimeSignature(0, 3, 4));
            state.dispatch_editor(EditorCommand::RemoveTimingPoint(0));

            state.editor.ui.selected_block_indices = vec![0];
            state.dispatch_editor(EditorCommand::Escape);
            assert!(state.editor.ui.selected_block_indices.is_empty());

            state.editor.timeline.clock.time_seconds = 2.0;
            state.dispatch_editor(EditorCommand::Escape);
            assert_eq!(state.editor.timeline.clock.time_seconds, 0.0);

            state.dispatch_editor(EditorCommand::Escape);
            assert_eq!(state.phase, AppPhase::Menu);
        });
    }

    #[test]
    fn dispatch_editor_routes_mode_and_pointer_pick_commands() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects = vec![stone(0.0, 0.0, 0.0)];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            state.dispatch_editor(EditorCommand::SetMode(EditorMode::Tapping));
            assert_eq!(state.editor.ui.mode, EditorMode::Tapping);
            state.dispatch_editor(EditorCommand::SetMode(EditorMode::Timing));
            assert_eq!(state.editor.ui.mode, EditorMode::Timing);
            state.dispatch_editor(EditorCommand::SetMode(EditorMode::Select));
            state.dispatch_editor(EditorCommand::PickSelectedBlock);
            assert_eq!(state.editor.ui.mode, EditorMode::Select);

            state.editor.ui.pointer_screen = Some([0.0, 0.0]);
            state.dispatch_editor(EditorCommand::MouseButton {
                button: 1,
                pressed: true,
            });
            state.dispatch_editor(EditorCommand::PickBlockAt { x: 0.0, y: 0.0 });
        });
    }
}
