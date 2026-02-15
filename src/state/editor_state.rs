use super::{EditorDirtyFlags, State};
use crate::editor_domain::{
    add_tap_with_indicator, clear_taps_with_indicators, remove_tap_with_indicator,
    retain_taps_up_to_duration_with_indicators,
};
use crate::types::{AppPhase, EditorMode, LevelObject, SpawnDirection, TimingPoint};

impl State {
    pub fn set_editor_pan_up_held(&mut self, held: bool) {
        self.editor.ui.pan_up_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_down_held(&mut self, held: bool) {
        self.editor.ui.pan_down_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_left_held(&mut self, held: bool) {
        self.editor.ui.pan_left_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_pan_right_held(&mut self, held: bool) {
        self.editor.ui.pan_right_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_shift_held(&mut self, held: bool) {
        self.editor.ui.shift_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_ctrl_held(&mut self, held: bool) {
        self.editor.ui.ctrl_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_alt_held(&mut self, held: bool) {
        self.editor.ui.alt_held = held && self.phase == AppPhase::Editor;
    }

    pub fn set_editor_block_id(&mut self, block_id: String) {
        self.editor.config.selected_block_id =
            crate::block_repository::normalize_block_id(&block_id);
    }

    pub(crate) fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor.ui.mode = mode;
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;
        if mode == EditorMode::Place {
            self.editor.ui.selected_block_index = None;
            self.editor.ui.selected_block_indices.clear();
            self.editor.ui.hovered_block_index = None;
        }
        if mode == EditorMode::Timing {
            self.editor.ui.selected_block_index = None;
            self.editor.ui.selected_block_indices.clear();
            self.editor.ui.hovered_block_index = None;
        }
        self.rebuild_editor_gizmo_vertices();
        self.rebuild_editor_hover_outline_vertices();
        self.rebuild_editor_selection_outline_vertices();
    }

    pub(crate) fn editor_mode(&self) -> EditorMode {
        self.editor.ui.mode
    }

    pub(crate) fn editor_snap_to_grid(&self) -> bool {
        self.editor.config.snap_to_grid
    }

    pub(crate) fn editor_snap_step(&self) -> f32 {
        self.editor.config.snap_step
    }

    pub(crate) fn set_editor_snap_to_grid(&mut self, snap: bool) {
        self.editor.config.snap_to_grid = snap;
        if self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn set_editor_snap_step(&mut self, step: f32) {
        self.editor.config.snap_step = step.max(0.05);
        if self.editor.config.snap_to_grid && self.editor.ui.selected_block_index.is_some() {
            if let Some(obj) = self.editor_selected_block() {
                self.set_editor_selected_block_position(obj.position);
                self.set_editor_selected_block_size(obj.size);
            }
        }
    }

    pub(crate) fn editor_selected_block(&self) -> Option<LevelObject> {
        self.selected_block_indices_normalized()
            .first()
            .copied()
            .and_then(|index| self.editor.objects.get(index).cloned())
    }

    pub(crate) fn set_editor_selected_block_position(&mut self, position: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor.runtime.interaction.gizmo_drag.is_none()
            && self.editor.runtime.interaction.block_drag.is_none()
        {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor
            .ui
            .selected_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            let bounds = self.editor.ui.bounds;
            let snap_step = self.editor.config.snap_step.max(0.05);
            let next_position = if self.editor.config.snap_to_grid {
                [
                    (position[0] / snap_step).round() * snap_step,
                    (position[1] / snap_step).round() * snap_step,
                    (position[2].max(0.0) / snap_step).round() * snap_step,
                ]
            } else {
                [position[0], position[1], position[2].max(0.0)]
            };
            self.editor.objects[index].position = next_position;
            self.editor.ui.cursor = [
                next_position[0].clamp(-bounds as f32, bounds as f32),
                next_position[1].clamp(-bounds as f32, bounds as f32),
                next_position[2].max(0.0),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_cursor_vertices();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_size(&mut self, size: [f32; 3]) {
        if self.phase != AppPhase::Editor {
            return;
        }

        if self.editor.runtime.interaction.gizmo_drag.is_none()
            && self.editor.runtime.interaction.block_drag.is_none()
        {
            self.record_editor_history_state();
        }

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor
            .ui
            .selected_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            let snap_step = self.editor.config.snap_step.max(0.05);
            let snapped_size = if self.editor.config.snap_to_grid {
                [
                    (size[0] / snap_step).round() * snap_step,
                    (size[1] / snap_step).round() * snap_step,
                    (size[2] / snap_step).round() * snap_step,
                ]
            } else {
                size
            };
            let min_size = if self.editor.config.snap_to_grid {
                snap_step
            } else {
                0.25
            };
            self.editor.objects[index].size = [
                snapped_size[0].max(min_size),
                snapped_size[1].max(min_size),
                snapped_size[2].max(min_size),
            ];
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_id(&mut self, block_id: String) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor
            .ui
            .selected_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            self.editor.objects[index].block_id =
                crate::block_repository::normalize_block_id(&block_id);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_rotation(&mut self, rotation_degrees: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor
            .ui
            .selected_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            self.editor.objects[index].rotation_degrees = rotation_degrees;
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub(crate) fn set_editor_selected_block_roundness(&mut self, roundness: f32) {
        if self.phase != AppPhase::Editor {
            return;
        }

        self.record_editor_history_state();

        self.sync_primary_selection_from_indices();

        if let Some(index) = self
            .editor
            .ui
            .selected_block_index
            .filter(|index| *index < self.editor.objects.len())
        {
            self.editor.objects[index].roundness = roundness.max(0.0);
            self.sync_editor_objects();
            self.rebuild_editor_gizmo_vertices();
            self.rebuild_editor_selection_outline_vertices();
        }
    }

    pub fn editor_selected_block_id(&self) -> &str {
        &self.editor.config.selected_block_id
    }

    pub fn editor_timeline_time_seconds(&self) -> f32 {
        self.editor.timeline.clock.time_seconds
    }

    pub fn editor_timeline_duration_seconds(&self) -> f32 {
        self.editor.timeline.clock.duration_seconds
    }

    pub fn editor_tap_times(&self) -> &[f32] {
        &self.editor.timeline.taps.tap_times
    }

    pub fn editor_fps(&self) -> f32 {
        self.editor.perf.fps_smoothed
    }

    pub fn set_editor_timeline_time_seconds(&mut self, time_seconds: f32) {
        let clamped_time =
            time_seconds.clamp(0.0, self.editor.timeline.clock.duration_seconds.max(0.0));
        if (clamped_time - self.editor.timeline.clock.time_seconds).abs() <= f32::EPSILON {
            return;
        }

        self.editor.timeline.clock.time_seconds = clamped_time;
        self.refresh_editor_timeline_position();
        self.resync_editor_timeline_playback_audio();
    }

    pub fn set_editor_timeline_duration_seconds(&mut self, duration_seconds: f32) {
        self.record_editor_history_state();
        self.editor.timeline.clock.duration_seconds = duration_seconds.max(0.1);
        self.editor.timeline.clock.time_seconds = self
            .editor
            .timeline
            .clock
            .time_seconds
            .min(self.editor.timeline.clock.duration_seconds);
        retain_taps_up_to_duration_with_indicators(
            &mut self.editor.timeline.taps.tap_times,
            &mut self.editor.timeline.taps.tap_indicator_positions,
            self.editor.timeline.clock.duration_seconds,
        );
        self.invalidate_editor_timeline_samples();
        self.refresh_editor_timeline_position();
        self.resync_editor_timeline_playback_audio();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_add_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor.timeline.clock.time_seconds;
        let indicator_cell =
            self.tap_indicator_position_from_world(self.editor.timeline.preview.position);
        add_tap_with_indicator(
            &mut self.editor.timeline.taps.tap_times,
            &mut self.editor.timeline.taps.tap_indicator_positions,
            tap_time,
            indicator_cell,
        );
        self.invalidate_editor_timeline_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_remove_tap(&mut self) {
        self.record_editor_history_state();
        let tap_time = self.editor.timeline.clock.time_seconds;
        remove_tap_with_indicator(
            &mut self.editor.timeline.taps.tap_times,
            &mut self.editor.timeline.taps.tap_indicator_positions,
            tap_time,
        );
        self.invalidate_editor_timeline_samples_from(tap_time);
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub fn editor_clear_taps(&mut self) {
        self.record_editor_history_state();
        clear_taps_with_indicators(
            &mut self.editor.timeline.taps.tap_times,
            &mut self.editor.timeline.taps.tap_indicator_positions,
        );
        self.invalidate_editor_timeline_samples();
        self.refresh_editor_timeline_position();
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_timeline_preview(&self) -> ([f32; 3], SpawnDirection) {
        (
            self.editor.timeline.preview.position,
            self.editor.timeline.preview.direction,
        )
    }

    pub(crate) fn editor_timing_points(&self) -> &[TimingPoint] {
        &self.editor.timing.timing_points
    }

    pub(crate) fn editor_playback_speed(&self) -> f32 {
        self.editor.timing.playback_speed
    }

    pub(crate) fn set_editor_playback_speed(&mut self, speed: f32) {
        self.editor.timing.playback_speed = speed.clamp(0.25, 2.0);
        self.audio
            .state
            .runtime
            .set_speed(self.editor.timing.playback_speed);
    }

    pub(crate) fn editor_timing_selected_index(&self) -> Option<usize> {
        self.editor.timing.timing_selected_index
    }

    pub(crate) fn set_editor_timing_selected_index(&mut self, index: Option<usize>) {
        self.editor.timing.timing_selected_index = index;
    }

    pub(crate) fn editor_waveform_zoom(&self) -> f32 {
        self.editor.timing.waveform_zoom
    }

    pub(crate) fn set_editor_waveform_zoom(&mut self, zoom: f32) {
        self.editor.timing.waveform_zoom = zoom.clamp(0.1, 100.0);
    }

    pub(crate) fn editor_waveform_scroll(&self) -> f32 {
        self.editor.timing.waveform_scroll
    }

    pub(crate) fn set_editor_waveform_scroll(&mut self, scroll: f32) {
        self.editor.timing.waveform_scroll = scroll.max(0.0);
    }

    pub(crate) fn editor_waveform_samples(&self) -> &[f32] {
        &self.editor.timing.waveform_samples
    }

    pub(crate) fn editor_waveform_sample_rate(&self) -> u32 {
        self.editor.timing.waveform_sample_rate
    }

    pub(crate) fn editor_bpm_tap_result(&self) -> Option<f32> {
        self.editor.timing.bpm_tap_result
    }

    pub(crate) fn editor_add_timing_point(&mut self, time_seconds: f32, bpm: f32) {
        self.record_editor_history_state();
        let tp = TimingPoint {
            time_seconds,
            bpm,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        self.editor.timing.timing_points.push(tp);
        self.editor
            .timing
            .timing_points
            .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
        self.editor.timing.timing_selected_index = self
            .editor
            .timing
            .timing_points
            .iter()
            .position(|tp| (tp.time_seconds - time_seconds).abs() < 1e-4);
    }

    pub(crate) fn editor_remove_timing_point(&mut self, index: usize) {
        if index < self.editor.timing.timing_points.len() {
            self.record_editor_history_state();
            self.editor.timing.timing_points.remove(index);
            self.editor.timing.timing_selected_index = None;
        }
    }

    pub(crate) fn editor_update_timing_point_time(&mut self, index: usize, time: f32) {
        if index < self.editor.timing.timing_points.len() {
            self.record_editor_history_state();
            let tp = &mut self.editor.timing.timing_points[index];
            tp.time_seconds = time.max(0.0);
            let bpm = tp.bpm;
            self.editor
                .timing
                .timing_points
                .sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
            self.editor.timing.timing_selected_index =
                self.editor.timing.timing_points.iter().position(|tp| {
                    (tp.time_seconds - time).abs() < 1e-4 && (tp.bpm - bpm).abs() < 1e-4
                });
        }
    }

    pub(crate) fn editor_update_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        if index < self.editor.timing.timing_points.len() {
            self.record_editor_history_state();
            self.editor.timing.timing_points[index].bpm = bpm.max(1.0);
        }
    }

    pub(crate) fn editor_update_timing_point_time_signature(
        &mut self,
        index: usize,
        numerator: u32,
        denominator: u32,
    ) {
        if index < self.editor.timing.timing_points.len() {
            self.record_editor_history_state();
            self.editor.timing.timing_points[index].time_signature_numerator =
                numerator.clamp(1, 32);
            self.editor.timing.timing_points[index].time_signature_denominator =
                denominator.clamp(1, 32);
        }
    }

    pub(crate) fn editor_bpm_tap(&mut self) {
        let _now = crate::platform::state_host::PlatformInstant::now();
        let now_secs = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0)
            }
            #[cfg(target_arch = "wasm32")]
            {
                js_sys::Date::now() / 1000.0
            }
        };
        self.editor.timing.bpm_tap_times.push(now_secs);

        // Keep only recent taps (last 3 seconds gap max)
        if self.editor.timing.bpm_tap_times.len() > 1 {
            let last = *self.editor.timing.bpm_tap_times.last().unwrap();
            let second_last =
                self.editor.timing.bpm_tap_times[self.editor.timing.bpm_tap_times.len() - 2];
            if last - second_last > 3.0 {
                self.editor.timing.bpm_tap_times = vec![now_secs];
                self.editor.timing.bpm_tap_result = None;
                return;
            }
        }

        if self.editor.timing.bpm_tap_times.len() >= 2 {
            let intervals: Vec<f64> = self
                .editor
                .timing
                .bpm_tap_times
                .windows(2)
                .map(|w| w[1] - w[0])
                .collect();
            let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
            if avg_interval > 0.0 {
                self.editor.timing.bpm_tap_result = Some((60.0 / avg_interval) as f32);
            }
        }
    }

    pub(crate) fn editor_bpm_tap_reset(&mut self) {
        self.editor.timing.bpm_tap_times.clear();
        self.editor.timing.bpm_tap_result = None;
    }

    pub(crate) fn load_waveform_for_current_audio(&mut self) {
        const WAVEFORM_WINDOW: usize = 256;

        let music_source = self.session.editor_music_metadata.source.clone();

        if let Some((samples, sample_rate)) =
            self.audio.state.editor.waveform_cache.get(&music_source)
        {
            self.editor.timing.waveform_samples = samples.clone();
            self.editor.timing.waveform_sample_rate = *sample_rate;
            self.audio.state.editor.waveform_loading_source = None;
            return;
        }

        if self.audio.state.editor.waveform_loading_source.as_deref() == Some(music_source.as_str())
        {
            return;
        }

        self.audio.state.editor.waveform_loading_source = Some(music_source.clone());
        self.editor.timing.waveform_samples.clear();
        self.editor.timing.waveform_sample_rate = 0;

        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::platform::audio::decode_audio_to_waveform;
            let source_for_thread = music_source.clone();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let cached_bytes = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&music_source)
                .cloned();
            let sender = self.audio.state.editor.waveform_load_channel.0.clone();

            std::thread::spawn(move || {
                let bytes = cached_bytes.or_else(|| {
                    let audio_path = format!("assets/levels/{}/{}", level_name, source_for_thread);
                    std::fs::read(&audio_path).ok()
                });

                let decoded = if let Some(bytes) = bytes {
                    decode_audio_to_waveform(bytes, WAVEFORM_WINDOW)
                } else {
                    None
                };

                let _ = sender.send((source_for_thread, decoded));
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast as _;
            use wasm_bindgen_futures::{spawn_local, JsFuture};

            let source_for_fetch = music_source.clone();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let cached_bytes = self
                .audio
                .state
                .editor
                .local_audio_cache
                .get(&music_source)
                .cloned();
            let sender = self.audio.state.editor.waveform_load_channel.0.clone();

            spawn_local(async move {
                let bytes = if let Some(bytes) = cached_bytes {
                    Some(bytes)
                } else {
                    let audio_path = format!("assets/levels/{}/{}", level_name, source_for_fetch);
                    let fetched = async {
                        let window = web_sys::window()?;
                        let response_value = JsFuture::from(window.fetch_with_str(&audio_path))
                            .await
                            .ok()?;
                        let response: web_sys::Response = response_value.dyn_into().ok()?;
                        if !response.ok() {
                            return None;
                        }
                        let array_buffer =
                            JsFuture::from(response.array_buffer().ok()?).await.ok()?;
                        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                        Some(uint8_array.to_vec())
                    }
                    .await;

                    fetched
                };

                let decoded = if let Some(bytes) = bytes {
                    crate::platform::audio::decode_audio_to_waveform_async(&bytes, WAVEFORM_WINDOW)
                        .await
                } else {
                    None
                };

                let _ = sender.send((music_source, decoded));
            });
        }
    }
}
