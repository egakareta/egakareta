/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::block_id::DEFAULT_BLOCK_ID;
use crate::level::PlayerLevelProgress;

macro_rules! serde_default {
    ($default_name:ident : $ty:ty = $value:expr) => {
        #[allow(dead_code)]
        fn $default_name() -> $ty {
            $value
        }
        paste::paste! {
            #[allow(dead_code)]
            fn [<is_ $default_name>](value: &$ty) -> bool {
                *value == $value
            }
        }
    };
}

macro_rules! serde_default_float {
    ($default_name:ident : $ty:ty = $value:expr) => {
        #[allow(dead_code)]
        fn $default_name() -> $ty {
            $value
        }
        paste::paste! {
            #[allow(dead_code)]
            fn [<is_ $default_name>](value: &$ty) -> bool {
                (*value - $value).abs() <= 1e-6
            }
        }
    };
}

pub const APP_SETTINGS_VERSION: u32 = 3;
serde_default!(default_app_settings_version: u32 = APP_SETTINGS_VERSION);
serde_default!(default_editor_selected_block_id: String = DEFAULT_BLOCK_ID.to_string());
serde_default!(default_editor_snap_to_grid_setting: bool = true);
serde_default_float!(default_editor_snap_step_setting: f32 = 1.0);
serde_default!(default_editor_rotation_snap_setting: bool = true);
serde_default_float!(default_editor_rotation_snap_step_setting: f32 = 15.0);

fn default_graphics_backend_setting() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "BrowserWebGpu".to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        "Auto".to_string()
    }
}

fn is_default_graphics_backend_setting(value: &String) -> bool {
    value == &default_graphics_backend_setting()
}

serde_default!(default_audio_backend_setting: String = "Default".to_string());
serde_default_float!(default_ui_scale_multiplier_setting: f32 = 1.0);

fn default_app_keybinds() -> Vec<KeybindBinding> {
    default_essential_keybinds()
}

fn is_default_app_keybinds(value: &[KeybindBinding]) -> bool {
    value == default_essential_keybinds().as_slice()
}

fn default_level_progress() -> HashMap<String, PlayerLevelProgress> {
    HashMap::new()
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    Backends,
    Keybinds,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct KeyChord {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyChord {
    pub fn new(key: impl Into<String>, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key: normalize_binding_key(&key.into()),
            ctrl,
            shift,
            alt,
        }
    }

    pub fn normalized(&self) -> Self {
        Self {
            key: normalize_binding_key(&self.key),
            ctrl: self.ctrl,
            shift: self.shift,
            alt: self.alt,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct KeybindBinding {
    pub action: String,
    pub chord: KeyChord,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AppSettings {
    #[serde(
        default = "default_app_settings_version",
        skip_serializing_if = "is_default_app_settings_version"
    )]
    pub version: u32,
    #[serde(
        default = "default_editor_selected_block_id",
        skip_serializing_if = "is_default_editor_selected_block_id"
    )]
    pub editor_selected_block_id: String,
    #[serde(
        default = "default_editor_snap_to_grid_setting",
        skip_serializing_if = "is_default_editor_snap_to_grid_setting"
    )]
    pub editor_snap_to_grid: bool,
    #[serde(
        default = "default_editor_snap_step_setting",
        skip_serializing_if = "is_default_editor_snap_step_setting"
    )]
    pub editor_snap_step: f32,
    #[serde(
        default = "default_editor_rotation_snap_setting",
        skip_serializing_if = "is_default_editor_rotation_snap_setting"
    )]
    pub editor_rotation_snap: bool,
    #[serde(
        default = "default_editor_rotation_snap_step_setting",
        skip_serializing_if = "is_default_editor_rotation_snap_step_setting"
    )]
    pub editor_rotation_snap_step: f32,
    #[serde(
        default = "default_graphics_backend_setting",
        skip_serializing_if = "is_default_graphics_backend_setting"
    )]
    pub graphics_backend: String,
    #[serde(
        default = "default_audio_backend_setting",
        skip_serializing_if = "is_default_audio_backend_setting"
    )]
    pub audio_backend: String,
    #[serde(
        default = "default_ui_scale_multiplier_setting",
        skip_serializing_if = "is_default_ui_scale_multiplier_setting"
    )]
    pub ui_scale_multiplier: f32,
    #[serde(
        default = "default_app_keybinds",
        skip_serializing_if = "is_default_app_keybinds"
    )]
    pub keybinds: Vec<KeybindBinding>,
    #[serde(
        default = "default_level_progress",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub level_progress: HashMap<String, PlayerLevelProgress>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: APP_SETTINGS_VERSION,
            editor_selected_block_id: default_editor_selected_block_id(),
            editor_snap_to_grid: default_editor_snap_to_grid_setting(),
            editor_snap_step: default_editor_snap_step_setting(),
            editor_rotation_snap: default_editor_rotation_snap_setting(),
            editor_rotation_snap_step: default_editor_rotation_snap_step_setting(),
            graphics_backend: default_graphics_backend_setting(),
            audio_backend: default_audio_backend_setting(),
            ui_scale_multiplier: default_ui_scale_multiplier_setting(),
            keybinds: default_app_keybinds(),
            level_progress: default_level_progress(),
        }
    }
}

