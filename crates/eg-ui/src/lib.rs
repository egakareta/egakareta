/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Egui user-interface layer for egakareta.
//!
//! This crate renders editor, menu, settings, and profiler UI from App-world
//! resources. Production UI writes semantic commands directly to the App
//! world's typed message buffers through `WorldUiCommandSink`.

#![warn(missing_docs)]

pub(crate) mod components;
pub(crate) mod menu;
pub(crate) mod modes;
pub mod plugin;

use std::collections::{BTreeMap, HashMap};

use bevy::ecs::world::World;
use eg_blocks::{BlockCategory, BlockDefinition, BlockId};
use eg_state::{
    events::{
        auth, block, bpm, camera, editor_mode, history, misc, navigation, pointer, selection,
        session, snap, spawn, timeline, timing,
    },
    state::EditorConfigState,
};
use eg_types::{
    AppSettings, BasicLevelMetadata, EditorMode, LevelObject, MusicMetadata, SettingsSection,
    essential_keybind_actions, format_key_chord,
};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use glam::{Mat3, Vec3};
pub use menu::{
    MenuTextures, load_menu_textures, register_custom_fonts, show_menu_favicon_ui,
    show_menu_play_ui, show_menu_topbar_ui, show_pause_menu_ui, show_practice_checkpoint_ui,
};
pub use plugin::UiPlugin;

use crate::{
    components::{
        TimelineBarInputs, WaveformPanelInputs, show_timeline_bar, show_waveform_panel,
        timeline_metrics,
    },
    modes::{
        compose::{
            show_block_preview_button, show_place_block_properties_window,
            show_recent_block_quick_strip, show_selected_block_properties_window,
        },
        shared::show_mode_selector_popup,
        tapping::{show_selected_tap_properties_window, show_tapping_mode_bottom_panel},
        timing::{TimingModePanelInputs, show_timing_mode_bottom_panel},
    },
};

