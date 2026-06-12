/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use crate::commands::AppCommand;
use crate::editor_domain::TimingDivisionDirection;
use crate::state::editor_command::EditorCommand;
use crate::types::{normalize_binding_key, AppPhase, EditorMode, KeyChord};

impl State {
    /// Central dispatcher: every `AppCommand` is routed here.
    /// This is the **only** place that maps intent → mutation,
    /// making it easy to log, replay, or test commands in isolation.
    pub(crate) fn dispatch(&mut self, cmd: AppCommand) {
        puffin::profile_scope!("CommandDispatch");
        match cmd {
            // ── Navigation / Phase ──────────────────────────────────
            AppCommand::TurnRight => self.turn_right(),
            AppCommand::NextLevel => self.next_level(),
            AppCommand::PrevLevel => self.prev_level(),
            AppCommand::ToggleEditor => self.toggle_editor(),
            AppCommand::GameResume => self.resume_game(),
            AppCommand::GameRestartLevel => self.restart_game_from_pause(),
            AppCommand::GameSetPracticeMode(enabled) => self.set_practice_mode_enabled(enabled),
            AppCommand::GameSetPracticeCheckpoint => self.set_practice_checkpoint(),
            AppCommand::GameRemovePracticeCheckpoint => self.remove_practice_checkpoint(),
            AppCommand::GameQuitToMenu => self.quit_game_from_pause(),

            // ── Auth ────────────────────────────────────────────────
            AppCommand::AuthSubmitSignIn => self.submit_auth_sign_in(),
            AppCommand::AuthSignOut => self.sign_out_auth_session(),
            AppCommand::AuthOpenSignup => self.open_auth_signup_page(),

            AppCommand::ResizeSurface { width, height } => {
                self.resize_surface(crate::types::PhysicalSize::new(width, height));
            }
            // ── Editor ────────────────────────────────────────────
            AppCommand::Editor(cmd) => self.dispatch_editor(cmd),
        }
    }