impl AppSettings {
    pub fn clamp_ui_scale_multiplier(value: f32) -> f32 {
        if !value.is_finite() {
            return default_ui_scale_multiplier_setting();
        }
        value.clamp(0.5, 3.0)
    }

    pub fn normalized_ui_scale_multiplier(&self) -> f32 {
        Self::clamp_ui_scale_multiplier(self.ui_scale_multiplier)
    }

    pub fn keybinds_for_action(&self, action: &str) -> Vec<&KeyChord> {
        self.keybinds
            .iter()
            .filter(|binding| binding.action == action)
            .map(|binding| &binding.chord)
            .collect()
    }

    /// Returns a formatted hotkey hint like `" (Ctrl+S)"` for use in tooltip text.
    /// Returns an empty string if no keybinding exists for the action.
    pub fn hotkey_hint(&self, action: &str) -> String {
        self.keybinds_for_action(action)
            .first()
            .map(|c| format!(" ({})", format_key_chord(c)))
            .unwrap_or_default()
    }

    /// Returns a formatted hotkey hint like `" (Ctrl+S)"`, falling back to
    /// `" ({default_label})"` when no keybinding exists for the action.
    pub fn hotkey_hint_or(&self, action: &str, default_label: &str) -> String {
        self.keybinds_for_action(action)
            .first()
            .map(|c| format!(" ({})", format_key_chord(c)))
            .unwrap_or_else(|| format!(" ({})", default_label))
    }

    pub fn set_keybind_at_slot(&mut self, action: &str, slot: usize, chord: KeyChord) {
        let normalized = chord.normalized();

        // Prevent duplicate chord for SAME action
        if self
            .keybinds
            .iter()
            .any(|b| b.action == action && b.chord == normalized)
        {
            return;
        }

        let existing_indices: Vec<usize> = self
            .keybinds
            .iter()
            .enumerate()
            .filter(|(_, b)| b.action == action)
            .map(|(i, _)| i)
            .collect();

        if slot < existing_indices.len() {
            self.keybinds[existing_indices[slot]].chord = normalized;
        } else {
            let capacity = essential_keybind_actions()
                .iter()
                .find(|m| m.action == action)
                .map(|m| m.capacity)
                .unwrap_or(1);
            if existing_indices.len() < capacity {
                self.keybinds.push(KeybindBinding {
                    action: action.to_string(),
                    chord: normalized,
                });
            }
        }
    }

    pub fn clear_keybind_slot(&mut self, action: &str, slot: usize) {
        let existing_indices: Vec<usize> = self
            .keybinds
            .iter()
            .enumerate()
            .filter(|(_, b)| b.action == action)
            .map(|(i, _)| i)
            .collect();

        if slot < existing_indices.len() {
            self.keybinds.remove(existing_indices[slot]);
        }
    }

    pub fn reset_keybind(&mut self, action: &str) {
        self.keybinds.retain(|b| b.action != action);
        for default_binding in default_essential_keybinds() {
            if default_binding.action == action {
                self.keybinds.push(default_binding);
            }
        }
    }