macro_rules! ui_command_sink {
    ($( $(#[$meta:meta])* fn $name:ident($($args:tt)*); )*) => {
        /// Destination for semantic UI commands emitted while egui is running.
        ///
        /// [`WorldUiCommandSink`] writes each command to the App world's typed
        /// message buffer immediately in both production and focused tests.
        pub trait UiCommandSink {
            $(
                $(#[$meta])*
                fn $name($($args)*);
            )*
        }
    };
}

ui_command_sink! {
    /// Select the block used by the editor placement tool.
    fn set_block_id(&mut self, block_id: BlockId);
    /// Change the active editor interaction mode.
    fn set_editor_mode(&mut self, mode: EditorMode);
    /// Apply changed properties to the selected authored block.
    fn update_selected_block(&mut self, object: LevelObject);
    /// Enable or disable position and size snapping.
    fn set_snap_to_grid(&mut self, enabled: bool);
    /// Set the position and size snap increment.
    fn set_snap_step(&mut self, step: f32);
    /// Enable or disable rotation snapping.
    fn set_snap_rotation(&mut self, enabled: bool);
    /// Set the rotation snap increment in degrees.
    fn set_snap_rotation_step(&mut self, step: f32);
    /// Add a tap at the current timeline position.
    fn add_tap(&mut self);
    /// Remove every authored tap.
    fn clear_taps(&mut self);
    /// Remove the selected tap.
    fn remove_tap(&mut self);
    /// Set the selected tap time in seconds.
    fn set_selected_tap_time(&mut self, time_seconds: f32);
    /// Set editor preview playback speed.
    fn set_playback_speed(&mut self, speed: f32);
    /// Set the editor timeline duration.
    fn set_timeline_duration(&mut self, duration_seconds: f32);
    /// Remove a timing point by index.
    fn remove_timing_point(&mut self, index: usize);
    /// Update a timing point's time offset.
    fn set_timing_point_time(&mut self, index: usize, time: f32);
    /// Update a timing point's BPM.
    fn set_timing_point_bpm(&mut self, index: usize, bpm: f32);
    /// Update a timing point's time signature.
    fn set_timing_point_time_signature(&mut self, index: usize, numerator: u32, denominator: u32);
    /// Select an optional timing point.
    fn set_timing_selected(&mut self, index: Option<usize>);
    /// Record one beat for BPM detection.
    fn bpm_tap(&mut self);
    /// Clear the BPM tap detector.
    fn bpm_tap_reset(&mut self);
    /// Seek the editor timeline.
    fn set_timeline_time(&mut self, time_seconds: f32);
    /// Select an optional authored tap.
    fn set_selected_tap(&mut self, index: Option<usize>);
    /// Set waveform/timeline scroll offset.
    fn set_waveform_scroll(&mut self, scroll: f32);
    /// Toggle editor timeline playback.
    fn toggle_timeline_playback(&mut self);
    /// Set waveform/timeline zoom.
    fn set_waveform_zoom(&mut self, zoom: f32);
    /// Remove an authored tap at an exact time.
    fn remove_tap_at(&mut self, time_seconds: f32);
    /// Select an optional camera trigger.
    fn set_trigger_selected(&mut self, index: Option<usize>);
    /// Start an editor playtest.
    fn playtest(&mut self);
    /// Add a timing point with the supplied time and BPM.
    fn add_timing_point(&mut self, time_seconds: f32, bpm: f32);
    /// Start the selected level or turn during gameplay.
    fn turn_right(&mut self);
    /// Toggle between the menu/gameplay and the editor.
    fn toggle_editor(&mut self);
    /// Resume paused gameplay.
    fn game_resume(&mut self);
    /// Restart the active gameplay level.
    fn game_restart_level(&mut self);
    /// Return from gameplay to the menu.
    fn game_quit_to_menu(&mut self);
    /// Enable or disable practice mode.
    fn game_set_practice_mode(&mut self, enabled: bool);
    /// Store a practice checkpoint.
    fn game_set_practice_checkpoint(&mut self);
    /// Remove the active practice checkpoint.
    fn game_remove_practice_checkpoint(&mut self);
    /// Toggle the settings panel.
    fn toggle_settings(&mut self);
    /// Start the configured sign-in flow.
    fn auth_submit_sign_in(&mut self);
    /// Sign out of the active account.
    fn auth_sign_out(&mut self);
    /// Open the configured signup page.
    fn auth_open_signup(&mut self);
    /// Undo the most recent editor edit.
    fn undo(&mut self);
    /// Redo the most recently undone editor edit.
    fn redo(&mut self);
    /// Copy the selected authored blocks.
    fn copy_block(&mut self);
    /// Remove the selected authored blocks.
    fn remove_block(&mut self);
    /// Paste copied authored blocks at the cursor.
    fn paste_block(&mut self);
    /// Duplicate selected authored blocks.
    fn duplicate_block(&mut self);
    /// Update the editor cursor from a screen-space location.
    fn update_cursor_from_screen(&mut self, x: f64, y: f64);
    /// Snap the selected authored item to the active grid.
    fn snap_selection_to_grid(&mut self);
    /// Focus the editor camera on the active selection.
    fn focus_camera_target(&mut self);
    /// Pick the selected block's placement type.
    fn pick_selected_block(&mut self);
    /// Add a transform trigger to the selection.
    fn add_transform_trigger(&mut self);
    /// Toggle the block-catalog placement window.
    fn toggle_place_window(&mut self);
    /// Move the editor spawn to the cursor.
    fn set_spawn_here(&mut self);
    /// Add a camera trigger at the cursor.
    fn add_camera_trigger(&mut self);
    /// Add a camera-follow trigger at the cursor.
    fn add_camera_follow_trigger(&mut self);
    /// Toggle editor hitbox visualization.
    fn toggle_hitbox_visualization(&mut self);
    /// Show or hide the settings sidebar.
    fn set_show_settings(&mut self, show: bool);
    /// Select the active settings sidebar section.
    fn set_settings_section(&mut self, section: SettingsSection);
    /// Set the selected graphics backend.
    fn set_graphics_backend(&mut self, backend: String);
    /// Set the selected audio backend.
    fn set_audio_backend(&mut self, backend: String);
    /// Set the UI-scale multiplier.
    fn set_ui_scale_multiplier(&mut self, scale: f32);
    /// Restore all configurable shortcuts to their defaults.
    fn reset_keybinds(&mut self);
    /// Load an editor level by name.
    fn load_level(&mut self, name: String);
    /// Export the active editor level.
    fn export_level(&mut self);
    /// Open the level-import flow.
    fn complete_import(&mut self);
    /// Show or hide the level-metadata window.
    fn set_show_metadata(&mut self, show: bool);
    /// Rename the active editor level.
    fn rename_level(&mut self, name: String);
    /// Open the audio-import flow.
    fn trigger_audio_import(&mut self);
    /// Apply edited music metadata.
    fn update_music(&mut self, metadata: MusicMetadata);
    /// Apply edited creator metadata.
    fn update_creator_metadata(&mut self, metadata: BasicLevelMetadata);
    /// Apply the editor sky color.
    fn update_sky_color(&mut self, color: [f32; 3]);
    /// Store the current camera as the menu preview camera.
    fn capture_menu_preview_camera(&mut self);
    /// Derive the menu preview camera from the spawn configuration.
    fn use_auto_menu_preview_camera(&mut self);
    /// Copy the latest profiler frame to the clipboard.
    fn copy_latest_perf_frame(&mut self);
    /// Toggle the profiler overlay.
    fn toggle_perf_overlay(&mut self);
    /// Restore one shortcut action to its default bindings.
    fn reset_keybind(&mut self, action: String);
    /// Set or clear the shortcut currently capturing a chord.
    fn set_keybind_capture(&mut self, capture: Option<(String, usize)>);
    /// Clear one shortcut-binding slot.
    fn clear_keybind_slot(&mut self, action: String, slot: usize);
    /// Set camera orientation, optionally animating the transition.
    fn set_camera_orientation(
        &mut self,
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    );
}

/// Writes UI commands directly into the App world's typed message buffers.
pub struct WorldUiCommandSink<'w> {
    world: &'w mut World,
}

impl<'w> WorldUiCommandSink<'w> {
    /// Create a sink that writes into the given App world.
    pub fn new(world: &'w mut World) -> Self {
        Self { world }
    }
}

impl UiCommandSink for WorldUiCommandSink<'_> {
    fn set_block_id(&mut self, block_id: BlockId) {
        self.world.write_message(editor_mode::SetBlockId(block_id));
    }

    fn set_editor_mode(&mut self, mode: EditorMode) {
        self.world.write_message(editor_mode::SetEditorMode(mode));
    }

    fn update_selected_block(&mut self, object: LevelObject) {
        self.world.write_message(block::UpdateSelectedBlock(object));
    }

    fn set_snap_to_grid(&mut self, enabled: bool) {
        self.world.write_message(snap::SetSnapToGrid(enabled));
    }

    fn set_snap_step(&mut self, step: f32) {
        self.world.write_message(snap::SetSnapStep(step));
    }

    fn set_snap_rotation(&mut self, enabled: bool) {
        self.world.write_message(snap::SetSnapRotation(enabled));
    }

    fn set_snap_rotation_step(&mut self, step: f32) {
        self.world.write_message(snap::SetSnapRotationStep(step));
    }

    fn add_tap(&mut self) {
        self.world.write_message(timeline::AddTap);
    }

    fn clear_taps(&mut self) {
        self.world.write_message(timeline::ClearTaps);
    }

    fn remove_tap(&mut self) {
        self.world.write_message(timeline::RemoveTap);
    }

    fn set_selected_tap_time(&mut self, time_seconds: f32) {
        self.world
            .write_message(timeline::SetSelectedTapTime(time_seconds));
    }

    fn set_playback_speed(&mut self, speed: f32) {
        self.world.write_message(timeline::SetPlaybackSpeed(speed));
    }

    fn set_timeline_duration(&mut self, duration_seconds: f32) {
        self.world
            .write_message(timeline::SetTimelineDuration(duration_seconds));
    }

    fn remove_timing_point(&mut self, index: usize) {
        self.world.write_message(timing::RemoveTimingPoint(index));
    }

    fn set_timing_point_time(&mut self, index: usize, time: f32) {
        self.world
            .write_message(timing::SetTimingPointTime { index, time });
    }

    fn set_timing_point_bpm(&mut self, index: usize, bpm: f32) {
        self.world
            .write_message(timing::SetTimingPointBpm { index, bpm });
    }

    fn set_timing_point_time_signature(&mut self, index: usize, numerator: u32, denominator: u32) {
        self.world
            .write_message(timing::SetTimingPointTimeSignature {
                index,
                numerator,
                denominator,
            });
    }

    fn set_timing_selected(&mut self, index: Option<usize>) {
        self.world.write_message(timing::SetTimingSelected(index));
    }

    fn bpm_tap(&mut self) {
        self.world.write_message(bpm::BpmTap);
    }

    fn bpm_tap_reset(&mut self) {
        self.world.write_message(bpm::BpmTapReset);
    }

    fn set_timeline_time(&mut self, time_seconds: f32) {
        self.world
            .write_message(timeline::SetTimelineTime(time_seconds));
    }

    fn set_selected_tap(&mut self, index: Option<usize>) {
        self.world.write_message(timeline::SetSelectedTap(index));
    }

    fn set_waveform_scroll(&mut self, scroll: f32) {
        self.world
            .write_message(timeline::SetWaveformScroll(scroll));
    }

    fn toggle_timeline_playback(&mut self) {
        self.world.write_message(timeline::ToggleTimelinePlayback);
    }

    fn set_waveform_zoom(&mut self, zoom: f32) {
        self.world.write_message(timeline::SetWaveformZoom(zoom));
    }

    fn remove_tap_at(&mut self, time_seconds: f32) {
        self.world
            .write_message(timeline::RemoveTapAt(time_seconds));
    }

    fn set_trigger_selected(&mut self, index: Option<usize>) {
        self.world.write_message(camera::SetTriggerSelected(index));
    }

    fn playtest(&mut self) {
        self.world.write_message(timeline::Playtest);
    }

    fn add_timing_point(&mut self, time_seconds: f32, bpm: f32) {
        self.world
            .write_message(timing::AddTimingPoint { time_seconds, bpm });
    }

    fn turn_right(&mut self) {
        self.world.write_message(navigation::TurnRight);
    }

    fn toggle_editor(&mut self) {
        self.world.write_message(navigation::ToggleEditor);
    }

    fn game_resume(&mut self) {
        self.world.write_message(navigation::GameResume);
    }

    fn game_restart_level(&mut self) {
        self.world.write_message(navigation::GameRestartLevel);
    }

    fn game_quit_to_menu(&mut self) {
        self.world.write_message(navigation::GameQuitToMenu);
    }

    fn game_set_practice_mode(&mut self, enabled: bool) {
        self.world
            .write_message(navigation::GameSetPracticeMode { enabled });
    }

    fn game_set_practice_checkpoint(&mut self) {
        self.world
            .write_message(navigation::GameSetPracticeCheckpoint);
    }

    fn game_remove_practice_checkpoint(&mut self) {
        self.world
            .write_message(navigation::GameRemovePracticeCheckpoint);
    }

    fn toggle_settings(&mut self) {
        self.world.write_message(session::ToggleSettings);
    }

    fn auth_submit_sign_in(&mut self) {
        self.world.write_message(auth::AuthSubmitSignIn);
    }

    fn auth_sign_out(&mut self) {
        self.world.write_message(auth::AuthSignOut);
    }

    fn auth_open_signup(&mut self) {
        self.world.write_message(auth::AuthOpenSignup);
    }

    fn undo(&mut self) {
        self.world.write_message(history::Undo);
    }

    fn redo(&mut self) {
        self.world.write_message(history::Redo);
    }

    fn copy_block(&mut self) {
        self.world.write_message(block::CopyBlock);
    }

    fn remove_block(&mut self) {
        self.world.write_message(block::RemoveBlock);
    }

    fn paste_block(&mut self) {
        self.world.write_message(block::PasteBlock);
    }

    fn duplicate_block(&mut self) {
        self.world.write_message(block::DuplicateBlock);
    }

    fn update_cursor_from_screen(&mut self, x: f64, y: f64) {
        self.world
            .write_message(pointer::UpdateCursorFromScreen { x, y });
    }

    fn snap_selection_to_grid(&mut self) {
        self.world.write_message(selection::SnapSelectionToGrid);
    }

    fn focus_camera_target(&mut self) {
        self.world.write_message(selection::FocusCameraTarget);
    }

    fn pick_selected_block(&mut self) {
        self.world.write_message(editor_mode::PickSelectedBlock);
    }

    fn add_transform_trigger(&mut self) {
        self.world.write_message(selection::AddTransformTrigger);
    }

    fn toggle_place_window(&mut self) {
        self.world.write_message(misc::TogglePlaceWindow);
    }

    fn set_spawn_here(&mut self) {
        self.world.write_message(spawn::SetSpawnHere);
    }

    fn add_camera_trigger(&mut self) {
        self.world.write_message(camera::AddCameraTrigger);
    }

    fn add_camera_follow_trigger(&mut self) {
        self.world.write_message(camera::AddCameraFollowTrigger);
    }

    fn toggle_hitbox_visualization(&mut self) {
        self.world.write_message(misc::ToggleHitboxVisualization);
    }

    fn set_show_settings(&mut self, show: bool) {
        self.world.write_message(session::SetShowSettings(show));
    }

    fn set_settings_section(&mut self, section: SettingsSection) {
        self.world
            .write_message(session::SetSettingsSection(section));
    }

    fn set_graphics_backend(&mut self, backend: String) {
        self.world
            .write_message(session::SetGraphicsBackend(backend));
    }

    fn set_audio_backend(&mut self, backend: String) {
        self.world.write_message(session::SetAudioBackend(backend));
    }

    fn set_ui_scale_multiplier(&mut self, scale: f32) {
        self.world
            .write_message(session::SetUiScaleMultiplier(scale));
    }

    fn reset_keybinds(&mut self) {
        self.world.write_message(session::ResetKeybinds);
    }

    fn load_level(&mut self, name: String) {
        self.world.write_message(session::LoadLevel(name));
    }

    fn export_level(&mut self) {
        self.world.write_message(session::ExportLevel);
    }

    fn complete_import(&mut self) {
        self.world.write_message(session::CompleteImport);
    }

    fn set_show_metadata(&mut self, show: bool) {
        self.world.write_message(session::SetShowMetadata(show));
    }

    fn rename_level(&mut self, name: String) {
        self.world.write_message(session::RenameLevel(name));
    }

    fn trigger_audio_import(&mut self) {
        self.world.write_message(session::TriggerAudioImport);
    }

    fn update_music(&mut self, metadata: MusicMetadata) {
        self.world.write_message(session::UpdateMusic(metadata));
    }

    fn update_creator_metadata(&mut self, metadata: BasicLevelMetadata) {
        self.world
            .write_message(session::UpdateCreatorMetadata(metadata));
    }

    fn update_sky_color(&mut self, color: [f32; 3]) {
        self.world.write_message(session::UpdateSkyColor(color));
    }

    fn capture_menu_preview_camera(&mut self) {
        self.world.write_message(session::CaptureMenuPreviewCamera);
    }

    fn use_auto_menu_preview_camera(&mut self) {
        self.world.write_message(session::UseAutoMenuPreviewCamera);
    }

    fn copy_latest_perf_frame(&mut self) {
        self.world.write_message(auth::CopyLatestPerfFrame);
    }

    fn toggle_perf_overlay(&mut self) {
        self.world.write_message(misc::TogglePerfOverlay);
    }

    fn reset_keybind(&mut self, action: String) {
        self.world.write_message(session::ResetKeybind(action));
    }

    fn set_keybind_capture(&mut self, capture: Option<(String, usize)>) {
        self.world
            .write_message(session::SetKeybindCapture(capture));
    }

    fn clear_keybind_slot(&mut self, action: String, slot: usize) {
        self.world
            .write_message(session::ClearKeybindSlot { action, slot });
    }

    fn set_camera_orientation(
        &mut self,
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    ) {
        self.world.write_message(camera::SetCameraOrientation {
            rotation,
            pitch,
            transition_seconds,
        });
    }
}

#[derive(Clone, Copy)]
struct ViewCubeFace {
    label: &'static str,
    normal: Vec3,
    indices: [usize; 4],
    rotation: f32,
    pitch: f32,
}

const VIEW_CUBE_FACES: [ViewCubeFace; 6] = [
    ViewCubeFace {
        label: "Front",
        normal: Vec3::Z,
        indices: [4, 5, 6, 7],
        rotation: std::f32::consts::PI,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Back",
        normal: Vec3::new(0.0, 0.0, -1.0),
        indices: [0, 1, 2, 3],
        rotation: 0.0,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Left",
        normal: Vec3::new(-1.0, 0.0, 0.0),
        indices: [0, 3, 7, 4],
        rotation: std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Right",
        normal: Vec3::X,
        indices: [2, 1, 5, 6],
        rotation: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    },
    ViewCubeFace {
        label: "Top",
        normal: Vec3::Y,
        indices: [3, 2, 6, 7],
        rotation: 0.0,
        pitch: 89.0f32.to_radians(),
    },
    ViewCubeFace {
        label: "Bottom",
        normal: Vec3::new(0.0, -1.0, 0.0),
        indices: [1, 0, 4, 5],
        rotation: 0.0,
        pitch: -89.0f32.to_radians(),
    },
];

const EASY_STAR_THRESHOLD: f32 = 2.0;
const NORMAL_STAR_THRESHOLD: f32 = 4.0;
const HARD_STAR_THRESHOLD: f32 = 6.0;
const HARDER_STAR_THRESHOLD: f32 = 8.0;
const INSANE_STAR_THRESHOLD: f32 = 10.0;

fn optional_trimmed_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn tags_to_text(tags: &[String]) -> String {
    tags.join(", ")
}

fn tags_from_text(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn difficulty_for_stars(stars: f32) -> &'static str {
    if stars < EASY_STAR_THRESHOLD {
        "easy"
    } else if stars < NORMAL_STAR_THRESHOLD {
        "normal"
    } else if stars < HARD_STAR_THRESHOLD {
        "hard"
    } else if stars < HARDER_STAR_THRESHOLD {
        "harder"
    } else if stars < INSANE_STAR_THRESHOLD {
        "insane"
    } else {
        "demon"
    }
}

const COMPACT_EDITOR_UI_BREAKPOINT: f32 = 720.0;
const SETTINGS_SIDEBAR_TOTAL_PADDING: f32 = 24.0;
const SETTINGS_SIDEBAR_MIN_WIDTH: f32 = 180.0;
const SETTINGS_SIDEBAR_DEFAULT_WIDTH: f32 = 320.0;
const SETTINGS_SIDEBAR_COMPACT_WIDTH_SCALE: f32 = 0.78;
const RESPONSIVE_UI_SCALE_MIN: f32 = 0.8;
const RESPONSIVE_UI_SCALE_MAX: f32 = 1.35;
const UI_SCALE_BASE_WIDTH: f32 = 1280.0;
const UI_SCALE_BASE_HEIGHT: f32 = 720.0;
const KEYBIND_ACTION_LABEL_WIDTH: f32 = 180.0;
const KEYBIND_CONTROLS_MIN_WIDTH: f32 = 220.0;
const KEYBIND_CONTENT_MIN_WIDTH: f32 =
    KEYBIND_ACTION_LABEL_WIDTH + KEYBIND_CONTROLS_MIN_WIDTH + 12.0;
const EDITOR_CONTEXT_MENU_WIDTH: f32 = 220.0;

const BLOCK_CATALOG_CATEGORIES: [Option<BlockCategory>; 5] = [
    None,
    Some(BlockCategory::Building),
    Some(BlockCategory::Danger),
    Some(BlockCategory::Tech),
    Some(BlockCategory::Trigger),
];
const PLACE_BLOCK_CATALOG_COLUMNS: usize = 9;
const PLACE_BLOCK_CATALOG_ROWS: usize = 4;
const PLACE_BLOCK_CATALOG_CELL_SIZE: f32 = 72.0;

fn block_catalog_category_label(category: Option<BlockCategory>) -> &'static str {
    match category {
        None => "All",
        Some(BlockCategory::Building) => "Building",
        Some(BlockCategory::Danger) => "Danger",
        Some(BlockCategory::Tech) => "Tech",
        Some(BlockCategory::Trigger) => "Trigger",
    }
}