    pub(super) fn toggle_game_pause(&mut self) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        if self.session.game_paused {
            self.resume_game();
        } else {
            self.pause_game();
        }
    }

    fn pause_game(&mut self) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = true;
        self.clear_pending_gameplay_inputs();
        self.pause_game_audio();
    }

    fn resume_game(&mut self) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        if self.session.game_paused {
            self.session.game_paused = false;
            if !self.start_gameplay_if_needed() {
                self.resume_game_audio();
            }
        }
    }

    fn restart_game_from_pause(&mut self) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = false;
        self.restart_level();
    }

    fn set_practice_mode_enabled(&mut self, enabled: bool) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.practice_mode_enabled = enabled;
        self.session.practice_checkpoints.clear();

        if enabled {
            self.resume_game();
            return;
        }

        self.rebuild_practice_checkpoint_vertices();
        self.restart_level();
        self.resume_game();
    }

    fn set_practice_checkpoint(&mut self) {
        if self.phase != crate::types::AppPhase::Playing
            || self.session.playtesting_editor
            || !self.session.practice_mode_enabled
            || self.session.game_paused
            || !self.gameplay.state.started
            || self.gameplay.state.game_over
            || self.gameplay.state.level_complete
        {
            return;
        }

        self.session
            .practice_checkpoints
            .push(super::PracticeCheckpoint {
                gameplay: self.gameplay.state.checkpoint_state(),
                trigger_base_objects: self.session.playing_trigger_base_objects.clone(),
            });
        self.rebuild_practice_checkpoint_vertices();
    }

    fn remove_practice_checkpoint(&mut self) {
        if self.phase != crate::types::AppPhase::Playing
            || self.session.playtesting_editor
            || !self.session.practice_mode_enabled
            || self.session.game_paused
        {
            return;
        }

        let _ = self.session.practice_checkpoints.pop();
        self.rebuild_practice_checkpoint_vertices();
    }

    fn quit_game_from_pause(&mut self) {
        if self.phase != crate::types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = false;
        self.back_to_menu();
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
                self.dispatch_editor(EditorCommand::SetKeybindCapture(None));
                return;
            }

            let chord = KeyChord::new(
                normalize_binding_key(key),
                self.editor.ui.ctrl_held,
                self.editor.ui.shift_held,
                self.editor.ui.alt_held,
            );
            self.dispatch_editor(EditorCommand::SetKeybind {
                action,
                slot,
                chord,
            });
            self.dispatch_editor(EditorCommand::SetKeybindCapture(None));
            return;
        }

        // Close settings sidebar on Escape when it's open (before any other Escape handling)
        if key == "Escape" && self.editor_show_settings() {
            self.dispatch_editor(EditorCommand::SetShowSettings(false));
            return;
        }

        for cmd in self.map_key_to_commands(key, just_pressed, pressed) {
            self.dispatch(cmd);
        }
    }

    fn map_modifier_key_to_command(&self, key: &str, pressed: bool) -> Option<AppCommand> {
        match key {
            "Shift" | "ShiftLeft" | "ShiftRight" => {
                Some(AppCommand::Editor(EditorCommand::SetShiftHeld(pressed)))
            }
            "Control" | "ControlLeft" | "ControlRight" => {
                Some(AppCommand::Editor(EditorCommand::SetCtrlHeld(pressed)))
            }
            "Alt" | "AltLeft" | "AltRight" => {
                Some(AppCommand::Editor(EditorCommand::SetAltHeld(pressed)))
            }
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
                    "pan_up" => {
                        commands.push(AppCommand::Editor(EditorCommand::SetPanUpHeld(pressed)))
                    }
                    "pan_down" => {
                        commands.push(AppCommand::Editor(EditorCommand::SetPanDownHeld(pressed)))
                    }
                    "pan_left" => {
                        commands.push(AppCommand::Editor(EditorCommand::SetPanLeftHeld(pressed)))
                    }
                    "pan_right" => {
                        commands.push(AppCommand::Editor(EditorCommand::SetPanRightHeld(pressed)))
                    }
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
                // Allow toggle in menu, editor, and paused states; block during active gameplay
                if just_pressed && (self.phase != AppPhase::Playing || self.is_game_paused()) {
                    Some(AppCommand::Editor(EditorCommand::ToggleSettings))
                } else {
                    None
                }
            }
            "toggle_timeline_playback" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback))
                } else {
                    None
                }
            }
            "playtest" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::Playtest))
                } else {
                    None
                }
            }
            "remove_block" => {
                if self.is_editor() && just_pressed {
                    if self.editor_effective_mode_for_playback() == EditorMode::Tapping {
                        self.editor.selected_tap().map(|(_, time_seconds, _)| {
                            AppCommand::Editor(EditorCommand::RemoveTapAt(time_seconds))
                        })
                    } else {
                        Some(AppCommand::Editor(EditorCommand::RemoveBlock))
                    }
                } else {
                    None
                }
            }
            "copy" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::CopyBlock))
                } else {
                    None
                }
            }
            "paste" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::PasteBlock))
                } else {
                    None
                }
            }
            "duplicate" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::DuplicateBlock))
                } else {
                    None
                }
            }
            "undo" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::Undo))
                } else {
                    None
                }
            }
            "redo" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::Redo))
                } else {
                    None
                }
            }
            "nudge_up" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                        dx: 0,
                        dy: 1,
                    }))
                } else {
                    None
                }
            }
            "nudge_down" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                        dx: 0,
                        dy: -1,
                    }))
                } else {
                    None
                }
            }
            "nudge_left" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                        dx: -1,
                        dy: 0,
                    }))
                } else {
                    None
                }
            }
            "nudge_right" => {
                if self.is_editor() && self.has_block_selection() {
                    Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                        dx: 1,
                        dy: 0,
                    }))
                } else {
                    None
                }
            }
            "snap_selection_to_grid" => {
                if self.is_editor()
                    && just_pressed
                    && (self.has_block_selection() || self.editor.selected_tap().is_some())
                {
                    Some(AppCommand::Editor(EditorCommand::SnapSelectionToGrid))
                } else {
                    None
                }
            }
            "pick_selected_block" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::PickSelectedBlock))
                } else {
                    None
                }
            }
            "select_recent_block_1" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SelectRecentBlock(0)))
                } else {
                    None
                }
            }
            "select_recent_block_2" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SelectRecentBlock(1)))
                } else {
                    None
                }
            }
            "select_recent_block_3" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SelectRecentBlock(2)))
                } else {
                    None
                }
            }
            "select_recent_block_4" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SelectRecentBlock(3)))
                } else {
                    None
                }
            }
            "focus_camera_target" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::FocusCameraTarget))
                } else {
                    None
                }
            }
            "add_transform_trigger" => {
                if self.is_editor() && just_pressed && self.has_block_selection() {
                    Some(AppCommand::Editor(
                        EditorCommand::BeginTransformTriggerCapture,
                    ))
                } else {
                    None
                }
            }
            "timeline_forward" => {
                if self.is_editor() && !self.has_block_selection() {
                    self.timeline_shift_to_division_command(TimingDivisionDirection::Forward)
                } else {
                    None
                }
            }
            "timeline_backward" => {
                if self.is_editor() && !self.has_block_selection() {
                    self.timeline_shift_to_division_command(TimingDivisionDirection::Backward)
                } else {
                    None
                }
            }
            "escape" => {
                if just_pressed {
                    Some(AppCommand::Editor(EditorCommand::Escape))
                } else {
                    None
                }
            }
            "zoom_in" => {
                if just_pressed {
                    Some(AppCommand::Editor(EditorCommand::AdjustZoom(1.0)))
                } else {
                    None
                }
            }
            "zoom_out" => {
                if just_pressed {
                    Some(AppCommand::Editor(EditorCommand::AdjustZoom(-1.0)))
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
            "toggle_place_window" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::TogglePlaceWindow))
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
            "practice_checkpoint" => {
                if self.is_practice_mode_enabled() && just_pressed {
                    Some(AppCommand::GameSetPracticeCheckpoint)
                } else {
                    None
                }
            }
            "practice_remove_checkpoint" => {
                if self.is_practice_mode_enabled() && just_pressed {
                    Some(AppCommand::GameRemovePracticeCheckpoint)
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
                    Some(AppCommand::Editor(EditorCommand::SetSpawnHere))
                } else {
                    None
                }
            }
            "spawn_rotate" => {
                if just_pressed {
                    if self.editor_effective_mode_for_playback() == EditorMode::Place {
                        Some(AppCommand::Editor(EditorCommand::RotatePlacementPreview))
                    } else {
                        Some(AppCommand::Editor(EditorCommand::RotateSpawnDirection))
                    }
                } else {
                    None
                }
            }
            "add_camera_trigger" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::AddCameraTrigger))
                } else {
                    None
                }
            }
            "add_camera_follow_trigger" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::AddCameraFollowTrigger))
                } else {
                    None
                }
            }
            "export_obj" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::ExportBlockObj))
                } else {
                    None
                }
            }
            "toggle_hitbox_visualization" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::ToggleHitboxVisualization))
                } else {
                    None
                }
            }
            "toggle_perf_overlay" => {
                if just_pressed {
                    Some(AppCommand::Editor(EditorCommand::TogglePerfOverlay))
                } else {
                    None
                }
            }
            "mode_select" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Select,
                    )))
                } else {
                    None
                }
            }
            "mode_move" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(EditorMode::Move)))
                } else {
                    None
                }
            }
            "mode_scale" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Scale,
                    )))
                } else {
                    None
                }
            }
            "mode_rotate" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Rotate,
                    )))
                } else {
                    None
                }
            }
            "tab_compose" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Place,
                    )))
                } else {
                    None
                }
            }
            "tab_timing" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Timing,
                    )))
                } else {
                    None
                }
            }
            "tab_tapping" => {
                if self.is_editor() && just_pressed {
                    Some(AppCommand::Editor(EditorCommand::SetMode(
                        EditorMode::Tapping,
                    )))
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
                self.dispatch_editor(EditorCommand::MouseButton { button, pressed });
            }
            InputEvent::PrimaryClick { x, y } => {
                self.dispatch_editor(EditorCommand::PrimaryClick { x, y });
            }
            InputEvent::PointerMoved { x, y } => {
                self.dispatch_editor(EditorCommand::PointerMoved { x, y });
            }
            InputEvent::CameraDrag { dx, dy } => {
                self.dispatch_editor(EditorCommand::CameraDrag { dx, dy });
            }
            InputEvent::Zoom(delta) => {
                self.dispatch_editor(EditorCommand::AdjustZoom(delta));
            }
            InputEvent::Resize { width, height } => {
                self.dispatch(AppCommand::ResizeSurface { width, height });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::commands::AppCommand;
    use crate::state::editor_command::EditorCommand;
    use crate::triggers::{
        camera_triggers_to_timed_triggers, CameraTrigger, CameraTriggerMode, TimedTrigger,
        TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget,
    };
    use crate::types::{
        AppPhase, EditorMode, KeyChord, LevelObject, MusicMetadata, SettingsSection, TimingPoint,
        CAMERA_TRIGGER_BLOCK_ID, TRANSFORM_TRIGGER_BLOCK_ID,
    };
    use glam::{Vec2, Vec3};

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
            use crate::state::editor_command::EditorCommand;

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Select);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Move,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Move);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Scale,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Scale);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Rotate,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Rotate);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Timing,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Timing);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Place,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Tapping,
            )));
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Tapping);
        });
    }

    #[test]
    fn entering_tapping_mode_invalidates_stale_tap_division_previews() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.editor.timeline.tap_division_preview_cache_revision =
                state.editor.timeline.simulation_revision;
            state
                .editor
                .timeline
                .tap_division_preview_cache_timing_revision = state.editor.timing.revision;
            state.editor.timeline.tap_division_preview_cache.push(
                crate::editor_domain::TapDivisionPreview {
                    time_seconds: 1.0,
                    indicator_position: [99.0, 99.0, 99.0],
                },
            );

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));

            assert_eq!(state.editor.ui.mode, EditorMode::Tapping);
            assert_eq!(state.editor.timeline.tap_division_preview_cache_revision, 0);
            assert_eq!(
                state
                    .editor
                    .timeline
                    .tap_division_preview_cache_timing_revision,
                0
            );
            assert!(state.editor.timeline.tap_division_preview_cache.is_empty());
            assert!(state.editor.runtime.dirty.rebuild_tap_indicators);
            assert!(state.editor.runtime.dirty.rebuild_cursor);
        });
    }

    #[test]
    fn entering_tapping_mode_does_not_move_editor_camera() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.editor.timeline.clock.time_seconds = 2.0;
            state.editor.timeline.preview.position = [12.0, 3.0, -7.0];
            state.editor.camera.editor_pan = [4.0, 8.0];
            state.editor.camera.editor_target_z = 6.0;

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));

            assert_eq!(state.editor.ui.mode, EditorMode::Tapping);
            assert_eq!(state.editor.camera.editor_pan, [4.0, 8.0]);
            assert_eq!(state.editor.camera.editor_target_z, 6.0);
        });
    }

    #[test]
    fn entering_tapping_mode_recomputes_stale_tap_positions_after_object_edit() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.editor.spawn.position = [0.0, 0.0, 0.0];
            state.editor.spawn.direction = crate::types::SpawnDirection::Forward;
            state.editor.timeline.taps.tap_times = vec![0.5];
            state.editor.timeline.taps.tap_indicator_positions = vec![[42.0, 0.0, 42.0]];
            state.editor.timeline.taps.selected_index = Some(0);

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));

            let expected = crate::editor_domain::derive_tap_indicator_positions(
                state.editor.spawn.position,
                state.editor.spawn.direction,
                &state.editor.timeline.taps.tap_times,
                &state.editor.objects,
            );
            assert_eq!(state.editor.timeline.taps.tap_indicator_positions, expected);
            assert_eq!(state.editor.timeline.taps.selected_index, Some(0));
        });
    }

    #[test]
    fn entering_tapping_mode_rebuilds_path_pick_samples_after_object_edit() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.editor.spawn.position = [0.0, 0.0, 0.0];
            state.editor.spawn.direction = crate::types::SpawnDirection::Forward;
            state.editor.timeline.clock.duration_seconds = 2.0;
            state.editor.invalidate_samples();

            assert!(state.editor.timeline.snapshot_cache.is_empty());
            assert_ne!(
                state.editor.timeline.snapshot_cache_revision,
                state.editor.timeline.simulation_revision
            );

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));

            assert_eq!(state.editor.ui.mode, EditorMode::Tapping);
            assert!(!state.editor.timeline.snapshot_cache.is_empty());
            assert_eq!(
                state.editor.timeline.snapshot_cache_revision,
                state.editor.timeline.simulation_revision
            );
            assert!(state
                .editor
                .tap_path_pick_near_world([0.5, 0.0, 0.5])
                .is_some());
        });
    }

    #[test]
    fn editor_set_mode_during_playback_keeps_null_mode() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Timing,
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback));
            assert!(state.editor.timeline.playback.playing);
            assert_eq!(state.editor.ui.mode, EditorMode::Null);
            assert_eq!(
                state.editor.runtime.interaction.last_mode,
                Some(EditorMode::Timing)
            );

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));

            assert!(state.editor.timeline.playback.playing);
            assert_eq!(state.editor.ui.mode, EditorMode::Null);
            assert_eq!(
                state.editor.runtime.interaction.last_mode,
                Some(EditorMode::Place)
            );
            assert_eq!(
                state.editor_effective_mode_for_playback(),
                EditorMode::Place
            );

            state.dispatch(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback));

            assert!(!state.editor.timeline.playback.playing);
            assert_eq!(state.editor.ui.mode, EditorMode::Place);
            assert!(state.editor.runtime.interaction.last_mode.is_none());
        });
    }

    #[test]
    fn editor_escape_deselects_blocks_and_taps_before_playback_or_timeline() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects = vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }];
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.timeline.taps.tap_times = vec![1.25];
            state.editor.timeline.taps.tap_indicator_positions = vec![[0.25, 0.0, 0.75]];
            state.editor.timeline.taps.selected_index = Some(0);
            state.editor.timeline.clock.time_seconds = 2.0;
            state.editor.timeline.playback.playing = true;

            state.dispatch(AppCommand::Editor(EditorCommand::Escape));

            assert!(state.editor.ui.selected_block_indices.is_empty());
            assert!(state.editor.ui.selected_block_index.is_none());
            assert_eq!(state.editor.timeline.taps.selected_index, None);
            assert!(state.editor.timeline.playback.playing);
            assert_eq!(state.editor.timeline.clock.time_seconds, 2.0);

            state.dispatch(AppCommand::Editor(EditorCommand::Escape));

            assert!(!state.editor.timeline.playback.playing);
            assert_eq!(state.editor.timeline.clock.time_seconds, 2.0);
        });
    }

    #[test]
    fn escape_toggles_pause_for_real_play_without_returning_to_menu() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Playing;
            state.session.playtesting_editor = false;
            state.gameplay.state.started = true;

            state.dispatch(AppCommand::Editor(EditorCommand::Escape));

            assert_eq!(state.phase, AppPhase::Playing);
            assert!(state.is_game_paused());

            state.dispatch(AppCommand::Editor(EditorCommand::Escape));

            assert_eq!(state.phase, AppPhase::Playing);
            assert!(!state.is_game_paused());
        });
    }

    #[test]
    fn resume_from_pause_starts_waiting_real_gameplay() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.phase = AppPhase::Playing;
            state.session.playtesting_editor = false;
            state.session.game_paused = true;
            state.gameplay.state.started = false;

            state.dispatch(AppCommand::GameResume);

            assert!(!state.is_game_paused());
            assert!(state.gameplay.state.started);
        });
    }

    #[test]
    fn practice_checkpoint_key_only_maps_during_practice_mode() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            state.phase = AppPhase::Playing;
            state.session.playtesting_editor = false;
            state.session.practice_mode_enabled = false;
            assert_eq!(
                state.command_for_keybind_action("practice_checkpoint", true),
                None
            );

            state.session.practice_mode_enabled = true;
            assert_eq!(
                state.command_for_keybind_action("practice_checkpoint", true),
                Some(AppCommand::GameSetPracticeCheckpoint)
            );
            assert_eq!(
                state.command_for_keybind_action("practice_remove_checkpoint", true),
                Some(AppCommand::GameRemovePracticeCheckpoint)
            );
            assert_eq!(
                state.command_for_keybind_action("practice_checkpoint", false),
                None
            );
            assert_eq!(
                state.command_for_keybind_action("practice_remove_checkpoint", false),
                None
            );

            state.phase = AppPhase::Editor;
            assert_eq!(
                state.command_for_keybind_action("practice_checkpoint", true),
                None
            );
            assert_eq!(
                state.command_for_keybind_action("practice_remove_checkpoint", true),
                None
            );
        });
    }

    #[test]
    fn escape_during_playtest_still_returns_to_editor() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            state.enter_editor_phase("Playtest Escape".to_string());
            state.editor.objects = vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }];
            state.editor_playtest();

            state.dispatch(AppCommand::Editor(EditorCommand::Escape));

            assert_eq!(state.phase, AppPhase::Editor);
            assert!(!state.is_game_paused());
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
            state.dispatch(AppCommand::Editor(EditorCommand::AdjustZoom(0.5)));
            // Zooming in (positive delta) should move the camera target Z forward (if look direction has positive Z)
            // or at least change the position.
            assert!(state.editor.camera.editor_target_z != initial_z);

            state.dispatch(AppCommand::Editor(EditorCommand::SetBlockId(
                "core/lava".to_string(),
            )));
            assert_eq!(state.editor.config.selected_block_id, "core/lava");
            assert_eq!(state.editor.config.recent_block_ids[0], "core/lava");

            state.editor.objects = vec![
                LevelObject {
                    position: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
                },
                LevelObject {
                    position: [1.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/grass".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
                },
            ];
            state.editor.ui.mode = EditorMode::Move;
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.hovered_block_index = Some(1);

            state.dispatch(AppCommand::Editor(EditorCommand::PickSelectedBlock));

            assert_eq!(state.editor.ui.mode, EditorMode::Place);
            assert_eq!(state.editor.config.selected_block_id, "core/stone");

            state.editor.ui.mode = EditorMode::Move;
            state.editor.ui.selected_block_index = None;
            state.editor.ui.selected_block_indices.clear();
            state.editor.ui.hovered_block_index = Some(1);

            state.dispatch(AppCommand::Editor(EditorCommand::PickSelectedBlock));

            assert_eq!(state.editor.ui.mode, EditorMode::Move);
            assert_eq!(state.editor.config.selected_block_id, "core/stone");
        });
    }

    #[test]
    fn placing_block_selects_it_and_enters_scale_mode_by_default() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::SetBlockId(
                "core/lava".to_string(),
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::RotatePlacementPreview));
            state.dispatch(AppCommand::TurnRight);

            assert_eq!(state.editor.objects.len(), 1);
            assert_eq!(state.editor.ui.mode, EditorMode::Scale);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert_eq!(state.editor.ui.hovered_block_index, Some(0));
            assert_eq!(state.editor.objects[0].block_id, "core/lava");
            assert_eq!(state.editor.objects[0].rotation_degrees, [0.0, 90.0, 0.0]);
        });
    }

    #[test]
    fn r_in_place_mode_rotates_preview_without_rotating_spawn() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.spawn.direction = crate::types::SpawnDirection::Forward;
            state.editor.config.selected_block_rotation_degrees = [0.0, 0.0, 0.0];

            let command = state.command_for_keybind_action("spawn_rotate", true);
            assert_eq!(
                command,
                Some(AppCommand::Editor(EditorCommand::RotatePlacementPreview))
            );
            state.dispatch(command.unwrap());

            assert_eq!(
                state.editor.spawn.direction,
                crate::types::SpawnDirection::Forward
            );
            assert_eq!(
                state.editor.config.selected_block_rotation_degrees,
                [0.0, 90.0, 0.0]
            );
        });
    }

    #[test]
    fn shift_placing_block_keeps_stamp_mode_without_selection() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::SetShiftHeld(true)));
            state.dispatch(AppCommand::TurnRight);

            assert_eq!(state.editor.objects.len(), 1);
            assert_eq!(state.editor.ui.mode, EditorMode::Place);
            assert!(state.editor.ui.selected_block_index.is_none());
            assert!(state.editor.ui.selected_block_indices.is_empty());
        });
    }

    #[test]
    fn pick_block_at_screen_samples_block_type_for_placement() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects = vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/grass".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }];
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(EditorMode::Move)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetBlockId(
                "core/stone".to_string(),
            )));

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let screen = state
                .editor
                .world_to_screen_v(Vec3::new(0.5, 0.5, 0.5), viewport)
                .expect("block center should project to the screen");

            state.dispatch(AppCommand::Editor(EditorCommand::PickBlockAt {
                x: screen.x as f64,
                y: screen.y as f64,
            }));

            assert_eq!(state.editor.ui.mode, EditorMode::Place);
            assert_eq!(state.editor.config.selected_block_id, "core/grass");
            assert_eq!(state.editor.config.recent_block_ids[0], "core/grass");
        });
    }

    #[test]
    fn editor_focus_camera_target_ignores_hover_and_uses_preview_fallback() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;
            state.editor.objects = vec![
                LevelObject {
                    position: [10.0, 2.0, 30.0],
                    size: [2.0, 4.0, 6.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
                },
                LevelObject {
                    position: [50.0, 4.0, 70.0],
                    size: [4.0, 2.0, 8.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/grass".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
                },
            ];
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.ui.hovered_block_index = Some(1);

            state.dispatch(AppCommand::Editor(EditorCommand::FocusCameraTarget));

            assert_eq!(state.editor.camera.editor_pan, [11.0, 33.0]);
            assert_eq!(state.editor.camera.editor_target_z, 4.0);

            state.editor.ui.selected_block_index = None;
            state.editor.ui.selected_block_indices.clear();
            state.editor.timeline.preview.position = [7.0, 8.0, 9.0];
            state.dispatch(AppCommand::Editor(EditorCommand::FocusCameraTarget));

            assert_eq!(state.editor.camera.editor_pan, [7.0, 9.0]);
            assert_eq!(state.editor.camera.editor_target_z, 8.0);

            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.ui.selected_block_index = Some(0);
            state.editor.ui.selected_block_indices = vec![0];
            state.editor.timeline.taps.tap_times = vec![1.25];
            state.editor.timeline.taps.tap_indicator_positions = vec![[5.0, 1.0, 6.0]];
            state.editor.timeline.taps.selected_index = Some(0);
            state.dispatch(AppCommand::Editor(EditorCommand::FocusCameraTarget));

            assert_eq!(state.editor.camera.editor_pan, [5.5, 6.5]);
            assert_eq!(state.editor.camera.editor_target_z, 1.0);

            state.editor.ui.mode = EditorMode::Place;
            state.editor.ui.selected_block_index = None;
            state.editor.ui.selected_block_indices.clear();
            state.editor.ui.hovered_block_index = Some(1);
            state.editor.timeline.taps.selected_index = None;
            state.editor.ui.pointer_screen = None;
            state.editor.timeline.preview.position = [7.0, 8.0, 9.0];
            state.dispatch(AppCommand::Editor(EditorCommand::FocusCameraTarget));

            assert_eq!(state.editor.camera.editor_pan, [7.0, 9.0]);
            assert_eq!(state.editor.camera.editor_target_z, 8.0);
        });
    }

    #[test]
    fn test_timeline_shift_updates_preview() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            let (pos_before, _) = state.editor_timeline_preview();

            // Shift timeline forward by 1 second
            state.dispatch(AppCommand::Editor(EditorCommand::ShiftTimeline(1.0)));

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
    fn primary_click_over_editor_ui_does_not_place_block() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.set_editor_ui_input_blocking_rects(vec![[10.0, 20.0, 110.0, 120.0]], 2.0);

            state.process_input_event(InputEvent::PrimaryClick { x: 100.0, y: 100.0 });

            assert!(state.editor.objects.is_empty());
            assert!(state.editor.ui.left_mouse_down);

            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });
            assert!(!state.editor.ui.left_mouse_down);
        });
    }

    #[test]
    fn primary_click_outside_editor_ui_still_places_block() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.editor.ui.mode = EditorMode::Place;
            state.set_editor_ui_input_blocking_rects(vec![[10.0, 20.0, 110.0, 120.0]], 2.0);

            state.process_input_event(InputEvent::PrimaryClick { x: 250.0, y: 250.0 });

            assert_eq!(state.editor.objects.len(), 1);
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
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Rotate);
        });
    }

    #[test]
    fn test_keyboard_tab_hotkeys_switch_editor_tabs() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.process_input_event(InputEvent::Key {
                key: "Control".to_string(),
                pressed: true,
                just_pressed: true,
            });

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Scale,
            )));
            state.process_input_event(InputEvent::Key {
                key: "1".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Place);

            state.process_input_event(InputEvent::Key {
                key: "2".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Timing);

            state.process_input_event(InputEvent::Key {
                key: "3".to_string(),
                pressed: true,
                just_pressed: true,
            });
            assert_eq!(state.editor.ui.mode, crate::types::EditorMode::Tapping);
        });
    }

    #[test]
    fn test_keyboard_qe_do_not_switch_modes_in_editor() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Scale,
            )));

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
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));

            state.editor.camera.editor_pan = [0.0, 0.0];
            state.editor.objects = vec![
                crate::types::LevelObject {
                    position: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
                },
                crate::types::LevelObject {
                    position: [2.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    block_id: "core/stone".to_string(),
                    color_tint: [1.0, 1.0, 1.0],
                    trigger: None,
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
    fn test_select_mode_marquee_selects_blocks_during_drag() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));

            state.editor.camera.editor_pan = [0.0, 0.0];
            state.editor.objects = vec![crate::types::LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
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

            // Process dirty flags to trigger the deferred selection outline rebuild
            state.process_editor_dirty(1.0 / 60.0);

            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert!(
                state
                    .render
                    .meshes
                    .editor_hover_outline
                    .draw_data()
                    .is_none(),
                "Hover outline should remain clear during marquee drag"
            );
            assert!(
                state
                    .render
                    .meshes
                    .editor_selection_outline
                    .draw_data()
                    .is_some(),
                "Selection outline should be populated during marquee drag"
            );

            // Finish marquee
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            // Process dirty flags to update the hover outline after release
            state.process_editor_dirty(1.0 / 60.0);

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
    fn test_select_mode_click_selects_block_on_mouse_down() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));
            state.editor.objects = vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }];

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

            assert_eq!(state.editor.ui.selected_block_indices, vec![0]);
            assert_eq!(state.editor.ui.selected_block_index, Some(0));
            assert!(state.editor.ui.marquee_start_screen.is_none());

            state.process_editor_dirty(1.0 / 60.0);
            assert!(state
                .render
                .meshes
                .editor_hover_outline
                .draw_data()
                .is_none());
            assert!(state
                .render
                .meshes
                .editor_selection_outline
                .draw_data()
                .is_some());
        });
    }

    #[test]
    fn tapping_mode_click_selects_tap_indicator_without_removing_it() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));
            state.editor.objects.clear();
            state.editor.timeline.clock.time_seconds = 0.0;
            state.editor.timeline.clock.duration_seconds = 4.0;
            state.editor.timeline.taps.tap_times = vec![1.25];
            state.editor.timeline.taps.tap_indicator_positions = vec![[0.25, 0.0, 0.75]];
            state.editor.timeline.taps.selected_index = None;
            state.editor.camera.editor_pan = [4.0, -3.0];
            state.editor.camera.editor_target_z = 2.5;
            let original_pan = state.editor.camera.editor_pan;
            let original_target_z = state.editor.camera.editor_target_z;

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let tap_center = glam::Vec3::new(0.75, 0.1, 1.25);
            let tap_screen = state
                .editor
                .world_to_screen_v(tap_center, viewport)
                .expect("tap indicator should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: tap_screen.x as f64,
                y: tap_screen.y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });

            assert_eq!(state.editor.timeline.taps.tap_times, vec![1.25]);
            assert_eq!(state.editor.timeline.taps.selected_index, Some(0));
            assert_eq!(state.editor.timeline.clock.time_seconds, 1.25);
            assert_eq!(state.editor.ui.cursor, [0.25, 0.0, 0.75]);
            assert_eq!(state.editor.camera.editor_pan, original_pan);
            assert_eq!(state.editor.camera.editor_target_z, original_target_z);

            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });

            assert_eq!(state.editor.timeline.taps.tap_times, vec![1.25]);
            assert_eq!(state.editor.timeline.taps.selected_index, Some(0));

            let view = state.editor_ui_view_model();
            let selected_tap = view
                .selected_tap
                .expect("selected tap should be exposed to the UI");
            assert_eq!(selected_tap.index, 0);
            assert_eq!(selected_tap.time_seconds, 1.25);
            assert_eq!(selected_tap.position, [0.25, 0.0, 0.75]);
        });
    }

    #[test]
    fn tapping_mode_click_selects_tap_indicator_by_matching_time_index() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));
            state.editor.objects.clear();
            state.editor.timeline.clock.time_seconds = 0.0;
            state.editor.timeline.clock.duration_seconds = 4.0;
            state.editor.timeline.taps.tap_times = vec![0.125, 0.25];
            state.editor.timeline.taps.tap_indicator_positions =
                crate::editor_domain::derive_tap_indicator_positions(
                    state.editor.spawn.position,
                    state.editor.spawn.direction,
                    &state.editor.timeline.taps.tap_times,
                    &state.editor.objects,
                );
            state.editor.timeline.taps.selected_index = None;
            state.editor.camera.editor_pan = [2.0, 1.0];
            state.editor.camera.editor_target_z = 1.5;

            let first_tap_position = state.editor.timeline.taps.tap_indicator_positions[0];
            let second_tap_position = state.editor.timeline.taps.tap_indicator_positions[1];
            assert!(first_tap_position[0] < second_tap_position[0]);
            assert!((first_tap_position[2] - second_tap_position[2]).abs() < 0.001);

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let second_tap_center = glam::Vec3::new(
                second_tap_position[0] + 0.5,
                second_tap_position[1] + 0.1,
                second_tap_position[2] + 0.5,
            );
            let second_tap_screen = state
                .editor
                .world_to_screen_v(second_tap_center, viewport)
                .expect("second tap indicator should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: second_tap_screen.x as f64,
                y: second_tap_screen.y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });

            assert_eq!(state.editor.timeline.taps.selected_index, Some(1));
            assert!((state.editor.timeline.clock.time_seconds - 0.25).abs() < 0.001);
            assert_eq!(state.editor.ui.cursor, second_tap_position);

            let selected_tap = state
                .editor_ui_view_model()
                .selected_tap
                .expect("selected tap should be exposed to the UI");
            assert_eq!(selected_tap.index, 1);
            assert_eq!(selected_tap.time_seconds, 0.25);
            assert_eq!(selected_tap.position, second_tap_position);
        });
    }

    #[test]
    fn tapping_mode_click_on_timing_division_seeks_then_creates_exact_tap() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));
            state.editor.timeline.clock.time_seconds = 0.0;
            state.editor.timeline.clock.duration_seconds = 0.5;
            state.editor.timing.timing_points = vec![TimingPoint {
                time_seconds: 0.1,
                bpm: 600.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];
            state.editor.config.snap_to_grid = true;
            state.editor.camera.editor_pan = [4.0, -3.0];
            state.editor.camera.editor_target_z = 2.5;
            let original_pan = state.editor.camera.editor_pan;
            let original_target_z = state.editor.camera.editor_target_z;

            let division = state
                .editor
                .timing_division_tap_previews()
                .iter()
                .copied()
                .find(|preview| (preview.time_seconds - 0.1).abs() < 0.001)
                .expect("division preview should exist");

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let division_center = glam::Vec3::new(
                division.indicator_position[0] + 0.5,
                division.indicator_position[1] + 0.1,
                division.indicator_position[2] + 0.5,
            );
            let division_screen = state
                .editor
                .world_to_screen_v(division_center, viewport)
                .expect("division indicator should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: division_screen.x as f64,
                y: division_screen.y as f64,
            });
            let hovered_division = state
                .editor
                .runtime
                .interaction
                .hovered_tap_division
                .expect("division preview should be hovered after pointer move");
            assert!((hovered_division.time_seconds - division.time_seconds).abs() < 0.001);
            assert_eq!(
                hovered_division.indicator_position,
                division.indicator_position
            );

            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });

            assert!(state.editor.timeline.taps.tap_times.is_empty());
            assert!((state.editor.timeline.clock.time_seconds - 0.1).abs() < 0.001);
            assert_eq!(state.editor.ui.cursor, division.indicator_position);
            assert_eq!(state.editor.camera.editor_pan, original_pan);
            assert_eq!(state.editor.camera.editor_target_z, original_target_z);

            state.editor.set_left_mouse_down(false);
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });

            assert_eq!(state.editor.timeline.taps.tap_times.len(), 1);
            assert!((state.editor.timeline.taps.tap_times[0] - 0.1).abs() < 0.001);
            assert_eq!(
                state.editor.timeline.taps.tap_indicator_positions[0],
                division.indicator_position
            );
            assert_eq!(state.editor.timeline.taps.selected_index, Some(0));
            assert_eq!(state.editor.ui.cursor, division.indicator_position);
            assert_eq!(state.editor.camera.editor_pan, original_pan);
            assert_eq!(state.editor.camera.editor_target_z, original_target_z);
        });
    }

    #[test]
    fn tapping_mode_pointer_snaps_cursor_to_snake_path() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Tapping,
            )));
            state.editor.timeline.clock.duration_seconds = 4.0;
            state.editor.config.snap_to_grid = true;
            state.editor.config.snap_step = 1.0;
            state.set_editor_timeline_time_seconds(1.0 / crate::game::BASE_PLAYER_SPEED);
            state.editor.camera.editor_pan = [4.0, -3.0];
            state.editor.camera.editor_target_z = 2.5;

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let off_path_world = glam::Vec3::new(4.5, 0.0, 1.5);
            let off_path_screen = state
                .editor
                .world_to_screen_v(off_path_world, viewport)
                .expect("off-path ground point should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: off_path_screen.x as f64,
                y: off_path_screen.y as f64,
            });

            assert!(
                state.editor.ui.cursor[0].abs() < 0.05,
                "cursor should be constrained to the forward snake lane, got {:?}",
                state.editor.ui.cursor
            );
            assert!(state.editor.ui.cursor[2].is_finite());
        });
    }

    #[test]
    fn test_select_mode_click_selects_camera_trigger_block() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));

            let camera_offset = state.editor.camera_offset();
            let target = state.editor.editor_camera_target()
                + (-camera_offset.normalize() * 8.0)
                + Vec3::Y * 4.0;
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
            let trigger_index = state.editor.objects.len();
            let trigger =
                camera_triggers_to_timed_triggers(std::slice::from_ref(&camera_trigger)).remove(0);

            state.editor.objects.push(LevelObject {
                position: camera_trigger.target_position,
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [
                    camera_trigger.pitch.to_degrees(),
                    camera_trigger.rotation.to_degrees(),
                    0.0,
                ],
                block_id: CAMERA_TRIGGER_BLOCK_ID.to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: Some(trigger),
            });
            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let marker_screen = state
                .editor
                .world_to_screen_v(
                    Vec3::from_array(camera_trigger.target_position) + Vec3::splat(0.5),
                    viewport,
                )
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

            assert_eq!(state.editor.ui.selected_block_index, Some(trigger_index));
            assert_eq!(state.editor.ui.selected_block_indices, vec![trigger_index]);
            assert_eq!(state.editor.selected_trigger_index(), None);
        });
    }

    #[test]
    fn test_select_mode_marquee_selects_camera_trigger_block() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));

            let camera_offset = state.editor.camera_offset();
            let target = state.editor.editor_camera_target()
                + (-camera_offset.normalize() * 8.0)
                + Vec3::Y * 4.0;
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
            let trigger_index = state.editor.objects.len();
            let trigger =
                camera_triggers_to_timed_triggers(std::slice::from_ref(&camera_trigger)).remove(0);

            state.editor.objects.push(LevelObject {
                position: camera_trigger.target_position,
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [
                    camera_trigger.pitch.to_degrees(),
                    camera_trigger.rotation.to_degrees(),
                    0.0,
                ],
                block_id: CAMERA_TRIGGER_BLOCK_ID.to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: Some(trigger),
            });
            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let marker_screen = state
                .editor
                .world_to_screen_v(
                    Vec3::from_array(camera_trigger.target_position) + Vec3::splat(0.5),
                    viewport,
                )
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

            assert_eq!(state.editor.ui.selected_block_index, Some(trigger_index));
            assert_eq!(state.editor.ui.selected_block_indices, vec![trigger_index]);
            assert_eq!(state.editor.selected_trigger_index(), None);
        });
    }

    #[test]
    fn test_select_mode_click_selects_transform_trigger_block() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                crate::types::EditorMode::Select,
            )));
            let trigger_index = state.editor.objects.len();
            state.editor.objects.push(LevelObject {
                position: [2.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: TRANSFORM_TRIGGER_BLOCK_ID.to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: Some(TimedTrigger {
                    time_seconds: 1.0,
                    duration_seconds: 1.0,
                    easing: TimedTriggerEasing::Linear,
                    target: TimedTriggerTarget::Objects {
                        object_ids: vec![0],
                    },
                    action: TimedTriggerAction::TransformObjects {
                        position: [2.0, 0.0, 0.0],
                        rotation_degrees: [0.0, 0.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                    },
                }),
            });

            let viewport = Vec2::new(
                state.render.gpu.config.width as f32,
                state.render.gpu.config.height as f32,
            );
            let target_center = glam::Vec3::new(2.5, 0.5, 0.5);
            let target_screen = state
                .editor
                .world_to_screen_v(target_center, viewport)
                .expect("transform trigger target should project to the screen");

            state.process_input_event(InputEvent::PointerMoved {
                x: target_screen.x as f64,
                y: target_screen.y as f64,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: true,
            });
            state.process_input_event(InputEvent::MouseButton {
                button: 0,
                pressed: false,
            });

            assert_eq!(state.editor.ui.selected_block_index, Some(trigger_index));
            assert_eq!(state.editor.ui.selected_block_indices, vec![trigger_index]);
            assert_eq!(state.editor.selected_trigger_index(), None);
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
            state.dispatch(AppCommand::Editor(EditorCommand::Undo));
            assert_eq!(state.editor.objects.len(), initial_count);

            // 4. Redo placement
            state.dispatch(AppCommand::Editor(EditorCommand::Redo));
            assert_eq!(state.editor.objects.len(), initial_count + 1);
        });
    }

    #[test]
    fn test_complex_command_sequence() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            let initial_count = state.editor.objects.len();

            // Set a specific block type
            state.dispatch(AppCommand::Editor(EditorCommand::SetBlockId(
                "core/lava".to_string(),
            )));

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
            state.dispatch(AppCommand::Editor(EditorCommand::Undo));
            assert_eq!(state.editor.objects.len(), initial_count + 1);
            assert_eq!(state.editor.objects.last().unwrap().position, pos1);

            // Undo twice
            state.dispatch(AppCommand::Editor(EditorCommand::Undo));
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
    fn test_perf_overlay_shortcut_works_in_menu() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = State::new_test().await;
            assert!(state.is_menu());
            assert!(!state.perf_overlay_enabled());

            for key in ["Control", "Shift", "Alt", "F12"] {
                state.process_input_event(InputEvent::Key {
                    key: key.to_string(),
                    pressed: true,
                    just_pressed: true,
                });
            }

            assert!(state.perf_overlay_enabled());
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

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybind {
                action: "copy".to_string(),
                slot: 0,
                chord: crate::types::KeyChord::new("b", true, false, false),
            }));

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
                Some(AppCommand::Editor(EditorCommand::SetShiftHeld(true)))
            );
            assert_eq!(
                state.map_modifier_key_to_command("ControlRight", false),
                Some(AppCommand::Editor(EditorCommand::SetCtrlHeld(false)))
            );
            assert_eq!(
                state.map_modifier_key_to_command("Alt", true),
                Some(AppCommand::Editor(EditorCommand::SetAltHeld(true)))
            );
            assert_eq!(state.map_modifier_key_to_command("m", true), None);

            assert!(State::is_modifier_key("Control"));
            assert!(State::is_modifier_key("AltLeft"));
            assert!(!State::is_modifier_key("m"));

            assert!(!state.has_block_selection());
            state.editor.objects = vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
            }];
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
                vec![AppCommand::Editor(EditorCommand::SetPanUpHeld(true))]
            );

            state.editor.ui.ctrl_held = true;
            assert!(state.map_pan_key_to_commands("w", true).is_empty());

            let released_pan = state.map_pan_key_to_commands("w", false);
            assert_eq!(
                released_pan,
                vec![AppCommand::Editor(EditorCommand::SetPanUpHeld(false))]
            );

            state.phase = AppPhase::Editor;
            assert_eq!(
                state.command_for_keybind_action("toggle_settings", true),
                Some(AppCommand::Editor(EditorCommand::ToggleSettings))
            );
            assert_eq!(
                state.command_for_keybind_action("toggle_settings", false),
                None
            );

            assert_eq!(
                state.command_for_keybind_action("toggle_timeline_playback", true),
                Some(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback))
            );
            assert_eq!(
                state.command_for_keybind_action("playtest", true),
                Some(AppCommand::Editor(EditorCommand::Playtest))
            );
            assert_eq!(
                state.command_for_keybind_action("remove_block", true),
                Some(AppCommand::Editor(EditorCommand::RemoveBlock))
            );
            state.editor.ui.mode = EditorMode::Tapping;
            state.editor.timeline.taps.tap_times = vec![1.25];
            state.editor.timeline.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0]];
            state.editor.timeline.taps.selected_index = Some(0);
            assert_eq!(
                state.command_for_keybind_action("remove_block", true),
                Some(AppCommand::Editor(EditorCommand::RemoveTapAt(1.25)))
            );
            state.editor.ui.mode = EditorMode::Place;
            assert_eq!(
                state.command_for_keybind_action("copy", true),
                Some(AppCommand::Editor(EditorCommand::CopyBlock))
            );
            assert_eq!(
                state.command_for_keybind_action("paste", true),
                Some(AppCommand::Editor(EditorCommand::PasteBlock))
            );
            assert_eq!(
                state.command_for_keybind_action("duplicate", true),
                Some(AppCommand::Editor(EditorCommand::DuplicateBlock))
            );
            assert_eq!(
                state.command_for_keybind_action("undo", true),
                Some(AppCommand::Editor(EditorCommand::Undo))
            );
            assert_eq!(
                state.command_for_keybind_action("redo", true),
                Some(AppCommand::Editor(EditorCommand::Redo))
            );

            assert_eq!(state.command_for_keybind_action("nudge_up", true), None);
            state.editor.ui.selected_block_index = Some(0);
            assert_eq!(
                state.command_for_keybind_action("nudge_up", true),
                Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                    dx: 0,
                    dy: 1
                }))
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_down", true),
                Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                    dx: 0,
                    dy: -1
                }))
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_left", true),
                Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                    dx: -1,
                    dy: 0
                }))
            );
            assert_eq!(
                state.command_for_keybind_action("nudge_right", true),
                Some(AppCommand::Editor(EditorCommand::NudgeSelected {
                    dx: 1,
                    dy: 0
                }))
            );
            assert_eq!(
                state.command_for_keybind_action("timeline_forward", true),
                None
            );
            state.editor.ui.selected_block_index = None;
            assert_eq!(
                state.command_for_keybind_action("pick_selected_block", true),
                Some(AppCommand::Editor(EditorCommand::PickSelectedBlock))
            );
            assert_eq!(
                state.command_for_keybind_action("pick_selected_block", false),
                None
            );
            assert_eq!(
                state.command_for_keybind_action("focus_camera_target", true),
                Some(AppCommand::Editor(EditorCommand::FocusCameraTarget))
            );
            assert_eq!(
                state.command_for_keybind_action("focus_camera_target", false),
                None
            );
            state.editor.timeline.clock.time_seconds = 1.26;
            state.editor.timing.timing_points = vec![
                TimingPoint {
                    time_seconds: 0.0,
                    bpm: 120.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
                TimingPoint {
                    time_seconds: 1.0,
                    bpm: 240.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
            ];
            let Some(AppCommand::Editor(EditorCommand::ShiftTimeline(forward_delta))) =
                state.command_for_keybind_action("timeline_forward", true)
            else {
                panic!("timeline forward should shift to the next timing division");
            };
            assert!((forward_delta - 0.24).abs() < 0.0001);

            let Some(AppCommand::Editor(EditorCommand::ShiftTimeline(backward_delta))) =
                state.command_for_keybind_action("timeline_backward", true)
            else {
                panic!("timeline backward should shift to the previous timing division");
            };
            assert!((backward_delta + 0.01).abs() < 0.0001);
            state.editor.timeline.clock.time_seconds = 1.25;
            let Some(AppCommand::Editor(EditorCommand::ShiftTimeline(backward_delta))) =
                state.command_for_keybind_action("timeline_backward", true)
            else {
                panic!("timeline backward should keep moving from an exact timing division");
            };
            assert!((backward_delta + 0.25).abs() < 0.0001);
            state.editor.timing.timing_points.clear();
            assert_eq!(
                state.command_for_keybind_action("timeline_forward", true),
                Some(AppCommand::Editor(EditorCommand::ShiftTimeline(0.1)))
            );
            assert_eq!(
                state.command_for_keybind_action("timeline_backward", true),
                Some(AppCommand::Editor(EditorCommand::ShiftTimeline(-0.1)))
            );
            state.editor.timing.timing_points = vec![TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];
            state.editor.timeline.clock.time_seconds = 0.0;
            assert_eq!(
                state.command_for_keybind_action("timeline_backward", true),
                Some(AppCommand::Editor(EditorCommand::ShiftTimeline(-0.1)))
            );
            state.editor.timeline.clock.time_seconds = state.editor.timeline.clock.duration_seconds;
            assert_eq!(
                state.command_for_keybind_action("timeline_forward", true),
                Some(AppCommand::Editor(EditorCommand::ShiftTimeline(0.1)))
            );

            assert_eq!(
                state.command_for_keybind_action("escape", true),
                Some(AppCommand::Editor(EditorCommand::Escape))
            );
            assert_eq!(state.command_for_keybind_action("escape", false), None);
            assert_eq!(
                state.command_for_keybind_action("zoom_in", true),
                Some(AppCommand::Editor(EditorCommand::AdjustZoom(1.0)))
            );
            assert_eq!(
                state.command_for_keybind_action("zoom_out", true),
                Some(AppCommand::Editor(EditorCommand::AdjustZoom(-1.0)))
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
                Some(AppCommand::Editor(EditorCommand::SetSpawnHere))
            );
            assert_eq!(
                state.command_for_keybind_action("spawn_rotate", true),
                Some(AppCommand::Editor(EditorCommand::RotatePlacementPreview))
            );

            state.editor.ui.mode = EditorMode::Move;
            assert_eq!(
                state.command_for_keybind_action("spawn_rotate", true),
                Some(AppCommand::Editor(EditorCommand::RotateSpawnDirection))
            );

            assert_eq!(
                state.command_for_keybind_action("add_camera_trigger", true),
                Some(AppCommand::Editor(EditorCommand::AddCameraTrigger))
            );
            assert_eq!(
                state.command_for_keybind_action("add_camera_follow_trigger", true),
                Some(AppCommand::Editor(EditorCommand::AddCameraFollowTrigger))
            );
            assert_eq!(
                state.command_for_keybind_action("export_obj", true),
                Some(AppCommand::Editor(EditorCommand::ExportBlockObj))
            );
            assert_eq!(
                state.command_for_keybind_action("toggle_perf_overlay", true),
                Some(AppCommand::Editor(EditorCommand::TogglePerfOverlay))
            );

            assert_eq!(
                state.command_for_keybind_action("mode_select", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Select
                )))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_move", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(EditorMode::Move)))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_scale", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Scale
                )))
            );
            assert_eq!(
                state.command_for_keybind_action("mode_rotate", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Rotate
                )))
            );
            assert_eq!(state.command_for_keybind_action("mode_trigger", true), None);
            assert_eq!(
                state.command_for_keybind_action("tab_compose", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Place
                )))
            );
            assert_eq!(
                state.command_for_keybind_action("tab_timing", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Timing
                )))
            );
            assert_eq!(
                state.command_for_keybind_action("tab_tapping", true),
                Some(AppCommand::Editor(EditorCommand::SetMode(
                    EditorMode::Tapping
                )))
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

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybindCapture(Some(
                ("copy".to_string(), 0),
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

            state.dispatch(AppCommand::Editor(EditorCommand::SetSnapToGrid(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetSnapStep(0.5)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetSnapRotation(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetSnapRotationStep(30.0)));
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
                block_id: "core/lava".to_string(),
                color_tint: [0.2, 0.4, 0.8],
                trigger: None,
            };
            state.dispatch(AppCommand::Editor(EditorCommand::UpdateSelectedBlock(
                updated.clone(),
            )));
            assert_eq!(state.editor.objects[0].position, updated.position);
            assert_eq!(state.editor.objects[0].size, updated.size);
            assert_eq!(state.editor.objects[0].block_id, updated.block_id);

            state.dispatch(AppCommand::Editor(EditorCommand::CopyBlock));
            assert!(state.editor.runtime.interaction.clipboard.is_some());

            let before_paste = state.editor.objects.len();
            state.dispatch(AppCommand::Editor(EditorCommand::PasteBlock));
            assert!(state.editor.objects.len() >= before_paste);

            let before_duplicate = state.editor.objects.len();
            state.dispatch(AppCommand::Editor(EditorCommand::DuplicateBlock));
            assert!(state.editor.objects.len() >= before_duplicate);

            state.dispatch(AppCommand::Editor(EditorCommand::NudgeSelected {
                dx: 1,
                dy: 0,
            }));
            assert!(!state.editor.objects.is_empty());

            state.dispatch(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback));
            state.dispatch(AppCommand::Editor(EditorCommand::ToggleTimelinePlayback));

            state.dispatch(AppCommand::Editor(EditorCommand::SetTimelineDuration(4.0)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetTimelineTime(1.0)));
            state.dispatch(AppCommand::Editor(EditorCommand::AddTap));
            assert!(!state.editor.timeline.taps.tap_times.is_empty());
            state.dispatch(AppCommand::Editor(EditorCommand::AddTap));
            state.dispatch(AppCommand::Editor(EditorCommand::RemoveTapAt(1.0)));
            assert!(state.editor.timeline.taps.tap_times.is_empty());
            state.dispatch(AppCommand::Editor(EditorCommand::AddTap));
            state.dispatch(AppCommand::Editor(EditorCommand::RemoveTap));
            state.dispatch(AppCommand::Editor(EditorCommand::ClearTaps));
            assert!(state.editor.timeline.taps.tap_times.is_empty());

            state.dispatch(AppCommand::Editor(EditorCommand::AddTimingPoint {
                time_seconds: 0.5,
                bpm: 120.0,
            }));
            assert!(!state.editor.timing.timing_points.is_empty());
            state.dispatch(AppCommand::Editor(EditorCommand::SetTimingPointTime(
                0, 0.75,
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::SetTimingPointBpm(
                0, 140.0,
            )));
            state.dispatch(AppCommand::Editor(
                EditorCommand::SetTimingPointTimeSignature(0, 3, 4),
            ));
            state.dispatch(AppCommand::Editor(EditorCommand::SetTimingSelected(Some(
                0,
            ))));
            assert_eq!(state.editor.timing.timing_selected_index, Some(0));
            state.dispatch(AppCommand::Editor(EditorCommand::RemoveTimingPoint(0)));

            state.dispatch(AppCommand::Editor(EditorCommand::BpmTap));
            state.dispatch(AppCommand::Editor(EditorCommand::BpmTapReset));

            state.editor.ui.cursor = [7.0, 0.0, 9.0];
            state.dispatch(AppCommand::Editor(EditorCommand::SetSpawnHere));
            assert_eq!(state.editor.spawn.position, [7.0, 0.0, 9.0]);
            let old_dir = state.editor.spawn.direction;
            state.dispatch(AppCommand::Editor(EditorCommand::RotateSpawnDirection));
            assert_ne!(state.editor.spawn.direction, old_dir);

            state.dispatch(AppCommand::Editor(EditorCommand::RemoveBlock));
        });
    }

    #[test]
    fn test_dispatch_additional_command_variants_update_editor_state() {
        pollster::block_on(async {
            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::Editor(EditorCommand::SetShowMetadata(true)));
            assert!(state.editor_show_metadata());

            state.dispatch(AppCommand::Editor(EditorCommand::SetShowSettings(true)));
            assert!(state.editor_show_settings());
            state.dispatch(AppCommand::Editor(EditorCommand::SetSettingsSection(
                SettingsSection::Keybinds,
            )));
            assert_eq!(state.editor_settings_section(), SettingsSection::Keybinds);

            state.dispatch(AppCommand::Editor(EditorCommand::RenameLevel(
                "CmdDispatchLevel".to_string(),
            )));
            assert_eq!(
                state.editor_level_name().as_deref(),
                Some("CmdDispatchLevel")
            );

            state.dispatch(AppCommand::Editor(EditorCommand::UpdateMusic(
                MusicMetadata {
                    source: "dispatch.mp3".to_string(),
                    ..MusicMetadata::default()
                },
            )));
            assert_eq!(state.editor_music_metadata().source, "dispatch.mp3");

            state.dispatch(AppCommand::Editor(EditorCommand::SetTimelineDuration(5.0)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetTimelineTime(1.25)));
            assert_eq!(state.editor_timeline_duration_seconds(), 5.0);
            assert_eq!(state.editor_timeline_time_seconds(), 1.25);

            state.dispatch(AppCommand::Editor(EditorCommand::SetPlaybackSpeed(1.5)));
            assert_eq!(state.editor_playback_speed(), 1.5);
            state.dispatch(AppCommand::Editor(EditorCommand::SetWaveformZoom(3.5)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetWaveformScroll(1.25)));
            assert_eq!(state.editor_waveform_zoom(), 3.5);
            assert_eq!(state.editor_waveform_scroll(), 1.25);

            state.dispatch(AppCommand::Editor(EditorCommand::SetShiftHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetCtrlHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetAltHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetPanUpHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetPanDownHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetPanLeftHeld(true)));
            state.dispatch(AppCommand::Editor(EditorCommand::SetPanRightHeld(true)));
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

            state.dispatch(AppCommand::Editor(
                EditorCommand::SetSimulateTriggerHitboxes(true),
            ));
            assert!(state.editor_simulate_trigger_hitboxes());

            state.dispatch(AppCommand::Editor(EditorCommand::ToggleHitboxVisualization));
            assert!(state.editor_hitbox_visualization_enabled());

            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Timing,
            )));
            assert_eq!(state.editor_mode(), EditorMode::Timing);
            state.dispatch(AppCommand::Editor(EditorCommand::SetMode(
                EditorMode::Place,
            )));
            assert_eq!(state.editor_mode(), EditorMode::Place);

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybindCapture(Some(
                ("copy".to_string(), 0),
            ))));
            assert!(state.editor_keybind_capture_action().is_some());

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybind {
                action: "copy".to_string(),
                slot: 0,
                chord: KeyChord::new("x", true, false, false),
            }));
            assert!(!state.app_settings().keybinds_for_action("copy").is_empty());

            state.dispatch(AppCommand::Editor(EditorCommand::ClearKeybindSlot {
                action: "copy".to_string(),
                slot: 0,
            }));
            state.dispatch(AppCommand::Editor(EditorCommand::ResetKeybind(
                "copy".to_string(),
            )));
            state.dispatch(AppCommand::Editor(EditorCommand::ResetKeybinds));
        });
    }

    #[test]
    fn test_keyboard_keybind_capture_sets_or_cancels_capture() {
        pollster::block_on(async {
            use crate::commands::InputEvent;

            let mut state = new_editor_state().await;

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybindCapture(Some(
                ("copy".to_string(), 0),
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

            state.dispatch(AppCommand::Editor(EditorCommand::SetKeybindCapture(Some(
                ("paste".to_string(), 0),
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
