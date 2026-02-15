use base64::Engine as _;

use super::State;
use crate::editor_domain::{
    build_editor_playtest_transition, build_playing_transition_from_metadata,
    derive_tap_indicator_positions, editor_session_init_from_metadata,
};
use crate::game::GameState;
use crate::level_repository::{
    build_ldz_archive, load_builtin_level_metadata, parse_level_metadata_json,
    read_metadata_from_ldz, serialize_level_metadata_pretty,
};
use crate::mesh::build_block_obj;
use crate::platform::io::{log_platform_error, read_editor_music_bytes, save_level_export};
use crate::types::{AppPhase, LevelMetadata, MusicMetadata};

impl State {
    pub(super) fn start_level(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();

        self.game = GameState::new();
        self.enter_playing_phase(Some(level_name.clone()), false);

        self.stop_audio();

        if let Some(metadata) = self.load_level_metadata(&level_name) {
            let transition = build_playing_transition_from_metadata(metadata);
            log::debug!("Starting level: {}", transition.level_name);
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        }

        self.rebuild_block_vertices();
        self.rebuild_editor_cursor_vertices();
        self.rebuild_spawn_marker_vertices();
    }

    pub(super) fn restart_level(&mut self) {
        self.stop_audio();
        self.game = GameState::new();

        if self.editor.session.playtesting_editor {
            let transition = build_editor_playtest_transition(
                &self.editor.objects,
                self.editor.session.editor_level_name.as_deref(),
                self.editor.spawn.clone(),
                &self.editor.timeline.taps.tap_times,
                self.editor.timeline.clock.time_seconds,
            );
            self.game.objects = transition.objects;
            self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
        } else if let Some(level_name) = self.editor.session.playing_level_name.clone() {
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let transition = build_playing_transition_from_metadata(metadata);
                self.game.objects = transition.objects;
                self.apply_spawn_to_game(transition.spawn_position, transition.spawn_direction);
            }
        }