fn is_compact_editor_ui(viewport_width: f32) -> bool {
    viewport_width <= COMPACT_EDITOR_UI_BREAKPOINT
}

fn block_matches_catalog_category(
    block: &BlockDefinition,
    category: Option<BlockCategory>,
) -> bool {
    category.is_none_or(|block_category| block.category == block_category)
}

fn settings_sidebar_default_width(viewport_width: f32) -> f32 {
    let max_width = (viewport_width - SETTINGS_SIDEBAR_TOTAL_PADDING).max(0.0);
    if max_width <= SETTINGS_SIDEBAR_MIN_WIDTH {
        return max_width;
    }

    (viewport_width * SETTINGS_SIDEBAR_COMPACT_WIDTH_SCALE)
        .min(SETTINGS_SIDEBAR_DEFAULT_WIDTH)
        .clamp(SETTINGS_SIDEBAR_MIN_WIDTH, max_width)
}

pub(crate) fn responsive_ui_scale_multiplier(viewport_size: egui::Vec2) -> f32 {
    if !viewport_size.x.is_finite()
        || !viewport_size.y.is_finite()
        || viewport_size.x <= 0.0
        || viewport_size.y <= 0.0
    {
        return 1.0;
    }

    let width_scale = viewport_size.x / UI_SCALE_BASE_WIDTH;
    let height_scale = viewport_size.y / UI_SCALE_BASE_HEIGHT;
    width_scale
        .min(height_scale)
        .clamp(RESPONSIVE_UI_SCALE_MIN, RESPONSIVE_UI_SCALE_MAX)
}

/// Combines responsive viewport scaling with the user's configured UI scale multiplier.
pub fn combined_ui_scale_factor(viewport_size: egui::Vec2, user_multiplier: f32) -> f32 {
    let responsive = responsive_ui_scale_multiplier(viewport_size);
    let user = eg_types::AppSettings::clamp_ui_scale_multiplier(user_multiplier);
    (responsive * user).clamp(0.5, 4.0)
}

fn sort_quad_by_angle(center: egui::Pos2, quad: [egui::Pos2; 4]) -> [egui::Pos2; 4] {
    let mut points = quad.to_vec();
    points.sort_by(|a, b| {
        let angle_a = (a.y - center.y).atan2(a.x - center.x);
        let angle_b = (b.y - center.y).atan2(b.x - center.x);
        angle_a.total_cmp(&angle_b)
    });
    [points[0], points[1], points[2], points[3]]
}

fn add_face_mesh(painter: &egui::Painter, quad: [egui::Pos2; 4], color: egui::Color32) {
    let mut mesh = Mesh::default();
    for pos in quad {
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

fn marquee_screen_pos_to_egui_pos(screen_pos: [f64; 2], pixels_per_point: f32) -> egui::Pos2 {
    let scale = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };

    egui::pos2(screen_pos[0] as f32 / scale, screen_pos[1] as f32 / scale)
}

fn rect_to_input_bounds(rect: egui::Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
}

fn point_in_rect_bounds(point: egui::Pos2, rect: [f32; 4]) -> bool {
    point.x >= rect[0] && point.x <= rect[2] && point.y >= rect[1] && point.y <= rect[3]
}

fn pointer_over_ui_rects(point: egui::Pos2, rects: &[[f32; 4]]) -> bool {
    rects.iter().any(|rect| point_in_rect_bounds(point, *rect))
}

fn context_menu_button<S: UiCommandSink>(
    ui: &mut egui::Ui,
    commands: &mut S,
    enabled: bool,
    icon: &str,
    label: &str,
    action: impl FnOnce(&mut S),
    hotkey: Option<&str>,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let text = format!("{icon} {label}");
        clicked = ui
            .add_enabled(enabled, egui::Button::new(text).frame(false))
            .clicked();
        if let Some(hotkey) = hotkey {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(hotkey)
                        .color(egui::Color32::from_gray(140))
                        .size(11.0),
                );
            });
        }
    });
    if clicked {
        action(commands);
        true
    } else {
        false
    }
}

#[derive(Clone, Copy)]
struct EditorContextMenuInputs<'a> {
    app_settings: &'a AppSettings,
    selected_tap: Option<(usize, f32, [f32; 3])>,
    editor_mode: EditorMode,
    editor_last_mode: Option<EditorMode>,
    selected_block_count: usize,
    clipboard_block_count: usize,
    can_undo: bool,
    can_redo: bool,
}

fn show_editor_context_menu(
    ctx: &egui::Context,
    inputs: EditorContextMenuInputs<'_>,
    ui_input_blocking_rects: &mut Vec<[f32; 4]>,
    commands: &mut impl UiCommandSink,
) {
    let EditorContextMenuInputs {
        app_settings,
        selected_tap,
        editor_mode,
        editor_last_mode,
        selected_block_count,
        clipboard_block_count,
        can_undo,
        can_redo,
    } = inputs;
    let menu_id = egui::Id::new("editor_canvas_context_menu_position");
    let secondary_clicked = ctx.input(|input| input.pointer.secondary_clicked());
    let mut opened_this_frame = false;
    if secondary_clicked
        && let Some(pointer) = ctx.input(|input| input.pointer.latest_pos())
        && !ctx.is_pointer_over_egui()
        && !pointer_over_ui_rects(pointer, ui_input_blocking_rects)
    {
        ctx.data_mut(|data| data.insert_temp(menu_id, pointer));
        opened_this_frame = true;
    }

    let Some(menu_position) = ctx.data_mut(|data| data.get_temp::<egui::Pos2>(menu_id)) else {
        return;
    };

    let has_block_selection = selected_block_count > 0;
    let has_clipboard = clipboard_block_count > 0;
    let has_tap_selection = selected_tap.is_some();
    let has_focus_target = has_block_selection || has_tap_selection;
    let is_tapping = editor_mode == EditorMode::Tapping
        || (editor_mode == EditorMode::Null && editor_last_mode == Some(EditorMode::Tapping));
    let selected_label = if selected_block_count == 1 {
        "selected block"
    } else {
        "selected blocks"
    };

    let hint = |action: &str| -> Option<String> {
        let h = app_settings.hotkey_hint(action);
        if h.is_empty() { None } else { Some(h) }
    };

    let response = egui::Area::new("editor_canvas_context_menu".into())
        .order(egui::Order::Foreground)
        .fixed_pos(menu_position)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(EDITOR_CONTEXT_MENU_WIDTH);
                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);

                let mut close_menu = false;
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    can_undo,
                    egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                    "Undo",
                    UiCommandSink::undo,
                    hint("undo").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    can_redo,
                    egui_phosphor::regular::ARROW_CLOCKWISE,
                    "Redo",
                    UiCommandSink::redo,
                    hint("redo").as_deref(),
                );

                ui.separator();

                {
                    let cut_hotkey_str = hint("copy");
                    let cut_hotkey = cut_hotkey_str.as_deref();
                    let mut cut_clicked = false;
                    ui.horizontal(|ui| {
                        cut_clicked = ui
                            .add_enabled(
                                has_block_selection && !is_tapping,
                                egui::Button::new(format!(
                                    "{} Cut",
                                    egui_phosphor::regular::SCISSORS
                                ))
                                .frame(false),
                            )
                            .on_hover_text(format!("Cut {selected_label}"))
                            .clicked();
                        if let Some(hotkey) = cut_hotkey {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(hotkey)
                                            .color(egui::Color32::from_gray(140))
                                            .size(11.0),
                                    );
                                },
                            );
                        }
                    });
                    if cut_clicked {
                        commands.copy_block();
                        commands.remove_block();
                        close_menu = true;
                    }
                }
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_block_selection,
                    egui_phosphor::regular::COPY,
                    "Copy",
                    UiCommandSink::copy_block,
                    hint("copy").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_clipboard,
                    egui_phosphor::regular::CLIPBOARD_TEXT,
                    "Paste",
                    |commands| {
                        commands.update_cursor_from_screen(
                            (menu_position.x * ctx.pixels_per_point()) as f64,
                            (menu_position.y * ctx.pixels_per_point()) as f64,
                        );
                        commands.paste_block();
                    },
                    hint("paste").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_block_selection && !is_tapping,
                    egui_phosphor::regular::COPY_SIMPLE,
                    "Duplicate",
                    UiCommandSink::duplicate_block,
                    hint("duplicate").as_deref(),
                );

                if let Some((_, selected_tap_time_seconds, _)) = selected_tap {
                    close_menu |= context_menu_button(
                        ui,
                        commands,
                        true,
                        egui_phosphor::regular::TRASH,
                        "Delete Tap",
                        |commands| commands.remove_tap_at(selected_tap_time_seconds),
                        hint("remove_block").as_deref(),
                    );
                } else {
                    close_menu |= context_menu_button(
                        ui,
                        commands,
                        has_block_selection,
                        egui_phosphor::regular::TRASH,
                        "Delete",
                        UiCommandSink::remove_block,
                        hint("remove_block").as_deref(),
                    );
                }

                ui.separator();

                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_block_selection || has_tap_selection,
                    egui_phosphor::regular::CORNERS_OUT,
                    "Snap to Grid",
                    UiCommandSink::snap_selection_to_grid,
                    hint("snap_selection_to_grid").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_focus_target,
                    egui_phosphor::regular::CROSSHAIR,
                    "Focus View",
                    UiCommandSink::focus_camera_target,
                    hint("focus_camera_target").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_block_selection,
                    egui_phosphor::regular::EYEDROPPER,
                    "Pick Block Type",
                    UiCommandSink::pick_selected_block,
                    hint("pick_selected_block").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    has_block_selection && !is_tapping,
                    egui_phosphor::regular::ARROWS_CLOCKWISE,
                    "Add Transform Trigger",
                    UiCommandSink::add_transform_trigger,
                    hint("add_transform_trigger").as_deref(),
                );

                ui.separator();

                close_menu |= context_menu_button(
                    ui,
                    commands,
                    true,
                    egui_phosphor::regular::CUBE,
                    "Block Catalog",
                    UiCommandSink::toggle_place_window,
                    hint("toggle_place_window").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    true,
                    egui_phosphor::regular::MAP_PIN,
                    "Set Spawn Here",
                    |commands| {
                        commands.update_cursor_from_screen(
                            (menu_position.x * ctx.pixels_per_point()) as f64,
                            (menu_position.y * ctx.pixels_per_point()) as f64,
                        );
                        commands.set_spawn_here();
                    },
                    hint("spawn_set").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    true,
                    egui_phosphor::regular::CAMERA,
                    "Add Camera Trigger",
                    UiCommandSink::add_camera_trigger,
                    hint("add_camera_trigger").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    true,
                    egui_phosphor::regular::CAMERA_ROTATE,
                    "Add Camera Follow Trigger",
                    UiCommandSink::add_camera_follow_trigger,
                    hint("add_camera_follow_trigger").as_deref(),
                );
                close_menu |= context_menu_button(
                    ui,
                    commands,
                    true,
                    egui_phosphor::regular::SELECTION,
                    "Toggle Hitboxes",
                    UiCommandSink::toggle_hitbox_visualization,
                    hint("toggle_hitbox_visualization").as_deref(),
                );

                if close_menu {
                    ctx.data_mut(|data| data.remove::<egui::Pos2>(menu_id));
                }
            });
        });

    let menu_rect = response.response.rect;
    ui_input_blocking_rects.push(rect_to_input_bounds(menu_rect));

    let clicked_outside = ctx.input(|input| {
        (input.pointer.primary_clicked() || input.pointer.secondary_clicked())
            && input
                .pointer
                .latest_pos()
                .is_some_and(|pointer| !menu_rect.contains(pointer))
    });
    if clicked_outside && !opened_this_frame {
        ctx.data_mut(|data| data.remove::<egui::Pos2>(menu_id));
    }
}

