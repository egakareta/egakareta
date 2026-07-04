/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Session / UI / level-management events.
//!
//! A subset of session events mutate App-world resources directly (same
//! pattern as `events::snap`, `events::editor_mode::handle_set_block_id`,
//! and `events::bpm`). The handlers mutate
//! `SessionStateRes.0` directly:
//!
//! - `RenameLevel` -> `session.editor_level_name`
//! - `SetShowMetadata` -> `session.editor_show_metadata`
//! - `SetShowSettings` -> `session.editor_show_settings`
//! - `ToggleSettings` -> toggles `session.editor_show_settings`
//! - `SetSettingsSection` -> `session.editor_settings_section`
//! - `SetGraphicsBackend` -> `session.app_settings.graphics_backend`
//! - `SetKeybindCapture` -> `session.editor_keybind_capture_action`
//! - `SetKeybind` / `ClearKeybindSlot` / `ResetKeybind` /
//!   `ResetKeybinds` -> `session.app_settings.keybinds`
//! - `SetUiScaleMultiplier` -> `session.app_settings.ui_scale_multiplier`
//! - `UpdateMusic` -> `session.editor_music_metadata`
//! - `UpdateCreatorMetadata` -> `session.editor_creator_metadata`
//! - `UpdateSkyColor` -> `session.editor_sky_color`
//! - `CaptureMenuPreviewCamera` / `UseAutoMenuPreviewCamera` ->
//!   `session.editor_menu_preview_camera`
//!
//! Session side-effect events now write typed request resources because their legacy setters have out-of-process side effects (level load, file IO, audio backend switch).
//!
//! `Runtime::sync_app_session_state` propagates the carved-out
//! fields back to the legacy `State.session` each frame so any
//! code path that still reads the legacy fields sees the new
//! values.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_types::{
    AppPhase, AppSettings, BasicLevelMetadata, LevelPreviewCameraMetadata, MusicMetadata,
    SettingsSection,
};

use crate::state::{
    AppPhaseRes, CompleteImportRequest, EditorCameraRes, ExportLevelRequest, LoadLevelRequest,
    SessionStateRes, SetAudioBackendRequest, TriggerAudioImportRequest,
};

/// Load a level from a builtin resource name.
#[derive(Message, Debug, Clone)]
pub struct LoadLevel(pub String);

/// Rename the current level.
#[derive(Message, Debug, Clone)]
pub struct RenameLevel(pub String);

/// Start the level export process.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ExportLevel;

/// Open or close the metadata editing window.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetShowMetadata(pub bool);

/// Toggle the settings sidebar visibility.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleSettings;

/// Open or close the settings sidebar.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetShowSettings(pub bool);

/// Select which settings section is displayed in the sidebar.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetSettingsSection(pub SettingsSection);

/// Set preferred graphics backend for subsequent launches.
#[derive(Message, Debug, Clone)]
pub struct SetGraphicsBackend(pub String);

/// Set preferred audio backend and apply immediately when possible.
#[derive(Message, Debug, Clone)]
pub struct SetAudioBackend(pub String);

/// Set UI scale multiplier used with responsive UI scaling.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetUiScaleMultiplier(pub f32);

/// Start or cancel keybind capture for an action slot.
#[derive(Message, Debug, Clone)]
pub struct SetKeybindCapture(pub Option<(String, usize)>);

/// Set a keybind mapping for an action at a specific slot.
#[derive(Message, Debug, Clone)]
pub struct SetKeybind {
    pub action: String,
    pub slot: usize,
    pub chord: eg_types::KeyChord,
}

/// Clear the keybind mapping for an action at a specific slot.
#[derive(Message, Debug, Clone)]
pub struct ClearKeybindSlot {
    pub action: String,
    pub slot: usize,
}

/// Reset a single keybind action to its default values.
#[derive(Message, Debug, Clone)]
pub struct ResetKeybind(pub String);

/// Reset all keybinds to defaults.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ResetKeybinds;

/// Trigger level import via platform file picker.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct CompleteImport;

/// Update music information for the level.
#[derive(Message, Debug, Clone)]
pub struct UpdateMusic(pub MusicMetadata);

/// Update creator-facing metadata for the level.
#[derive(Message, Debug, Clone)]
pub struct UpdateCreatorMetadata(pub BasicLevelMetadata);

/// Update the level sky clear color.
#[derive(Message, Debug, Clone, Copy)]
pub struct UpdateSkyColor(pub [f32; 3]);

/// Trigger the platform audio import dialog.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TriggerAudioImport;

/// Store the current editor camera as menu preview camera metadata.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct CaptureMenuPreviewCamera;

/// Remove manual menu preview camera metadata and use automatic camera.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct UseAutoMenuPreviewCamera;

