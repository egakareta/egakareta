/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
mod audio_state;
mod auth_state;
mod command_dispatch;
mod editor_actions;
mod editor_camera;
pub mod editor_command;
mod editor_config_state;
mod editor_dispatch;
mod editor_interaction;
mod editor_scene;
mod editor_state;
mod editor_timeline;
mod editor_timing;
mod editor_triggers;
mod history;
mod level_management;
mod lifecycle;
mod perf;
mod render;
mod runtime;
mod state_helpers;
mod update;
mod view_model;

#[cfg(test)]
mod editor_domain_tests;
#[cfg(test)]
mod editor_snap_tests;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

pub const TAP_CLICK_SCREEN_EPSILON_PIXELS: f64 = 0.5;

pub use audio_state::{AudioState, AudioSubsystem};
pub use auth_state::AuthSubsystem;
pub use editor_camera::EditorCameraState;
pub use editor_config_state::EditorConfigState;
use editor_interaction::EditorPickSpatialIndex;
pub use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionState, EditorPendingTapClick,
};
pub use editor_timeline::EditorTimelineState;
pub use editor_timing::EditorTimingState;
pub use editor_triggers::EditorTriggerState;
pub use history::EditorHistoryState;
pub use perf::EditorPerfState;
pub use render::RenderSubsystem;
pub use runtime::{EditorDirtyFlags, EditorRuntimeState, FrameRuntimeState};
pub use view_model::EditorUiViewModel;

use crate::state::editor_command::EditorCommand;
use eg_game::{GameCheckpointState, GameState};
use eg_mesh::MeshGeometry;
#[cfg(not(target_arch = "wasm32"))]
use eg_platform::state_host::NativeWindow;
use eg_types::{
    AppPhase, AppSettings, EditorMode, EditorState, LevelCreatorMetadata, LevelObject,
    LevelPreviewCameraMetadata, MenuState, MusicMetadata, PhysicalSize, SettingsSection,
    SpawnMetadata,
};

