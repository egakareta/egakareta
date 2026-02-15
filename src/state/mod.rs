mod audio_state;
mod command_dispatch;
mod editor_actions;
mod editor_camera;
mod editor_config_state;
mod editor_interaction;
mod editor_scene;
mod editor_state;
mod editor_timeline;
mod editor_timing;
mod history;
mod lifecycle;
mod perf;
mod render;
mod runtime;
mod state_helpers;
mod update;

pub(crate) use audio_state::{AudioState, AudioSubsystem};
pub(crate) use editor_camera::EditorCameraState;
pub(crate) use editor_config_state::EditorConfigState;
pub(crate) use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionChange, EditorInteractionState, EditorPickResult, GizmoAxis, GizmoDragKind,
};
pub(crate) use editor_timeline::{EditorTimelineSample, EditorTimelineState};
pub(crate) use editor_timing::EditorTimingState;
pub(crate) use history::EditorHistoryState;
pub(crate) use perf::{EditorPerfState, PerfStage};
pub(crate) use render::{GpuContext, MeshSlot, RenderSubsystem, SceneMeshes, DEPTH_FORMAT};
pub(crate) use runtime::{
    EditorDirtyFlags, EditorFrameState, EditorGizmoState, EditorRuntimeState, FrameRuntimeState,
    PlayerRenderState,
};

use crate::game::GameState;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
use crate::types::{
    AppPhase, EditorMode, EditorState, LevelObject, MenuState, MusicMetadata, PhysicalSize,
    SpawnMetadata,
};

pub(crate) struct GameplaySubsystem {
    pub(crate) state: GameState,
}

pub(crate) struct SessionSubsystem {
    pub(crate) editor_level_name: Option<String>,
    pub(crate) editor_music_metadata: MusicMetadata,
    pub(crate) editor_show_metadata: bool,
    pub(crate) editor_show_import: bool,
    pub(crate) editor_import_text: String,
    pub(crate) playing_level_name: Option<String>,
    pub(crate) playtesting_editor: bool,
}

/// Bundles all editor-related state into a single subsystem.
/// Separates editor concern from the top-level application state.
pub(crate) struct EditorSubsystem {
    pub(crate) ui: EditorState,
    pub(crate) config: EditorConfigState,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) camera: EditorCameraState,
    pub(crate) timeline: EditorTimelineState,
    pub(crate) runtime: EditorRuntimeState,
    pub(crate) perf: EditorPerfState,
    pub(crate) timing: EditorTimingState,
}

pub(crate) struct MenuSubsystem {
    pub(crate) state: MenuState,
}

pub struct State {
    render: RenderSubsystem,
    gameplay: GameplaySubsystem,
    editor: EditorSubsystem,
    audio: AudioSubsystem,
    session: SessionSubsystem,
    menu: MenuSubsystem,
    phase: AppPhase,
    frame_runtime: FrameRuntimeState,
}

