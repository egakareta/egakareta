/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use crate::commands::AppCommand;
use crate::types::{normalize_binding_key, EditorMode, KeyChord};

impl State {
    /// Central dispatcher: every `AppCommand` is routed here.
    /// This is the **only** place that maps intent → mutation,
    /// making it easy to log, replay, or test commands in isolation.
    pub(crate) fn dispatch(&mut self, cmd: AppCommand) {
        match cmd {
            // ── Navigation / Phase ──────────────────────────────────
            AppCommand::TurnRight => self.turn_right(),
            AppCommand::NextLevel => self.next_level(),
            AppCommand::PrevLevel => self.prev_level(),
            AppCommand::ToggleEditor => self.toggle_editor(),

            // ── Editor – mode switching ─────────────────────────────
            AppCommand::EditorSetMode(mode) => {
                let old_mode = self.editor_mode();
                self.set_editor_mode(mode);
                if mode == EditorMode::Timing && old_mode != EditorMode::Timing {
                    self.load_waveform_for_current_audio();
                }
            }
            AppCommand::EditorSetBlockId(id) => self.set_editor_block_id(id),
            AppCommand::EditorSetSnapToGrid(snap) => self.set_editor_snap_to_grid(snap),
            AppCommand::EditorSetSnapStep(step) => self.set_editor_snap_step(step),
            AppCommand::EditorSetSnapRotation(snap) => self.set_editor_snap_rotation(snap),
            AppCommand::EditorSetSnapRotationStep(step) => self.set_editor_snap_rotation_step(step),

            // ── Editor – block ops ──────────────────────────────────
            AppCommand::EditorRemoveBlock => self.editor_remove_block(),
            AppCommand::EditorDuplicateBlock => self.editor_duplicate_selected_block_in_place(),
            AppCommand::EditorCopyBlock => self.editor_copy_block(),
            AppCommand::EditorPasteBlock => self.editor_paste_block(),
            AppCommand::EditorUpdateSelectedBlock(obj) => {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
                self.set_editor_selected_block_id(obj.block_id);
                self.set_editor_selected_block_rotation(obj.rotation_degrees);
                self.set_editor_selected_block_roundness(obj.roundness);
                self.set_editor_selected_block_color_tint(obj.color_tint);
            }

            // ── Editor – selection / transform ──────────────────────
            AppCommand::EditorNudgeSelected { dx, dy } => {
                self.editor_nudge_selected_blocks(dx, dy);
            }

            // ── Editor – timeline / playback ────────────────────────
            AppCommand::EditorToggleTimelinePlayback => self.toggle_editor_timeline_playback(),
            AppCommand::EditorShiftTimeline(delta) => self.editor_shift_timeline_time(delta),
            AppCommand::EditorSetTimelineTime(time) => self.set_editor_timeline_time_seconds(time),
            AppCommand::EditorSetTimelineDuration(duration) => {
                self.set_editor_timeline_duration_seconds(duration)
            }
            AppCommand::EditorToggleTapAtPointer => self.editor_add_tap_at_pointer_position(),
            AppCommand::EditorAddTap => self.editor_add_tap(),
            AppCommand::EditorRemoveTap => self.editor_remove_tap(),
            AppCommand::EditorClearTaps => self.editor_clear_taps(),
            AppCommand::EditorSetPlaybackSpeed(speed) => self.set_editor_playback_speed(speed),
            AppCommand::EditorSetWaveformZoom(zoom) => self.set_editor_waveform_zoom(zoom),
            AppCommand::EditorSetWaveformScroll(scroll) => self.set_editor_waveform_scroll(scroll),
            AppCommand::EditorPlaytest => self.editor_playtest(),

            // ── Editor – timing points ──────────────────────────────
            AppCommand::EditorAddTimingPoint { time_seconds, bpm } => {
                self.editor_add_timing_point(time_seconds, bpm)
            }
            AppCommand::EditorRemoveTimingPoint(idx) => self.editor_remove_timing_point(idx),
            AppCommand::EditorSetTimingPointTime(idx, time) => {
                self.editor_update_timing_point_time(idx, time)
            }
            AppCommand::EditorSetTimingPointBpm(idx, bpm) => {
                self.editor_update_timing_point_bpm(idx, bpm)
            }
            AppCommand::EditorSetTimingPointTimeSignature(idx, num, den) => {
                self.editor_update_timing_point_time_signature(idx, num, den)
            }
            AppCommand::EditorSetTimingSelected(selected) => {
                self.set_editor_timing_selected_index(selected)
            }
            AppCommand::EditorBpmTap => self.editor_bpm_tap(),
            AppCommand::EditorBpmTapReset => self.editor_bpm_tap_reset(),

            // ── Editor – spawn ──────────────────────────────────────
            AppCommand::EditorSetSpawnHere => self.editor_set_spawn_here(),
            AppCommand::EditorRotateSpawnDirection => self.editor_rotate_spawn_direction(),

            // ── Editor – history ────────────────────────────────────
            AppCommand::EditorUndo => self.editor_undo(),
            AppCommand::EditorRedo => self.editor_redo(),

            // ── Editor – zoom ───────────────────────────────────────
            AppCommand::EditorAdjustZoom(delta) => self.adjust_editor_zoom(delta),
            AppCommand::EditorSetCameraOrientation {
                rotation,
                pitch,
                transition_seconds,
            } => self.set_editor_camera_orientation(rotation, pitch, transition_seconds),
            AppCommand::EditorAddCameraTrigger => self.editor_add_camera_trigger(),
            AppCommand::EditorAddTrigger(trigger) => self.editor_add_trigger(trigger),
            AppCommand::EditorCaptureSelectedCameraTrigger => {
                self.editor_capture_selected_camera_trigger()
            }
            AppCommand::EditorApplySelectedCameraTrigger => {
                self.editor_apply_selected_camera_trigger()
            }
            AppCommand::EditorRemoveTrigger(index) => self.editor_remove_trigger(index),
            AppCommand::EditorSetTriggerSelected(selected) => {
                self.set_editor_trigger_selected(selected)
            }
            AppCommand::EditorUpdateTrigger(index, trigger) => {
                self.editor_update_trigger(index, trigger)
            }
            AppCommand::EditorSetSimulateTriggerHitboxes(enabled) => {
                self.set_editor_simulate_trigger_hitboxes(enabled)
            }

            // ── Editor – misc ───────────────────────────────────────
            AppCommand::EditorTogglePerfOverlay => self.toggle_editor_perf_overlay(),
            AppCommand::EditorTogglePerfProfilerPause => self.toggle_editor_perf_pause(),
            AppCommand::EditorSelectPerfHistoryIndex(index) => {
                self.select_editor_perf_history_index(index)
            }
            AppCommand::EditorClearPerfSelection => self.clear_editor_perf_selection(),
            AppCommand::EditorExportBlockObj => self.trigger_selected_block_obj_export(),

            // ── Editor – UI / Session ───────────────────────────────
            AppCommand::EditorLoadLevel(name) => self.load_builtin_level_into_editor(&name),
            AppCommand::EditorRenameLevel(name) => self.set_editor_level_name(name),
            AppCommand::EditorExportLevel => self.trigger_level_export(),
            AppCommand::EditorSetShowMetadata(show) => self.set_editor_show_metadata(show),
            AppCommand::EditorSetShowImport(show) => self.set_editor_show_import(show),
            AppCommand::EditorToggleSettings => {
                self.set_editor_show_settings(!self.editor_show_settings())
            }
            AppCommand::EditorSetShowSettings(show) => self.set_editor_show_settings(show),
            AppCommand::EditorSetSettingsSection(section) => {
                self.set_editor_settings_section(section)
            }
            AppCommand::EditorSetGraphicsBackend(backend) => {
                self.set_preferred_graphics_backend(backend)
            }
            AppCommand::EditorSetAudioBackend(backend) => self.set_preferred_audio_backend(backend),
            AppCommand::EditorSetUiScaleMultiplier(multiplier) => {
                self.set_ui_scale_multiplier(multiplier)
            }
            AppCommand::EditorSetKeybindCapture(action) => {
                self.set_editor_keybind_capture_action(action)
            }
            AppCommand::EditorSetKeybind {
                action,
                slot,
                chord,
            } => self.set_keybind_for_action(action, slot, chord),
            AppCommand::EditorClearKeybindSlot { action, slot } => {
                self.clear_keybind_slot_for_action(&action, slot)
            }
            AppCommand::EditorResetKeybind(action) => self.reset_keybind_for_action(&action),
            AppCommand::EditorResetKeybinds => self.reset_essential_keybinds(),
            AppCommand::EditorSetImportText(text) => self.set_editor_import_text(text),
            AppCommand::EditorCompleteImport => self.complete_import(),
            AppCommand::EditorUpdateMusic(metadata) => self.set_editor_music_metadata(metadata),
            AppCommand::EditorTriggerAudioImport => self.trigger_audio_import(),

            // ── Editor – keyboard state routing ───────────────────
            AppCommand::EditorSetShiftHeld(held) => self.set_editor_shift_held(held),
            AppCommand::EditorSetCtrlHeld(held) => self.set_editor_ctrl_held(held),
            AppCommand::EditorSetAltHeld(held) => self.set_editor_alt_held(held),
            AppCommand::EditorSetPanUpHeld(held) => self.set_editor_pan_up_held(held),
            AppCommand::EditorSetPanDownHeld(held) => self.set_editor_pan_down_held(held),
            AppCommand::EditorSetPanLeftHeld(held) => self.set_editor_pan_left_held(held),
            AppCommand::EditorSetPanRightHeld(held) => self.set_editor_pan_right_held(held),

            // ── Editor – pointer/input routing ─────────────────────
            AppCommand::EditorMouseButton { button, pressed } => {
                if button == 0 && pressed {
                    if let Some(pos) = self.editor.ui.pointer_screen {
                        self.handle_primary_click(pos[0], pos[1]);
                    } else {
                        self.handle_mouse_button(button, pressed);
                    }
                } else {
                    self.handle_mouse_button(button, pressed);
                }
            }
            AppCommand::EditorPrimaryClick { x, y } => self.handle_primary_click(x, y),
            AppCommand::EditorPointerMoved { x, y } => self.handle_pointer_moved(x, y),
            AppCommand::EditorCameraDrag { dx, dy } => self.drag_editor_camera_by_pixels(dx, dy),
            AppCommand::ResizeSurface { width, height } => {
                self.resize_surface(crate::types::PhysicalSize::new(width, height));
            }

            // ── Editor – escape context ─────────────────────────────
            AppCommand::EditorEscape => self.handle_editor_escape(),
        }
    }