    pub fn reset_essential_keybinds(&mut self) {
        let mut preserved = self.keybinds.clone();
        for metadata in essential_keybind_actions() {
            preserved.retain(|binding| binding.action != metadata.action);
        }
        preserved.extend(default_essential_keybinds());
        self.keybinds = preserved;
    }

    pub fn update_level_progress(
        &mut self,
        level_name: &str,
        attempt: PlayerLevelProgress,
    ) -> bool {
        let level_name = level_name.trim();
        if level_name.is_empty() {
            return false;
        }

        let attempt = attempt.sanitized();
        let entry = self
            .level_progress
            .entry(level_name.to_string())
            .or_default();
        let mut changed = false;

        if attempt.progress_percent > entry.progress_percent {
            entry.progress_percent = attempt.progress_percent;
            changed = true;
        }
        if attempt.completed && !entry.completed {
            entry.completed = true;
            changed = true;
        }
        if attempt.gems_max > entry.gems_max {
            entry.gems_max = attempt.gems_max;
            changed = true;
        }
        if attempt.gems_collected > entry.gems_collected {
            entry.gems_collected = attempt
                .gems_collected
                .min(entry.gems_max.max(attempt.gems_max));
            changed = true;
        }

        changed
    }

    pub fn migrate_defaults(&mut self) -> bool {
        if self.version >= APP_SETTINGS_VERSION {
            return false;
        }

        let mut changed = self.version != APP_SETTINGS_VERSION;
        for default_binding in default_essential_keybinds() {
            if !self
                .keybinds
                .iter()
                .any(|binding| binding.action == default_binding.action)
            {
                self.keybinds.push(default_binding);
                changed = true;
            }
        }
        self.version = APP_SETTINGS_VERSION;
        changed
    }
}

pub fn normalize_binding_key(key: &str) -> String {
    match key {
        " " => "Space".to_string(),
        "ControlLeft" | "ControlRight" => "Control".to_string(),
        "AltLeft" | "AltRight" => "Alt".to_string(),
        _ => {
            if key.len() == 1 {
                key.to_ascii_lowercase()
            } else {
                key.to_string()
            }
        }
    }
}

pub fn format_key_chord(chord: &KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push("Ctrl".to_string());
    }
    if chord.shift {
        parts.push("Shift".to_string());
    }
    if chord.alt {
        parts.push("Alt".to_string());
    }

    let key = if chord.key.len() == 1 {
        chord.key.to_ascii_uppercase()
    } else {
        chord.key.clone()
    };
    parts.push(key);
    parts.join("+")
}

pub struct KeybindActionMetadata {
    pub group: &'static str,
    pub action: &'static str,
    pub label: &'static str,
    pub capacity: usize,
}