/// Bundles all gameplay-related state into a single subsystem.
/// Separates gameplay concern from the top-level application state.
pub struct GameplaySubsystem {
    pub state: GameState,
    pub pending_turn_inputs: VecDeque<TimedGameplayTurn>,
    pub death_sfx_played: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedGameplayTurn {
    pub time_seconds: f32,
}

/// Bundles all session-related state into a single subsystem.
/// Manages level names, music metadata, and playtesting settings.
pub struct SessionSubsystem {
    pub editor_level_name: Option<String>,
    pub editor_music_metadata: MusicMetadata,
    pub editor_creator_metadata: LevelCreatorMetadata,
    pub editor_sky_color: [f32; 3],
    pub editor_menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    pub editor_show_metadata: bool,
    pub editor_show_place_window: bool,
    pub editor_show_settings: bool,
    pub editor_settings_section: SettingsSection,
    pub editor_keybind_capture_action: Option<(String, usize)>,
    pub editor_level_import_channel: (
        std::sync::mpsc::Sender<Vec<u8>>,
        std::sync::mpsc::Receiver<Vec<u8>>,
    ),
    pub settings_restart_required: bool,
    pub graphics_backend_locked: bool,
    pub audio_backend_locked: bool,
    pub available_graphics_backends: Vec<String>,
    pub available_audio_backends: Vec<String>,
    pub app_settings: AppSettings,
    pub playing_level_name: Option<String>,
    pub playtesting_editor: bool,
    pub game_paused: bool,
    pub playtest_audio_start_seconds: Option<f32>,
    pub playing_sky_color: [f32; 3],
    pub playing_trigger_base_objects: Option<Vec<LevelObject>>,
    pub practice_mode_enabled: bool,
    pub practice_checkpoints: Vec<PracticeCheckpoint>,
}

#[derive(Clone)]
pub struct PracticeCheckpoint {
    pub gameplay: GameCheckpointState,
    pub trigger_base_objects: Option<Vec<LevelObject>>,
}

/// Bundles all editor-related state into a single subsystem.
/// Separates editor concern from the top-level application state.
pub struct EditorSubsystem {
    pub ui: EditorState,
    pub config: EditorConfigState,
    pub objects: Vec<LevelObject>,
    pub spawn: SpawnMetadata,
    pub camera: EditorCameraState,
    pub triggers: EditorTriggerState,
    pub timeline: EditorTimelineState,
    pub runtime: EditorRuntimeState,
    pub perf: EditorPerfState,
    pub timing: EditorTimingState,
    pub selected_mask_cache: Option<Vec<bool>>,
    pub block_static_vertex_cache: MeshGeometry,
    pub block_static_vertex_cache_complete_len: Option<usize>,
    pub(crate) pick_spatial_index: EditorPickSpatialIndex,
}

#[cfg(test)]
impl EditorSubsystem {
    /// Construct a standalone `EditorSubsystem` for domain tests — no GPU required.
    pub fn new_test() -> Self {
        Self {
            ui: EditorState::new(),
            config: EditorConfigState {
                selected_block_id: eg_types::DEFAULT_BLOCK_ID.to_string(),
                recent_block_ids: vec![eg_types::DEFAULT_BLOCK_ID.to_string()],
                snap_to_grid: true,
                snap_step: 1.0,
                snap_rotation: true,
                snap_rotation_step_degrees: 15.0,
                selected_block_rotation_degrees: [0.0, 0.0, 0.0],
            },
            objects: Vec::new(),
            spawn: SpawnMetadata::default(),
            camera: EditorCameraState::default(),
            triggers: EditorTriggerState::new(),
            timeline: EditorTimelineState::new(),
            runtime: EditorRuntimeState {
                dirty: EditorDirtyFlags::default(),
                pending_block_mesh_appends: Vec::new(),
                gizmo: runtime::EditorGizmoState {
                    rebuild_accumulator: 0.0,
                    last_pan: [0.0, 0.0],
                    last_target_z: 0.0,
                    last_rotation: 0.0,
                    last_pitch: 0.0,
                },
                drag_heavy_rebuild_accumulator: 0.0,
                interaction: EditorInteractionState::new(),
                history: EditorHistoryState {
                    undo: Vec::new(),
                    redo: Vec::new(),
                },
            },
            perf: EditorPerfState::new(),
            timing: EditorTimingState::new(),
            selected_mask_cache: None,
            block_static_vertex_cache: MeshGeometry::default(),
            block_static_vertex_cache_complete_len: None,
            pick_spatial_index: EditorPickSpatialIndex::default(),
        }
    }
}

pub struct MenuSubsystem {
    pub state: MenuState,
}

/// The central state structure that manages all subsystems of the game engine.
/// It contains subsystems for rendering, gameplay, editor, audio, session, and menu,
/// and handles the overall application phase and frame runtime.
pub struct State {
    render: RenderSubsystem,
    auth: AuthSubsystem,
    gameplay: GameplaySubsystem,
    editor: EditorSubsystem,
    audio: AudioSubsystem,
    session: SessionSubsystem,
    menu: MenuSubsystem,
    phase: AppPhase,
    frame_runtime: FrameRuntimeState,
}

impl State {
    /// Returns a reference to the wgpu device for GPU operations.
    pub fn device(&self) -> &wgpu::Device {
        &self.render.gpu.device
    }

    /// Returns a reference to the wgpu queue for command submission.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.render.gpu.queue
    }

    /// Returns the GPU render context for render-owned helpers.
    pub fn render_gpu(&self) -> &eg_render::GpuContext {
        &self.render.gpu
    }