impl State {
    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.render.gpu.device
    }

    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.render.gpu.queue
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn window(&self) -> &NativeWindow {
        self.render.gpu.window()
    }

    pub(crate) fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        if let Some(host) = &self.render.gpu.surface_host {
            host.prepare_resize(new_size);
        }
        self.render.gpu.apply_resize(new_size);
    }

    pub(crate) fn turn_right(&mut self) {
        match self.phase {
            AppPhase::Menu => {
                self.start_level(self.menu.state.selected_level);
            }
            AppPhase::Playing => {
                if !self.gameplay.state.started {
                    self.gameplay.state.started = true;
                    if self.session.playtesting_editor {
                        let metadata = self.current_editor_metadata();
                        let level_name = self
                            .session
                            .editor_level_name
                            .clone()
                            .unwrap_or_else(|| "Untitled".to_string());
                        let start_seconds = self.editor_timeline_elapsed_seconds(
                            self.editor.timeline.clock.time_seconds,
                        );
                        self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
                    } else if let Some(level_name) = self.session.playing_level_name.clone() {
                        if let Some(metadata) = self.load_level_metadata(&level_name) {
                            self.start_audio(&level_name, &metadata);
                        }
                    }
                } else if self.gameplay.state.game_over {
                    self.restart_level();
                } else {
                    self.gameplay.state.turn_right();
                }
            }
            AppPhase::Editor => {
                self.place_editor_block();
            }
            AppPhase::GameOver => {
                self.phase = AppPhase::Menu;
            }
        }
    }

    pub(crate) fn next_level(&mut self) {
        if self.phase == AppPhase::Menu {
            self.menu.state.selected_level =
                (self.menu.state.selected_level + 1) % self.menu.state.levels.len();
        } else if self.phase == AppPhase::Editor {
            self.move_editor_cursor(1, 0);
        }
    }

    pub(crate) fn prev_level(&mut self) {
        if self.phase == AppPhase::Menu {
            if self.menu.state.selected_level == 0 {
                self.menu.state.selected_level = self.menu.state.levels.len() - 1;
            } else {
                self.menu.state.selected_level -= 1;
            }
        } else if self.phase == AppPhase::Editor {
            self.move_editor_cursor(-1, 0);
        }
    }

    pub(crate) fn toggle_editor(&mut self) {
        match self.phase {
            AppPhase::Menu => self.start_editor(self.menu.state.selected_level),
            AppPhase::Editor => self.back_to_menu(),
            AppPhase::Playing if self.session.playtesting_editor => {
                self.phase = AppPhase::Editor;
                self.stop_audio();
                self.sync_editor_objects();
            }
            _ => {}
        }
    }

    pub fn is_editor(&self) -> bool {
        self.phase == AppPhase::Editor
    }

    pub fn is_menu(&self) -> bool {
        self.phase == AppPhase::Menu
    }

    pub(crate) fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor.ui.right_dragging = dragging;
    }

    pub(crate) fn handle_mouse_button(&mut self, button: u32, pressed: bool) {
        match button {
            0 => {
                self.editor.ui.left_mouse_down = pressed;
                if !pressed {
                    let had_drag = self.editor.runtime.interaction.gizmo_drag.is_some()
                        || self.editor.runtime.interaction.block_drag.is_some();
                    self.editor.runtime.interaction.gizmo_drag = None;
                    self.editor.runtime.interaction.block_drag = None;
                    if had_drag {
                        self.sync_editor_objects();
                    }
                } else {
                    self.turn_right();
                }
            }
            2 => {
                self.set_editor_right_dragging(pressed);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_primary_click(&mut self, x: f64, y: f64) {
        self.editor.ui.pointer_screen = Some([x, y]);
        self.editor.ui.left_mouse_down = true;
        if self.phase == AppPhase::Editor {
            match self.editor.ui.mode {
                EditorMode::Place => {
                    self.update_editor_cursor_from_screen(x, y);
                    self.place_editor_block();
                }
                EditorMode::Select => {
                    if self.begin_editor_gizmo_drag(x, y) {
                        return;
                    }
                    if self.begin_editor_selected_block_drag(x, y) {
                        return;
                    }
                    self.select_editor_block_from_screen(x, y);
                }
                EditorMode::Timing => {
                    // Timing mode: clicks handled by egui waveform panel
                }
            }
            return;
        }

        self.turn_right();
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorDirtyFlags, LevelObject};
    use crate::editor_domain::derive_timeline_position;
    use crate::types::SpawnDirection;

    // ── EditorDirtyFlags contract tests ─────────────────────────────
    #[test]
    fn dirty_flags_default_is_clean() {
        let flags = EditorDirtyFlags::default();
        assert!(!flags.any());
    }

    #[test]
    fn dirty_flags_from_object_sync_sets_all() {
        let flags = EditorDirtyFlags::from_object_sync();
        assert!(flags.sync_game_objects);
        assert!(flags.rebuild_block_mesh);
        assert!(flags.rebuild_selection_overlays);
        assert!(flags.rebuild_tap_indicators);
        assert!(flags.rebuild_preview_player);
        assert!(flags.any());
    }

    #[test]
    fn dirty_flags_merge_is_union() {
        let mut a = EditorDirtyFlags {
            rebuild_block_mesh: true,
            ..EditorDirtyFlags::default()
        };
        let b = EditorDirtyFlags {
            rebuild_tap_indicators: true,
            ..EditorDirtyFlags::default()
        };
        a.merge(b);
        assert!(a.rebuild_block_mesh);
        assert!(a.rebuild_tap_indicators);
        assert!(!a.sync_game_objects);
        assert!(a.any());
    }

    // ── Timeline position tests (pre-existing) ─────────────────────

    #[test]
    fn derives_position_without_taps() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &[],
            3.0 * step_time,
            &[],
        );
        assert!((position[0] - 0.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn derives_position_with_taps() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [2.0 * step_time, 4.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &taps,
            5.0 * step_time,
            &[],
        );
        assert!((position[0] - 2.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn tap_at_zero_changes_direction() {
        let taps = [0.0];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 0.0, &[]);
        assert!((position[0] - 0.5).abs() < 0.1);
        assert!((position[1] - 0.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Right));
    }

    #[test]
    fn ignores_taps_after_step() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [5.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [1.0, 1.0, 0.0],
            SpawnDirection::Forward,
            &taps,
            2.0 * step_time,
            &[],
        );
        assert!((position[0] - 1.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn supports_offset_spawn_with_tap() {
        let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
        let taps = [2.0 * step_time];
        let (position, direction) = derive_timeline_position(
            [2.0, 2.0, 0.0],
            SpawnDirection::Right,
            &taps,
            3.0 * step_time,
            &[],
        );
        assert!((position[0] - 4.5).abs() < 0.1);
        assert!((position[1] - 3.5).abs() < 0.1);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn falls_from_elevated_platform() {
        let objects = [LevelObject {
            position: [0.0, 0.0, 2.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        }];
        let (position, direction) = derive_timeline_position(
            [0.0, 0.0, 3.0],
            SpawnDirection::Forward,
            &[],
            1.0 / crate::game::BASE_PLAYER_SPEED,
            &objects,
        );
        assert!(position[2] <= 3.0);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn test_state_phase_integrity() {
        pollster::block_on(async {
            let mut state = super::State::new_test().await;
            assert_eq!(state.phase, crate::types::AppPhase::Menu);

            state.start_editor(0);
            assert_eq!(state.phase, crate::types::AppPhase::Editor);

            state.toggle_editor(); // Should go back to menu from editor
            assert_eq!(state.phase, crate::types::AppPhase::Menu);
        });
    }

    #[test]
    fn test_state_input_routing() {
        pollster::block_on(async {
            let mut state = super::State::new_test().await;

            // Test primary click in menu starts level
            state.handle_primary_click(0.0, 0.0);
            assert_eq!(state.phase, crate::types::AppPhase::Playing);
        });
    }
}