pub fn essential_keybind_actions() -> &'static [KeybindActionMetadata] {
    &[
        KeybindActionMetadata {
            group: "General",
            action: "toggle_settings",
            label: "Toggle Settings",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "toggle_editor",
            label: "Toggle Editor",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "game_turn",
            label: "Turn / Action",
            capacity: 3,
        },
        KeybindActionMetadata {
            group: "General",
            action: "menu_prev_level",
            label: "Previous Level",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "General",
            action: "menu_next_level",
            label: "Next Level",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Gameplay",
            action: "practice_checkpoint",
            label: "Place Practice Checkpoint",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Gameplay",
            action: "practice_remove_checkpoint",
            label: "Remove Practice Checkpoint",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "toggle_timeline_playback",
            label: "Toggle Timeline Playback",
            capacity: 4,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "playtest",
            label: "Start Playtest",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "remove_block",
            label: "Remove Block",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "copy",
            label: "Copy",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "paste",
            label: "Paste",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "duplicate",
            label: "Duplicate",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "undo",
            label: "Undo",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "redo",
            label: "Redo",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_up",
            label: "Pan Up",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_down",
            label: "Pan Down",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_left",
            label: "Pan Left",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_right",
            label: "Pan Right",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_up",
            label: "Nudge Block Up",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_down",
            label: "Nudge Block Down",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_left",
            label: "Nudge Block Left",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_right",
            label: "Nudge Block Right",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "snap_selection_to_grid",
            label: "Snap Selection to Grid",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "toggle_grid_snap",
            label: "Toggle Grid Snap",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "toggle_rotation_snap",
            label: "Toggle Rotation Snap",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pick_selected_block",
            label: "Pick Selected Block",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "select_recent_block_1",
            label: "Select Recent Block 1",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "select_recent_block_2",
            label: "Select Recent Block 2",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "select_recent_block_3",
            label: "Select Recent Block 3",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "select_recent_block_4",
            label: "Select Recent Block 4",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "focus_camera_target",
            label: "Focus Camera Target",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "spawn_set",
            label: "Set Spawn Position",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "spawn_rotate",
            label: "Rotate Spawn Direction",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "add_camera_trigger",
            label: "Add Camera Trigger",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "add_transform_trigger",
            label: "Add Transform Trigger",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "timeline_forward",
            label: "Shift Timeline Forward",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "timeline_backward",
            label: "Shift Timeline Backward",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "zoom_in",
            label: "Zoom In",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "zoom_out",
            label: "Zoom Out",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Debug",
            action: "toggle_hitbox_visualization",
            label: "Toggle Hitbox Visualization",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Debug",
            action: "toggle_perf_overlay",
            label: "Toggle Performance Overlay",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_select",
            label: "Select Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_move",
            label: "Move Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_scale",
            label: "Scale Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_rotate",
            label: "Rotate Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Tabs",
            action: "tab_compose",
            label: "Compose Tab",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Tabs",
            action: "tab_timing",
            label: "Timing Tab",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Tabs",
            action: "tab_tapping",
            label: "Tapping Tab",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "escape",
            label: "Context Escape",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "toggle_place_window",
            label: "Place Window",
            capacity: 1,
        },
    ]
}