#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub struct SettingsSidebarInputs<'a> {
    pub show_settings: bool,
    pub is_menu: bool,
    pub is_editor: bool,
    pub is_game_paused: bool,
    pub app_settings: &'a AppSettings,
    pub graphics_backend: &'a str,
    pub graphics_backend_options: &'a [String],
    pub graphics_backend_locked: bool,
    pub settings_restart_required: bool,
    pub audio_backend: &'a str,
    pub audio_backend_options: &'a [String],
    pub audio_backend_locked: bool,
    pub settings_section: SettingsSection,
    pub keybind_capture_action: Option<&'a (String, usize)>,
}

fn show_settings_sidebar_area(
    ctx: &egui::Context,
    inputs: SettingsSidebarInputs<'_>,
    commands: &mut impl UiCommandSink,
) -> egui::InnerResponse<()> {
    let viewport_width = ctx.content_rect().width();
    let viewport_height = ctx.content_rect().height();
    let sidebar_width = settings_sidebar_default_width(viewport_width);

    egui::Area::new("editor_settings_sidebar_overlay".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::side_top_panel(ui.style()).show(ui, |ui| {
                show_settings_sidebar_panel(ui, inputs, commands, sidebar_width, viewport_height);
            });
        })
}

fn show_settings_sidebar_panel(
    ui: &mut egui::Ui,
    inputs: SettingsSidebarInputs<'_>,
    commands: &mut impl UiCommandSink,
    sidebar_width: f32,
    viewport_height: f32,
) {
    let SettingsSidebarInputs {
        app_settings,
        settings_section,
        keybind_capture_action,
        ..
    } = inputs;
    ui.set_width(sidebar_width);
    ui.set_min_height(viewport_height);

    show_settings_sidebar_header(ui, commands);
    show_settings_section_tabs(ui, settings_section, commands);

    ui.separator();

    match settings_section {
        SettingsSection::Backends => show_backend_settings(ui, inputs, commands),
        SettingsSection::Keybinds => {
            show_keybind_settings(ui, app_settings, keybind_capture_action, commands)
        }
    }
}

fn show_settings_sidebar_header(ui: &mut egui::Ui, commands: &mut impl UiCommandSink) {
    ui.horizontal(|ui| {
        ui.heading(format!("{} Settings", egui_phosphor::regular::GEAR));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close_button_size =
                egui::vec2(ui.spacing().interact_size.y, ui.spacing().interact_size.y);
            if ui
                .add_sized(
                    close_button_size,
                    egui::Button::new(egui_phosphor::regular::X).frame(false),
                )
                .on_hover_text("Close settings")
                .clicked()
            {
                commands.set_show_settings(false);
            }
        });
    });
}

fn show_settings_section_tabs(
    ui: &mut egui::Ui,
    settings_section: SettingsSection,
    commands: &mut impl UiCommandSink,
) {
    ui.horizontal(|ui| {
        show_settings_section_tab(
            ui,
            settings_section,
            SettingsSection::Backends,
            format!("{} Backends", egui_phosphor::regular::MONITOR),
            commands,
        );
        show_settings_section_tab(
            ui,
            settings_section,
            SettingsSection::Keybinds,
            format!("{} Keybinds", egui_phosphor::regular::KEYBOARD),
            commands,
        );
    });
}

fn show_settings_section_tab(
    ui: &mut egui::Ui,
    current_section: SettingsSection,
    tab_section: SettingsSection,
    label: String,
    commands: &mut impl UiCommandSink,
) {
    if ui
        .selectable_label(current_section == tab_section, label)
        .clicked()
    {
        commands.set_settings_section(tab_section);
    }
}

fn show_backend_settings(
    ui: &mut egui::Ui,
    inputs: SettingsSidebarInputs<'_>,
    commands: &mut impl UiCommandSink,
) {
    let SettingsSidebarInputs {
        app_settings,
        graphics_backend,
        graphics_backend_options,
        graphics_backend_locked,
        settings_restart_required,
        audio_backend,
        audio_backend_options,
        audio_backend_locked,
        ..
    } = inputs;
    show_backend_selector(
        ui,
        commands,
        BackendSelector {
            label: format!("{} Graphics backend", egui_phosphor::regular::DESKTOP),
            id_salt: "settings_graphics_backend",
            configured_backend: graphics_backend,
            backend_options: graphics_backend_options,
            locked: graphics_backend_locked,
        },
        UiCommandSink::set_graphics_backend,
    );

    if settings_restart_required {
        ui.colored_label(
            egui::Color32::from_rgb(255, 196, 96),
            format!(
                "{} Graphics backend change will apply after restart.",
                egui_phosphor::regular::WARNING
            ),
        );
    }

    ui.separator();
    show_backend_selector(
        ui,
        commands,
        BackendSelector {
            label: format!("{} Audio backend", egui_phosphor::regular::SPEAKER_HIGH),
            id_salt: "settings_audio_backend",
            configured_backend: audio_backend,
            backend_options: audio_backend_options,
            locked: audio_backend_locked,
        },
        UiCommandSink::set_audio_backend,
    );

    ui.separator();
    show_ui_scale_setting(ui, app_settings, commands);
}

struct BackendSelector<'a> {
    label: String,
    id_salt: &'static str,
    configured_backend: &'a str,
    backend_options: &'a [String],
    locked: bool,
}

fn show_backend_selector<S: UiCommandSink>(
    ui: &mut egui::Ui,
    commands: &mut S,
    selector: BackendSelector<'_>,
    set_backend: impl FnOnce(&mut S, String),
) {
    ui.label(selector.label);

    let mut backend_choice = selector.configured_backend.to_string();
    ui.add_enabled_ui(!selector.locked, |ui| {
        egui::ComboBox::from_id_salt(selector.id_salt)
            .selected_text(backend_choice.as_str())
            .show_ui(ui, |ui| {
                for backend in selector.backend_options {
                    if ui
                        .selectable_label(backend_choice == backend.as_str(), backend)
                        .clicked()
                    {
                        backend_choice = backend.clone();
                    }
                }
            });
    });

    if !selector.locked && backend_choice != selector.configured_backend {
        set_backend(commands, backend_choice);
    }
}

fn show_ui_scale_setting(
    ui: &mut egui::Ui,
    app_settings: &AppSettings,
    commands: &mut impl UiCommandSink,
) {
    ui.label(format!("{} UI Scale", egui_phosphor::regular::GEAR));
    let mut ui_scale_multiplier = app_settings.normalized_ui_scale_multiplier();
    if ui
        .add(
            egui::DragValue::new(&mut ui_scale_multiplier)
                .speed(0.05)
                .range(0.5..=3.0)
                .suffix("x"),
        )
        .changed()
    {
        commands.set_ui_scale_multiplier(ui_scale_multiplier);
    }
}

fn show_keybind_settings(
    ui: &mut egui::Ui,
    app_settings: &AppSettings,
    keybind_capture_action: Option<&(String, usize)>,
    commands: &mut impl UiCommandSink,
) {
    let mut grouped_actions: BTreeMap<&'static str, Vec<&'static eg_types::KeybindActionMetadata>> =
        BTreeMap::new();
    for metadata in essential_keybind_actions() {
        grouped_actions
            .entry(metadata.group)
            .or_default()
            .push(metadata);
    }

    let default_keybinds = eg_types::default_essential_keybinds();

    egui::ScrollArea::both()
        .id_salt("keybinds_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(KEYBIND_CONTENT_MIN_WIDTH);

            for (group, actions) in grouped_actions {
                ui.collapsing(group, |ui| {
                    for metadata in actions {
                        let action = metadata.action;

                        let current_chords = app_settings.keybinds_for_action(action);
                        let default_chords: Vec<_> = default_keybinds
                            .iter()
                            .filter(|b| b.action == action)
                            .map(|b| &b.chord)
                            .collect();

                        show_keybind_action_row(
                            ui,
                            commands,
                            metadata,
                            &current_chords,
                            &default_chords,
                            keybind_capture_action,
                        );
                        ui.separator();
                    }
                });
            }

            ui.separator();
            if ui.button("Reset to Defaults").clicked() {
                commands.reset_keybinds();
            }
        });
}

/// Shows the settings sidebar overlay. Works in menu, editor, and paused phases.
/// Hidden during active gameplay.
pub fn show_settings_sidebar(
    ctx: &egui::Context,
    inputs: SettingsSidebarInputs<'_>,
    commands: &mut impl UiCommandSink,
) {
    if !inputs.show_settings {
        return;
    }
    // Hide settings during active gameplay (not menu, not editor, not paused)
    if !inputs.is_menu && !inputs.is_editor && !inputs.is_game_paused {
        return;
    }

    show_settings_sidebar_area(ctx, inputs, commands);
}

/// Direct values used while rendering a single editor frame. This is a
/// borrowed function input, not an App-world resource or cached UI snapshot.
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub struct EditorUiInputs<'a> {
    pub is_editor: bool,
    pub available_levels: &'a [String],
    pub level_name: Option<&'a str>,
    pub music_metadata: &'a MusicMetadata,
    pub creator_metadata: &'a BasicLevelMetadata,
    pub sky_color: [f32; 3],
    pub settings: SettingsSidebarInputs<'a>,
    pub editor_fps: f32,
    pub editor_object_count: usize,
    pub editor_marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
    pub editor_config: &'a EditorConfigState,
    pub editor_place_preview_position: [f32; 3],
    pub editor_selected_block: Option<&'a LevelObject>,
    pub editor_selected_block_count: usize,
    pub editor_clipboard_block_count: usize,
    pub editor_can_undo: bool,
    pub editor_can_redo: bool,
    pub editor_mode: EditorMode,
    pub editor_last_mode: Option<EditorMode>,
    pub editor_camera_position: [f32; 3],
    pub editor_camera_preview_position: [f32; 3],
    pub editor_camera_preview_target: [f32; 3],
    pub editor_camera_rotation: f32,
    pub editor_camera_pitch: f32,
    pub show_metadata: bool,
    pub show_place_window: bool,
    pub timeline_playing: bool,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub timeline_tap_times: &'a [f32],
    pub timeline_selected_tap: Option<(usize, f32, [f32; 3])>,
    pub timeline_preview_position: [f32; 3],
    pub timeline_preview_direction: eg_types::Direction,
    pub timing_points: &'a [eg_types::TimingPoint],
    pub playback_speed: f32,
    pub timing_selected_index: Option<usize>,
    pub waveform_zoom: f32,
    pub waveform_samples: &'a [f32],
    pub waveform_sample_rate: u32,
    pub waveform_window_size: usize,
    pub waveform_loading: bool,
    pub waveform_complete: bool,
    pub bpm_tap_result: Option<f32>,
    pub trigger_objects: &'a [LevelObject],
    pub trigger_selected_index: Option<usize>,
    pub block_icon_texture_ids: &'a HashMap<String, egui::TextureId>,
}