        self.game.started = false;
        self.reset_playing_camera_defaults();
        self.rebuild_block_vertices();
    }

    pub(super) fn start_editor(&mut self, index: usize) {
        let level_name = self.menu.levels[index].clone();
        self.stop_audio();

        self.enter_editor_phase(level_name.clone());

        let init = editor_session_init_from_metadata(self.load_level_metadata(&level_name));
        self.editor.objects = init.objects;
        self.editor.spawn = init.spawn;
        self.editor.session.editor_music_metadata = init.music;
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

    pub(super) fn stop_audio(&mut self) {
        self.audio_state.runtime.stop();
    }

    pub(super) fn start_audio(&mut self, level_name: &str, metadata: &LevelMetadata) {
        self.start_audio_at_seconds(level_name, metadata, 0.0);
    }

    pub(super) fn start_audio_at_seconds(
        &mut self,
        level_name: &str,
        metadata: &LevelMetadata,
        start_seconds: f32,
    ) {
        if let Some(bytes) = self
            .audio_state
            .editor
            .local_audio_cache
            .get(&metadata.music.source)
        {
            self.audio_state.runtime.start_with_bytes_at(
                &metadata.music.source,
                bytes,
                start_seconds,
            );
        } else {
            self.audio_state
                .runtime
                .start_at(level_name, &metadata.music.source, start_seconds);
        }
    }

    pub fn trigger_audio_import(&self) {
        let sender = self.audio_state.editor.audio_import_channel.0.clone();
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                    let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                    let _ = sender.send((filename, bytes));
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                pollster::block_on(async {
                    if let Some((filename, bytes)) = crate::platform::io::pick_audio_file().await {
                        let _ = crate::platform::io::save_audio_to_storage(&filename, &bytes).await;
                        let _ = sender.send((filename, bytes));
                    }
                });
            });
        }
    }

    pub(super) fn update_audio_imports(&mut self) {
        while let Ok((filename, bytes)) = self.audio_state.editor.audio_import_channel.1.try_recv()
        {
            self.editor.session.editor_music_metadata.source = filename.clone();
            self.audio_state
                .editor
                .local_audio_cache
                .insert(filename, bytes);
            self.audio_state
                .editor
                .waveform_cache
                .remove(&self.editor.session.editor_music_metadata.source);
            self.audio_state.editor.waveform_loading_source = None;
            self.load_waveform_for_current_audio();
        }
    }

    pub(super) fn update_waveform_loading(&mut self) {
        while let Ok((source, decoded)) = self.audio_state.editor.waveform_load_channel.1.try_recv()
        {
            if let Some((samples, sample_rate)) = decoded {
                self.audio_state
                    .editor
                    .waveform_cache
                    .insert(source.clone(), (samples.clone(), sample_rate));

                if source != self.editor.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples = samples;
                self.editor.timing.waveform_sample_rate = sample_rate;
            } else {
                if source != self.editor.session.editor_music_metadata.source {
                    continue;
                }

                self.editor.timing.waveform_samples.clear();
                self.editor.timing.waveform_sample_rate = 0;
            }

            if self.audio_state.editor.waveform_loading_source.as_deref() == Some(source.as_str()) {
                self.audio_state.editor.waveform_loading_source = None;
            }
        }
    }

    pub fn export_level_ldz(&self) -> Result<Vec<u8>, String> {
        let metadata = self.current_editor_metadata();
        let audio_bytes = self
            .audio_state
            .editor
            .local_audio_cache
            .get(&metadata.music.source)
            .cloned()
            .or_else(|| {
                read_editor_music_bytes(
                    self.editor.session.editor_level_name.as_deref(),
                    &metadata.music.source,
                )
            });
        let audio_file = audio_bytes
            .as_ref()
            .map(|bytes| (metadata.music.source.as_str(), bytes.as_slice()));

        build_ldz_archive(&metadata, audio_file)
    }

    pub fn import_level_ldz(&mut self, data: &[u8]) -> Result<(), String> {
        let metadata = read_metadata_from_ldz(data)?;
        self.apply_imported_level_metadata(metadata);
        Ok(())
    }

    pub fn export_level(&self) -> String {
        serialize_level_metadata_pretty(&self.current_editor_metadata()).unwrap_or_default()
    }

    pub fn import_level(&mut self, json: &str) -> Result<(), String> {
        let metadata = parse_level_metadata_json(json)?;
        self.apply_imported_level_metadata(metadata);

        Ok(())
    }

    pub(super) fn current_editor_metadata(&self) -> LevelMetadata {
        LevelMetadata::from_editor_state(
            self.editor
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            self.editor.session.editor_music_metadata.clone(),
            self.editor.spawn.clone(),
            self.editor.timeline.taps.tap_times.clone(),
            self.editor.timing.timing_points.clone(),
            self.editor.timeline.clock.time_seconds,
            self.editor.timeline.clock.duration_seconds,
            self.editor.objects.clone(),
        )
    }

    fn apply_imported_level_metadata(&mut self, metadata: LevelMetadata) {
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
        self.editor.session.editor_level_name = Some(level_name);
        self.editor.session.editor_music_metadata = init.music;
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
            let _ = self.import_level(&serde_json::to_string(&metadata).unwrap());
            self.editor.session.editor_level_name = Some(name.to_string());
        }
    }

    pub fn editor_level_name(&self) -> Option<String> {
        self.editor.session.editor_level_name.clone()
    }

    pub fn set_editor_level_name(&mut self, name: String) {
        self.editor.session.editor_level_name = Some(name);
    }

    pub(crate) fn editor_music_metadata(&self) -> &MusicMetadata {
        &self.editor.session.editor_music_metadata
    }

    pub(crate) fn set_editor_music_metadata(&mut self, metadata: MusicMetadata) {
        self.editor.session.editor_music_metadata = metadata;
    }

    pub fn editor_show_import(&self) -> bool {
        self.editor.session.editor_show_import
    }

    pub fn set_editor_show_import(&mut self, show: bool) {
        self.editor.session.editor_show_import = show;
    }

    pub fn editor_import_text(&self) -> &str {
        &self.editor.session.editor_import_text
    }

    pub fn set_editor_import_text(&mut self, text: String) {
        self.editor.session.editor_import_text = text;
    }

    pub(crate) fn editor_show_metadata(&self) -> bool {
        self.editor.session.editor_show_metadata
    }

    pub(crate) fn set_editor_show_metadata(&mut self, show: bool) {
        self.editor.session.editor_show_metadata = show;
    }

    pub fn available_levels(&self) -> &[String] {
        &self.menu.levels
    }

    pub fn trigger_level_export(&self) {
        match self.export_level_ldz() {
            Ok(data) => {
                let filename = format!(
                    "{}.ldz",
                    self.editor_level_name()
                        .unwrap_or_else(|| "level".to_string())
                );

                if let Err(error) = save_level_export(&filename, &data) {
                    log_platform_error(&format!("Export failed: {}", error));
                }
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

        if let Err(error) = save_level_export(&filename, obj.as_bytes()) {
            log_platform_error(&format!("OBJ export failed: {}", error));
        }
    }

    pub fn complete_import(&mut self) {
        let text = self.editor.session.editor_import_text.clone();
        if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            if let Err(e) = self.import_level_ldz(&data) {
                log_platform_error(&format!("LDZ Import failed: {}", e));
            } else {
                self.editor.session.editor_show_import = false;
                self.editor.session.editor_import_text.clear();
                return;
            }
        }

        let text = self.editor.session.editor_import_text.clone();
        if let Err(e) = self.import_level(&text) {
            log_platform_error(&format!("JSON Import failed: {}", e));
        } else {
            self.editor.session.editor_show_import = false;
            self.editor.session.editor_import_text.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AppPhase;

    #[test]
    fn test_lifecycle_transitions() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            // Start level 0 (should be Flowerfield)
            state.start_level(0);
            assert_eq!(state.phase, AppPhase::Playing);
            assert_eq!(
                state.editor.session.playing_level_name,
                Some("Flowerfield".to_string())
            );

            // Start editor for level 1 (Golden Haze)
            state.start_editor(1);
            assert_eq!(state.phase, AppPhase::Editor);
            assert_eq!(
                state.editor.session.editor_level_name,
                Some("Golden Haze".to_string())
            );

            // Back to menu
            state.back_to_menu();
            assert_eq!(state.phase, AppPhase::Menu);
        });
    }

    #[test]
    fn test_lifecycle_audio_side_effects() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            // Mock that audio is playing (not actually possible without a real backend but we can check the call)
            // For now we just check if it resets the phase correctly
            state.start_level(0);
            assert_eq!(state.phase, AppPhase::Playing);

            state.stop_audio(); // Should not crash
        });
    }
}