pub fn default_essential_keybinds() -> Vec<KeybindBinding> {
    vec![
        KeybindBinding {
            action: "toggle_settings".to_string(),
            chord: KeyChord::new("o", true, false, false),
        },
        KeybindBinding {
            action: "toggle_editor".to_string(),
            chord: KeyChord::new("e", false, false, false),
        },
        KeybindBinding {
            action: "toggle_place_window".to_string(),
            chord: KeyChord::new("e", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("w", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("Space", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("ArrowUp", false, false, false),
        },
        KeybindBinding {
            action: "menu_prev_level".to_string(),
            chord: KeyChord::new("a", false, false, false),
        },
        KeybindBinding {
            action: "menu_prev_level".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "menu_next_level".to_string(),
            chord: KeyChord::new("d", false, false, false),
        },
        KeybindBinding {
            action: "menu_next_level".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "practice_checkpoint".to_string(),
            chord: KeyChord::new("z", false, false, false),
        },
        KeybindBinding {
            action: "practice_remove_checkpoint".to_string(),
            chord: KeyChord::new("x", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("Space", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPlayPause", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPlay", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPause", false, false, false),
        },
        KeybindBinding {
            action: "playtest".to_string(),
            chord: KeyChord::new("Enter", false, false, false),
        },
        KeybindBinding {
            action: "remove_block".to_string(),
            chord: KeyChord::new("Delete", false, false, false),
        },
        KeybindBinding {
            action: "copy".to_string(),
            chord: KeyChord::new("c", true, false, false),
        },
        KeybindBinding {
            action: "paste".to_string(),
            chord: KeyChord::new("v", true, false, false),
        },
        KeybindBinding {
            action: "duplicate".to_string(),
            chord: KeyChord::new("d", true, false, false),
        },
        KeybindBinding {
            action: "undo".to_string(),
            chord: KeyChord::new("z", true, false, false),
        },
        KeybindBinding {
            action: "redo".to_string(),
            chord: KeyChord::new("y", true, false, false),
        },
        KeybindBinding {
            action: "redo".to_string(),
            chord: KeyChord::new("z", true, true, false),
        },
        KeybindBinding {
            action: "pan_up".to_string(),
            chord: KeyChord::new("w", false, false, false),
        },
        KeybindBinding {
            action: "pan_down".to_string(),
            chord: KeyChord::new("s", false, false, false),
        },
        KeybindBinding {
            action: "pan_left".to_string(),
            chord: KeyChord::new("a", false, false, false),
        },
        KeybindBinding {
            action: "pan_right".to_string(),
            chord: KeyChord::new("d", false, false, false),
        },
        KeybindBinding {
            action: "nudge_up".to_string(),
            chord: KeyChord::new("ArrowUp", false, false, false),
        },
        KeybindBinding {
            action: "nudge_down".to_string(),
            chord: KeyChord::new("ArrowDown", false, false, false),
        },
        KeybindBinding {
            action: "nudge_left".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "nudge_right".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "snap_selection_to_grid".to_string(),
            chord: KeyChord::new("g", false, false, false),
        },
        KeybindBinding {
            action: "toggle_grid_snap".to_string(),
            chord: KeyChord::new("g", true, false, false),
        },
        KeybindBinding {
            action: "pick_selected_block".to_string(),
            chord: KeyChord::new("b", false, false, false),
        },
        KeybindBinding {
            action: "focus_camera_target".to_string(),
            chord: KeyChord::new("f", false, false, false),
        },
        KeybindBinding {
            action: "spawn_set".to_string(),
            chord: KeyChord::new("p", false, false, false),
        },
        KeybindBinding {
            action: "spawn_rotate".to_string(),
            chord: KeyChord::new("r", false, false, false),
        },
        KeybindBinding {
            action: "add_camera_trigger".to_string(),
            chord: KeyChord::new("k", false, true, false),
        },
        KeybindBinding {
            action: "add_transform_trigger".to_string(),
            chord: KeyChord::new("t", false, false, false),
        },
        KeybindBinding {
            action: "timeline_forward".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "timeline_backward".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "zoom_in".to_string(),
            chord: KeyChord::new("=", false, false, false),
        },
        KeybindBinding {
            action: "zoom_in".to_string(),
            chord: KeyChord::new("+", false, false, false),
        },
        KeybindBinding {
            action: "zoom_out".to_string(),
            chord: KeyChord::new("-", false, false, false),
        },
        KeybindBinding {
            action: "zoom_out".to_string(),
            chord: KeyChord::new("_", false, false, false),
        },
        KeybindBinding {
            action: "toggle_hitbox_visualization".to_string(),
            chord: KeyChord::new("h", true, false, false),
        },
        KeybindBinding {
            action: "toggle_perf_overlay".to_string(),
            chord: KeyChord::new("F12", true, true, true),
        },
        KeybindBinding {
            action: "mode_select".to_string(),
            chord: KeyChord::new("1", false, false, false),
        },
        KeybindBinding {
            action: "mode_move".to_string(),
            chord: KeyChord::new("2", false, false, false),
        },
        KeybindBinding {
            action: "mode_scale".to_string(),
            chord: KeyChord::new("3", false, false, false),
        },
        KeybindBinding {
            action: "mode_rotate".to_string(),
            chord: KeyChord::new("4", false, false, false),
        },
        KeybindBinding {
            action: "tab_compose".to_string(),
            chord: KeyChord::new("1", true, false, false),
        },
        KeybindBinding {
            action: "tab_timing".to_string(),
            chord: KeyChord::new("2", true, false, false),
        },
        KeybindBinding {
            action: "tab_tapping".to_string(),
            chord: KeyChord::new("3", true, false, false),
        },
        KeybindBinding {
            action: "select_recent_block_1".to_string(),
            chord: KeyChord::new("1", false, false, true),
        },
        KeybindBinding {
            action: "select_recent_block_2".to_string(),
            chord: KeyChord::new("2", false, false, true),
        },
        KeybindBinding {
            action: "select_recent_block_3".to_string(),
            chord: KeyChord::new("3", false, false, true),
        },
        KeybindBinding {
            action: "select_recent_block_4".to_string(),
            chord: KeyChord::new("4", false, false, true),
        },
        KeybindBinding {
            action: "escape".to_string(),
            chord: KeyChord::new("Escape", false, false, false),
        },
    ]
}