/// Render the editor UI from an App-owned view and write semantic commands to
/// the supplied sink. Returns the rectangles used by platform input routing.
pub fn show_editor_ui(
    ctx: &egui::Context,
    inputs: EditorUiInputs<'_>,
    commands: &mut impl UiCommandSink,
) -> Vec<[f32; 4]> {
    let EditorUiInputs {
        is_editor,
        available_levels,
        level_name,
        music_metadata,
        creator_metadata,
        sky_color,
        settings,
        editor_fps,
        editor_object_count,
        editor_marquee_selection_rect_screen,
        editor_config,
        editor_place_preview_position,
        editor_selected_block,
        editor_selected_block_count,
        editor_clipboard_block_count,
        editor_can_undo,
        editor_can_redo,
        editor_mode,
        editor_last_mode,
        editor_camera_position,
        editor_camera_preview_position,
        editor_camera_preview_target,
        editor_camera_rotation,
        editor_camera_pitch,
        show_metadata,
        show_place_window,
        timeline_playing,
        timeline_time_seconds,
        timeline_duration_seconds,
        timeline_tap_times,
        timeline_selected_tap,
        timeline_preview_position,
        timeline_preview_direction,
        timing_points,
        playback_speed,
        timing_selected_index,
        waveform_zoom,
        waveform_samples,
        waveform_sample_rate,
        waveform_window_size,
        waveform_loading,
        waveform_complete,
        bpm_tap_result,
        trigger_objects,
        trigger_selected_index,
        block_icon_texture_ids,
    } = inputs;
    let app_settings = settings.app_settings;
    let show_settings = settings.show_settings;
    if !is_editor {
        return Vec::new();
    }

    let mut ui_input_blocking_rects = Vec::<[f32; 4]>::new();

    let mode = editor_mode;
    let last_mode = editor_last_mode;
    let is_timing = mode == EditorMode::Timing
        || (mode == EditorMode::Null && last_mode == Some(EditorMode::Timing));
    let is_tapping = mode == EditorMode::Tapping
        || (mode == EditorMode::Null && last_mode == Some(EditorMode::Tapping));
    let is_compose = mode.is_compose_mode() && !is_timing;
    let content_rect = ctx.content_rect();
    let viewport_width = content_rect.width();
    let is_compact_ui = is_compact_editor_ui(viewport_width);

    if show_settings {
        let settings_response = show_settings_sidebar_area(ctx, settings, commands);
        ui_input_blocking_rects.push(rect_to_input_bounds(settings_response.response.rect));
    }

    let top_bar_response = egui::Area::new("editor_top_bar".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_min_width(ctx.content_rect().width());
            egui::Frame::side_top_panel(&ctx.global_style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Top-level tabs: Compose / Tapping / Timing
                    if ui
                        .selectable_label(is_compose, "Compose")
                        .on_hover_text(format!(
                            "Compose Tab{}",
                            app_settings.hotkey_hint("tab_compose")
                        ))
                        .clicked()
                        && !is_compose
                    {
                        commands.set_editor_mode(EditorMode::Place);
                    }
                    if ui
                        .selectable_label(is_timing, "Timing")
                        .on_hover_text(format!(
                            "Timing Tab{}",
                            app_settings.hotkey_hint("tab_timing")
                        ))
                        .clicked()
                        && !is_timing
                    {
                        commands.set_editor_mode(EditorMode::Timing);
                    }
                    if ui
                        .selectable_label(is_tapping, "Tapping")
                        .on_hover_text(format!(
                            "Tapping Tab{}",
                            app_settings.hotkey_hint("tab_tapping")
                        ))
                        .clicked()
                        && !is_tapping
                    {
                        commands.set_editor_mode(EditorMode::Tapping);
                    }

                    ui.separator();

                    ui.label(format!("{} Level:", egui_phosphor::regular::MAP_TRIFOLD));

                    let selected = level_name.unwrap_or("Untitled");

                    egui::ComboBox::from_id_salt("level_select")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for level in available_levels {
                                if ui.selectable_label(selected == level, level).clicked() {
                                    commands.load_level(level.clone());
                                }
                            }
                        });

                    ui.separator();

                    if ui
                        .button(format!("{} Export", egui_phosphor::regular::DOWNLOAD))
                        .clicked()
                    {
                        commands.export_level();
                    }

                    if ui
                        .button(format!("{} Import", egui_phosphor::regular::UPLOAD))
                        .clicked()
                    {
                        commands.complete_import();
                    }

                    if ui
                        .button(format!("{} Metadata", egui_phosphor::regular::INFO))
                        .clicked()
                    {
                        commands.set_show_metadata(!show_metadata);
                    }

                    if ui
                        .button(format!("{} Settings", egui_phosphor::regular::GEAR))
                        .on_hover_text(format!(
                            "Settings{}",
                            app_settings.hotkey_hint("toggle_settings")
                        ))
                        .clicked()
                    {
                        commands.toggle_settings();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{editor_fps:.0}fps"));
                    });
                });
            });
        });
    ui_input_blocking_rects.push(rect_to_input_bounds(top_bar_response.response.rect));

    if show_metadata {
        let mut metadata_open = true;

        let metadata_response =
            egui::Window::new(format!("{} Level Metadata", egui_phosphor::regular::INFO))
                .open(&mut metadata_open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} Level Name:", egui_phosphor::regular::PENCIL));
                        let mut name = level_name.unwrap_or("Untitled").to_string();
                        if ui.text_edit_singleline(&mut name).changed() {
                            commands.rename_level(name);
                        }
                    });

                    let mut creator_metadata = creator_metadata.clone();
                    let mut creator_metadata_changed = false;

                    ui.horizontal(|ui| {
                        ui.label(format!("{} Creator:", egui_phosphor::regular::USER));
                        let mut creator = creator_metadata.creator.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut creator).changed() {
                            creator_metadata.creator = optional_trimmed_string(creator);
                            creator_metadata_changed = true;
                        }
                    });

                    ui.separator();

                    let mut music = music_metadata.clone();
                    let mut changed = false;

                    ui.horizontal(|ui| {
                        if ui
                            .button(format!(
                                "{} Import Audio",
                                egui_phosphor::regular::FILE_AUDIO
                            ))
                            .clicked()
                        {
                            commands.trigger_audio_import();
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(format!("{} Title:", egui_phosphor::regular::TEXT_T));
                        let mut title = music.title.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut title).changed() {
                            music.title = Some(title);
                            changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(format!("{} Author:", egui_phosphor::regular::USER));
                        let mut author = music.author.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut author).changed() {
                            music.author = Some(author);
                            changed = true;
                        }
                    });

                    if changed {
                        commands.update_music(music);
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(format!("{} Stars:", egui_phosphor::regular::STAR));
                        let mut stars = creator_metadata.stars.clamp(0.0, 10.0);
                        if ui
                            .add(
                                egui::Slider::new(&mut stars, 0.0..=10.0)
                                    .step_by(0.1)
                                    .suffix(" stars"),
                            )
                            .changed()
                        {
                            creator_metadata.stars = stars.clamp(0.0, 10.0);
                            creator_metadata_changed = true;
                        }
                        ui.label(difficulty_for_stars(stars));
                    });

                    ui.horizontal(|ui| {
                        ui.label(format!("{} Tags:", egui_phosphor::regular::HASH));
                        let mut tags = tags_to_text(&creator_metadata.tags);
                        if ui.text_edit_singleline(&mut tags).changed() {
                            creator_metadata.tags = tags_from_text(tags);
                            creator_metadata_changed = true;
                        }
                    });

                    ui.label(format!(
                        "{} Description:",
                        egui_phosphor::regular::TEXT_ALIGN_LEFT
                    ));
                    let mut description = creator_metadata.description.clone().unwrap_or_default();
                    if ui
                        .add(egui::TextEdit::multiline(&mut description).desired_rows(3))
                        .changed()
                    {
                        creator_metadata.description = optional_trimmed_string(description);
                        creator_metadata_changed = true;
                    }

                    if creator_metadata_changed {
                        commands.update_creator_metadata(creator_metadata);
                    }

                    ui.separator();
                    ui.heading(format!("{} Visuals", egui_phosphor::regular::IMAGE));
                    ui.horizontal(|ui| {
                        ui.label(format!("{} Sky:", egui_phosphor::regular::CIRCLE));
                        let mut sky_color = sky_color;
                        if ui.color_edit_button_rgb(&mut sky_color).changed() {
                            commands.update_sky_color(sky_color);
                        }
                    });

                    ui.separator();
                    ui.heading(format!(
                        "{} Menu Preview Camera",
                        egui_phosphor::regular::CAMERA
                    ));
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(format!(
                                "{} Capture Current Camera",
                                egui_phosphor::regular::CAMERA
                            ))
                            .clicked()
                        {
                            commands.capture_menu_preview_camera();
                        }
                        if ui.button("Use Auto from Spawn").clicked() {
                            commands.use_auto_menu_preview_camera();
                        }
                    });

                    ui.separator();

                    ui.label(format!(
                        "{} Objects: {}",
                        egui_phosphor::regular::CUBE,
                        editor_object_count
                    ));
                });
        if let Some(metadata_response) = metadata_response {
            ui_input_blocking_rects.push(rect_to_input_bounds(metadata_response.response.rect));
        }

        if !metadata_open {
            commands.set_show_metadata(false);
        }
    }

    // Floating place window with block catalog
    if show_place_window {
        let mut place_open = true;
        let place_category_id = egui::Id::new("editor_place_block_catalog_category");
        let mut selected_place_category = ctx.data_mut(|data| {
            data.get_persisted::<Option<BlockCategory>>(place_category_id)
                .unwrap_or_default()
        });

        let place_response =
            egui::Window::new(format!("{} Place Block", egui_phosphor::regular::CUBE))
                .open(&mut place_open)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for category in BLOCK_CATALOG_CATEGORIES {
                            ui.selectable_value(
                                &mut selected_place_category,
                                category,
                                block_catalog_category_label(category),
                            );
                        }
                    });

                    egui::Grid::new("editor_place_block_catalog_grid")
                        .num_columns(PLACE_BLOCK_CATALOG_COLUMNS)
                        .show(ui, |ui| {
                            let blocks: Vec<_> = eg_blocks::BlockDefinition::all_placeable()
                                .iter()
                                .filter(|block| {
                                    block.placeable
                                        && block_matches_catalog_category(
                                            block,
                                            selected_place_category,
                                        )
                                })
                                .take(PLACE_BLOCK_CATALOG_COLUMNS * PLACE_BLOCK_CATALOG_ROWS)
                                .collect();

                            for cell in 0..(PLACE_BLOCK_CATALOG_COLUMNS * PLACE_BLOCK_CATALOG_ROWS)
                            {
                                if let Some(block) = blocks.get(cell) {
                                    if show_block_preview_button(
                                        ui,
                                        block,
                                        block_icon_texture_ids.get(&block.id.to_string()).copied(),
                                    ) {
                                        commands.set_block_id(BlockId::new(&block.id));
                                        commands.set_editor_mode(EditorMode::Place);
                                        commands.toggle_place_window();
                                    }
                                } else {
                                    ui.allocate_exact_size(
                                        egui::Vec2::splat(PLACE_BLOCK_CATALOG_CELL_SIZE),
                                        egui::Sense::hover(),
                                    );
                                }

                                if (cell + 1) % PLACE_BLOCK_CATALOG_COLUMNS == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });

        if let Some(place_response) = place_response {
            ui_input_blocking_rects.push(rect_to_input_bounds(place_response.response.rect));
        }

        ctx.data_mut(|data| data.insert_persisted(place_category_id, selected_place_category));

        if !place_open {
            commands.toggle_place_window();
        }
    }

    let bottom_bar_response = egui::Area::new("block_selection_bar".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(ctx.content_rect().width());
            egui::Frame::side_top_panel(&ctx.global_style()).show(ui, |ui| {
                ui.vertical(|ui| {
                    let duration_seconds = timeline_metrics(timeline_duration_seconds);

                    if is_timing {
                        show_timing_mode_bottom_panel(
                            ui,
                            &TimingModePanelInputs {
                                timeline_time_seconds,
                                timeline_duration_seconds,
                                timing_points,
                                playback_speed,
                                timing_selected_index,
                                bpm_tap_result,
                            },
                            commands,
                        );
                    } else if is_tapping {
                        show_tapping_mode_bottom_panel(
                            ui,
                            timeline_preview_position,
                            timeline_preview_direction,
                            editor_camera_preview_position,
                            editor_camera_preview_target,
                            commands,
                        );
                    }

                    show_timeline_bar(
                        ui,
                        &TimelineBarInputs {
                            app_settings,
                            mode: editor_mode,
                            last_mode: editor_last_mode,
                            playing: timeline_playing,
                            timeline_time_seconds,
                            tap_times: timeline_tap_times,
                            timing_points,
                            waveform_zoom,
                            trigger_objects,
                            trigger_selected_index,
                            duration_seconds,
                        },
                        commands,
                    );
                });
            });
        });
    ui_input_blocking_rects.push(rect_to_input_bounds(bottom_bar_response.response.rect));

    // Massive plus button in compose tab, bottom-right above the bottom bar
    if is_compose {
        let bottom_bar_height = bottom_bar_response.response.rect.height();
        egui::Area::new("place_button_area".into())
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::Vec2::new(-12.0, -12.0 - bottom_bar_height),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Max), |ui| {
                    let hotkey_hint = app_settings.hotkey_hint("toggle_place_window");
                    if show_recent_block_quick_strip(
                        ui,
                        editor_config,
                        block_icon_texture_ids,
                        commands,
                    ) {
                        ui.add_space(6.0);
                    }

                    egui::Frame::popup(&ctx.global_style()).show(ui, |ui| {
                        if ui
                            .selectable_label(
                                show_place_window,
                                egui::RichText::new(egui_phosphor::regular::PLUS).size(28.0),
                            )
                            .on_hover_text(format!("Place Block{}", hotkey_hint))
                            .clicked()
                        {
                            commands.toggle_place_window();
                        }
                    });
                });
            });

        show_selected_block_properties_window(
            ctx,
            editor_mode,
            editor_selected_block,
            bottom_bar_height,
            commands,
        );
        show_place_block_properties_window(
            ctx,
            editor_mode,
            editor_config,
            editor_place_preview_position,
            bottom_bar_height,
        );
        let top_bar_height = top_bar_response.response.rect.height();
        show_mode_selector_popup(
            ctx,
            app_settings,
            editor_mode,
            editor_config,
            top_bar_height,
            commands,
        );
    }

    // Selected tap properties floating window
    if is_tapping {
        let bottom_bar_height = bottom_bar_response.response.rect.height();
        show_selected_tap_properties_window(
            ctx,
            timeline_selected_tap,
            timeline_duration_seconds,
            bottom_bar_height,
            commands,
        );
    }

    // Waveform visualization central panel (Timing mode only)
    if is_timing {
        let viewport = ctx.content_rect();
        egui::Area::new("timing_waveform_panel".into())
            .order(egui::Order::Background)
            .fixed_pos(viewport.min)
            .show(ctx, |ui| {
                ui.set_min_size(viewport.size());
                egui::Frame::central_panel(&ctx.global_style())
                    .fill(egui::Color32::from_rgb(15, 20, 28))
                    .show(ui, |ui| {
                        show_waveform_panel(
                            ui,
                            &WaveformPanelInputs {
                                timeline_time_seconds,
                                timeline_duration_seconds,
                                timing_points,
                                waveform_zoom,
                                waveform_samples,
                                waveform_sample_rate,
                                waveform_window_size,
                                waveform_loading,
                                waveform_complete,
                            },
                            commands,
                        );
                    });
            });
    }

    if !is_timing
        && !is_compact_ui
        && let Some(rect) = show_view_selector_cube(
            ctx,
            editor_camera_rotation,
            editor_camera_pitch,
            editor_camera_position,
            commands,
        )
    {
        ui_input_blocking_rects.push(rect_to_input_bounds(rect));
    }

    if editor_mode.is_transform_mode()
        && let Some((start, current, is_active_drag)) = editor_marquee_selection_rect_screen
        && is_active_drag
    {
        let pixels_per_point = ctx.pixels_per_point();
        let rect = egui::Rect::from_two_pos(
            marquee_screen_pos_to_egui_pos(start, pixels_per_point),
            marquee_screen_pos_to_egui_pos(current, pixels_per_point),
        );
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(25, 153, 255));
        let layer = egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("editor_marquee_overlay"),
        );
        ctx.layer_painter(layer).rect(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(25, 153, 255, 38),
            stroke,
            egui::StrokeKind::Outside,
        );
    }

    show_editor_context_menu(
        ctx,
        EditorContextMenuInputs {
            app_settings,
            selected_tap: timeline_selected_tap,
            editor_mode,
            editor_last_mode,
            selected_block_count: editor_selected_block_count,
            clipboard_block_count: editor_clipboard_block_count,
            can_undo: editor_can_undo,
            can_redo: editor_can_redo,
        },
        &mut ui_input_blocking_rects,
        commands,
    );

    ui_input_blocking_rects
}