    /// Context-sensitive escape for the editor.
    fn handle_editor_escape(&mut self) {
        if !self.is_editor() {
            self.back_to_menu();
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

    /// Translate a keyboard event into zero or more `AppCommand`s and
    /// execute them. This replaces the monolithic `handle_keyboard_input`.
    pub fn process_keyboard_input(&mut self, key: &str, pressed: bool, just_pressed: bool) {
        if let Some(cmd) = self.map_modifier_key_to_command(key, pressed) {
            self.dispatch(cmd);
            return;
        }

        for cmd in self.map_pan_key_to_commands(key, pressed) {
            self.dispatch(cmd);
        }

        if !pressed {
            return;
        }

        if let Some((action, slot)) = self.editor_keybind_capture_action().cloned() {
            if !just_pressed {
                return;
            }

            if Self::is_modifier_key(key) {
                return;
            }

            if key == "Escape" {
                self.dispatch(AppCommand::EditorSetKeybindCapture(None));
                return;
            }

            let chord = KeyChord::new(
                normalize_binding_key(key),
                self.editor.ui.ctrl_held,
                self.editor.ui.shift_held,
                self.editor.ui.alt_held,
            );
            self.dispatch(AppCommand::EditorSetKeybind {
                action,
                slot,
                chord,
            });
            self.dispatch(AppCommand::EditorSetKeybindCapture(None));
            return;
        }

        for cmd in self.map_key_to_commands(key, just_pressed, pressed) {
            self.dispatch(cmd);
        }
    }

    fn map_modifier_key_to_command(&self, key: &str, pressed: bool) -> Option<AppCommand> {
        match key {
            "Shift" | "ShiftLeft" | "ShiftRight" => Some(AppCommand::EditorSetShiftHeld(pressed)),
            "Control" | "ControlLeft" | "ControlRight" => {
                Some(AppCommand::EditorSetCtrlHeld(pressed))
            }
            "Alt" | "AltLeft" | "AltRight" => Some(AppCommand::EditorSetAltHeld(pressed)),
            _ => None,
        }
    }

    fn map_pan_key_to_commands(&self, key: &str, pressed: bool) -> Vec<AppCommand> {
        if !self.is_editor() {
            return Vec::new();
        }

        let normalized_key = crate::types::normalize_binding_key(key);
        let ctrl = self.editor.ui.ctrl_held;
        let alt = self.editor.ui.alt_held;

        let mut commands = Vec::new();
        for binding in &self.app_settings().keybinds {
            let chord = binding.chord.normalized();
            let key_match = chord.key == normalized_key;
            let modifiers_match = chord.ctrl == ctrl && chord.alt == alt;

            if key_match && (!pressed || modifiers_match) {
                match binding.action.as_str() {
                    "pan_up" => commands.push(AppCommand::EditorSetPanUpHeld(pressed)),
                    "pan_down" => commands.push(AppCommand::EditorSetPanDownHeld(pressed)),
                    "pan_left" => commands.push(AppCommand::EditorSetPanLeftHeld(pressed)),
                    "pan_right" => commands.push(AppCommand::EditorSetPanRightHeld(pressed)),
                    _ => {}
                }
            }
        }
        commands
    }

    /// Pure mapping from key string + modifiers → command.
    /// Returns `Vec<AppCommand>` for keys that have command bindings.
    fn map_key_to_commands(&self, key: &str, just_pressed: bool, pressed: bool) -> Vec<AppCommand> {
        let commands = self.map_keybind_to_commands(key, just_pressed, pressed);

        match key {
            "Enter" => {
                let _ = just_pressed;
            }
            "Backspace" | "Delete" => {
                let _ = just_pressed;
            }
            "Escape" => {
                let _ = just_pressed;
            }
            "q" | "Q" => {}
            "c" | "C" => {
                let _ = just_pressed;
            }
            "v" | "V" => {
                let _ = just_pressed;
            }
            "z" | "Z" => {
                let _ = just_pressed;
            }
            "y" | "Y" => {
                let _ = just_pressed;
            }
            _ => {}
        }
        commands
    }

    fn map_keybind_to_commands(
        &self,
        key: &str,
        just_pressed: bool,
        pressed: bool,
    ) -> Vec<AppCommand> {
        if !pressed {
            return Vec::new();
        }

        let normalized_key = normalize_binding_key(key);
        let ctrl = self.editor.ui.ctrl_held;
        let shift = self.editor.ui.shift_held;
        let alt = self.editor.ui.alt_held;

        let mut commands = Vec::new();
        for binding in &self.app_settings().keybinds {
            let chord = binding.chord.normalized();
            if chord.key == normalized_key
                && chord.ctrl == ctrl
                && chord.shift == shift
                && chord.alt == alt
            {
                if let Some(command) =
                    self.command_for_keybind_action(&binding.action, just_pressed)
                {
                    commands.push(command);
                }
            }
        }

        commands
    }

    fn command_for_keybind_action(&self, action: &str, just_pressed: bool) -> Option<AppCommand> {
        match action {
            "toggle_settings" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorToggleSettings)
                } else {
                    None
                }
            }
            "toggle_timeline_playback" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorToggleTimelinePlayback)
                } else {
                    None
                }
            }
            "playtest" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorPlaytest)
                } else {
                    None
                }
            }
            "remove_block" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorRemoveBlock)
                } else {
                    None
                }
            }
            "copy" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorCopyBlock)
                } else {
                    None
                }
            }
            "paste" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorPasteBlock)
                } else {
                    None
                }
            }
            "duplicate" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorDuplicateBlock)
                } else {
                    None
                }
            }
            "undo" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorUndo)
                } else {
                    None
                }
            }
            "redo" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorRedo)
                } else {
                    None
                }
            }
            "nudge_up" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::EditorNudgeSelected { dx: 0, dy: 1 })
                } else {
                    None
                }
            }
            "nudge_down" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::EditorNudgeSelected { dx: 0, dy: -1 })
                } else {
                    None
                }
            }
            "nudge_left" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::EditorNudgeSelected { dx: -1, dy: 0 })
                } else {
                    None
                }
            }
            "nudge_right" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::EditorNudgeSelected { dx: 1, dy: 0 })
                } else {
                    None
                }
            }
            "timeline_forward" => {
                if self.is_editor() && !self.has_block_selection() {
                    Some(AppCommand::EditorShiftTimeline(0.1))
                } else {
                    None
                }
            }
            "timeline_backward" => {
                if self.is_editor() && !self.has_block_selection() {
                    Some(AppCommand::EditorShiftTimeline(-0.1))
                } else {
                    None
                }
            }
            "escape" => {
                if just_pressed {
                    Some(AppCommand::EditorEscape)
                } else {
                    None
                }
            }
            "zoom_in" => {
                if just_pressed {
                    Some(AppCommand::EditorAdjustZoom(1.0))
                } else {
                    None
                }
            }
            "zoom_out" => {
                if just_pressed {
                    Some(AppCommand::EditorAdjustZoom(-1.0))
                } else {
                    None
                }
            }
            "toggle_editor" => {
                if !self.is_editor() && just_pressed {
                    Some(AppCommand::ToggleEditor)
                } else {
                    None
                }
            }
            "game_turn" => {
                if !self.is_editor() && just_pressed {
                    Some(AppCommand::TurnRight)
                } else {
                    None
                }
            }
            "menu_prev_level" => {
                if !self.is_editor() && just_pressed {
                    Some(AppCommand::PrevLevel)
                } else {
                    None
                }
            }
            "menu_next_level" => {
                if !self.is_editor() && just_pressed {
                    Some(AppCommand::NextLevel)
                } else {
                    None
                }
            }
            "spawn_set" => {
                if just_pressed {
                    Some(AppCommand::EditorSetSpawnHere)
                } else {
                    None
                }
            }
            "spawn_rotate" => {
                if just_pressed {
                    Some(AppCommand::EditorRotateSpawnDirection)
                } else {
                    None
                }
            }
            "toggle_tap_timing" => {
                if just_pressed && self.is_editor() {
                    if self.editor.ui.mode == EditorMode::Place {
                        Some(AppCommand::EditorToggleTapAtPointer)
                    } else if self.editor.ui.mode != EditorMode::Timing {
                        Some(AppCommand::EditorSetMode(EditorMode::Timing))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "add_camera_trigger" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorAddCameraTrigger)
                } else {
                    None
                }
            }
            "export_obj" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorExportBlockObj)
                } else {
                    None
                }
            }
            "toggle_perf_overlay" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorTogglePerfOverlay)
                } else {
                    None
                }
            }
            "mode_select" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Select))
                } else {
                    None
                }
            }
            "mode_move" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Move))
                } else {
                    None
                }
            }
            "mode_scale" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Scale))
                } else {
                    None
                }
            }
            "mode_rotate" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Rotate))
                } else {
                    None
                }
            }
            "mode_place" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Place))
                } else {
                    None
                }
            }
            "mode_trigger" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Trigger))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn is_modifier_key(key: &str) -> bool {
        matches!(
            key,
            "Shift" | "Control" | "ControlLeft" | "ControlRight" | "Alt" | "AltLeft" | "AltRight"
        )
    }

    /// Process a unified `InputEvent`.
    pub fn process_input_event(&mut self, event: crate::commands::InputEvent) {
        use crate::commands::InputEvent;
        match event {
            InputEvent::Key {
                key,
                pressed,
                just_pressed,
            } => {
                self.process_keyboard_input(&key, pressed, just_pressed);
            }
            InputEvent::MouseButton { button, pressed } => {
                self.dispatch(AppCommand::EditorMouseButton { button, pressed });
            }
            InputEvent::PrimaryClick { x, y } => {
                self.dispatch(AppCommand::EditorPrimaryClick { x, y });
            }
            InputEvent::PointerMoved { x, y } => {
                self.dispatch(AppCommand::EditorPointerMoved { x, y });
            }
            InputEvent::CameraDrag { dx, dy } => {
                self.dispatch(AppCommand::EditorCameraDrag { dx, dy });
            }
            InputEvent::Zoom(delta) => {
                self.dispatch(AppCommand::EditorAdjustZoom(delta));
            }
            InputEvent::Resize { width, height } => {
                self.dispatch(AppCommand::ResizeSurface { width, height });
            }
        }
    }

    // ── helpers for command mapping ──────────────────────────────────

    /// Whether any blocks are currently selected in the editor.
    fn has_block_selection(&self) -> bool {
        self.editor.ui.selected_block_index.is_some()
            || !self.editor.ui.selected_block_indices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::commands::AppCommand;
    use crate::types::{
        camera_triggers_to_timed_triggers, AppPhase, CameraTrigger, CameraTriggerMode, EditorMode,
        KeyChord, LevelObject, MusicMetadata, SettingsSection, TimedTrigger, TimedTriggerAction,
        TimedTriggerEasing, TimedTriggerTarget,
    };
    use glam::Vec2;

    async fn new_editor_state() -> State {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state
    }

    #[test]
    fn test_command_routing_navigation() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Menu;
            state.menu.state.levels = vec!["__unit_test_level__".to_string()];
            state.menu.state.selected_level = 0;

            // Initial state should be Menu
            assert_eq!(state.phase, AppPhase::Menu);

            // ToggleEditor from Menu should go to Editor
            state.dispatch(AppCommand::ToggleEditor);
            assert_eq!(state.phase, AppPhase::Editor);
        });
    }

    #[test]
    fn test_command_routing_editor_modes() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Select);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Move));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Move);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Scale));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Scale);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Rotate));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Rotate);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Timing));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Timing);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Place));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Trigger));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Trigger);
        });
    }

    #[test]
    fn test_resize_routing() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            let new_width = 1280;
            let new_height = 720;

            state.process_input_event(crate::commands::InputEvent::Resize {
                width: new_width,
                height: new_height,
            });

            assert_eq!(state.render.gpu.config.width, new_width);
            assert_eq!(state.render.gpu.config.height, new_height);
        });
    }

    #[test]
    fn test_command_routing_editor_ops() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            let initial_z = state.editor.camera.editor_target_z;
            state.dispatch(AppCommand::EditorAdjustZoom(0.5));
            // Zooming in (positive delta) should move the camera target Z forward (if look direction has positive Z)
            // or at least change the position.
            assert!(state.editor.camera.editor_target_z != initial_z);

            state.dispatch(AppCommand::EditorSetBlockId("core/lava".to_string()));
            assert_eq!(state.editor.config.selected_block_id, "core/lava");
        });
    }

    #[test]
    fn test_timeline_shift_updates_preview() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            let (pos_before, _) = state.editor_timeline_preview();

            // Shift timeline forward by 1 second
            state.dispatch(AppCommand::EditorShiftTimeline(1.0));

            let (pos_after, _) = state.editor_timeline_preview();

            // Player should have moved
            assert!(
                (pos_after[0] - pos_before[0]).abs() > 0.001
                    || (pos_after[1] - pos_before[1]).abs() > 0.001
                    || (pos_after[2] - pos_before[2]).abs() > 0.001,
                "Preview position should update when shifting timeline. Before: {:?}, After: {:?}",
                pos_before,
                pos_after
            );
        });
    }

    #[test]
    fn test_input_event_interaction_state() {
        pollster::block_on(async {
            use crate::commands::InputEvent;
            let mut state = new_editor_state().await;

            // 1. Pointer move updates screen coordinates
            state.process_input_event(InputEvent::PointerMoved { x: 100.0, y: 200.0 });
            assert_eq!(state.editor.ui.pointer_screen, Some([100.0, 200.0]));

            // 2. Mouse down sets interaction state
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            assert!(state.editor.ui.left_mouse_down);

            // 3. Mouse up clears interaction state
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });
            assert!(!state.editor.ui.left_mouse_down);
        });
    }

    #[test]
    fn test_input_event_zoom_and_resize() {
        pollster::block_on(async {
            use crate::commands::InputEvent;
            let mut state = new_editor_state().await;

            // Zoom
            let initial_z = state.editor.camera.editor_target_z;
            state.process_input_event(InputEvent::Zoom(1.0));
            assert!(state.editor.camera.editor_target_z != initial_z);

            // Resize
            state.process_input_event(InputEvent::Resize {
                width: 1280,
                height: 720,
            });
            assert_eq!(state.render.gpu.config.width, 1280);
            assert_eq!(state.render.gpu.config.height, 720);
        });
    }

    #[test]
    fn test_keyboard_modifier_aliases_keep_consistent_state() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.process_input_event(InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor.ui.ctrl_held);

            state.process_input_event(InputEvent::Key {
                key: "ControlLeft".to_string(),
                pressed: false,
                just_pressed: false,
            });
            assert!(!state.editor.ui.ctrl_held);

            state.process_input_event(InputEvent::Key {
                key: "AltRight".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor.ui.alt_held);

            state.process_input_event(InputEvent::Key {
                key: "Alt".to_string(),
                pressed: false,
                just_pressed: false,
            });
            assert!(!state.editor.ui.alt_held);
        });
    }

    #[test]
    fn test_keyboard_space_aliases_have_matching_behavior() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            for key in [" ", "Space"] {
                let mut state = State::new_test().await;
                state.phase = AppPhase::Menu;

                state.process_input_event(InputEvent::Key {
                    key: key.to_string(),
                    pressed: true,
                    just_pressed: true,
                });

                assert_eq!(state.phase, AppPhase::Playing);
            }
        });
    }

    #[test]
    fn test_keyboard_pan_keys_set_held_state_via_input_events() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.process_input_event(InputEvent::Key {
                key: "w".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor.ui.pan_up_held);

            state.process_input_event(InputEvent::Key {
                key: "w".to_string(),
                pressed: false,
                just_pressed: false,
            });
            assert!(!state.editor.ui.pan_up_held);
        });
    }

    #[test]
    fn test_keyboard_numeric_hotkeys_switch_editor_modes() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.process_input_event(InputEvent::Key {
                key: "1".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Select);

            state.process_input_event(InputEvent::Key {
                key: "2".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Move);

            state.process_input_event(InputEvent::Key {
                key: "3".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Scale);

            state.process_input_event(InputEvent::Key {
                key: "4".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Rotate);

            state.process_input_event(InputEvent::Key {
                key: "5".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);

            state.process_input_event(InputEvent::Key {
                key: "6".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Trigger);
        });
    }

    #[test]
    fn test_keyboard_qe_do_not_switch_modes_in_editor() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Scale));

            state.process_input_event(InputEvent::Key {
                key: "q".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Scale);

            state.process_input_event(InputEvent::Key {
                key: "e".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Scale);
        });
    }

    #[test]
    fn test_native_web_primary_click_event_sequences_are_equivalent() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut native_style = new_editor_state().await;

            let mut web_style = new_editor_state().await;

            // Native path: pointer move then left mouse button press.
            native_style.process_input_event(InputEvent::PointerMoved { x: 120.0, y: 240.0 });
            native_style.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            native_style.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            // Web path: explicit primary click event followed by release.
            web_style.process_input_event(InputEvent::PrimaryClick { x: 120.0, y: 240.0 });
            web_style.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert_eq!(
                native_style.editor.objects.len(),
                web_style.editor.objects.len()
            );
            assert_eq!(
                native_style.editor.ui.pointer_screen,
                web_style.editor.ui.pointer_screen
            );
            assert_eq!(
                native_style.editor.ui.left_mouse_down,
                web_style.editor.ui.left_mouse_down
            );
            assert_eq!(native_style.editor.ui.cursor, web_style.editor.ui.cursor);
        });
    }

    #[test]
    fn test_native_web_right_drag_sequences_are_equivalent() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut native_style = new_editor_state().await;

            let mut web_style = new_editor_state().await;

            // Native-like order around right-drag camera movement.
            native_style.process_input_event(InputEvent::MouseButton {
                button: 2,
                pressed: true,
            });
            native_style.process_input_event(InputEvent::PointerMoved { x: 300.0, y: 180.0 });
            native_style.process_input_event(InputEvent::CameraDrag {
                dx: 24.0,
                dy: -12.0,
            });
            native_style.process_input_event(InputEvent::MouseButton {
                button: 2,
                pressed: false,
            });

            // Web-like order emits pointer move together with drag deltas.
            web_style.process_input_event(InputEvent::PointerMoved { x: 300.0, y: 180.0 });
            web_style.process_input_event(InputEvent::MouseButton {
                button: 2,
                pressed: true,
            });
            web_style.process_input_event(InputEvent::CameraDrag {
                dx: 24.0,
                dy: -12.0,
            });
            web_style.process_input_event(InputEvent::MouseButton {
                button: 2,
                pressed: false,
            });

            assert_eq!(
                native_style.editor.ui.right_dragging,
                web_style.editor.ui.right_dragging
            );
            assert_eq!(
                native_style.editor.ui.pointer_screen,
                web_style.editor.ui.pointer_screen
            );
            assert_eq!(
                native_style.editor.camera.editor_pan,
                web_style.editor.camera.editor_pan
            );
        });
    }

    #[test]
    fn test_select_mode_marquee_drag_selects_multiple_blocks() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            state.editor.camera.editor_pan = [0.0, 0.0];
            state.editor.objects = vec![
                crate::types::LevelObject {
                    position: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    roundness: 0.18,
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                },
                crate::types::LevelObject {
                    position: [2.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    roundness: 0.18,
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                },
            ];

            let start_x = 0.0;
            let start_y = 0.0;
            let end_x = state.render.gpu.config.width.max(1) as f64;
            let end_y = state.render.gpu.config.height.max(1) as f64;

            state.process_input_event(InputEvent::PointerMoved {
                x: start_x as f64,
                y: start_y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::PointerMoved {
                x: end_x as f64,
                y: end_y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert_eq!(state.editor.ui.selected_block_indices.len(), 2);
            assert!(state.editor.ui.selected_block_indices.contains(&0));
            assert!(state.editor.ui.selected_block_indices.contains(&1));
        });
    }

    #[test]
    fn test_select_mode_marquee_hover_highlights_blocks_during_drag() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            state.editor.camera.editor_pan = [0.0, 0.0];
            state.editor.objects = vec![crate::types::LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            }];

            // Ensure we are not hovering anything initially
            state.editor.ui.hovered_block_index = None;
            state.rebuild_editor_hover_outline_vertices();
            assert!(state
                .render
                .meshes
                .editor_hover_outline
                .draw_data()
                .is_none());

            let start_x = 0.0;
            let start_y = 0.0;
            let end_x = state.render.gpu.config.width.max(1) as f64;
            let end_y = state.render.gpu.config.height.max(1) as f64;

            state.process_input_event(InputEvent::PointerMoved {
                x: start_x,
                y: start_y,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::PointerMoved { x: end_x, y: end_y });

            // Now the hover outline should be populated because the marquee is active and covers the block
            assert!(
                state
                    .render
                    .meshes
                    .editor_hover_outline
                    .draw_data()
                    .is_some(),
                "Hover outline should be populated during marquee drag"
            );

            // Finish marquee
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            // After release, the block is selected, so it shouldn't have a hover outline
            assert!(
                state
                    .render
                    .meshes
                    .editor_hover_outline
                    .draw_data()
                    .is_none(),
                "Hover outline should be cleared after marquee release"
            );
        });
    }

    #[test]
    fn test_select_mode_click_selects_camera_trigger_marker() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            let camera_offset = state.editor.camera_offset();
            let target = state.editor.editor_camera_target() + (-camera_offset.normalize() * 8.0);
            let camera_trigger = CameraTrigger {
                time_seconds: 1.0,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::Linear,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: target.to_array(),
                rotation: state.editor.camera.editor_rotation,
                pitch: state.editor.camera.editor_pitch,
            };
            state
                .editor
                .set_triggers(camera_triggers_to_timed_triggers(std::slice::from_ref(
                    &camera_trigger,
                )));
            state.editor.set_trigger_selected(None);

            let marker_eye = state.editor.camera_trigger_marker_eye(&camera_trigger);
            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let marker_screen = state
                .editor
                .world_to_screen_v(marker_eye, viewport)
                .expect("camera marker should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: marker_screen.x as f64,
                y: marker_screen.y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert_eq!(state.editor.selected_trigger_index(), Some(0));
        });
    }

    #[test]
    fn test_select_mode_marquee_selects_camera_trigger_marker() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            let camera_offset = state.editor.camera_offset();
            let target = state.editor.editor_camera_target() + (-camera_offset.normalize() * 8.0);
            let camera_trigger = CameraTrigger {
                time_seconds: 1.0,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::Linear,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: target.to_array(),
                rotation: state.editor.camera.editor_rotation,
                pitch: state.editor.camera.editor_pitch,
            };
            state
                .editor
                .set_triggers(camera_triggers_to_timed_triggers(std::slice::from_ref(
                    &camera_trigger,
                )));
            state.editor.set_trigger_selected(None);

            let marker_eye = state.editor.camera_trigger_marker_eye(&camera_trigger);
            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let marker_screen = state
                .editor
                .world_to_screen_v(marker_eye, viewport)
                .expect("camera marker should project to the screen");

            let start_x = marker_screen.x as f64 - 24.0;
            let start_y = marker_screen.y as f64 - 24.0;
            let end_x = marker_screen.x as f64 + 24.0;
            let end_y = marker_screen.y as f64 + 24.0;

            state.process_input_event(InputEvent::PointerMoved {
                x: start_x,
                y: start_y,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::PointerMoved { x: end_x, y: end_y });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert_eq!(state.editor.selected_trigger_index(), Some(0));
        });
    }

    #[test]
    fn test_trigger_mode_click_does_not_select_blocks() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Trigger));

            state.editor.objects.push(crate::types::LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            });

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let block_center = glam::Vec3::new(0.5, 0.5, 0.5);
            let block_screen = state
                .editor
                .world_to_screen_v(block_center, viewport)
                .expect("block center should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: block_screen.x as f64,
                y: block_screen.y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert!(state.editor.ui.selected_block_index.is_none());
            assert!(state.editor.ui.selected_block_indices.is_empty());
        });
    }

    #[test]
    fn test_command_chain_undo_redo() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            // 1. Enter Editor
            assert_eq!(state.phase, crate::types::AppPhase::Editor);

            // 2. Place a block
            let initial_count = state.editor.objects.len();
            state.dispatch(AppCommand::TurnRight); // In editor, TurnRight = Place Block
            assert_eq!(state.editor.objects.len(), initial_count + 1);

            // 3. Undo placement
            state.dispatch(AppCommand::EditorUndo);
            assert_eq!(state.editor.objects.len(), initial_count);

            // 4. Redo placement
            state.dispatch(AppCommand::EditorRedo);
            assert_eq!(state.editor.objects.len(), initial_count + 1);
        });
    }

    #[test]
    fn test_complex_command_sequence() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            let initial_count = state.editor.objects.len();

            // Set a specific block type
            state.dispatch(AppCommand::EditorSetBlockId("core/lava".to_string()));

            // Move cursor to a known position
            state.dispatch(AppCommand::NextLevel); // Move X+1

            // Place block at new position
            state.dispatch(AppCommand::TurnRight);
            assert_eq!(state.editor.objects.len(), initial_count + 1);
            let block = state.editor.objects.last().unwrap();
            assert_eq!(block.block_id, "core/lava");
            let pos1 = block.position;

            // Move cursor again
            state.dispatch(AppCommand::NextLevel); // Move X+1
            state.dispatch(AppCommand::TurnRight);
            assert_eq!(state.editor.objects.len(), initial_count + 2);
            let pos2 = state.editor.objects.last().unwrap().position;

            assert!(
                pos1 != pos2,
                "Blocks should be at different positions. Pos1: {:?}, Pos2: {:?}",
                pos1,
                pos2
            );

            // Undo once
            state.dispatch(AppCommand::EditorUndo);
            assert_eq!(state.editor.objects.len(), initial_count + 1);
            assert_eq!(state.editor.objects.last().unwrap().position, pos1);

            // Undo twice
            state.dispatch(AppCommand::EditorUndo);
            assert_eq!(state.editor.objects.len(), initial_count);
        });
    }

    #[test]
    fn test_ctrl_o_toggles_settings_sidebar() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            assert!(!state.editor_show_settings());

            state.process_input_event(InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            });
            state.process_input_event(InputEvent::Key {
                key: "o".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor_show_settings());

            state.process_input_event(InputEvent::Key {
                key: "o".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(!state.editor_show_settings());
        });
    }

    #[test]
    fn test_custom_copy_keybind_replaces_default_combo() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::TurnRight);
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.runtime.interaction.clipboard = None;

            state.dispatch(AppCommand::EditorSetKeybind {
                action: "copy".to_string(),
                slot: 0,
                chord: crate::types::KeyChord::new("b", true, false, false),
            });

            state.process_input_event(InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            });
            state.process_input_event(InputEvent::Key {
                key: "b".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            state.editor.runtime.interaction.clipboard = None;
            state.process_input_event(InputEvent::Key {
                key: "c".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor.runtime.interaction.clipboard.is_none());
        });
    }

    #[test]
    fn test_mapping_helpers_cover_editor_menu_and_selection_gates() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            assert_eq!(
                state.map_modifier_key_to_command("ShiftLeft", true),
                Some(AppCommand::EditorSetShiftHeld(true))
            );
            assert_eq!(
                state.map_modifier_key_to_command("ControlRight", false),
                Some(AppCommand::EditorSetCtrlHeld(false))
            );
            assert_eq!(
                state.map_modifier_key_to_command("Alt", true),
                Some(AppCommand::EditorSetAltHeld(true))
            );
            assert_eq!(state.map_modifier_key_to_command("m", true), None);

            assert!(State::is_modifier_key("Control"));
            assert!(State::is_modifier_key("AltLeft"));
            assert!(!State::is_modifier_key("m"));

            assert!(!state.has_block_selection());
            state.editor.ui.selected_block_index = Some(0);
            assert!(state.has_block_selection());
            state.editor.ui.selected_block_index = None;
            state.editor.ui.selected_block_indices.clear();

            state.phase = AppPhase::Menu;
            assert!(state.map_pan_key_to_commands("w", true).is_empty());

            state.phase = AppPhase::Editor;
            state.editor.ui.ctrl_held = false;
            state.editor.ui.alt_held = false;
            assert_eq!(
                state.map_pan_key_to_commands("w", true),
                vec![AppCommand::EditorSetPanUpHeld(true)]
            );

            state.editor.ui.ctrl_held = true;
            assert!(state.map_pan_key_to_commands("w", true).is_empty());

            let released_pan = state.map_pan_key_to_commands("w", false);
            assert_eq!(released_pan, vec![AppCommand::EditorSetPanUpHeld(false)]);

            state.phase = AppPhase::Editor;
            assert_eq!(
                state.command_for_keybind_action("toggle_settings", true),
                Some(AppCommand::EditorToggleSettings)
            );
            assert_eq!(
                state.command_for_keybind_action("toggle_settings", false),
                None
            );

            assert_eq!(
                state.command_for_keybind_action("toggle_timeline_playback", true),
                Some(AppCommand::EditorToggleTimelinePlayback)
            );
            assert_eq!(
                state.command_for_keybind_action("playtest", true),
                Some(AppCommand::EditorPlaytest)
            );
            assert_eq!(
                state.command_for_keybind_action("remove_block", true),
                Some(AppCommand::EditorRemoveBlock)
            );
            assert_eq!(
                state.command_for_keybind_action("copy", true),
                Some(AppCommand::EditorCopyBlock)
            );
            assert_eq!(
                state.command_for_keybind_action("paste", true),
                Some(AppCommand::EditorPasteBlock)
            );
            assert_eq!(
                state.command_for_keybind_action("duplicate", true),
                Some(AppCommand::EditorDuplicateBlock)
            );
            assert_eq!(
                state.command_for_keybind_action("undo", true),
                Some(AppCommand::EditorUndo)
            );
            assert_eq!(
                state.command_for_keybind_action("redo", true),
                Some(AppCommand::EditorRedo)
            );

            assert_eq!(state.command_for_keybind_action("nudge_up", true), None);
            state.editor.ui.selected_block_index = Some(0);
            assert_eq!(
                state.command_for_keybind_action("nudge_up", true),
                Some(AppCommand::EditorNudgeSelected { dx: 0, dy: 1 })
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_down", true),
                Some(AppCommand::EditorNudgeSelected { dx: 0, dy: -1 })
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_left", true),
                Some(AppCommand::EditorNudgeSelected { dx: -1, dy: 0 })
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_right", true),
                Some(AppCommand::EditorNudgeSelected { dx: 1, dy: 0 })
            );
            assert_eq!(
                state.command_for_keybind_action("timeline_forward", true),
                None
            );
            state.editor.ui.selected_block_index = None;
            assert_eq!(
                state.command_for_keybind_action("timeline_forward", true),
                Some(AppCommand::EditorShiftTimeline(0.1))
            );
            assert_eq!(
                state.command_for_keybind_action("timeline_backward", true),
                Some(AppCommand::EditorShiftTimeline(-0.1))
            );

            assert_eq!(
                state.command_for_keybind_action("escape", true),
                Some(AppCommand::EditorEscape)
            );
            assert_eq!(state.command_for_keybind_action("escape", false), None);
            assert_eq!(
                state.command_for_keybind_action("zoom_in", true),
                Some(AppCommand::EditorAdjustZoom(1.0))
            );
            assert_eq!(
                state.command_for_keybind_action("zoom_out", true),
                Some(AppCommand::EditorAdjustZoom(-1.0))
            );

            state.phase = AppPhase::Menu;
            assert_eq!(
                state.command_for_keybind_action("toggle_editor", true),
                Some(AppCommand::ToggleEditor)
            );
            assert_eq!(
                state.command_for_keybind_action("game_turn", true),
                Some(AppCommand::TurnRight)
            );
            assert_eq!(
                state.command_for_keybind_action("menu_prev_level", true),
                Some(AppCommand::PrevLevel)
            );
            assert_eq!(
                state.command_for_keybind_action("menu_next_level", true),
                Some(AppCommand::NextLevel)
            );

            state.phase = AppPhase::Editor;
            assert_eq!(
                state.command_for_keybind_action("spawn_set", true),
                Some(AppCommand::EditorSetSpawnHere)
            );
            assert_eq!(
                state.command_for_keybind_action("spawn_rotate", true),
                Some(AppCommand::EditorRotateSpawnDirection)
            );

            state.editor.ui.mode = EditorMode::Place;
            assert_eq!(
                state.command_for_keybind_action("toggle_tap_timing", true),
                Some(AppCommand::EditorToggleTapAtPointer)
            );
            state.editor.ui.mode = EditorMode::Select;
            assert_eq!(
                state.command_for_keybind_action("toggle_tap_timing", true),
                Some(AppCommand::EditorSetMode(EditorMode::Timing))
            );
            state.editor.ui.mode = EditorMode::Timing;
            assert_eq!(
                state.command_for_keybind_action("toggle_tap_timing", true),
                None
            );

            assert_eq!(
                state.command_for_keybind_action("add_camera_trigger", true),
                Some(AppCommand::EditorAddCameraTrigger)
            );
            assert_eq!(
                state.command_for_keybind_action("export_obj", true),
                Some(AppCommand::EditorExportBlockObj)
            );
            assert_eq!(
                state.command_for_keybind_action("toggle_perf_overlay", true),
                Some(AppCommand::EditorTogglePerfOverlay)
            );

            assert_eq!(
                state.command_for_keybind_action("mode_select", true),
                Some(AppCommand::EditorSetMode(EditorMode::Select))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_move", true),
                Some(AppCommand::EditorSetMode(EditorMode::Move))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_scale", true),
                Some(AppCommand::EditorSetMode(EditorMode::Scale))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_rotate", true),
                Some(AppCommand::EditorSetMode(EditorMode::Rotate))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_place", true),
                Some(AppCommand::EditorSetMode(EditorMode::Place))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_trigger", true),
                Some(AppCommand::EditorSetMode(EditorMode::Trigger))
            );

            assert_eq!(
                state.command_for_keybind_action("does_not_exist", true),
                None
            );
        });
    }

    #[test]
    fn test_keyboard_capture_branch_handles_non_just_pressed_and_modifier_keys() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::EditorSetKeybindCapture(Some((
                "copy".to_string(),
                0,
            ))));

            state.process_keyboard_input("k", true, false);
            assert!(state.editor_keybind_capture_action().is_some());

            state.process_keyboard_input("Shift", true, true);
            assert!(state.editor_keybind_capture_action().is_some());

            state.process_keyboard_input("m", true, true);
            assert!(state.editor_keybind_capture_action().is_none());
            assert!(
                state
                    .app_settings()
                    .keybinds_for_action("copy")
                    .iter()
                    .any(|chord| chord.key == "m"),
                "captured key should be persisted as a new mapping"
            );
        });
    }

    #[test]
    fn test_dispatch_block_timeline_timing_and_spawn_commands() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::EditorSetSnapToGrid(true));
            state.dispatch(AppCommand::EditorSetSnapStep(0.5));
            state.dispatch(AppCommand::EditorSetSnapRotation(true));
            state.dispatch(AppCommand::EditorSetSnapRotationStep(30.0));
            assert!(state.editor.config.snap_to_grid);
            assert_eq!(state.editor.config.snap_step, 0.5);
            assert!(state.editor.config.snap_rotation);
            assert_eq!(state.editor.config.snap_rotation_step_degrees, 30.0);

            state.dispatch(AppCommand::TurnRight);
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];

            let updated = LevelObject {
                position: [2.0, 1.0, 3.0],
                size: [1.5, 1.0, 0.5],
                rotation_degrees: [5.0, 10.0, 15.0],
                roundness: 0.3,
                block_id: "core/lava".to_string(),
                color_tint: [0.2, 0.4, 0.8],
            };
            state.dispatch(AppCommand::EditorUpdateSelectedBlock(updated.clone()));
            assert_eq!(state.editor.objects[0].position, updated.position);
            assert_eq!(state.editor.objects[0].size, updated.size);
            assert_eq!(state.editor.objects[0].roundness, updated.roundness);
            assert_eq!(state.editor.objects[0].block_id, updated.block_id);

            state.dispatch(AppCommand::EditorCopyBlock);
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            let before_paste = state.editor.objects.len();
            state.dispatch(AppCommand::EditorPasteBlock);
            assert!(state.editor.objects.len() >= before_paste);

            let before_duplicate = state.editor.objects.len();
            state.dispatch(AppCommand::EditorDuplicateBlock);
            assert!(state.editor.objects.len() >= before_duplicate);

            state.dispatch(AppCommand::EditorNudgeSelected { dx: 1, dy: 0 });
            assert!(!state.editor.objects.is_empty());

            state.dispatch(AppCommand::EditorToggleTimelinePlayback);
            state.dispatch(AppCommand::EditorToggleTimelinePlayback);

            state.dispatch(AppCommand::EditorSetTimelineDuration(4.0));
            state.dispatch(AppCommand::EditorSetTimelineTime(1.0));
            state.dispatch(AppCommand::EditorAddTap);
            assert!(!state.editor.timeline.taps.tap_times.is_empty());
            state.dispatch(AppCommand::EditorRemoveTap);
            state.dispatch(AppCommand::EditorClearTaps);
            assert!(state.editor.timeline.taps.tap_times.is_empty());

            state.dispatch(AppCommand::EditorAddTimingPoint {
                time_seconds: 0.5,
                bpm: 120.0,
            });
            assert!(!state.editor.timing.timing_points.is_empty());
            state.dispatch(AppCommand::EditorSetTimingPointTime(0, 0.75));
            state.dispatch(AppCommand::EditorSetTimingPointBpm(0, 140.0));
            state.dispatch(AppCommand::EditorSetTimingPointTimeSignature(0, 3, 4));
            state.dispatch(AppCommand::EditorSetTimingSelected(Some(0)));
            assert_eq!(state.editor.timing.timing_selected_index, Some(0));
            state.dispatch(AppCommand::EditorRemoveTimingPoint(0));

            state.dispatch(AppCommand::EditorBpmTap);
            state.dispatch(AppCommand::EditorBpmTapReset);

            state.editor.ui.cursor = [7.0, 0.0, 9.0];
            state.dispatch(AppCommand::EditorSetSpawnHere);
            assert_eq!(state.editor.spawn.position, [7.0, 0.0, 9.0]);
            let old_dir = state.editor.spawn.direction;
            state.dispatch(AppCommand::EditorRotateSpawnDirection);
            assert_ne!(state.editor.spawn.direction, old_dir);

            state.dispatch(AppCommand::EditorRemoveBlock);
        });
    }

    #[test]
    fn test_dispatch_additional_command_variants_update_editor_state() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::EditorSetShowMetadata(true));
            assert!(state.editor_show_metadata());

            state.dispatch(AppCommand::EditorSetShowImport(true));
            assert!(state.editor_show_import());
            state.dispatch(AppCommand::EditorSetImportText("abc".to_string()));
            assert_eq!(state.editor_import_text(), "abc");

            state.dispatch(AppCommand::EditorSetShowSettings(true));
            assert!(state.editor_show_settings());
            state.dispatch(AppCommand::EditorSetSettingsSection(
                SettingsSection::Keybinds,
            ));
            assert_eq!(state.editor_settings_section(), SettingsSection::Keybinds);

            state.dispatch(AppCommand::EditorRenameLevel(
                "CmdDispatchLevel".to_string(),
            ));
            assert_eq!(
                state.editor_level_name().as_deref(),
                Some("CmdDispatchLevel")
            );

            state.dispatch(AppCommand::EditorUpdateMusic(MusicMetadata {
                source: "dispatch.mp3".to_string(),
                ..MusicMetadata::default()
            }));
            assert_eq!(state.editor_music_metadata().source, "dispatch.mp3");

            state.dispatch(AppCommand::EditorSetTimelineDuration(5.0));
            state.dispatch(AppCommand::EditorSetTimelineTime(1.25));
            assert_eq!(state.editor_timeline_duration_seconds(), 5.0);
            assert_eq!(state.editor_timeline_time_seconds(), 1.25);

            state.dispatch(AppCommand::EditorSetPlaybackSpeed(1.5));
            assert_eq!(state.editor_playback_speed(), 1.5);
            state.dispatch(AppCommand::EditorSetWaveformZoom(3.5));
            state.dispatch(AppCommand::EditorSetWaveformScroll(1.25));
            assert_eq!(state.editor_waveform_zoom(), 3.5);
            assert_eq!(state.editor_waveform_scroll(), 1.25);

            state.dispatch(AppCommand::EditorSetShiftHeld(true));
            state.dispatch(AppCommand::EditorSetCtrlHeld(true));
            state.dispatch(AppCommand::EditorSetAltHeld(true));
            state.dispatch(AppCommand::EditorSetPanUpHeld(true));
            state.dispatch(AppCommand::EditorSetPanDownHeld(true));
            state.dispatch(AppCommand::EditorSetPanLeftHeld(true));
            state.dispatch(AppCommand::EditorSetPanRightHeld(true));
            assert!(state.editor.ui.shift_held);
            assert!(state.editor.ui.ctrl_held);
            assert!(state.editor.ui.alt_held);
            assert!(state.editor.ui.pan_up_held);
            assert!(state.editor.ui.pan_down_held);
            assert!(state.editor.ui.pan_left_held);
            assert!(state.editor.ui.pan_right_held);

            state.dispatch(AppCommand::ResizeSurface {
                width: 1024,
                height: 576,
            });
            assert_eq!(state.render.gpu.config.width, 1024);
            assert_eq!(state.render.gpu.config.height, 576);

            let trigger = TimedTrigger {
                time_seconds: 0.5,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Object { object_id: 0 },
                action: TimedTriggerAction::MoveTo {
                    position: [1.0, 0.0, 0.0],
                },
            };
            state.dispatch(AppCommand::EditorAddTrigger(trigger.clone()));
            assert_eq!(state.editor_triggers().len(), 1);

            state.dispatch(AppCommand::EditorUpdateTrigger(
                0,
                TimedTrigger {
                    time_seconds: 0.75,
                    ..trigger
                },
            ));
            assert_eq!(state.editor_triggers()[0].time_seconds, 0.75);

            state.dispatch(AppCommand::EditorSetTriggerSelected(Some(0)));
            assert_eq!(state.editor_selected_trigger_index(), Some(0));
            state.dispatch(AppCommand::EditorRemoveTrigger(0));
            assert!(state.editor_triggers().is_empty());

            state.dispatch(AppCommand::EditorSetSimulateTriggerHitboxes(true));
            assert!(state.editor_simulate_trigger_hitboxes());

            state.dispatch(AppCommand::EditorSetMode(EditorMode::Timing));
            assert_eq!(state.editor_mode(), EditorMode::Timing);
            state.dispatch(AppCommand::EditorSetMode(EditorMode::Place));
            assert_eq!(state.editor_mode(), EditorMode::Place);

            state.dispatch(AppCommand::EditorSetKeybindCapture(Some((
                "copy".to_string(),
                0,
            ))));
            assert!(state.editor_keybind_capture_action().is_some());

            state.dispatch(AppCommand::EditorSetKeybind {
                action: "copy".to_string(),
                slot: 0,
                chord: KeyChord::new("x", true, false, false),
            });
            assert!(!state.app_settings().keybinds_for_action("copy").is_empty());

            state.dispatch(AppCommand::EditorClearKeybindSlot {
                action: "copy".to_string(),
                slot: 0,
            });
            state.dispatch(AppCommand::EditorResetKeybind("copy".to_string()));
            state.dispatch(AppCommand::EditorResetKeybinds);
        });
    }

    #[test]
    fn test_keyboard_keybind_capture_sets_or_cancels_capture() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::EditorSetKeybindCapture(Some((
                "copy".to_string(),
                0,
            ))));
            state.process_input_event(InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            });
            state.process_input_event(InputEvent::Key {
                key: "k".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor_keybind_capture_action().is_none());
            assert!(state
                .app_settings()
                .keybinds_for_action("copy")
                .iter()
                .any(|chord| chord.key == "k" && chord.ctrl));

            state.dispatch(AppCommand::EditorSetKeybindCapture(Some((
                "paste".to_string(),
                0,
            ))));
            state.process_input_event(InputEvent::Key {
                key: "Escape".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert!(state.editor_keybind_capture_action().is_none());
        });
    }
}
