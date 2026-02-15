use super::State;
use crate::editor_domain::{
    build_editor_playtest_transition, build_playing_transition_from_metadata,
    derive_tap_indicator_positions, editor_session_init_from_metadata,
};
use crate::game::GameState;
use crate::import_export_service::{
    build_level_export, build_level_json_export, parse_level_import, parse_level_ldz_import,
};
use crate::level_repository::load_builtin_level_metadata;
use crate::mesh::build_block_obj;
use crate::platform::io::{log_platform_error, read_editor_music_bytes};
use crate::platform::services::trigger_level_export;
use crate::types::{AppPhase, LevelMetadata, MusicMetadata};
use base64::Engine as _;

impl State {
    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.state.levels[index].clone();

        self.gameplay.state = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.gameplay.state.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
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
                self.editor.timeline.clock.time_seconds,
            );
            self.session.playtest_audio_start_seconds =
                Some(transition.playtest_audio_start_seconds);
            self.gameplay.state.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        } else if let Some(level_name) = self.session.playing_level_name.clone() {
            self.session.playtest_audio_start_seconds = None;
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let transition = build_playing_transition_from_metadata(metadata);
                self.gameplay.state.objects = transition.objects;
                self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
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

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn load_level_metadata(&self, level_name: &str) -> Option<LevelMetadata> {
        load_builtin_level_metadata(level_name)
    }

    pub fn export_level_ldz(&self) -> Result<Vec<u8>, String> {
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

    pub fn import_level_ldz(&mut self, data: &[u8]) -> Result<(), String> {
        let (metadata, audio_bytes) = parse_level_ldz_import(data)?;
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

    pub fn export_level(&self) -> String {
        build_level_json_export(&self.current_editor_metadata())
    }

    pub fn import_level(&mut self, json: &str) -> Result<(), String> {
        let metadata = parse_level_import(json)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(
            self.session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            self.session.editor_music_metadata.clone(),
            self.editor.spawn.clone(),
            self.editor.timeline.taps.tap_times.clone(),
            self.editor.timing.timing_points.clone(),
            self.editor.timeline.clock.time_seconds,
            self.editor.timeline.clock.duration_seconds,
            self.editor.objects.clone(),
        )
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

        self.editor.runtime.history.undo.clear();
        self.editor.runtime.history.redo.clear();

        self.sync_editor_objects();
        self.set_editor_timeline_time_seconds(self.editor.timeline.clock.time_seconds);
        self.rebuild_spawn_marker_vertices();
    }

    pub fn load_builtin_level_into_editor(&mut self, name: &str) {
        if let Some(metadata) = self.load_level_metadata(name) {
            self.stop_audio();
            self.editor.timeline.playback.playing = false;
            self.editor.timeline.playback.runtime = None;
            self.editor.runtime.interaction.clipboard = None;
            let _ = self.import_level(&serde_json::to_string(&metadata).unwrap());
            self.session.editor_level_name = Some(name.to_string());
        }
    }

    pub fn editor_level_name(&self) -> Option<String> {
        self.session.editor_level_name.clone()
    }

    pub fn set_editor_level_name(&mut self, name: String) {
        self.session.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.session.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.session.editor_music_metadata = metadata;
    }

    pub fn editor_show_import(&self) -> bool {
        self.session.editor_show_import
    }

    pub fn set_editor_show_import(&mut self, show: bool) {
        self.session.editor_show_import = show;
    }

    pub fn editor_import_text(&self) -> &str {
        &self.session.editor_import_text
    }

    pub fn set_editor_import_text(&mut self, text: String) {
        self.session.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.session.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.session.editor_show_metadata = show;
    }

    pub fn available_levels(&self) -> &[String] {
        &self.menu.state.levels
    }

    pub fn trigger_level_export(&self) {
        match self.export_level_ldz() {
            Ok(data) => {
                let filename = format!(
                    "{}.ldz",
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

    pub fn complete_import(&mut self) {
        let text = self.session.editor_import_text.clone();
        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            if let Err(e) = self.import_level_ldz(&data) {
                log_platform_error(&format!("LDZ Import failed: {}", e));
            } else {
                self.session.editor_show_import = false;
                self.session.editor_import_text.clear();
                return;
            }
        }

        let text = self.session.editor_import_text.clone();
        if let Err(e) = self.import_level(&text) {
            log_platform_error(&format!("JSON Import failed: {}", e));
        } else {
            self.session.editor_show_import = false;
            self.session.editor_import_text.clear();
        }
    }
}
