/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use super::State;
use crate::editor_domain::{
    build_editor_playtest_transition, build_playing_transition_from_metadata,
    derive_tap_indicator_positions, editor_session_init_from_metadata,
};
use crate::game::GameState;
use crate::import_export_service::{
    build_level_binary_export, build_level_export, parse_level_binary_import,
    parse_level_egz_import,
};
use crate::level_repository::load_builtin_level_metadata;
use crate::mesh::build_block_obj;
use crate::platform::io::{log_platform_error, read_editor_music_bytes};
use crate::platform::services::trigger_level_export;
use crate::types::{
    AppPhase, AppSettings, EditorMode, KeyChord, LevelMetadata, MusicMetadata, SettingsSection,
};
use base64::Engine as _;

impl State {
    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();

        self.gameplay.state = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            self.preload_runtime_audio(&level_name, &metadata.music.source);
            self.editor.set_triggers(metadata.resolved_triggers());
            self.editor.set_trigger_selected(None);
            self.session.playing_trigger_hitboxes = metadata.simulate_trigger_hitboxes;
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.gameplay.state.objects = transition.objects;
            self.gameplay.state.rebuild_behavior_cache();
            self.session.playing_trigger_base_objects = Some(self.gameplay.state.objects.clone());
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction, None);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn restart_level(&mut self) {
        self.stop_audio();
        self.gameplay.state = GameState::new();

        if self.session.playtesting_editor {
            let transition = build_editor_playtest_transition(
                &self.editor.objects,
                self.session.editor_level_name.as_deref(),
                self.editor.spawn.clone(),
                &self.editor.timeline.taps.tap_times,
                self.editor.triggers(),
                self.editor.simulate_trigger_hitboxes(),
                self.editor.timeline.clock.time_seconds,
            );
            let metadata = self.current_editor_metadata();
            let level_name = transition
                .playing_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            self.warmup_audio_at_seconds(
                &level_name,
                &metadata,
                transition.playtest_audio_start_seconds,
            );
            self.session.playtest_audio_start_seconds =
                Some(transition.playtest_audio_start_seconds);
            self.gameplay.state.objects = transition.objects;
            self.gameplay.state.rebuild_behavior_cache();
            self.session.playing_trigger_hitboxes = self.editor.simulate_trigger_hitboxes();
            self.session.playing_trigger_base_objects = Some(self.gameplay.state.objects.clone());
            self.apply_spawn_exact_to_game(
                transition.spawn_position,
                transition.spawn_direction,
                Some(transition.spawn_speed),
            );
            self.gameplay.state.elapsed_seconds = transition.playtest_audio_start_seconds;
        } else if let Some(level_name) = self.session.playing_level_name.clone() {
            self.session.playtest_audio_start_seconds = None;
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                self.editor.set_triggers(metadata.resolved_triggers());
                self.editor.set_trigger_selected(None);
                self.session.playing_trigger_hitboxes = metadata.simulate_trigger_hitboxes;
                let transition = build_playing_transition_from_metadata(metadata);
                self.gameplay.state.objects = transition.objects;
                self.gameplay.state.rebuild_behavior_cache();
                self.session.playing_trigger_base_objects =
                    Some(self.gameplay.state.objects.clone());
                self.apply_spawn_to_game(
                    transition.spawn_position,
                    transition.spawn_direction,
                    None,
                );
            }
        }

        self.gameplay.state.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    pub(super) fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let init = editor_session_init_from_metadata(self.load_level_metadata(&level_name));
        self.editor.objects = init.objects;
        self.editor.spawn = init.spawn;
        self.session.editor_music_metadata = init.music;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor.timing.timing_selected_index = None;
        self.editor.set_triggers(init.triggers);
        self.editor.set_trigger_selected(None);
        self.editor
            .set_simulate_trigger_hitboxes(init.simulate_trigger_hitboxes);
        self.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.timeline.taps.tap_times,
            &self.editor.objects,
        );
        self.editor.timeline.clock.time_seconds = init.timeline_time_seconds;
        self.editor.timeline.clock.duration_seconds = init.timeline_duration_seconds;
        self.editor.ui.cursor = init.cursor;
        self.editor.camera.editor_pan = init.camera_pan;
        self.editor.camera.editor_target_z = init.cursor[1];

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        load_builtin_level_metadata(level_name)
    }

    /// Exports the current editor level to the egakareta zip (.egz) format.
    ///
    /// This format bundles level metadata with the required audio file into a single binary blob.
    pub fn export_level_egz(&self) -> Result<Vec<u8>, String> {
        let metadata = self.current_editor_metadata();
        let audio_bytes = self
            .audio
            .state
            .editor
            .local_audio_cache
            .get(&metadata.music.source)
            .cloned()
            .or_else(|| {
                read_editor_music_bytes(
                    self.session.editor_level_name.as_deref(),
                    &metadata.music.source,
                )
            });

        build_level_export(&metadata, audio_bytes)
    }

    /// Imports a level from the egakareta Zip (.egz) binary format.
    ///
    /// This replaces the current editor level and caches any bundled audio data.
    pub fn import_level_egz(&mut self, data: &[u8]) -> Result<(), String> {
        let (metadata, audio_bytes) = parse_level_egz_import(data)?;
        if let Some(bytes) = audio_bytes {
            self.audio
                .state
                .editor
                .local_audio_cache
                .insert(metadata.music.source.clone(), bytes);
        }
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    /// Exports the current editor level metadata to binary bytes.
    pub fn export_level(&self) -> Result<Vec<u8>, String> {
        build_level_binary_export(&self.current_editor_metadata())
    }

    /// Imports level metadata from a binary payload.
    ///
    /// This replaces the current editor level metadata.
    pub fn import_level(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = parse_level_binary_import(data)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(crate::types::EditorStateParams {
            name: self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            music: self.session.editor_music_metadata.clone(),
            spawn: self.editor.spawn.clone(),
            tap_times: self.editor.timeline.taps.tap_times.clone(),
            timing_points: self.editor.timing.timing_points.clone(),
            timeline_time_seconds: self.editor.timeline.clock.time_seconds,
            timeline_duration_seconds: self.editor.timeline.clock.duration_seconds,
            triggers: self.editor.triggers().to_vec(),
            simulate_trigger_hitboxes: self.editor.simulate_trigger_hitboxes(),
            objects: self.editor.objects.clone(),
        })
    }

    pub(crate) fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
        let level_name = metadata.name.clone();
        let init = editor_session_init_from_metadata(Some(metadata));

        self.editor.objects = init.objects;
        self.editor.ui.selected_block_index = None;
        self.editor.ui.selected_block_indices.clear();
        self.editor.ui.hovered_block_index = None;
        self.editor.spawn = init.spawn;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor
            .timing
            .timing_points
            .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
        self.editor.timing.timing_selected_index = None;
        self.editor.set_triggers(init.triggers);
        self.editor.set_trigger_selected(None);
        self.editor
            .set_simulate_trigger_hitboxes(init.simulate_trigger_hitboxes);
        self.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.timeline.taps.tap_times,
            &self.editor.objects,
        );
        self.editor.timeline.clock.time_seconds = init.timeline_time_seconds;
        self.editor.timeline.clock.duration_seconds = init.timeline_duration_seconds;
        self.session.editor_level_name = Some(level_name);
        self.session.editor_music_metadata = init.music;
        self.editor.ui.cursor = init.cursor;
        self.editor.camera.editor_pan = init.camera_pan;
        self.editor.camera.editor_target_z = init.cursor[1];

        self.editor.runtime.history.undo.clear();
        self.editor.runtime.history.redo.clear();

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    /// Loads a built-in level by name into the editor.
    ///
    /// This stops any active audio and resets the editor's internal state
    /// before performing the import.
    pub fn load_builtin_level_into_editor(&mut self, name: &str) {
        if let Some(metadata) = self.load_level_metadata(name) {
            self.stop_audio();
            self.editor.timeline.playback.playing = false;
            self.editor.timeline.playback.runtime = None;
            self.editor.runtime.interaction.clipboard = None;
            self.apply_imported_level_metadata(metadata);
            self.session.editor_level_name = Some(name.to_string());

            if self.editor_mode() == EditorMode::Timing {
                self.load_waveform_for_current_audio();
            }
        }
    }

    /// Returns the name of the level currently being edited.
    pub fn editor_level_name(&self) -> Option<String> {
        self.session.editor_level_name.clone()
    }

    /// Sets the name for the level currently being edited.
    pub fn set_editor_level_name(&mut self, name: String) {
        self.session.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.session.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.session.editor_music_metadata = metadata;
    }

    /// Indicates whether the level import UI is currently visible.
    pub fn editor_show_import(&self) -> bool {
        self.session.editor_show_import
    }

    /// Toggles the visibility of the level import UI.
    pub fn set_editor_show_import(&mut self, show: bool) {
        self.session.editor_show_import = show;
    }

    pub(crate) fn editor_show_settings(&self) -> bool {
        self.session.editor_show_settings
    }

    pub(crate) fn set_editor_show_settings(&mut self, show: bool) {
        self.session.editor_show_settings = show;
        if !show {
            self.session.editor_keybind_capture_action = None;
        }
    }

    pub(crate) fn editor_settings_section(&self) -> SettingsSection {
        self.session.editor_settings_section
    }

    pub(crate) fn set_editor_settings_section(&mut self, section: SettingsSection) {
        self.session.editor_settings_section = section;
    }

    pub(crate) fn editor_keybind_capture_action(&self) -> Option<&(String, usize)> {
        self.session.editor_keybind_capture_action.as_ref()
    }

    pub(crate) fn set_editor_keybind_capture_action(&mut self, action: Option<(String, usize)>) {
        self.session.editor_keybind_capture_action = action;
    }

    pub(crate) fn app_settings(&self) -> &AppSettings {
        &self.session.app_settings
    }

    pub(crate) fn available_graphics_backends(&self) -> &[String] {
        &self.session.available_graphics_backends
    }

    pub(crate) fn available_audio_backends(&self) -> &[String] {
        &self.session.available_audio_backends
    }

    pub(crate) fn settings_restart_required(&self) -> bool {
        self.session.settings_restart_required
    }

    pub(crate) fn set_preferred_graphics_backend(&mut self, backend_name: String) {
        if self.session.app_settings.graphics_backend == backend_name {
            return;
        }

        self.session.app_settings.graphics_backend = backend_name;
        self.session.settings_restart_required = true;
        self.persist_app_settings();
    }

    pub(crate) fn set_preferred_audio_backend(&mut self, backend_name: String) {
        if !self
            .audio
            .state
            .runtime
            .set_preferred_backend_name(&backend_name)
        {
            return;
        }

        self.session.app_settings.audio_backend = backend_name;
        self.persist_app_settings();
    }

    pub(crate) fn set_keybind_for_action(&mut self, action: String, slot: usize, chord: KeyChord) {
        self.session
            .app_settings
            .set_keybind_at_slot(&action, slot, chord);
        self.persist_app_settings();
    }

    pub(crate) fn clear_keybind_slot_for_action(&mut self, action: &str, slot: usize) {
        self.session.app_settings.clear_keybind_slot(action, slot);
        self.persist_app_settings();
    }

    pub(crate) fn reset_keybind_for_action(&mut self, action: &str) {
        self.session.app_settings.reset_keybind(action);
        self.persist_app_settings();
    }

    pub(crate) fn reset_essential_keybinds(&mut self) {
        self.session.app_settings.reset_essential_keybinds();
        self.persist_app_settings();
    }

    pub(crate) fn persist_app_settings(&self) {
        let settings = self.session.app_settings.clone();
        crate::platform::task::spawn_background(async move {
            if let Err(error) = crate::platform::io::save_app_settings_to_storage(&settings).await {
                crate::platform::io::log_platform_error(&format!(
                    "Failed to persist app settings: {error}"
                ));
            }
        });
    }

    /// Returns the raw text content currently held in the editor's import buffer.
    pub fn editor_import_text(&self) -> &str {
        &self.session.editor_import_text
    }

    /// Sets the raw text content for the editor's import buffer.
    pub fn set_editor_import_text(&mut self, text: String) {
        self.session.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.session.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.session.editor_show_metadata = show;
    }

    /// Returns a list of all level names available in the application's built-in repository.
    pub fn available_levels(&self) -> &[String] {
        &self.menu.state.levels
    }

    /// Returns the name of the currently selected level in the menu.
    pub fn menu_level_name(&self) -> Option<&str> {
        self.menu
            .state
            .levels
            .get(self.menu.state.selected_level)
            .map(|s| s.as_str())
    }

    /// Triggers a platform-specific export of the current level as an `.egz` file.
    pub fn trigger_level_export(&self) {
        match self.export_level_egz() {
            Ok(data) => {
                let filename = format!(
                    "{}.egz",
                    self.editor_level_name()
                        .unwrap_or_else(|| "level".to_string())
                );

                trigger_level_export(&filename, &data);
            }
            Err(e) => {
                log_platform_error(&format!("Export failed: {}", e));
            }
        }
    }

    /// Triggers a platform-specific export of the currently selected block as an `.obj` 3D model.
    ///
    /// This is useful for exporting custom block geometry for use in other 3D software.
    pub fn trigger_selected_block_obj_export(&self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let Some(block) = self.editor_selected_block() else {
            log_platform_error("OBJ export failed: no selected block");
            return;
        };

        let sanitized_id = block
            .block_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();

        let object_name = if sanitized_id.is_empty() {
            "block".to_string()
        } else {
            sanitized_id
        };

        let filename = format!("{}_selected.obj", object_name);
        let obj = build_block_obj(&block, &object_name);

        trigger_level_export(&filename, obj.as_bytes());
    }

    /// Finalizes the level import process by decoding and parsing the current import text.
    ///
    /// The input text is expected to be Base64-encoded `.egz` or binary metadata bytes.
    pub fn complete_import(&mut self) {
        let text = self.session.editor_import_text.clone();
        let data = match base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            Ok(data) => data,
            Err(error) => {
                log_platform_error(&format!(
                    "Binary import expects Base64 input (.egz or binary metadata): {}",
                    error
                ));
                return;
            }
        };

        if self.import_level_egz(&data).is_ok() {
            self.session.editor_show_import = false;
            self.session.editor_import_text.clear();
            return;
        }

        if let Err(error) = self.import_level(&data) {
            log_platform_error(&format!("Binary import failed: {}", error));
        } else {
            self.session.editor_show_import = false;
            self.session.editor_import_text.clear();
        }
    }
}
