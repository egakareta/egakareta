/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use eg_codec::{decode_level_metadata_binary, encode_level_metadata_binary};
use eg_editor_domain::{
    EditorPlaytestTransitionParams, build_editor_playtest_transition,
    derive_tap_indicator_positions, editor_session_init_from_metadata,
};
use eg_game::GameState;
use eg_level::{build_level_export, load_builtin_level_metadata, read_metadata_from_egz};
use eg_platform::io::{read_editor_music_bytes, save_level_export};
use eg_platform::task::spawn_background;
use eg_types::{AppPhase, AppSettings, EditorMode, KeyChord, KeybindBinding, SettingsSection};
use eg_types::{
    BasicLevelMetadata, Direction, EditorStateParams, LevelMetadata, LevelObject,
    LevelPreviewCameraMetadata, MusicMetadata, SpawnMetadata,
};
#[cfg(test)]
use eg_types::{DEFAULT_MENU_PREVIEW_CAMERA_POSITION, DEFAULT_MENU_PREVIEW_CAMERA_TARGET};
use glam::Vec3;

impl State {
    pub(super) fn apply_playing_transition_objects(
        &mut self,
        objects: Vec<LevelObject>,
        level_duration_seconds: f32,
    ) {
        self.gameplay.state.objects = objects;
        self.gameplay.state.rebuild_behavior_cache();
        self.gameplay.state.initialize_level_progress_from_objects();
        self.gameplay
            .state
            .set_level_duration_seconds(level_duration_seconds);
        self.session.playing_trigger_base_objects = Some(self.gameplay.state.objects.clone());
    }

    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();

        self.gameplay.state = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            self.preload_runtime_audio(&level_name, "audio.mp3");
            self.warmup_audio_at_seconds(&level_name, 0.0);
            self.editor.objects = metadata.objects.clone();
            self.editor.sync_trigger_selection_from_objects();
            self.editor.set_trigger_selected(None);
            log::debug!("Starting level: {}", metadata.basic.name);
            self.session.playing_sky_color = metadata.sky_color;
            self.apply_playing_transition_objects(
                metadata.objects,
                metadata.timeline_duration_seconds,
            );
            self.apply_spawn_to_game(metadata.spawn.position, metadata.spawn.direction, None);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn restart_level(&mut self) {
        self.session.game_paused = false;
        self.session.practice_mode_enabled = false;
        self.session.practice_checkpoints.clear();
        self.gameplay.death_sfx_played = false;
        self.rebuild_practice_checkpoint_vertices();
        self.stop_audio();
        self.clear_pending_gameplay_inputs();
        self.gameplay.state = GameState::new();

        if self.session.playtesting_editor {
            let transition = build_editor_playtest_transition(EditorPlaytestTransitionParams {
                objects: &self.editor.objects,
                level_name: self.session.editor_level_name.as_deref(),
                spawn: self.editor.spawn.clone(),
                sky_color: self.session.editor_sky_color,
                tap_times: &self.editor.timeline.taps.tap_times,
                timeline_seconds: (
                    self.editor.timeline.clock.time_seconds,
                    self.editor.timeline.clock.duration_seconds,
                ),
            });
            let level_name = transition
                .playing_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            self.warmup_audio_at_seconds(&level_name, transition.playtest_audio_start_seconds);
            self.session.playtest_audio_start_seconds =
                Some(transition.playtest_audio_start_seconds);
            self.session.playing_sky_color = transition.sky_color;
            self.apply_playing_transition_objects(
                transition.objects,
                transition.level_duration_seconds,
            );
            self.apply_spawn_exact_to_game(
                transition.spawn_position,
                transition.spawn_direction,
                Some(transition.spawn_speed),
            );
            self.gameplay.state.vertical_velocity = transition.spawn_vertical_velocity;
            self.gameplay.state.is_grounded = transition.spawn_is_grounded;
            self.gameplay.state.elapsed_seconds = transition.playtest_audio_start_seconds;
        } else if let Some(level_name) = self.session.playing_level_name.clone() {
            self.session.playtest_audio_start_seconds = None;
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                self.editor.objects = metadata.objects.clone();
                self.editor.sync_trigger_selection_from_objects();
                self.editor.set_trigger_selected(None);
                self.session.playing_sky_color = metadata.sky_color;
                self.apply_playing_transition_objects(
                    metadata.objects,
                    metadata.timeline_duration_seconds,
                );
                self.apply_spawn_to_game(metadata.spawn.position, metadata.spawn.direction, None);
            }
        }

