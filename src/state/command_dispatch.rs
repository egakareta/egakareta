use super::State;
use crate::commands::AppCommand;
use crate::types::EditorMode;

impl State {
    /// Central dispatcher: every `AppCommand` is routed here.
    /// This is the **only** place that maps intent → mutation,
    /// making it easy to log, replay, or test commands in isolation.
    pub fn execute_command(&mut self, cmd: AppCommand) {
        match cmd {
            // ── Navigation / Phase ──────────────────────────────────
            AppCommand::TurnRight => self.turn_right(),
            AppCommand::NextLevel => self.next_level(),
            AppCommand::PrevLevel => self.prev_level(),
            AppCommand::ToggleEditor => self.toggle_editor(),
            AppCommand::BackToMenu => self.back_to_menu(),

            // ── Gameplay ────────────────────────────────────────────
            AppCommand::RestartLevel => self.restart_level(),

            // ── Editor – mode switching ─────────────────────────────
            AppCommand::EditorModeSelect => self.set_editor_mode(EditorMode::Select),
            AppCommand::EditorModePlace => self.set_editor_mode(EditorMode::Place),
            AppCommand::EditorModeTiming => self.set_editor_mode(EditorMode::Timing),

            // ── Editor – block ops ──────────────────────────────────
            AppCommand::EditorPlaceBlock => self.place_editor_block(),
            AppCommand::EditorRemoveBlock => self.editor_remove_block(),
            AppCommand::EditorDuplicateBlock => self.editor_duplicate_selected_block_in_place(),
            AppCommand::EditorCopyBlock => self.editor_copy_block(),
            AppCommand::EditorPasteBlock => self.editor_paste_block(),
            AppCommand::EditorSetBlockId(id) => self.set_editor_block_id(id),

            // ── Editor – selection / transform ──────────────────────
            AppCommand::EditorNudgeSelected { dx, dy } => {
                self.editor_nudge_selected_blocks(dx, dy);
            }

            // ── Editor – timeline / playback ────────────────────────
            AppCommand::EditorToggleTimelinePlayback => self.toggle_editor_timeline_playback(),
            AppCommand::EditorShiftTimeline(delta) => self.editor_shift_timeline_time(delta),
            AppCommand::EditorToggleTapAtPointer => self.editor_add_tap_at_pointer_position(),
            AppCommand::EditorPlaytest => self.editor_playtest(),

            // ── Editor – spawn ──────────────────────────────────────
            AppCommand::EditorSetSpawnHere => self.editor_set_spawn_here(),
            AppCommand::EditorRotateSpawnDirection => self.editor_rotate_spawn_direction(),

            // ── Editor – history ────────────────────────────────────
            AppCommand::EditorUndo => self.editor_undo(),
            AppCommand::EditorRedo => self.editor_redo(),

            // ── Editor – zoom ───────────────────────────────────────
            AppCommand::EditorAdjustZoom(delta) => self.adjust_editor_zoom(delta),

            // ── Editor – misc ───────────────────────────────────────
            AppCommand::EditorTogglePerfOverlay => self.toggle_editor_perf_overlay(),
            AppCommand::EditorExportBlockObj => self.trigger_selected_block_obj_export(),

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
        // Modifier key state is tracked directly (not as commands)
        // because it's held state, not a discrete action.
        if key == "Shift" {
            self.set_editor_shift_held(pressed);
            return;
        }
        if key == "Control" || key == "ControlLeft" || key == "ControlRight" {
            self.set_editor_ctrl_held(pressed);
            return;
        }
        if key == "Alt" || key == "AltLeft" || key == "AltRight" {
            self.set_editor_alt_held(pressed);
            return;
        }

        // Key-release tracking for held-state keys (WASD pan).
        if !pressed {
            match key {
                "w" | "W" => self.set_editor_pan_up_held(false),
                "s" | "S" => self.set_editor_pan_down_held(false),
                "a" | "A" => self.set_editor_pan_left_held(false),
                "d" | "D" => self.set_editor_pan_right_held(false),
                _ => {}
            }
            return;
        }

        // WASD pan keys are continuous held-state, not discrete commands.
        // Handle them before command dispatch.
        match key {
            "w" | "W" if self.is_editor() => {
                self.set_editor_pan_up_held(true);
                return;
            }
            "s" | "S" if self.is_editor() => {
                self.set_editor_pan_down_held(true);
                return;
            }
            "a" | "A" if self.is_editor() => {
                self.set_editor_pan_left_held(true);
                return;
            }
            "d" | "D" if self.is_editor() && !self.editor.ui.ctrl_held => {
                self.set_editor_pan_right_held(true);
                return;
            }
            _ => {}
        }

        // Map key-press to command(s).
        if let Some(cmd) = self.map_key_to_command(key, just_pressed) {
            self.execute_command(cmd);
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
                    Some(AppCommand::EditorModeSelect)
                } else {
                    None
                }
            }
            "e" | "E" => {
                if just_pressed {
                    if self.is_editor() {
                        Some(AppCommand::EditorModePlace)
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
                        Some(AppCommand::EditorModeTiming)
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
                    Some(AppCommand::EditorSetBlockId("core/standard".to_string()))
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
                self.handle_mouse_button(button, pressed);
            }
            InputEvent::PrimaryClick { x, y } => {
                self.handle_primary_click(x, y);
            }
            InputEvent::PointerMoved { x, y } => {
                self.update_editor_cursor_from_screen(x, y);
            }
            InputEvent::CameraDrag { dx, dy } => {
                self.drag_editor_camera_by_pixels(dx, dy);
            }
            InputEvent::Zoom(delta) => {
                self.execute_command(AppCommand::EditorAdjustZoom(delta));
            }
            InputEvent::Resize { width, height } => {
                self.resize_surface(crate::types::PhysicalSize::new(width, height));
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
            let mut state = State::new_test().await;

            // Initial state should be Menu
            assert_eq!(state.phase, AppPhase::Menu);

            // ToggleEditor from Menu should go to Editor
            state.execute_command(AppCommand::ToggleEditor);
            assert_eq!(state.phase, AppPhase::Editor);

            // BackToMenu from Editor should go to Menu
            state.execute_command(AppCommand::BackToMenu);
            assert_eq!(state.phase, AppPhase::Menu);
        });
    }

    #[test]
    fn test_command_routing_editor_modes() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.execute_command(AppCommand::ToggleEditor);

            state.execute_command(AppCommand::EditorModeSelect);
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Select);

            state.execute_command(AppCommand::EditorModeTiming);
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Timing);

            state.execute_command(AppCommand::EditorModePlace);
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);
        });
    }

    #[test]
    fn test_command_routing_editor_ops() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.execute_command(AppCommand::ToggleEditor);

            let initial_zoom = state.editor.camera.editor_zoom;
            state.execute_command(AppCommand::EditorAdjustZoom(0.5));
            assert!(state.editor.camera.editor_zoom > initial_zoom);

            state.execute_command(AppCommand::EditorSetBlockId("core/lava".to_string()));
            assert_eq!(state.editor.config.selected_block_id, "core/lava");
        });
    }
}