    /// Returns a reference to the native window (desktop only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> Option<&NativeWindow> {
        self.render.window()
    }

    /// Resizes the rendering surface to the new dimensions.
    ///
    /// Skips resizing if either dimension is zero. Prepares the surface host
    /// for resize if available, then applies the resize to the GPU resources.
    pub fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        if let Some(host) = &self.render.surface_host {
            host.prepare_resize(new_size);
        }
        self.render.gpu.apply_resize(new_size);
    }

    fn current_gameplay_input_time_seconds(&self) -> f32 {
        self.audio
            .state
            .runtime
            .playback_time_seconds()
            .unwrap_or(self.gameplay.state.elapsed_seconds)
            .max(0.0)
    }

    pub fn queue_gameplay_turn_right(&mut self) {
        self.queue_gameplay_turn_right_at(self.current_gameplay_input_time_seconds());
    }

    pub fn queue_gameplay_turn_right_at(&mut self, time_seconds: f32) {
        if !time_seconds.is_finite() {
            return;
        }

        let time_seconds = time_seconds.max(0.0);
        let insert_at = self
            .gameplay
            .pending_turn_inputs
            .iter()
            .position(|input| time_seconds < input.time_seconds)
            .unwrap_or(self.gameplay.pending_turn_inputs.len());
        self.gameplay
            .pending_turn_inputs
            .insert(insert_at, TimedGameplayTurn { time_seconds });
    }

    pub fn clear_pending_gameplay_inputs(&mut self) {
        self.gameplay.pending_turn_inputs.clear();
    }

    /// Handles the "turn right" input action based on the current application phase.
    ///
    /// - In Menu: Starts the selected level
    /// - In Playing: Starts gameplay, handles game over restart, or turns the player right
    /// - In Editor: Places a block at the cursor
    /// - In GameOver: Returns to menu
    pub fn turn_right(&mut self) {
        match self.phase {
            AppPhase::Menu => {
                self.start_level(self.menu.state.selected_level);
            }
            AppPhase::Playing => {
                if self.session.game_paused {
                    return;
                }
                if !self.start_gameplay_if_needed() {
                    if self.gameplay.state.game_over {
                        if self.session.practice_mode_enabled
                            && self.respawn_from_practice_checkpoint()
                        {
                            return;
                        }
                        self.restart_level();
                    } else {
                        self.queue_gameplay_turn_right();
                    }
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

    pub fn start_gameplay_if_needed(&mut self) -> bool {
        if self.gameplay.state.started {
            return false;
        }

        self.gameplay.state.started = true;
        if self.session.playtesting_editor {
            let metadata = self.current_editor_metadata();
            let level_name = self
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());
            let start_seconds = self
                .session
                .playtest_audio_start_seconds
                .unwrap_or_else(|| {
                    self.editor_timeline_elapsed_seconds(self.editor.timeline_time_seconds())
                });
            self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
        } else if let Some(level_name) = self.session.playing_level_name.clone() {
            if let Some(metadata) = self.load_level_metadata(&level_name) {
                let start_seconds = self.gameplay.state.elapsed_seconds;
                if start_seconds > 0.001 {
                    self.start_audio_at_seconds(&level_name, &metadata, start_seconds);
                } else {
                    self.start_audio(&level_name, &metadata);
                }
            }
        }

        true
    }

    /// Advances to the next level or moves the editor cursor right.
    ///
    /// - In Menu: Selects the next level in the list (wraps around)
    /// - In Editor: Moves the cursor one unit to the right
    pub fn next_level(&mut self) {
        if self.phase == AppPhase::Menu {
            self.menu.state.selected_level =
                (self.menu.state.selected_level + 1) % self.menu.state.levels.len();
        } else if self.phase == AppPhase::Editor {
            self.move_editor_cursor(1, 0);
        }
    }

    /// Goes to the previous level or moves the editor cursor left.
    ///
    /// - In Menu: Selects the previous level in the list (wraps around)
    /// - In Editor: Moves the cursor one unit to the left
    pub fn prev_level(&mut self) {
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

    /// Toggles between editor and other modes.
    ///
    /// - From Menu: Starts the editor for the selected level
    /// - From Editor: Returns to the menu
    /// - From Playing (during playtest): Switches back to editor mode
    pub fn toggle_editor(&mut self) {
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

    /// Returns true if the application is currently in editor mode.
    pub fn is_editor(&self) -> bool {
        self.phase == AppPhase::Editor
    }

    /// Returns true if the application is currently in menu mode.
    pub fn is_menu(&self) -> bool {
        self.phase == AppPhase::Menu
    }

    /// Returns true when a real gameplay session is paused.
    pub fn is_game_paused(&self) -> bool {
        self.phase == AppPhase::Playing
            && !self.session.playtesting_editor
            && self.session.game_paused
    }

    /// Returns true when real gameplay is currently using practice-mode checkpoints.
    pub fn is_practice_mode_enabled(&self) -> bool {
        self.phase == AppPhase::Playing
            && !self.session.playtesting_editor
            && self.session.practice_mode_enabled
    }

    /// Returns the latest practice checkpoint time, if one has been placed.
    pub fn practice_checkpoint_time_seconds(&self) -> Option<f32> {
        self.session
            .practice_checkpoints
            .last()
            .map(|checkpoint| checkpoint.gameplay.elapsed_seconds)
    }

    /// Returns how many practice checkpoints are currently stacked.
    pub fn practice_checkpoint_count(&self) -> usize {
        self.session.practice_checkpoints.len()
    }

    /// Sets whether the right mouse button is currently being dragged in the editor.
    pub fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor.set_right_dragging(dragging);
    }

    /// Handles mouse button events for editor interactions.
    ///
    /// Processes left and right mouse button presses/releases, updating editor state
    /// and triggering selection or interaction logic as appropriate.
    pub fn handle_mouse_button(&mut self, button: u32, pressed: bool) {
        match button {
            0 => {
                self.editor.set_left_mouse_down(pressed);
                self.mark_editor_dirty(EditorDirtyFlags::selection_overlay_changed());
                if !pressed {
                    if let Some(pointer) = self.editor.ui.pointer_screen {
                        self.finish_editor_marquee_selection(pointer[0], pointer[1]);
                    } else {
                        self.editor.ui.marquee_start_screen = None;
                        self.editor.ui.marquee_current_screen = None;
                    }
                    let had_drag = self.editor.has_gizmo_drag() || self.editor.has_block_drag();
                    self.editor.clear_interaction_drags();
                    if had_drag {
                        self.sync_editor_objects_after_drag_release();
                    }
                } else if self.phase != AppPhase::Menu {
                    self.turn_right();
                }
            }
            2 => {
                self.set_editor_right_dragging(pressed);
            }
            _ => {}
        }
    }

    /// Handles primary click (left mouse button) at the given screen coordinates.
    ///
    /// Updates the editor pointer position and processes the click based on the current
    /// editor mode (Place or Select), potentially starting block placement, gizmo drag,
    /// or selection operations.
    pub fn handle_primary_click(&mut self, x: f64, y: f64) {
        let previous_pointer = self.editor.ui.pointer_screen;
        let pointer_moved = previous_pointer.is_none_or(|pointer| {
            (pointer[0] - x).abs() > TAP_CLICK_SCREEN_EPSILON_PIXELS
                || (pointer[1] - y).abs() > TAP_CLICK_SCREEN_EPSILON_PIXELS
        });
        if pointer_moved {
            self.editor.runtime.interaction.hovered_tap_index = None;
            self.editor.runtime.interaction.hovered_tap_division = None;
            self.editor.runtime.interaction.pending_tap_click = None;
        }
        self.editor.set_pointer_screen(Some([x, y]));
        self.editor.set_left_mouse_down(true);
        self.mark_editor_dirty(EditorDirtyFlags::selection_overlay_changed());
        if self.phase == AppPhase::Editor {
            if self.editor_pointer_over_ui_input(x, y) {
                return;
            }

            if self.editor.ui.alt_held {
                self.dispatch(crate::commands::AppCommand::Editor(
                    EditorCommand::PickBlockAt { x, y },
                ));
                return;
            }

            let mode = self.editor.mode();
            if mode == EditorMode::Place {
                self.update_editor_cursor_from_screen(x, y);
                self.place_editor_block();
            } else if mode == EditorMode::Tapping {
                self.editor_handle_tapping_click_from_screen(x, y);
            } else if mode.is_selection_mode() {
                if self.begin_editor_gizmo_drag(x, y) {
                    return;
                }
                if !self.editor.ui.shift_held && self.begin_editor_selected_block_drag(x, y) {
                    return;
                }
                let viewport_size = glam::Vec2::new(
                    self.render.gpu.config.width as f32,
                    self.render.gpu.config.height as f32,
                );
                let hit_selectable = self
                    .editor
                    .pick_from_screen(x, y, viewport_size)
                    .is_some_and(|pick| {
                        pick.hit_block_index.is_some() || pick.hit_trigger_index.is_some()
                    });
                if !self.editor.ui.shift_held && hit_selectable {
                    self.editor
                        .select_block_from_screen(x, y, viewport_size, self.phase);
                    return;
                }
                self.begin_editor_marquee_selection(x, y);
            } else if mode == EditorMode::Timing {
                // Timing mode: clicks handled by egui waveform panel
            }
            return;
        }

        if self.phase != AppPhase::Menu {
            self.turn_right();
        }
    }

    /// Handles pointer movement to the given screen coordinates.
    ///
    /// Updates editor interactions such as gizmo dragging, selection dragging,
    /// or marquee selection if the left mouse button is held down in editor mode.
    pub fn handle_pointer_moved(&mut self, x: f64, y: f64) {
        let mut handled = false;
        if self.editor.left_mouse_down() && self.is_editor() {
            handled = self.drag_editor_gizmo_from_screen(x, y)
                || self.drag_editor_selection_from_screen(x, y)
                || self.update_editor_marquee_selection(x, y);
        }

        if !handled && self.is_editor() {
            let viewport_size = glam::Vec2::new(
                self.render.gpu.config.width as f32,
                self.render.gpu.config.height as f32,
            );
            let next_hover = self.editor.pick_gizmo_handle(x, y, viewport_size);
            if self.editor.runtime.interaction.hovered_gizmo != next_hover {
                self.editor.runtime.interaction.hovered_gizmo = next_hover;
                self.rebuild_editor_gizmo_vertices();
            }

            self.update_editor_cursor_from_screen(x, y);
        }
        self.editor.set_pointer_screen(Some([x, y]));
    }
}