        self.gameplay.state.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    pub(super) fn respawn_from_practice_checkpoint(&mut self) -> bool {
        if self.phase != AppPhase::Playing
            || self.session.playtesting_editor
            || !self.session.practice_mode_enabled
        {
            return false;
        }

        let Some(checkpoint) = self.session.practice_checkpoints.last().cloned() else {
            self.restart_level();
            self.session.practice_mode_enabled = true;
            self.session.practice_checkpoints.clear();
            return true;
        };

        self.stop_audio();
        self.clear_pending_gameplay_inputs();
        self.gameplay
            .state
            .restore_checkpoint_state(&checkpoint.gameplay);
        self.gameplay.death_sfx_played = false;
        self.session.playing_trigger_base_objects = checkpoint.trigger_base_objects;

        if let Some(level_name) = self.session.playing_level_name.clone() {
            self.warmup_audio_at_seconds(&level_name, self.gameplay.state.elapsed_seconds);
        }

        self.rebuild_block_vertices();
        true
    }

    pub(super) fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let metadata = self.load_level_metadata(&level_name);
        self.preload_runtime_audio(&level_name, "audio.mp3");
        self.warmup_audio_at_seconds(&level_name, 0.0);

        let init = editor_session_init_from_metadata(metadata);
        self.editor.objects = init.objects;
        self.editor.spawn = init.spawn;
        self.session.editor_music_metadata = init.music;
        self.session.editor_creator_metadata = init.creator_metadata;
        self.session.editor_sky_color = init.sky_color;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor.timing.mark_timing_points_changed();
        self.editor.timing.timing_selected_index = None;
        self.editor.sync_trigger_selection_from_objects();
        self.editor.set_trigger_selected(None);
        self.session.editor_menu_preview_camera = init.menu_preview_camera;
        self.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
            self.editor.spawn.position,
            self.editor.spawn.direction,
            &self.editor.timeline.taps.tap_times,
            &self.editor.objects,
        );
        self.editor.timeline.clock.time_seconds = init.timeline_time_seconds;
        self.editor.timeline.clock.duration_seconds = init.timeline_duration_seconds;
        self.editor.input.cursor = init.cursor;
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
            .cache
            .bytes_for_level_audio(&metadata.basic.name)
            .or_else(|| read_editor_music_bytes("audio.mp3"));

        build_level_export(&metadata, audio_bytes)
    }

    /// Imports a level from the egakareta Zip (.egz) binary format.
    ///
    /// This replaces the current editor level and caches any bundled audio data.
    pub fn import_level_egz(&mut self, data: &[u8]) -> Result<(), String> {
        let (metadata, audio_bytes) = read_metadata_from_egz(data)?;
        if let Some(bytes) = audio_bytes {
            self.audio
                .state
                .editor
                .cache
                .insert_level_audio(&metadata.basic.name, bytes);
        }
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    /// Exports the current editor level metadata to binary bytes.
    pub fn export_level(&self) -> Result<Vec<u8>, String> {
        encode_level_metadata_binary(&self.current_editor_metadata())
    }

    /// Imports level metadata from a binary payload.
    ///
    /// This replaces the current editor level metadata.
    pub fn import_level(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = decode_level_metadata_binary(data)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        let mut basic = self.session.editor_creator_metadata.clone();
        if let Some(name) = &self.session.editor_level_name {
            basic.name = name.clone();
        }

        LevelMetadata::from_editor_state(EditorStateParams {
            basic,
            music: self.session.editor_music_metadata.clone(),
            spawn: self.editor.spawn.clone(),
            sky_color: self.session.editor_sky_color,
            tap_times: self.editor.timeline.taps.tap_times.clone(),
            timing_points: self.editor.timing.timing_points.clone(),
            timeline_time_seconds: self.editor.timeline.clock.time_seconds,
            timeline_duration_seconds: self.editor.timeline.clock.duration_seconds,
            menu_preview_camera: self.session.editor_menu_preview_camera.clone(),
            objects: self.editor.objects.clone(),
        })
    }

    pub fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
        let level_name = metadata.basic.name.clone();
        let init = editor_session_init_from_metadata(Some(metadata));

        self.editor.objects = init.objects;
        self.editor.clear_block_selection();
        self.editor.spawn = init.spawn;
        self.editor.timeline.taps.tap_times = init.tap_times;
        self.editor.timing.timing_points = init.timing_points;
        self.editor.timing.mark_timing_points_changed();
        self.editor
            .timing
            .timing_points
            .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
        self.editor.timing.timing_selected_index = None;
        self.editor.sync_trigger_selection_from_objects();
        self.editor.set_trigger_selected(None);
        self.session.editor_menu_preview_camera = init.menu_preview_camera;
        self.session.editor_creator_metadata = init.creator_metadata;
        self.session.editor_sky_color = init.sky_color;
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
        self.editor.input.cursor = init.cursor;
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

    pub fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.session.editor_music_metadata
    }

    pub fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.session.editor_music_metadata = metadata;
    }

    pub fn editor_creator_metadata(&self) -> &BasicLevelMetadata {
        &self.session.editor_creator_metadata
    }

    pub fn set_editor_creator_metadata(&mut self, metadata: BasicLevelMetadata) {
        self.session.editor_creator_metadata = metadata;
    }

    pub fn editor_sky_color(&self) -> [f32; 3] {
        self.session.editor_sky_color
    }

    pub fn set_editor_sky_color(&mut self, color: [f32; 3]) {
        self.session.editor_sky_color = color.map(|component| component.clamp(0.0, 1.0));
    }

    pub fn editor_menu_preview_camera(&self) -> Option<&LevelPreviewCameraMetadata> {
        self.session.editor_menu_preview_camera.as_ref()
    }

    pub fn set_editor_menu_preview_camera(&mut self, camera: Option<LevelPreviewCameraMetadata>) {
        self.session.editor_menu_preview_camera = camera;
    }

    pub fn editor_capture_menu_preview_camera(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let target = Vec3::new(
            self.editor.camera.editor_pan[0],
            self.editor.camera.editor_target_z,
            self.editor.camera.editor_pan[1],
        );
        let position = (target + self.editor_camera_offset()).to_array();
        self.session.editor_menu_preview_camera = Some(LevelPreviewCameraMetadata {
            position,
            target: target.to_array(),
        });
    }

    pub fn editor_use_auto_menu_preview_camera(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }
        self.session.editor_menu_preview_camera = None;
    }

    #[cfg(test)]
    pub(super) fn refresh_menu_level_preview_if_needed(&mut self) {
        if self.phase != AppPhase::Menu {
            return;
        }

        if self.menu.state.preview_level_index == Some(self.menu.state.selected_level) {
            return;
        }

        self.menu.state.preview_level_index = Some(self.menu.state.selected_level);

        let Some(level_name) = self.menu.state.levels.get(self.menu.state.selected_level) else {
            self.reset_menu_preview_to_default_scene();
            return;
        };

        let Some(metadata) = self.load_level_metadata(level_name) else {
            self.reset_menu_preview_to_default_scene();
            return;
        };

        let (position, target) = menu_preview_camera_for_metadata(&metadata);
        self.gameplay.state.objects = metadata.objects;
        self.gameplay.state.rebuild_behavior_cache();
        self.menu.state.preview_sky_color = metadata.sky_color;
        self.menu.state.preview_camera_position = position;
        self.menu.state.preview_camera_target = target;
        self.rebuild_block_vertices();
    }

    #[cfg(test)]
    fn reset_menu_preview_to_default_scene(&mut self) {
        self.gameplay.state.objects = vec![];
        self.gameplay.state.rebuild_behavior_cache();
        self.menu.state.preview_sky_color = LevelMetadata::default().sky_color;
        self.menu.state.preview_camera_position = DEFAULT_MENU_PREVIEW_CAMERA_POSITION;
        self.menu.state.preview_camera_target = DEFAULT_MENU_PREVIEW_CAMERA_TARGET;
        self.rebuild_block_vertices();
    }

    pub fn editor_show_settings(&self) -> bool {
        self.session.editor_show_settings
    }

    pub fn set_editor_show_settings(&mut self, show: bool) {
        self.session.editor_show_settings = show;
        if !show {
            self.session.editor_keybind_capture_action = None;
        }
    }

    pub fn editor_settings_section(&self) -> SettingsSection {
        self.session.editor_settings_section
    }

    pub fn set_editor_settings_section(&mut self, section: SettingsSection) {
        self.session.editor_settings_section = section;
    }

    pub fn editor_keybind_capture_action(&self) -> Option<&(String, usize)> {
        self.session.editor_keybind_capture_action.as_ref()
    }

    pub fn set_editor_keybind_capture_action(&mut self, action: Option<(String, usize)>) {
        self.session.editor_keybind_capture_action = action;
    }

    pub fn app_settings(&self) -> &AppSettings {
        &self.session.app_settings
    }

    pub fn menu_selected_level_progress(&self) -> Option<eg_types::PlayerLevelProgress> {
        let level_name = self.menu_level_name()?;
        self.session
            .app_settings
            .level_progress
            .get(level_name)
            .copied()
    }

    pub fn record_current_level_progress(&mut self) {
        if self.session.playtesting_editor {
            return;
        }

        let Some(level_name) = self.session.playing_level_name.clone() else {
            return;
        };

        let progress = self.gameplay.state.level_progress();
        if self
            .session
            .app_settings
            .update_level_progress(&level_name, progress)
        {
            self.persist_app_settings();
        }
    }

    pub fn available_graphics_backends(&self) -> &[String] {
        &self.session.available_graphics_backends
    }

    pub fn available_audio_backends(&self) -> &[String] {
        &self.session.available_audio_backends
    }

    pub fn settings_restart_required(&self) -> bool {
        self.session.settings_restart_required
    }

    pub fn graphics_backend_locked(&self) -> bool {
        self.session.graphics_backend_locked
    }

    pub fn audio_backend_locked(&self) -> bool {
        self.session.audio_backend_locked
    }

    pub fn set_preferred_graphics_backend(&mut self, backend_name: String) {
        if self.session.graphics_backend_locked {
            return;
        }

        if self.session.app_settings.graphics_backend == backend_name {
            return;
        }

        self.session.app_settings.graphics_backend = backend_name;
        self.session.settings_restart_required = true;
        self.persist_app_settings();
    }

    pub fn set_ui_scale_multiplier(&mut self, multiplier: f32) {
        let clamped = AppSettings::clamp_ui_scale_multiplier(multiplier);
        if (self.session.app_settings.ui_scale_multiplier - clamped).abs() <= 1e-6 {
            return;
        }
        self.session.app_settings.ui_scale_multiplier = clamped;
        self.persist_app_settings();
    }

    pub fn set_keybind_for_action(&mut self, action: String, slot: usize, chord: KeyChord) {
        self.session
            .app_settings
            .set_keybind_at_slot(&action, slot, chord);
        self.persist_app_settings();
    }

    pub fn clear_keybind_slot_for_action(&mut self, action: &str, slot: usize) {
        self.session.app_settings.clear_keybind_slot(action, slot);
        self.persist_app_settings();
    }

    pub fn reset_keybind_for_action(&mut self, action: &str) {
        self.session.app_settings.reset_keybind(action);
        self.persist_app_settings();
    }

    pub fn reset_essential_keybinds(&mut self) {
        self.session.app_settings.reset_essential_keybinds();
        self.persist_app_settings();
    }

    pub fn set_app_settings_keybinds(&mut self, keybinds: Vec<KeybindBinding>) {
        if self.session.app_settings.keybinds == keybinds {
            return;
        }
        self.session.app_settings.keybinds = keybinds;
        self.persist_app_settings();
    }

    pub fn persist_app_settings(&self) {
        let settings = self.session.app_settings.clone();
        eg_platform::task::spawn_background(async move {
            if let Err(error) = eg_platform::io::save_app_settings_to_storage(&settings).await {
                log::error!("{}", &format!("Failed to persist app settings: {error}"));
            }
        });
    }

    pub fn editor_show_metadata(&self) -> bool {
        self.session.editor_show_metadata
    }

    pub fn set_editor_show_metadata(&mut self, show: bool) {
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn menu_selected_level_index(&self) -> usize {
        self.menu.state.selected_level
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_menu_selected_level_index(&mut self, selected_level: usize) {
        self.menu.state.selected_level = selected_level;
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

                spawn_background(async move {
                    if let Err(error) = save_level_export(&filename, &data).await {
                        log::error!("Export failed: {}", error);
                    }
                });
            }
            Err(e) => {
                log::error!("Export failed: {}", e);
            }
        }
    }
}

