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
//! Session side-effect events write typed request resources while their legacy
//! setters still own level load, export IO, and audio backend changes. Level
//! picker launch is App-owned and uses sender resources directly.
//!
//! Settings handlers persist App-owned changes directly. Runtime still mirrors
//! the remaining compatibility fields until their legacy readers move.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_types::{
    AppPhase, AppSettings, BasicLevelMetadata, EditorMode, EditorStateParams,
    LevelPreviewCameraMetadata, MusicMetadata, SettingsSection,
};

use crate::state::{
    AppPhaseRes, AudioBackendRes, AudioImportSenderRes, BuiltinLevelCatalogRes,
    EditorAudioCacheRes, EditorCameraRes, EditorInputRes, EditorObjectsRes, EditorSpawnRes,
    EditorTimelineRes, EditorTimingRes, LevelExportServiceRes, LevelImportSenderRes,
    RuntimeAudioPreloadRes, SessionStateRes, WaveformLoadSenderRes, apply_imported_level_to_world,
    load_waveform_for_level_resources,
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

pub fn handle_load_level(world: &mut World) {
    use bevy_ecs::message::Messages;

    let events: Vec<LoadLevel> = world
        .resource_mut::<Messages<LoadLevel>>()
        .drain()
        .collect();
    for event in events {
        let Some(metadata) = world.resource::<BuiltinLevelCatalogRes>().get(&event.0) else {
            continue;
        };

        apply_imported_level_to_world(world, metadata);
        world.resource_mut::<EditorTimelineRes>().0.playback = Default::default();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.editor_level_name = Some(event.0.clone());
            session.audio_stop_pending = true;
        }

        if world.resource::<EditorInputRes>().0.mode == EditorMode::Timing {
            load_waveform_for_level(world, event.0);
        }
    }
}

fn load_waveform_for_level(world: &mut World, level_name: String) {
    let editor_cache = world.resource::<EditorAudioCacheRes>().clone();
    let runtime_preload = world.resource::<RuntimeAudioPreloadRes>().clone();
    let sender = world.resource::<WaveformLoadSenderRes>().0.clone();
    let mut timing = world.resource_mut::<EditorTimingRes>();
    load_waveform_for_level_resources(
        level_name,
        &editor_cache,
        &runtime_preload,
        &mut timing.0,
        sender,
    );
}

pub fn handle_rename_level(
    mut events: MessageReader<RenameLevel>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.editor_level_name = Some(event.0.clone());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_export_level(
    mut events: MessageReader<ExportLevel>,
    session: Res<SessionStateRes>,
    objects: Res<EditorObjectsRes>,
    spawn: Res<EditorSpawnRes>,
    timeline: Res<EditorTimelineRes>,
    timing: Res<EditorTimingRes>,
    audio_cache: Res<EditorAudioCacheRes>,
    mut export_service: ResMut<LevelExportServiceRes>,
) {
    for _ in events.read() {
        let mut basic = session.editor_creator_metadata.clone();
        if let Some(level_name) = &session.editor_level_name {
            basic.name = level_name.clone();
        }
        let metadata = eg_types::LevelMetadata::from_editor_state(EditorStateParams {
            basic,
            music: session.editor_music_metadata.clone(),
            spawn: spawn.0.clone(),
            sky_color: session.editor_sky_color,
            tap_times: timeline.0.taps.tap_times.clone(),
            timing_points: timing.0.timing_points.clone(),
            timeline_time_seconds: timeline.0.clock.time_seconds,
            timeline_duration_seconds: timeline.0.clock.duration_seconds,
            menu_preview_camera: session.editor_menu_preview_camera.clone(),
            objects: objects.objects.clone(),
        });
        let audio_bytes = audio_cache
            .bytes_for_level_audio(&metadata.basic.name)
            .or_else(|| eg_platform::io::read_editor_music_bytes("audio.mp3"));

        match eg_level::build_level_export(&metadata, audio_bytes) {
            Ok(data) => {
                export_service.export(format!("{}.egz", metadata.basic.name), data);
            }
            Err(error) => log::error!("Export failed: {error}"),
        }
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
    let mut changed = false;
    for event in events.read() {
        if session.graphics_backend_locked || session.app_settings.graphics_backend == event.0 {
            continue;
        }
        session.app_settings.graphics_backend = event.0.clone();
        session.settings_restart_required = true;
        changed = true;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub fn handle_set_audio_backend(
    mut events: MessageReader<SetAudioBackend>,
    backend: Res<AudioBackendRes>,
    mut session: ResMut<SessionStateRes>,
) {
    let mut changed = false;
    for event in events.read() {
        if session.audio_backend_locked
            || !backend.set_preferred_backend_name(&event.0)
            || session.app_settings.audio_backend == event.0
        {
            continue;
        }
        session.app_settings.audio_backend = event.0.clone();
        changed = true;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub fn handle_set_ui_scale_multiplier(
    mut events: MessageReader<SetUiScaleMultiplier>,
    mut session: ResMut<SessionStateRes>,
) {
    let mut changed = false;
    for event in events.read() {
        let multiplier = AppSettings::clamp_ui_scale_multiplier(event.0);
        if (session.app_settings.ui_scale_multiplier - multiplier).abs() <= 1e-6 {
            continue;
        }
        session.app_settings.ui_scale_multiplier = multiplier;
        changed = true;
    }
    if changed {
        persist_app_settings(&session.app_settings);
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
    let mut changed = false;
    for event in events.read() {
        let previous = session.app_settings.keybinds.clone();
        session
            .app_settings
            .set_keybind_at_slot(&event.action, event.slot, event.chord.clone());
        changed |= session.app_settings.keybinds != previous;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub fn handle_clear_keybind_slot(
    mut events: MessageReader<ClearKeybindSlot>,
    mut session: ResMut<SessionStateRes>,
) {
    let mut changed = false;
    for event in events.read() {
        let previous = session.app_settings.keybinds.clone();
        session
            .app_settings
            .clear_keybind_slot(&event.action, event.slot);
        changed |= session.app_settings.keybinds != previous;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub fn handle_reset_keybind(
    mut events: MessageReader<ResetKeybind>,
    mut session: ResMut<SessionStateRes>,
) {
    let mut changed = false;
    for event in events.read() {
        let previous = session.app_settings.keybinds.clone();
        session.app_settings.reset_keybind(&event.0);
        changed |= session.app_settings.keybinds != previous;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub fn handle_reset_keybinds(
    mut events: MessageReader<ResetKeybinds>,
    mut session: ResMut<SessionStateRes>,
) {
    let mut changed = false;
    for _ in events.read() {
        let previous = session.app_settings.keybinds.clone();
        session.app_settings.reset_essential_keybinds();
        changed |= session.app_settings.keybinds != previous;
    }
    if changed {
        persist_app_settings(&session.app_settings);
    }
}

pub(crate) fn persist_app_settings(settings: &AppSettings) {
    let settings = settings.clone();
    eg_platform::task::spawn_background(async move {
        if let Err(error) = eg_platform::io::save_app_settings_to_storage(&settings).await {
            log::error!("Failed to persist app settings: {error}");
        }
    });
}

pub fn handle_complete_import(
    mut events: MessageReader<CompleteImport>,
    mut sender: ResMut<LevelImportSenderRes>,
) {
    for _ in events.read() {
        sender.trigger_picker();
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
    mut sender: ResMut<AudioImportSenderRes>,
) {
    for _ in events.read() {
        sender.trigger_picker();
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
