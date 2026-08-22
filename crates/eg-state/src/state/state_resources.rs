/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS resource definitions for eg-state.
//!
//! These resources are the App-world state boundary for the per-subsystem
//! values that historically lived in the hand-rolled state machine. App
//! bootstrap inserts the resources directly, and migrated production systems
//! own the live values. Platform services remain behind explicit resource
//! boundaries where they require external channels or handles.
//!
//! Naming convention: `<Domain>Res` for top-level domain resources, and
//! `<Domain><Noun>Res` for grouped sub-state.
//!
//! Only types with a usable `Default` impl are exposed in Phase 2a. Types
//! that wrap non-`Default` channel pairs (audio, auth) are added in
//! Phase 2b alongside their resource registration helpers.

use std::{collections::HashMap, sync::mpsc};

use bevy_ecs::prelude::*;
use eg_platform::services::AuthServiceMessage;
use eg_types::{
    AppSettings, AuthSession, BasicLevelMetadata, Direction, LevelObject,
    LevelPreviewCameraMetadata, MenuState, PlayerLevelProgress, SpawnMetadata,
};

use crate::state::{
    PracticeCheckpoint,
    editor_camera::EditorCameraState,
    editor_config_state::EditorConfigState,
    editor_timeline::EditorTimelineState,
    editor_timing::EditorTimingState,
    editor_triggers::EditorTriggerState,
    history::EditorHistoryState,
    perf::PerfState,
    runtime::{BlockVisualEffect, EditorRuntimeState},
};

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Frame timing
// ---------------------------------------------------------------------------

/// App-owned frame timing resource used by scheduled systems.
///
/// `Time<Virtual>` is authoritative for production App frames; `FrameTimeRes` retains derived FPS and presentation metrics. Bare-world direct schedules use its bounded `Instant` fallback.
#[derive(Resource, Clone, Copy)]
pub struct FrameTimeRes {
    pub last_frame: web_time::Instant,
    pub delta_seconds: f32,
    pub global_time_seconds: f32,
    pub fps_smoothed: f32,
}

impl FrameTimeRes {
    pub fn advance(&mut self) {
        let now = web_time::Instant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.25);
        self.last_frame = now;
        self.apply_delta(frame_dt, None);
    }

    pub fn advance_from_bevy_time(&mut self, delta_seconds: f32, global_time_seconds: f32) {
        self.last_frame = web_time::Instant::now();
        self.apply_delta(delta_seconds, Some(global_time_seconds));
    }

    fn apply_delta(&mut self, delta_seconds: f32, global_time_seconds: Option<f32>) {
        let frame_dt = delta_seconds.clamp(0.0, 0.25);
        self.delta_seconds = frame_dt;

        let instant_fps = 1.0 / frame_dt.max(1e-4);
        if self.fps_smoothed <= 0.0 {
            self.fps_smoothed = instant_fps;
        } else {
            self.fps_smoothed = self.fps_smoothed * 0.9 + instant_fps * 0.1;
        }

        self.global_time_seconds = global_time_seconds
            .unwrap_or(self.global_time_seconds + frame_dt)
            .max(0.0);
    }
}