pub(crate) fn menu_preview_camera_for_metadata(metadata: &LevelMetadata) -> ([f32; 3], [f32; 3]) {
    metadata
        .menu_preview_camera
        .as_ref()
        .map(|preview| (preview.position, preview.target))
        .unwrap_or_else(|| auto_menu_preview_camera_from_spawn(&metadata.spawn))
}

fn auto_menu_preview_camera_from_spawn(spawn: &SpawnMetadata) -> ([f32; 3], [f32; 3]) {
    // Tuned to keep a broad, readable menu composition:
    // - target a little ahead/above the spawn so the route direction is visible
    // - place camera behind and to the side with elevated height for coverage
    const PREVIEW_TARGET_FORWARD_DISTANCE: f32 = 10.0;
    const PREVIEW_TARGET_HEIGHT_OFFSET: f32 = 1.0;
    const PREVIEW_CAMERA_BACK_DISTANCE: f32 = 20.0;
    const PREVIEW_CAMERA_SIDE_OFFSET: f32 = 12.0;
    const PREVIEW_CAMERA_HEIGHT_OFFSET: f32 = 14.0;

    let spawn_position = Vec3::from_array(spawn.position);
    let forward = match spawn.direction {
        Direction::Forward => Vec3::new(0.0, 0.0, 1.0),
        Direction::Right => Vec3::new(1.0, 0.0, 0.0),
    };
    let side = Vec3::new(forward.z, 0.0, -forward.x);
    let target = spawn_position
        + forward * PREVIEW_TARGET_FORWARD_DISTANCE
        + Vec3::new(0.0, PREVIEW_TARGET_HEIGHT_OFFSET, 0.0);
    let eye = spawn_position - forward * PREVIEW_CAMERA_BACK_DISTANCE
        + side * PREVIEW_CAMERA_SIDE_OFFSET
        + Vec3::new(0.0, PREVIEW_CAMERA_HEIGHT_OFFSET, 0.0);
    (eye.to_array(), target.to_array())
}

