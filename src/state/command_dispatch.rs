use super::State;
use crate::commands::AppCommand;
use crate::types::EditorMode;

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
            AppCommand::EditorSetCameraOrientation { rotation, pitch } => {
                self.set_editor_camera_orientation(rotation, pitch)
            }

            // ── Editor – misc ───────────────────────────────────────
            AppCommand::EditorTogglePerfOverlay => self.toggle_editor_perf_overlay(),
            AppCommand::EditorExportBlockObj => self.trigger_selected_block_obj_export(),

            // ── Editor – UI / Session ───────────────────────────────
            AppCommand::EditorLoadLevel(name) => self.load_builtin_level_into_editor(&name),
            AppCommand::EditorRenameLevel(name) => self.set_editor_level_name(name),
            AppCommand::EditorExportLevel => self.trigger_level_export(),
            AppCommand::EditorSetShowMetadata(show) => self.set_editor_show_metadata(show),
            AppCommand::EditorSetShowImport(show) => self.set_editor_show_import(show),
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

        if let Some(cmd) = self.map_pan_key_to_command(key, pressed) {
            self.dispatch(cmd);
            return;
        }

        if !pressed {
            return;
        }

        // Map key-press to command(s).
        if let Some(cmd) = self.map_key_to_command(key, just_pressed) {
            self.dispatch(cmd);
        }
    }

    fn map_modifier_key_to_command(&self, key: &str, pressed: bool) -> Option<AppCommand> {
        match key {
            "Shift" => Some(AppCommand::EditorSetShiftHeld(pressed)),
            "Control" | "ControlLeft" | "ControlRight" => {
                Some(AppCommand::EditorSetCtrlHeld(pressed))
            }
            "Alt" | "AltLeft" | "AltRight" => Some(AppCommand::EditorSetAltHeld(pressed)),
            _ => None,
        }
    }

    fn map_pan_key_to_command(&self, key: &str, pressed: bool) -> Option<AppCommand> {
        match key {
            "w" | "W" => Some(AppCommand::EditorSetPanUpHeld(pressed && self.is_editor())),
            "s" | "S" => Some(AppCommand::EditorSetPanDownHeld(
                pressed && self.is_editor(),
            )),
            "a" | "A" => Some(AppCommand::EditorSetPanLeftHeld(
                pressed && self.is_editor(),
            )),
            "d" | "D" => {
                if pressed {
                    if self.is_editor() && !self.editor.ui.ctrl_held {
                        Some(AppCommand::EditorSetPanRightHeld(true))
                    } else {
                        None
                    }
                } else {
                    Some(AppCommand::EditorSetPanRightHeld(false))
                }
            }
            _ => None,
        }
    }

    /// Pure mapping from key string + modifiers → command.
    /// Returns `None` for keys that have no command binding.
    fn map_key_to_command(&self, key: &str, just_pressed: bool) -> Option<AppCommand> {
        match key {
            "ArrowUp" => {
                if self.is_editor() {
                    if self.has_block_selection() {
                        Some(AppCommand::EditorNudgeSelected { dx: 0, dy: 1 })
                    } else {
                        Some(AppCommand::EditorShiftTimeline(0.1))
                    }
                } else if just_pressed {
                    Some(AppCommand::TurnRight)
                } else {
                    None
                }
            }
            "ArrowDown" => {
                if self.is_editor() {
                    if self.has_block_selection() {
                        Some(AppCommand::EditorNudgeSelected { dx: 0, dy: -1 })
                    } else {
                        Some(AppCommand::EditorShiftTimeline(-0.1))
                    }
                } else {
                    None
                }
            }
            "ArrowRight" => {
                if self.is_editor() {
                    if self.has_block_selection() {
                        Some(AppCommand::EditorNudgeSelected { dx: 1, dy: 0 })
                    } else {
                        Some(AppCommand::EditorShiftTimeline(0.1))
                    }
                } else if just_pressed {
                    Some(AppCommand::NextLevel)
                } else {
                    None
                }
            }
            "ArrowLeft" => {
                if self.is_editor() {
                    if self.has_block_selection() {
                        Some(AppCommand::EditorNudgeSelected { dx: -1, dy: 0 })
                    } else {
                        Some(AppCommand::EditorShiftTimeline(-0.1))
                    }
                } else if just_pressed {
                    Some(AppCommand::PrevLevel)
                } else {
                    None
                }
            }
            "w" | "W" => {
                // Editor pan handled above; non-editor falls through here.
                if just_pressed {
                    Some(AppCommand::TurnRight)
                } else {
                    None
                }
            }
            "s" | "S" => {
                // Editor pan handled above; nothing else for S.
                None
            }
            " " | "Space" => {
                if just_pressed {
                    if self.is_editor() {
                        Some(AppCommand::EditorToggleTimelinePlayback)
                    } else {
                        Some(AppCommand::TurnRight)
                    }
                } else {
                    None
                }
            }
            "d" | "D" => {
                // In editor with Ctrl: duplicate (pan handled above).
                if self.is_editor() && self.editor.ui.ctrl_held && just_pressed {
                    Some(AppCommand::EditorDuplicateBlock)
                } else if !self.is_editor() && just_pressed {
                    Some(AppCommand::NextLevel)
                } else {
                    None
                }
            }
            "a" | "A" => {
                // Editor pan handled above; non-editor falls through.
                if !self.is_editor() && just_pressed {
                    Some(AppCommand::PrevLevel)
                } else {
                    None
                }
            }
            "Enter" => {
                if just_pressed {
                    Some(AppCommand::EditorPlaytest)
                } else {
                    None
                }
            }
            "Backspace" | "Delete" => {
                if just_pressed {
                    Some(AppCommand::EditorRemoveBlock)
                } else {
                    None
                }
            }
            "Escape" => {
                if just_pressed {
                    Some(AppCommand::EditorEscape)
                } else {
                    None
                }
            }
            "q" | "Q" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetMode(EditorMode::Select))
                } else {
                    None
                }
            }
            "e" | "E" => {
                if just_pressed {
                    if self.is_editor() {
                        Some(AppCommand::EditorSetMode(EditorMode::Place))
                    } else {
                        Some(AppCommand::ToggleEditor)
                    }
                } else {
                    None
                }
            }
            "p" | "P" => {
                if just_pressed {
                    Some(AppCommand::EditorSetSpawnHere)
                } else {
                    None
                }
            }
            "r" | "R" => {
                if just_pressed {
                    Some(AppCommand::EditorRotateSpawnDirection)
                } else {
                    None
                }
            }
            "t" | "T" => {
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
            "+" | "=" => {
                if just_pressed {
                    Some(AppCommand::EditorAdjustZoom(1.0))
                } else {
                    None
                }
            }
            "-" | "_" => {
                if just_pressed {
                    Some(AppCommand::EditorAdjustZoom(-1.0))
                } else {
                    None
                }
            }
            "1" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetBlockId("core/stone".to_string()))
                } else {
                    None
                }
            }
            "2" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetBlockId("core/grass".to_string()))
                } else {
                    None
                }
            }
            "3" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetBlockId("core/dirt".to_string()))
                } else {
                    None
                }
            }
            "4" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::EditorSetBlockId("core/void".to_string()))
                } else {
                    None
                }
            }
            "o" | "O" => {
                if self.is_editor()
                    && self.editor.ui.ctrl_held
                    && self.editor.ui.shift_held
                    && self.editor.ui.alt_held
                    && just_pressed
                {
                    Some(AppCommand::EditorExportBlockObj)
                } else {
                    None
                }
            }
            "F12" => {
                if self.editor.ui.ctrl_held
                    && self.editor.ui.shift_held
                    && self.editor.ui.alt_held
                    && just_pressed
                {
                    Some(AppCommand::EditorTogglePerfOverlay)
                } else {
                    None
                }
            }
            "c" | "C" => {
                if self.is_editor() && self.editor.ui.ctrl_held && just_pressed {
                    Some(AppCommand::EditorCopyBlock)
                } else {
                    None
                }
            }
            "v" | "V" => {
                if self.is_editor() && self.editor.ui.ctrl_held && just_pressed {
                    Some(AppCommand::EditorPasteBlock)
                } else {
                    None
                }
            }
            "z" | "Z" => {
                if self.is_editor() && self.editor.ui.ctrl_held && just_pressed {
                    Some(AppCommand::EditorUndo)
                } else {
                    None
                }
            }
            "y" | "Y" => {
                if self.is_editor() && self.editor.ui.ctrl_held && just_pressed {
                    Some(AppCommand::EditorRedo)
                } else {
                    None
                }
            }
            _ => None,
        }
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
    use crate::types::AppPhase;

    #[test]
    fn test_command_routing_navigation() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;

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
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Select);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Timing));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Timing);

            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Place));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);
        });
    }

    #[test]
    fn test_command_routing_editor_ops() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

            let initial_zoom = state.editor.camera.editor_zoom;
            state.dispatch(AppCommand::EditorAdjustZoom(0.5));
            assert!(state.editor.camera.editor_zoom > initial_zoom);

            state.dispatch(AppCommand::EditorSetBlockId("core/lava".to_string()));
            assert_eq!(state.editor.config.selected_block_id, "core/lava");
        });
    }

    #[test]
    fn test_timeline_shift_updates_preview() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

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
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

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
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

            // Zoom
            let initial_zoom = state.editor.camera.editor_zoom;
            state.process_input_event(InputEvent::Zoom(1.0));
            assert!(state.editor.camera.editor_zoom > initial_zoom);

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

            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

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
                let mut state = match State::new_test().await {
                    Some(s) => s,
                    None => return,
                };
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

            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

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
    fn test_native_web_primary_click_event_sequences_are_equivalent() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut native_style = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            native_style.phase = AppPhase::Menu;
            native_style.dispatch(AppCommand::ToggleEditor);

            let mut web_style = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            web_style.phase = AppPhase::Menu;
            web_style.dispatch(AppCommand::ToggleEditor);

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

            let mut native_style = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            native_style.phase = AppPhase::Menu;
            native_style.dispatch(AppCommand::ToggleEditor);

            let mut web_style = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            web_style.phase = AppPhase::Menu;
            web_style.dispatch(AppCommand::ToggleEditor);

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

            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);
            state.dispatch(AppCommand::EditorSetMode(crate::types::EditorMode::Select));

            state.editor.camera.editor_pan = [0.0, 0.0];
            state.editor.objects = vec![
                crate::types::LevelObject {
                    position: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: 0.0,
                    roundness: 0.18,
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                },
                crate::types::LevelObject {
                    position: [2.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: 0.0,
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
    fn test_command_chain_undo_redo() {
        pollster::block_on(async {
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;

            // 1. Enter Editor
            state.dispatch(AppCommand::ToggleEditor);
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
            let mut state = match State::new_test().await {
                Some(s) => s,
                None => return,
            };
            state.phase = AppPhase::Menu;
            state.dispatch(AppCommand::ToggleEditor);

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
}