/// Shows the app-wide profiler window when the performance overlay is enabled.
pub fn show_perf_overlay(
    ctx: &egui::Context,
    perf_overlay_enabled: bool,
    perf_graphics_backend: &str,
    perf_audio_backend: &str,
    fps: f32,
    perf_has_latest_frame: bool,
    commands: &mut impl UiCommandSink,
) {
    if !perf_overlay_enabled {
        return;
    }

    let mut profiler_open = true;

    egui::Window::new("Profiler")
        .order(perf_overlay_window_order())
        .default_size([1024.0, 600.0])
        .open(&mut profiler_open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!(
                    "{} | {} | FPS {:.1}",
                    perf_graphics_backend, perf_audio_backend, fps
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(perf_has_latest_frame, egui::Button::new("📋 Copy Frame"))
                        .on_hover_text("Copy the latest profiling frame to clipboard")
                        .clicked()
                    {
                        commands.copy_latest_perf_frame();
                    }
                });
            });
            ui.separator();
            puffin_egui::profiler_ui(ui);
        });

    if !profiler_open {
        commands.toggle_perf_overlay();
    }
}

fn perf_overlay_window_order() -> egui::Order {
    egui::Order::Foreground
}

fn show_keybind_action_row(
    ui: &mut egui::Ui,
    commands: &mut impl UiCommandSink,
    metadata: &eg_types::KeybindActionMetadata,
    current_chords: &[&eg_types::KeyChord],
    default_chords: &[&eg_types::KeyChord],
    keybind_capture_action: Option<&(String, usize)>,
) {
    let action = metadata.action;
    let is_not_default = current_chords != default_chords;

    ui.horizontal_top(|ui| {
        ui.scope(|ui| {
            ui.set_min_width(KEYBIND_ACTION_LABEL_WIDTH);
            ui.add(egui::Label::new(metadata.label).wrap_mode(egui::TextWrapMode::Extend));
        });
        ui.allocate_ui_with_layout(
            egui::vec2(KEYBIND_CONTROLS_MIN_WIDTH, 0.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                show_keybind_controls(
                    ui,
                    commands,
                    action,
                    metadata.capacity,
                    current_chords,
                    is_not_default,
                    keybind_capture_action,
                );
            },
        );
    });
}

fn show_keybind_controls(
    ui: &mut egui::Ui,
    commands: &mut impl UiCommandSink,
    action: &str,
    capacity: usize,
    current_chords: &[&eg_types::KeyChord],
    is_not_default: bool,
    keybind_capture_action: Option<&(String, usize)>,
) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);

    if is_not_default {
        ui.horizontal(|ui| {
            if ui
                .button(egui_phosphor::regular::ARROW_CLOCKWISE)
                .on_hover_text("Reset to default")
                .clicked()
            {
                commands.reset_keybind(action.to_string());
                commands.set_keybind_capture(None);
            }
        });
    }

    for slot in 0..capacity {
        ui.horizontal(|ui| {
            let chord = current_chords.get(slot).copied();
            let key_label = chord
                .map(format_key_chord)
                .unwrap_or_else(|| "Unbound".to_string());
            let is_capturing = keybind_capture_action == Some(&(action.to_string(), slot));

            let (bg_color, display_label) = if is_capturing {
                (egui::Color32::from_rgb(0, 120, 215), "...".to_string())
            } else {
                (egui::Color32::from_gray(32), key_label)
            };

            if ui
                .add(
                    egui::Button::new(egui::RichText::new(display_label).monospace())
                        .fill(bg_color),
                )
                .clicked()
            {
                if is_capturing {
                    commands.set_keybind_capture(None);
                } else {
                    commands.set_keybind_capture(Some((action.to_string(), slot)));
                }
            }

            if chord.is_some()
                && ui
                    .button(egui_phosphor::regular::X)
                    .on_hover_text("Clear binding")
                    .clicked()
            {
                commands.clear_keybind_slot(action.to_string(), slot);
                commands.set_keybind_capture(None);
            }
        });
    }
}