impl Default for FrameTimeRes {
    fn default() -> Self {
        Self {
            last_frame: web_time::Instant::now(),
            delta_seconds: 0.0,
            global_time_seconds: 0.0,
            fps_smoothed: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Session / menu / editor domain resources (default-constructible only)
// ---------------------------------------------------------------------------

/// App-owned catalog of built-in level metadata used by scheduled handlers.
#[derive(Resource, Clone)]
pub struct BuiltinLevelCatalogRes {
    levels: HashMap<String, eg_types::LevelMetadata>,
}

impl BuiltinLevelCatalogRes {
    pub fn from_levels(levels: impl IntoIterator<Item = eg_types::LevelMetadata>) -> Self {
        Self {
            levels: levels
                .into_iter()
                .map(|metadata| (metadata.basic.name.clone(), metadata))
                .collect(),
        }
    }

    pub fn get(&self, level_name: &str) -> Option<eg_types::LevelMetadata> {
        self.levels.get(level_name).cloned()
    }
}

impl Default for BuiltinLevelCatalogRes {
    fn default() -> Self {
        Self::from_levels(
            eg_level::builtin_level_names()
                .into_iter()
                .filter_map(|name| eg_level::load_builtin_level_metadata(&name)),
        )
    }
}

/// App-owned session state used directly by handlers, UI view construction,
/// gameplay transitions, and async level-import results.
#[derive(Resource, Default)]
pub struct SessionStateRes {
    pub editor_level_name: Option<String>,
    pub editor_music_metadata: eg_types::MusicMetadata,
    pub editor_creator_metadata: BasicLevelMetadata,
    pub editor_sky_color: [f32; 3],
    pub editor_menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    pub editor_show_metadata: bool,
    pub editor_show_place_window: bool,
    pub editor_show_settings: bool,
    pub editor_settings_section: eg_types::SettingsSection,
    pub editor_keybind_capture_action: Option<(String, usize)>,
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
    /// Snapshot of the editor's authored block list captured at the moment a
    /// playtest begins. `replace_app_gameplay_world` despawns every entity
    /// tagged `Object` (which includes the editor's authored blocks), so this
    /// snapshot is the only authority for restoring the editor's blocks when
    /// the playtest is left via Escape, the back-to-editor button, or
    /// completion. Cleared once the editor has been repopulated.
    pub playtest_editor_objects: Option<Vec<LevelObject>>,
    pub practice_mode_enabled: bool,
    pub practice_checkpoints: Vec<PracticeCheckpoint>,
    /// One-shot audio compatibility intent for an App-owned quit-to-menu
    /// transition. The audio service remains external until its resource
    /// migration, so Runtime consumes this marker once to stop playback.
    pub audio_stop_pending: bool,
}

impl SessionStateRes {
    /// Clear App-owned gameplay/session fields for a transition to menu.
    /// External audio remains a one-shot compatibility intent until the audio
    /// backend itself is an App resource.
    pub fn transition_to_menu(&mut self) {
        self.game_paused = false;
        self.playtesting_editor = false;
        self.playtest_audio_start_seconds = None;
        self.playing_level_name = None;
        self.playing_trigger_base_objects = None;
        self.playtest_editor_objects = None;
        self.practice_mode_enabled = false;
        self.practice_checkpoints.clear();
        self.audio_stop_pending = true;
    }
}

/// App-world seed for a gameplay transition that must replace the active
/// player/object world atomically before the next gameplay tick.
#[derive(Clone)]
pub struct GameplayResetSeed {
    pub objects: Vec<LevelObject>,
    pub position: [f32; 3],
    pub direction: Direction,
    pub speed: f32,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
    pub elapsed_seconds: f32,
    pub level_duration_seconds: f32,
    pub progress: Option<PlayerLevelProgress>,
    pub preserve_practice_state: bool,
}

/// App-world marker for a gameplay world replacement. The navigation and
/// timeline handlers record intent here and the exclusive gameplay-world
/// system applies the entity/resource reset before the next gameplay tick.
#[derive(Resource, Default)]
pub struct GameplayResetRes {
    pub pending: bool,
    pub seed: Option<GameplayResetSeed>,
}

impl GameplayResetRes {
    pub fn request(&mut self) {
        self.pending = true;
        self.seed = None;
    }

    pub fn request_with_seed(&mut self, seed: GameplayResetSeed) {
        self.pending = true;
        self.seed = Some(seed);
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// `MenuState` resource so UI systems can read the menu directly from the App
/// world. `MenuState`
/// itself doesn't derive `Default` (its fields like `preview_sky_color`
/// aren't all `Default`), so this resource is constructed explicitly
/// via [`MenuStateRes::new`].
#[derive(Resource)]
pub struct MenuStateRes(pub MenuState);

impl MenuStateRes {
    pub fn new(menu: MenuState) -> Self {
        Self(menu)
    }
}

/// App-owned authored objects for the menu's selected-level preview.
///
/// The menu preview is not the editor object list: returning from an editor
/// session must preserve the authored editor objects while the menu renders
/// the selected built-in level. This resource keeps that distinction explicit;
/// the render extraction schedule builds the preview mesh from it.
#[derive(Resource, Default, Clone)]
pub struct MenuPreviewObjectsRes {
    pub objects: Vec<LevelObject>,
}

/// App-owned active block visual effects.
///
/// Playback systems enqueue new effects through `EditorRuntimeRes`; the
/// scheduled effect system consumes, ages, and expires them here for extraction.
#[derive(Resource, Default, Clone)]
pub struct BlockEffectsRes {
    pub effects: Vec<BlockVisualEffect>,
}

/// `EditorInputState` mirrored as a resource. UI + interaction systems
/// (Phase 3+) read/write this resource instead of `state.editor.input`.
#[derive(Resource, Default)]
pub struct EditorInputRes(pub eg_types::EditorInputState);

/// `EditorConfigState` mirrored as a resource.
#[derive(Resource, Default)]
pub struct EditorConfigRes(pub EditorConfigState);

/// Authoritative App-world editor camera and transition state.
#[derive(Resource, Default)]
pub struct EditorCameraRes(pub EditorCameraState);

/// `EditorTriggerState` mirrored as a resource.
#[derive(Resource, Default)]
pub struct EditorTriggerRes(pub EditorTriggerState);

/// `EditorTimelineState` mirrored as a resource.
#[derive(Resource, Default)]
pub struct EditorTimelineRes(pub EditorTimelineState);

/// `EditorTimingState` mirrored as a resource.
#[derive(Resource, Default)]
pub struct EditorTimingRes(pub EditorTimingState);

/// Authoritative App-world editor spawn metadata.
///
/// App bootstrap seeds this resource once from level/editor data.
/// `SetSpawnHere` and `RotateSpawnDirection` mutate it directly.
#[derive(Resource, Default)]
pub struct EditorSpawnRes(pub SpawnMetadata);

/// Authoritative App-world editor interaction runtime and transient effects.
#[derive(Resource, Default)]
pub struct EditorRuntimeRes(pub EditorRuntimeState);

/// App-owned undo/redo history separated from transient interaction runtime.
#[derive(Resource, Default)]
pub struct EditorHistoryRes(pub EditorHistoryState);

/// `PerfState` mirrored as a resource so the profiler overlay can read it
/// from a system.
#[derive(Resource, Default)]
pub struct PerfRes(pub PerfState);

/// App-world request to copy the latest profiler frame.
///
/// `state::perf::process_perf_frame_copy_request_system` consumes this in
/// `StateSchedule` and writes the formatted latest profiler frame to the
/// system clipboard when frame data is available.
#[derive(Resource, Default)]
pub struct PerfFrameCopyRequest {
    pub pending: bool,
}

impl PerfFrameCopyRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world auth state and service sender. Auth request consumption, async
/// result application, and UI reads all use this resource directly.
#[derive(Resource, Clone)]
pub struct AuthStateRes {
    pub session: Option<AuthSession>,
    pub pending: bool,
    pub message: Option<String>,
    pub refresh_started: bool,
    pub external_navigation_enabled: bool,
    pub sender: mpsc::Sender<AuthServiceMessage>,
}

impl Default for AuthStateRes {
    fn default() -> Self {
        let (sender, _receiver) = mpsc::channel();
        Self {
            session: None,
            pending: false,
            message: None,
            refresh_started: false,
            external_navigation_enabled: false,
            sender,
        }
    }
}

/// App-world request to start the browser sign-in flow.
///
/// The sign-in side effect touches network IO (it calls
/// `trigger_auth_sign_in` which sends a message on the auth channel).
/// Phase 6e consumes this request in `StateSchedule` via `AuthStateRes`.
#[derive(Resource, Default)]
pub struct AuthSubmitSignInRequest {
    pub pending: bool,
}

impl AuthSubmitSignInRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to sign out the current account.
///
/// The sign-out side effect touches network IO (it calls
/// `trigger_auth_sign_out` which sends a message on the auth
/// channel). Phase 6e consumes this request in `StateSchedule` via
/// `AuthStateRes`.
#[derive(Resource, Default)]
pub struct AuthSignOutRequest {
    pub pending: bool,
}

impl AuthSignOutRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to open the web signup page.
///
/// The signup-page open side effect touches platform code
/// (`eg_platform::auth::open_signup_page`). Phase 6e consumes this request in
/// `StateSchedule` via `AuthStateRes`.
#[derive(Resource, Default)]
pub struct AuthOpenSignupRequest {
    pub pending: bool,
}

impl AuthOpenSignupRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-owned platform service for saving encoded editor levels.
#[derive(Resource, Default)]
pub struct LevelExportServiceRes {
    #[cfg(test)]
    last_request: Option<(String, Vec<u8>)>,
}

impl LevelExportServiceRes {
    pub fn export(&mut self, filename: String, data: Vec<u8>) {
        #[cfg(test)]
        {
            self.last_request = Some((filename, data));
        }

        #[cfg(not(test))]
        eg_platform::task::spawn_background(async move {
            if let Err(error) = eg_platform::io::save_level_export(&filename, &data).await {
                log::error!("Export failed: {error}");
            }
        });
    }

    #[cfg(test)]
    pub fn last_request(&self) -> Option<(&str, &[u8])> {
        self.last_request
            .as_ref()
            .map(|(filename, data)| (filename.as_str(), data.as_slice()))
    }
}

/// App-owned sender used to launch the platform level picker directly from
/// `StateSchedule`. The matching receiver is drained by `LevelImportChannelRes`.
#[derive(Resource)]
pub struct LevelImportSenderRes {
    sender: mpsc::Sender<Vec<u8>>,
    #[cfg(test)]
    trigger_count: usize,
}

impl LevelImportSenderRes {
    pub fn new(sender: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            sender,
            #[cfg(test)]
            trigger_count: 0,
        }
    }

    pub fn trigger_picker(&mut self) {
        #[cfg(not(test))]
        eg_platform::services::trigger_level_import(self.sender.clone());
        #[cfg(test)]
        {
            let _sender = self.sender.clone();
            self.trigger_count += 1;
        }
    }

    #[cfg(test)]
    pub fn trigger_count(&self) -> usize {
        self.trigger_count
    }
}

impl Default for LevelImportSenderRes {
    fn default() -> Self {
        let (sender, _receiver) = mpsc::channel();
        Self::new(sender)
    }
}

/// App-owned sender used to launch the platform audio picker directly from
/// `StateSchedule`. The matching receiver is drained by `AudioImportChannelRes`.
#[derive(Resource)]
pub struct AudioImportSenderRes {
    sender: mpsc::Sender<(String, Vec<u8>)>,
    #[cfg(test)]
    trigger_count: usize,
}

impl AudioImportSenderRes {
    pub fn new(sender: mpsc::Sender<(String, Vec<u8>)>) -> Self {
        Self {
            sender,
            #[cfg(test)]
            trigger_count: 0,
        }
    }

    pub fn trigger_picker(&mut self) {
        #[cfg(not(test))]
        eg_platform::services::trigger_audio_import(self.sender.clone());
        #[cfg(test)]
        {
            let _sender = self.sender.clone();
            self.trigger_count += 1;
        }
    }

    #[cfg(test)]
    pub fn trigger_count(&self) -> usize {
        self.trigger_count
    }
}

impl Default for AudioImportSenderRes {
    fn default() -> Self {
        let (sender, _receiver) = mpsc::channel();
        Self::new(sender)
    }
}