pub fn register_events(app: &mut App) {
    app.add_message::<LoadLevel>()
        .add_message::<RenameLevel>()
        .add_message::<ExportLevel>()
        .add_message::<SetShowMetadata>()
        .add_message::<ToggleSettings>()
        .add_message::<SetShowSettings>()
        .add_message::<SetSettingsSection>()
        .add_message::<SetGraphicsBackend>()
        .add_message::<SetAudioBackend>()
        .add_message::<SetUiScaleMultiplier>()
        .add_message::<SetKeybindCapture>()
        .add_message::<SetKeybind>()
        .add_message::<ClearKeybindSlot>()
        .add_message::<ResetKeybind>()
        .add_message::<ResetKeybinds>()
        .add_message::<CompleteImport>()
        .add_message::<UpdateMusic>()
        .add_message::<UpdateCreatorMetadata>()
        .add_message::<UpdateSkyColor>()
        .add_message::<TriggerAudioImport>()
        .add_message::<CaptureMenuPreviewCamera>()
        .add_message::<UseAutoMenuPreviewCamera>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

pub fn handle_load_level(
    mut events: MessageReader<LoadLevel>,
    mut request: ResMut<LoadLevelRequest>,
) {
    for event in events.read() {
        request.request(event.0.clone());
    }
}

pub fn handle_rename_level(
    mut events: MessageReader<RenameLevel>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_level_name = Some(event.0.clone());
    }
}

pub fn handle_export_level(
    mut events: MessageReader<ExportLevel>,
    mut request: ResMut<ExportLevelRequest>,
) {
    for _ in events.read() {
        request.request();
    }
}

pub fn handle_set_show_metadata(
    mut events: MessageReader<SetShowMetadata>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_show_metadata = event.0;
    }
}

pub fn handle_toggle_settings(
    mut events: MessageReader<ToggleSettings>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.editor_show_settings = !session.editor_show_settings;
    }
}

pub fn handle_set_show_settings(
    mut events: MessageReader<SetShowSettings>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_show_settings = event.0;
    }
}

pub fn handle_set_settings_section(
    mut events: MessageReader<SetSettingsSection>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_settings_section = event.0;
    }
}

pub fn handle_set_graphics_backend(
    mut events: MessageReader<SetGraphicsBackend>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        if session.graphics_backend_locked || session.app_settings.graphics_backend == event.0 {
            continue;
        }
        session.app_settings.graphics_backend = event.0.clone();
        session.settings_restart_required = true;
    }
}

pub fn handle_set_audio_backend(
    mut events: MessageReader<SetAudioBackend>,
    mut request: ResMut<SetAudioBackendRequest>,
) {
    for event in events.read() {
        request.request(event.0.clone());
    }
}

pub fn handle_set_ui_scale_multiplier(
    mut events: MessageReader<SetUiScaleMultiplier>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.app_settings.ui_scale_multiplier = AppSettings::clamp_ui_scale_multiplier(event.0);
    }
}

pub fn handle_set_keybind_capture(
    mut events: MessageReader<SetKeybindCapture>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_keybind_capture_action = event.0.clone();
    }
}

pub fn handle_set_keybind(
    mut events: MessageReader<SetKeybind>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session
            .app_settings
            .set_keybind_at_slot(&event.action, event.slot, event.chord.clone());
    }
}

pub fn handle_clear_keybind_slot(
    mut events: MessageReader<ClearKeybindSlot>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session
            .app_settings
            .clear_keybind_slot(&event.action, event.slot);
    }
}

pub fn handle_reset_keybind(
    mut events: MessageReader<ResetKeybind>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.app_settings.reset_keybind(&event.0);
    }
}

pub fn handle_reset_keybinds(
    mut events: MessageReader<ResetKeybinds>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.app_settings.reset_essential_keybinds();
    }
}

pub fn handle_complete_import(
    mut events: MessageReader<CompleteImport>,
    mut request: ResMut<CompleteImportRequest>,
) {
    for _ in events.read() {
        request.request();
    }
}

pub fn handle_update_music(
    mut events: MessageReader<UpdateMusic>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_music_metadata = event.0.clone();
    }
}

pub fn handle_update_creator_metadata(
    mut events: MessageReader<UpdateCreatorMetadata>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_creator_metadata = event.0.clone();
    }
}

pub fn handle_update_sky_color(
    mut events: MessageReader<UpdateSkyColor>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_sky_color = event.0.map(|component| component.clamp(0.0, 1.0));
    }
}

pub fn handle_trigger_audio_import(
    mut events: MessageReader<TriggerAudioImport>,
    mut request: ResMut<TriggerAudioImportRequest>,
) {
    for _ in events.read() {
        request.request();
    }
}

pub fn handle_capture_menu_preview_camera(
    mut events: MessageReader<CaptureMenuPreviewCamera>,
    phase: Res<AppPhaseRes>,
    camera: Res<EditorCameraRes>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Editor {
            continue;
        }

        let camera = &camera.0;
        let target = camera.editor_camera_target();
        let position = (target + camera.editor_camera_offset()).to_array();
        session.editor_menu_preview_camera = Some(LevelPreviewCameraMetadata {
            position,
            target: target.to_array(),
        });
    }
}

pub fn handle_use_auto_menu_preview_camera(
    mut events: MessageReader<UseAutoMenuPreviewCamera>,
    phase: Res<AppPhaseRes>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Editor {
            continue;
        }
        session.editor_menu_preview_camera = None;
    }
}