fn show_view_selector_cube(
    ctx: &egui::Context,
    camera_rotation: f32,
    camera_pitch: f32,
    camera_position: [f32; 3],
    commands: &mut impl UiCommandSink,
) -> Option<egui::Rect> {
    const ROTATE_SPEED: f32 = 0.004;
    const PITCH_SPEED: f32 = 0.006;

    struct RenderedFace {
        label: &'static str,
        polygon: [egui::Pos2; 4],
        center: egui::Pos2,
        avg_depth: f32,
        facing: f32,
        rotation: f32,
        pitch: f32,
    }

    fn point_in_quad(point: egui::Pos2, quad: &[egui::Pos2; 4]) -> bool {
        let mut last_non_zero_sign = 0.0;
        for i in 0..4 {
            let a = quad[i];
            let b = quad[(i + 1) % 4];
            let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
            if cross.abs() <= f32::EPSILON {
                continue;
            }
            if last_non_zero_sign == 0.0 {
                last_non_zero_sign = cross.signum();
                continue;
            }
            if last_non_zero_sign * cross < 0.0 {
                return false;
            }
        }
        true
    }

    fn convex_hull(points: &[egui::Pos2]) -> Vec<egui::Pos2> {
        if points.len() <= 3 {
            return points.to_vec();
        }

        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));

        fn cross(o: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
            (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
        }

        let mut lower: Vec<egui::Pos2> = Vec::new();
        for p in &sorted {
            while lower.len() >= 2
                && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0
            {
                lower.pop();
            }
            lower.push(*p);
        }

        let mut upper: Vec<egui::Pos2> = Vec::new();
        for p in sorted.iter().rev() {
            while upper.len() >= 2
                && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0
            {
                upper.pop();
            }
            upper.push(*p);
        }

        lower.pop();
        upper.pop();
        lower.extend(upper);
        lower
    }

    fn polygon_winding(points: &[egui::Pos2]) -> f32 {
        let mut area = 0.0;
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            area += (b.x - a.x) * (b.y + a.y);
        }
        area
    }

    fn rounded_convex_polygon(
        points: &[egui::Pos2],
        radius: f32,
        arc_segments: usize,
    ) -> Vec<egui::Pos2> {
        if points.len() < 3 || radius <= 0.0 || arc_segments == 0 {
            return points.to_vec();
        }

        let winding = polygon_winding(points);
        let ccw = winding < 0.0;
        let mut rounded = Vec::new();

        for i in 0..points.len() {
            let prev = points[(i + points.len() - 1) % points.len()];
            let curr = points[i];
            let next = points[(i + 1) % points.len()];

            let v1 = (prev - curr).normalized();
            let v2 = (next - curr).normalized();

            let dot = (v1.x * v2.x + v1.y * v2.y).clamp(-1.0, 1.0);
            let angle = dot.acos();
            if angle <= f32::EPSILON {
                rounded.push(curr);
                continue;
            }

            let offset = radius / (angle * 0.5).tan();
            let offset = offset.min((prev - curr).length() * 0.5);
            let offset = offset.min((next - curr).length() * 0.5);

            let tangent1 = curr + v1 * offset;
            let tangent2 = curr + v2 * offset;
            let bisector = (v1 + v2).normalized();
            if bisector.length_sq() <= f32::EPSILON {
                rounded.push(curr);
                continue;
            }
            let center_distance = radius / (angle * 0.5).sin();
            let center = curr + bisector * center_distance;

            let start_angle = (tangent1.y - center.y).atan2(tangent1.x - center.x);
            let mut end_angle = (tangent2.y - center.y).atan2(tangent2.x - center.x);

            if ccw {
                if end_angle < start_angle {
                    end_angle += std::f32::consts::TAU;
                }
            } else if end_angle > start_angle {
                end_angle -= std::f32::consts::TAU;
            }

            rounded.push(tangent1);
            let step = (end_angle - start_angle) / arc_segments as f32;
            for s in 1..arc_segments {
                let angle = start_angle + step * s as f32;
                rounded.push(egui::pos2(
                    center.x + angle.cos() * radius,
                    center.y + angle.sin() * radius,
                ));
            }
            rounded.push(tangent2);
        }

        rounded
    }

    let response = egui::Area::new("editor_view_selector_cube".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 52.0))
        .show(ctx, |ui| {
            let side = 128.0;
            let label_height = 24.0;
            let (widget_rect, response) = ui.allocate_exact_size(
                egui::vec2(side, side + label_height),
                egui::Sense::click_and_drag(),
            );
            let rect = egui::Rect::from_min_size(widget_rect.min, egui::vec2(side, side));
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(widget_rect.min.x, rect.max.y),
                egui::vec2(side, label_height),
            );
            let painter = ui.painter_at(widget_rect);

            painter.rect_filled(
                widget_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(16, 24, 32, 56),
            );

            let pitch = camera_pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
            let horizontal = pitch.cos();
            let offset =
                Mat3::from_rotation_y(camera_rotation) * Vec3::new(0.0, pitch.sin(), -horizontal);
            let forward = (-offset).normalize_or_zero();

            if forward.length_squared() <= f32::EPSILON {
                return;
            }

            let to_camera = -forward;
            let mut right = forward.cross(Vec3::Y);
            if right.length_squared() <= f32::EPSILON {
                right = Vec3::X;
            } else {
                right = right.normalize();
            }
            let up = right.cross(forward).normalize();

            let cube_vertices = [
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(-1.0, -1.0, 1.0),
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(-1.0, 1.0, 1.0),
            ];

            let center = rect.center();
            let cube_radius = side * 0.23;
            let perspective = 3.6;

            let mut projected = [egui::Pos2::ZERO; 8];
            let mut depth = [0.0; 8];
            for (idx, vertex) in cube_vertices.iter().enumerate() {
                let local_x = vertex.dot(right);
                let local_y = vertex.dot(up);
                let local_z = vertex.dot(to_camera);
                let scale = perspective / (perspective - local_z);
                projected[idx] = egui::pos2(
                    center.x + local_x * cube_radius * scale,
                    center.y - local_y * cube_radius * scale,
                );
                depth[idx] = local_z;
            }

            let hull = convex_hull(&projected);
            if hull.len() >= 3 {
                let rounded_hull = rounded_convex_polygon(&hull, 6.0, 4);
                painter.add(egui::Shape::convex_polygon(
                    rounded_hull,
                    egui::Color32::from_rgba_unmultiplied(182, 214, 236, 72),
                    egui::Stroke::NONE,
                ));
            }

            let mut rendered_faces = Vec::<RenderedFace>::new();
            for face in VIEW_CUBE_FACES {
                let facing = face.normal.dot(to_camera);
                if facing <= 0.02 {
                    continue;
                }

                let face_inset: f32 = 1.2;
                let raw_poly = [
                    projected[face.indices[0]],
                    projected[face.indices[1]],
                    projected[face.indices[2]],
                    projected[face.indices[3]],
                ];
                let avg_depth = (depth[face.indices[0]]
                    + depth[face.indices[1]]
                    + depth[face.indices[2]]
                    + depth[face.indices[3]])
                    * 0.25;
                let center = egui::pos2(
                    (raw_poly[0].x + raw_poly[1].x + raw_poly[2].x + raw_poly[3].x) * 0.25,
                    (raw_poly[0].y + raw_poly[1].y + raw_poly[2].y + raw_poly[3].y) * 0.25,
                );

                // sort to avoid aa seam artifacts
                let mut poly = [egui::Pos2::ZERO; 4];
                for i in 0..4 {
                    let point = raw_poly[i];
                    let to_center = center - point;
                    let distance = to_center.length();
                    poly[i] = if distance > f32::EPSILON {
                        point + to_center / distance * face_inset.min(distance * 0.5)
                    } else {
                        point
                    };
                }
                let poly = sort_quad_by_angle(center, poly);
                rendered_faces.push(RenderedFace {
                    label: face.label,
                    polygon: poly,
                    center,
                    avg_depth,
                    facing,
                    rotation: face.rotation,
                    pitch: face.pitch,
                });
            }

            rendered_faces.sort_by(|a, b| a.avg_depth.total_cmp(&b.avg_depth));

            let pointer_pos = response.interact_pointer_pos();
            let mut hovered_face: Option<usize> = None;
            if let Some(pointer) = pointer_pos {
                for idx in (0..rendered_faces.len()).rev() {
                    if point_in_quad(pointer, &rendered_faces[idx].polygon) {
                        hovered_face = Some(idx);
                        break;
                    }
                }
            }

            for (idx, face) in rendered_faces.iter().enumerate() {
                let hover_boost = if hovered_face == Some(idx) { 28 } else { 0 };
                let alpha = (40.0 + face.facing * 90.0).round() as u8;
                let face_color = egui::Color32::from_rgba_unmultiplied(
                    182,
                    214,
                    236,
                    alpha.saturating_add(hover_boost),
                );
                add_face_mesh(&painter, face.polygon, face_color);
                if face.facing > 0.2 {
                    painter.text(
                        face.center,
                        egui::Align2::CENTER_CENTER,
                        face.label,
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgba_unmultiplied(235, 246, 255, 220),
                    );
                }
            }

            let axis_origin = projected[0];
            let axes = [
                ("X", projected[1], egui::Color32::from_rgb(240, 104, 104)),
                ("Y", projected[3], egui::Color32::from_rgb(116, 232, 152)),
                ("Z", projected[4], egui::Color32::from_rgb(104, 154, 255)),
            ];
            for (label, tip, color) in axes {
                painter.line_segment([axis_origin, tip], egui::Stroke::new(1.6, color));

                let direction = tip - axis_origin;
                let direction_len = direction.length();
                let label_pos = if direction_len > f32::EPSILON {
                    tip + direction * (6.0 / direction_len)
                } else {
                    tip
                };
                painter.text(
                    label_pos,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    color,
                );
            }

            let dragging_cube = response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Secondary);
            if dragging_cube {
                let pointer_delta = ui.input(|input| input.pointer.delta());
                if pointer_delta != egui::Vec2::ZERO {
                    commands.set_camera_orientation(
                        camera_rotation - pointer_delta.x * ROTATE_SPEED,
                        camera_pitch + pointer_delta.y * PITCH_SPEED,
                        None,
                    );
                }
            }

            if response.clicked()
                && !dragging_cube
                && let Some(idx) = hovered_face
            {
                let face = &rendered_faces[idx];
                commands.set_camera_orientation(face.rotation, face.pitch, Some(0.25));
            }

            painter.text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "{:.1}, {:.1}, {:.1}",
                    camera_position[0], camera_position[1], camera_position[2]
                ),
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgba_unmultiplied(235, 246, 255, 220),
            );
        });
    Some(response.response.rect)
}

#[cfg(test)]
mod tests {
    use eg_blocks::{BlockCategory, BlockDefinition};
    use eg_state::state::EditorConfigState;
    use eg_types::{AppSettings, EditorMode, MusicMetadata, SettingsSection};
    use glam::{Mat3, Vec3};

    use super::{
        EditorUiInputs, SettingsSidebarInputs, UiCommandSink, VIEW_CUBE_FACES, WorldUiCommandSink,
        block_matches_catalog_category, combined_ui_scale_factor, is_compact_editor_ui,
        marquee_screen_pos_to_egui_pos, perf_overlay_window_order, responsive_ui_scale_multiplier,
        settings_sidebar_default_width, show_editor_ui, show_perf_overlay, sort_quad_by_angle,
    };

    fn camera_forward_from_orientation(rotation: f32, pitch: f32) -> Vec3 {
        let pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        let horizontal = pitch.cos();
        let offset = Mat3::from_rotation_y(rotation) * Vec3::new(0.0, pitch.sin(), -horizontal);
        (-offset).normalize_or_zero()
    }

