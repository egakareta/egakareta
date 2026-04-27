/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
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
mod editor_snap_tests;
#[cfg(test)]
mod shader_tests;
#[cfg(test)]
mod tests;

pub(crate) use audio_state::{AudioState, AudioSubsystem};
pub(crate) use editor_camera::EditorCameraState;
pub(crate) use editor_config_state::EditorConfigState;
pub(crate) use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionState,
};
pub(crate) use editor_timeline::EditorTimelineState;
pub(crate) use editor_timing::EditorTimingState;
pub(crate) use editor_triggers::EditorTriggerState;
pub(crate) use history::EditorHistoryState;
pub(crate) use perf::{EditorPerfState, PerfOverlayEntry, PerfStage};
pub(crate) use render::RenderSubsystem;
pub(crate) use runtime::{EditorDirtyFlags, EditorRuntimeState, FrameRuntimeState};
pub(crate) use view_model::EditorUiViewModel;

use crate::game::GameState;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
use crate::types::{
    AppPhase, AppSettings, EditorMode, EditorState, LevelObject, LevelPreviewCameraMetadata,
    MenuState, MusicMetadata, PhysicalSize, SettingsSection, SpawnMetadata,
};

/// Bundles all gameplay-related state into a single subsystem.
/// Separates gameplay concern from the top-level application state.
pub(crate) struct GameplaySubsystem {
    pub(crate) state: GameState,
}

/// Bundles all session-related state into a single subsystem.
/// Manages level names, music metadata, and playtesting settings.
pub(crate) struct SessionSubsystem {
    pub(crate) editor_level_name: Option<String>,
    pub(crate) editor_music_metadata: MusicMetadata,
    pub(crate) editor_menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    pub(crate) editor_show_metadata: bool,
    pub(crate) editor_show_settings: bool,
    pub(crate) editor_settings_section: SettingsSection,
    pub(crate) editor_keybind_capture_action: Option<(String, usize)>,
    pub(crate) editor_level_import_channel: (
        std::sync::mpsc::Sender<Vec<u8>>,
        std::sync::mpsc::Receiver<Vec<u8>>,
    ),
    pub(crate) settings_restart_required: bool,
    pub(crate) available_graphics_backends: Vec<String>,
    pub(crate) available_audio_backends: Vec<String>,
    pub(crate) app_settings: AppSettings,
    pub(crate) playing_level_name: Option<String>,
    pub(crate) playtesting_editor: bool,
    pub(crate) playtest_audio_start_seconds: Option<f32>,
    pub(crate) playing_trigger_hitboxes: bool,
    pub(crate) playing_trigger_base_objects: Option<Vec<LevelObject>>,
}

/// Bundles all editor-related state into a single subsystem.
/// Separates editor concern from the top-level application state.
pub(crate) struct EditorSubsystem {
    pub(crate) ui: EditorState,
    pub(crate) config: EditorConfigState,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) camera: EditorCameraState,
    pub(crate) triggers: EditorTriggerState,
    pub(crate) timeline: EditorTimelineState,
    pub(crate) runtime: EditorRuntimeState,
    pub(crate) perf: EditorPerfState,
    pub(crate) timing: EditorTimingState,
    pub(crate) selected_mask_cache: Option<Vec<bool>>,
}

pub(crate) struct MenuSubsystem {
    pub(crate) state: MenuState,
}

/// The central state structure that manages all subsystems of the game engine.
/// It contains subsystems for rendering, gameplay, editor, audio, session, and menu,
/// and handles the overall application phase and frame runtime.
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
    /// Returns a reference to the wgpu device for GPU operations.
    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.render.gpu.device
    }

    /// Returns a reference to the wgpu queue for command submission.
    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.render.gpu.queue
    }

    /// Returns a reference to the native window (desktop only).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn window(&self) -> Option<&NativeWindow> {
        self.render.gpu.window()
    }

    /// Resizes the rendering surface to the new dimensions.
    ///
    /// Skips resizing if either dimension is zero. Prepares the surface host
    /// for resize if available, then applies the resize to the GPU resources.
    pub(crate) fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        if let Some(host) = &self.render.gpu.surface_host {
            host.prepare_resize(new_size);
        }
        self.render.gpu.apply_resize(new_size);
    }

    /// Handles the "turn right" input action based on the current application phase.
    ///
    /// - In Menu: Starts the selected level
    /// - In Playing: Starts gameplay, handles game over restart, or turns the player right
    /// - In Editor: Places a block at the cursor
    /// - In GameOver: Returns to menu
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
                        let start_seconds = self
                            .session
                            .playtest_audio_start_seconds
                            .unwrap_or_else(|| {
                                self.editor_timeline_elapsed_seconds(
                                    self.editor.timeline_time_seconds(),
                                )
                            });
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

    /// Advances to the next level or moves the editor cursor right.
    ///
    /// - In Menu: Selects the next level in the list (wraps around)
    /// - In Editor: Moves the cursor one unit to the right
    pub(crate) fn next_level(&mut self) {
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

    /// Toggles between editor and other modes.
    ///
    /// - From Menu: Starts the editor for the selected level
    /// - From Editor: Returns to the menu
    /// - From Playing (during playtest): Switches back to editor mode
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

    /// Returns true if the application is currently in editor mode.
    pub fn is_editor(&self) -> bool {
        self.phase == AppPhase::Editor
    }

    /// Returns true if the application is currently in menu mode.
    pub fn is_menu(&self) -> bool {
        self.phase == AppPhase::Menu
    }

    /// Sets whether the right mouse button is currently being dragged in the editor.
    pub(crate) fn set_editor_right_dragging(&mut self, dragging: bool) {
        self.editor.set_right_dragging(dragging);
    }

    /// Handles mouse button events for editor interactions.
    ///
    /// Processes left and right mouse button presses/releases, updating editor state
    /// and triggering selection or interaction logic as appropriate.
    pub(crate) fn handle_mouse_button(&mut self, button: u32, pressed: bool) {
        match button {
            0 => {
                self.editor.set_left_mouse_down(pressed);
                self.mark_editor_dirty(EditorDirtyFlags {
                    rebuild_selection_overlays: true,
                    ..EditorDirtyFlags::default()
                });
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
    pub(crate) fn handle_primary_click(&mut self, x: f64, y: f64) {
        self.editor.set_pointer_screen(Some([x, y]));
        self.editor.set_left_mouse_down(true);
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_selection_overlays: true,
            ..EditorDirtyFlags::default()
        });
        if self.phase == AppPhase::Editor {
            let mode = self.editor.mode();
            if mode == EditorMode::Place {
                self.update_editor_cursor_from_screen(x, y);
                self.place_editor_block();
            } else if mode.is_selection_mode() {
                if self.begin_editor_gizmo_drag(x, y) {
                    return;
                }
                if !self.editor.ui.shift_held && self.begin_editor_selected_block_drag(x, y) {
                    return;
                }
                self.begin_editor_marquee_selection(x, y);
            } else if mode == EditorMode::Trigger {
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
    pub(crate) fn handle_pointer_moved(&mut self, x: f64, y: f64) {
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