#[cfg(test)]
mod tests {
    use super::{auto_menu_preview_camera_from_spawn, menu_preview_camera_for_metadata};
    use eg_types::{
        BasicLevelMetadata, Direction, EditorStateParams, LevelMetadata,
        LevelPreviewCameraMetadata, MusicMetadata, SpawnMetadata,
    };

    #[test]
    fn menu_preview_camera_for_metadata_prefers_manual_camera() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            basic: BasicLevelMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: LevelMetadata::default().sky_color,
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            menu_preview_camera: Some(LevelPreviewCameraMetadata {
                position: [1.0, 2.0, 3.0],
                target: [4.0, 5.0, 6.0],
            }),
            objects: Vec::new(),
        });

        assert_eq!(
            menu_preview_camera_for_metadata(&metadata),
            ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])
        );
    }

    #[test]
    fn auto_menu_preview_camera_positions_follow_spawn_direction() {
        let (forward_eye, forward_target) = auto_menu_preview_camera_from_spawn(&SpawnMetadata {
            position: [2.0, 3.0, 4.0],
            direction: Direction::Forward,
        });
        assert_eq!(forward_eye, [14.0, 17.0, -16.0]);
        assert_eq!(forward_target, [2.0, 4.0, 14.0]);

        let (right_eye, right_target) = auto_menu_preview_camera_from_spawn(&SpawnMetadata {
            position: [2.0, 3.0, 4.0],
            direction: Direction::Right,
        });
        assert_eq!(right_eye, [-18.0, 17.0, -8.0]);
        assert_eq!(right_target, [12.0, 4.0, 4.0]);
    }
}