    fn is_convex_quad(quad: &[egui::Pos2; 4]) -> bool {
        let mut sign = 0.0f32;
        for i in 0..4 {
            let a = quad[i];
            let b = quad[(i + 1) % 4];
            let c = quad[(i + 2) % 4];
            let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
            if cross.abs() <= 1.0e-6 {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if sign * cross < 0.0 {
                return false;
            }
        }
        sign != 0.0
    }

    fn run_editor_ui_once(
        view: &crate::test_utils::EditorUiTestInputs,
        editor_mode: EditorMode,
        is_editor: bool,
        show_settings: bool,
        show_metadata: bool,
        settings_section: SettingsSection,
        marquee_selection_rect_screen: Option<([f64; 2], [f64; 2], bool)>,
    ) {
        let ctx = egui::Context::default();
        let ui_ctx = ctx.clone();
        let block_icon_texture_ids = std::collections::HashMap::new();
        let editor_config = EditorConfigState::default();
        let (_world, _) = crate::test_utils::with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |_root_ui| {
                let app_settings = AppSettings::default();
                let music_metadata = MusicMetadata::default();
                let creator_metadata = eg_types::BasicLevelMetadata::default();
                let _ = show_editor_ui(
                    &ui_ctx,
                    EditorUiInputs {
                        is_editor,
                        available_levels: &[],
                        level_name: Some("Test Level"),
                        music_metadata: &music_metadata,
                        creator_metadata: &creator_metadata,
                        sky_color: eg_types::LevelMetadata::default().sky_color,
                        settings: SettingsSidebarInputs {
                            show_settings,
                            is_menu: false,
                            is_editor,
                            is_game_paused: false,
                            app_settings: &app_settings,
                            graphics_backend: "Auto",
                            graphics_backend_options: &[],
                            graphics_backend_locked: false,
                            settings_restart_required: false,
                            audio_backend: "Default",
                            audio_backend_options: &[],
                            audio_backend_locked: false,
                            settings_section,
                            keybind_capture_action: None,
                        },
                        editor_fps: 60.0,
                        editor_object_count: 0,
                        editor_marquee_selection_rect_screen: marquee_selection_rect_screen,
                        editor_config: &editor_config,
                        editor_place_preview_position: [0.0, 0.0, 0.0],
                        editor_selected_block: None,
                        editor_selected_block_count: 0,
                        editor_clipboard_block_count: 0,
                        editor_can_undo: false,
                        editor_can_redo: false,
                        editor_mode,
                        editor_last_mode: Some(editor_mode),
                        editor_camera_position: [0.0, 0.0, 0.0],
                        editor_camera_preview_position: [0.0, 0.0, 0.0],
                        editor_camera_preview_target: [0.0, 0.0, 0.0],
                        editor_camera_rotation: 0.0,
                        editor_camera_pitch: 0.0,
                        show_metadata,
                        show_place_window: false,
                        timeline_playing: view.playing,
                        timeline_time_seconds: view.timeline_time_seconds,
                        timeline_duration_seconds: view.timeline_duration_seconds,
                        timeline_tap_times: &view.tap_times,
                        timeline_selected_tap: view.selected_tap,
                        timeline_preview_position: view.timeline_preview_position,
                        timeline_preview_direction: view.timeline_preview_direction,
                        timing_points: &view.timing_points,
                        playback_speed: view.playback_speed,
                        timing_selected_index: view.timing_selected_index,
                        waveform_zoom: view.waveform_zoom,
                        waveform_samples: &view.waveform_samples,
                        waveform_sample_rate: view.waveform_sample_rate,
                        waveform_window_size: view.waveform_window_size,
                        waveform_loading: view.waveform_loading,
                        waveform_complete: view.waveform_complete,
                        bpm_tap_result: view.bpm_tap_result,
                        trigger_objects: &view.trigger_objects,
                        trigger_selected_index: view.trigger_selected_index,
                        block_icon_texture_ids: &block_icon_texture_ids,
                    },
                    commands,
                );
            })
        });
    }

    #[test]
    fn world_command_sink_writes_messages_immediately() {
        let mut app = bevy::app::App::new();
        app.add_plugins(crate::plugin::UiPlugin);

        let mut sink = WorldUiCommandSink::new(app.world_mut());
        sink.turn_right();
        drop(sink);

        let messages = app
            .world()
            .resource::<bevy::ecs::message::Messages<eg_state::events::navigation::TurnRight>>();
        let mut reader = messages.get_cursor();
        assert_eq!(reader.read(messages).count(), 1);
    }

    #[test]
    fn sort_quad_by_angle_returns_convex_winding_for_scrambled_square() {
        let center = egui::pos2(0.0, 0.0);
        let quad = [
            egui::pos2(1.0, 1.0),
            egui::pos2(-1.0, 1.0),
            egui::pos2(1.0, -1.0),
            egui::pos2(-1.0, -1.0),
        ];
        let sorted = sort_quad_by_angle(center, quad);
        assert!(is_convex_quad(&sorted));
    }

    #[test]
    fn sort_quad_by_angle_handles_skewed_quads() {
        let center = egui::pos2(0.05, -0.12);
        let quad = [
            egui::pos2(1.3, 0.8),
            egui::pos2(-0.9, 1.1),
            egui::pos2(0.9, -1.4),
            egui::pos2(-1.1, -0.6),
        ];
        let sorted = sort_quad_by_angle(center, quad);
        assert!(is_convex_quad(&sorted));
    }

    #[test]
    fn view_cube_face_rotations_match_face_normals() {
        for face in VIEW_CUBE_FACES {
            let forward = camera_forward_from_orientation(face.rotation, face.pitch);
            let dot = forward.dot(face.normal.normalize_or_zero());
            assert!(
                dot < -0.999 || approx::abs_diff_eq!(dot, -1.0, epsilon = 0.002),
                "Face {} should align camera forward opposite the normal (dot={})",
                face.label,
                dot
            );
        }
    }

    #[test]
    fn compact_editor_ui_detects_small_viewports() {
        assert!(is_compact_editor_ui(720.0));
        assert!(is_compact_editor_ui(480.0));
        assert!(!is_compact_editor_ui(720.1));
        assert!(!is_compact_editor_ui(960.0));
    }

    #[test]
    fn settings_sidebar_width_uses_compact_fit_and_desktop_default() {
        approx::assert_abs_diff_eq!(settings_sidebar_default_width(320.0), 249.6, epsilon = 0.01);
        approx::assert_abs_diff_eq!(settings_sidebar_default_width(200.0), 176.0, epsilon = 0.01);
        approx::assert_abs_diff_eq!(
            settings_sidebar_default_width(1200.0),
            320.0,
            epsilon = 0.01
        );
        approx::assert_abs_diff_eq!(settings_sidebar_default_width(180.0), 156.0, epsilon = 0.01);
    }

    #[test]
    fn responsive_ui_scale_multiplier_tracks_screen_size() {
        approx::assert_abs_diff_eq!(
            responsive_ui_scale_multiplier(egui::vec2(1280.0, 720.0)),
            1.0,
            epsilon = 0.001
        );
        approx::assert_abs_diff_eq!(
            responsive_ui_scale_multiplier(egui::vec2(320.0, 240.0)),
            0.8,
            epsilon = 0.001
        );
        approx::assert_abs_diff_eq!(
            responsive_ui_scale_multiplier(egui::vec2(3840.0, 2160.0)),
            1.35,
            epsilon = 0.001
        );
    }

    #[test]
    fn combined_ui_scale_factor_applies_user_multiplier_with_clamping() {
        approx::assert_abs_diff_eq!(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), 1.25),
            1.25,
            epsilon = 0.001
        );
        approx::assert_abs_diff_eq!(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), 99.0),
            3.0,
            epsilon = 0.001
        );
        approx::assert_abs_diff_eq!(
            combined_ui_scale_factor(egui::vec2(1280.0, 720.0), f32::NAN),
            1.0,
            epsilon = 0.001
        );
        approx::assert_abs_diff_eq!(
            combined_ui_scale_factor(egui::vec2(3840.0, 2160.0), 3.0),
            4.0,
            epsilon = 0.001
        );
    }

    #[test]
    fn marquee_screen_pos_to_egui_pos_scales_by_pixels_per_point() {
        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], 1.5);
        approx::assert_abs_diff_eq!(p.x, 200.0, epsilon = 0.001);
        approx::assert_abs_diff_eq!(p.y, 120.0, epsilon = 0.001);
    }

    #[test]
    fn marquee_screen_pos_to_egui_pos_handles_invalid_or_non_positive_scale() {
        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], f32::NAN);
        approx::assert_abs_diff_eq!(p.x, 300.0, epsilon = 0.001);
        approx::assert_abs_diff_eq!(p.y, 180.0, epsilon = 0.001);

        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], 0.0);
        approx::assert_abs_diff_eq!(p.x, 300.0, epsilon = 0.001);
        approx::assert_abs_diff_eq!(p.y, 180.0, epsilon = 0.001);

        let p = marquee_screen_pos_to_egui_pos([300.0, 180.0], -2.0);
        approx::assert_abs_diff_eq!(p.x, 300.0, epsilon = 0.001);
        approx::assert_abs_diff_eq!(p.y, 180.0, epsilon = 0.001);
    }

    #[test]
    fn block_catalog_category_groups_builtin_blocks() {
        let stone = BlockDefinition::get("core/stone");
        let lava = BlockDefinition::get("core/lava");
        let void = BlockDefinition::get("core/void");
        let speed_portal = BlockDefinition::get("core/speedportal");
        let torch = BlockDefinition::get("core/torch");

        assert!(block_matches_catalog_category(
            stone,
            Some(BlockCategory::Building)
        ));
        assert!(block_matches_catalog_category(
            lava,
            Some(BlockCategory::Danger)
        ));
        assert!(block_matches_catalog_category(
            void,
            Some(BlockCategory::Danger)
        ));
        assert!(block_matches_catalog_category(
            speed_portal,
            Some(BlockCategory::Tech)
        ));
        assert!(block_matches_catalog_category(
            torch,
            Some(BlockCategory::Tech)
        ));

        assert!(block_matches_catalog_category(lava, None));
        assert!(block_matches_catalog_category(
            lava,
            Some(BlockCategory::Danger)
        ));
        assert!(!block_matches_catalog_category(
            lava,
            Some(BlockCategory::Building)
        ));
    }

    #[test]
    fn show_editor_ui_returns_early_when_not_in_editor() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = crate::test_utils::editor_ui_view(&app_settings, &music_metadata);

        run_editor_ui_once(
            &view,
            EditorMode::Select,
            false,
            false,
            false,
            SettingsSection::Backends,
            None,
        );
    }

    #[test]
    fn show_perf_overlay_renders_outside_editor() {
        let ctx = egui::Context::default();
        let (_world, _) = crate::test_utils::with_world_command_sink(|commands| {
            ctx.run_ui(egui::RawInput::default(), |root_ui| {
                let ctx = root_ui.ctx();
                show_perf_overlay(ctx, true, "", "", 60.0, false, commands);
            })
        });
    }

    #[test]
    fn perf_overlay_window_stays_below_tooltips() {
        assert!(perf_overlay_window_order() < egui::Order::Tooltip);
    }

    #[test]
    fn show_editor_ui_renders_marquee_selection() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = crate::test_utils::editor_ui_view(&app_settings, &music_metadata);
        run_editor_ui_once(
            &view,
            EditorMode::Select,
            true,
            false,
            false,
            SettingsSection::Backends,
            Some(([10.0, 10.0], [100.0, 100.0], false)),
        );
    }

    #[test]
    fn show_editor_ui_composes_timing_tapping_compose_and_trigger_modes() {
        let app_settings = AppSettings::default();
        let music_metadata = MusicMetadata::default();
        let view = crate::test_utils::editor_ui_view(&app_settings, &music_metadata);
        run_editor_ui_once(
            &view,
            EditorMode::Timing,
            true,
            true,
            true,
            SettingsSection::Backends,
            None,
        );

        run_editor_ui_once(
            &view,
            EditorMode::Place,
            true,
            false,
            false,
            SettingsSection::Keybinds,
            None,
        );

        run_editor_ui_once(
            &view,
            EditorMode::Tapping,
            true,
            false,
            false,
            SettingsSection::Keybinds,
            None,
        );
    }
}

#[cfg(test)]
mod test_utils {
    use bevy::ecs::world::World;
    use eg_types::{AppSettings, Direction, LevelObject, MusicMetadata, TimingPoint};

    use crate::WorldUiCommandSink;

    /// Run a UI test with the same typed-message sink used by production.
    pub fn with_world_command_sink<R>(
        f: impl FnOnce(&mut WorldUiCommandSink<'_>) -> R,
    ) -> (World, R) {
        let mut world = World::new();
        eg_state::events::register_command_events_world(&mut world);
        let result = {
            let mut sink = WorldUiCommandSink::new(&mut world);
            f(&mut sink)
        };
        (world, result)
    }

    pub struct EditorUiTestInputs {
        pub playing: bool,
        pub timeline_time_seconds: f32,
        pub timeline_duration_seconds: f32,
        pub tap_times: Vec<f32>,
        pub selected_tap: Option<(usize, f32, [f32; 3])>,
        pub timeline_preview_position: [f32; 3],
        pub timeline_preview_direction: Direction,
        pub timing_points: Vec<TimingPoint>,
        pub playback_speed: f32,
        pub timing_selected_index: Option<usize>,
        pub waveform_zoom: f32,
        pub waveform_samples: Vec<f32>,
        pub waveform_sample_rate: u32,
        pub waveform_window_size: usize,
        pub waveform_loading: bool,
        pub waveform_complete: bool,
        pub bpm_tap_result: Option<f32>,
        pub trigger_objects: Vec<LevelObject>,
        pub trigger_selected_index: Option<usize>,
    }

    pub fn editor_ui_view(
        _app_settings: &AppSettings,
        _music_metadata: &MusicMetadata,
    ) -> EditorUiTestInputs {
        EditorUiTestInputs {
            playing: false,
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            tap_times: Vec::new(),
            selected_tap: None,
            timeline_preview_position: [0.0, 0.0, 0.0],
            timeline_preview_direction: Direction::Forward,
            timing_points: Vec::new(),
            playback_speed: 1.0,
            timing_selected_index: None,
            waveform_zoom: 1.0,
            waveform_samples: Vec::new(),
            waveform_sample_rate: 0,
            waveform_window_size: 256,
            waveform_loading: false,
            waveform_complete: false,
            bpm_tap_result: None,
            trigger_objects: vec![],
            trigger_selected_index: None,
        }
    }
}
