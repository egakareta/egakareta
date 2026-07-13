/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS resource definitions for eg-state.
//!
//! These resources are the canonical mirror of the per-subsystem state that
//! lives on the legacy `State` struct (see [`crate::state::State`]). During
//! the Phase 2 migration the legacy fields are kept authoritative and
//! the resources are written from those fields each frame; in later phases
//! the resources will become authoritative and the legacy fields will be
//! removed.
//!
//! Naming convention: `<Domain>Res` for top-level domain resources, and
//! `<Domain><Noun>Res` for grouped sub-state.
//!
//! Only types with a usable `Default` impl are exposed in Phase 2a. Types
//! that wrap non-`Default` channel pairs (audio, auth) are added in
//! Phase 2b alongside their resource registration helpers.

use bevy_ecs::prelude::*;
use eg_platform::services::AuthServiceMessage;
use eg_types::{
    AppPhase, AppSettings, AuthSession, BasicLevelMetadata, LevelObject,
    LevelPreviewCameraMetadata, MenuState,
};
use std::sync::mpsc;

use crate::state::PracticeCheckpoint;
use crate::state::editor_camera::EditorCameraState;
use crate::state::editor_config_state::EditorConfigState;
use crate::state::editor_timeline::EditorTimelineState;
use crate::state::editor_timing::EditorTimingState;
use crate::state::editor_triggers::EditorTriggerState;
use crate::state::history::EditorHistoryState;
use crate::state::perf::PerfState;
use crate::state::runtime::EditorRuntimeState;

use eg_types::SpawnMetadata;

// ---------------------------------------------------------------------------
// AppPhase (Bevy States target)
// ---------------------------------------------------------------------------

/// Wrapper resource that holds the current `AppPhase`. Used as a Bevy
/// `States` derive target so transition systems (`OnEnter(Playing)` etc.)
/// can react to phase changes. The legacy `State::phase` field mirrors
/// this resource; updates flow both ways during Phase 2.
///
/// `previous_app_phase` is the value the resource held on the previous
/// frame. The transition system in [`crate::state::transitions`] compares
/// the current and previous values to detect phase changes and emit
/// `OnEnter` / `OnExit` side effects. The default value is `Menu` on
/// both fields so the first frame is a no-op.
///
/// REFACTOR POINT(full-bevy): `AppPhase` is already a `Copy + Eq + Hash`
/// enum, so the full-Bevy migration just adds the `States` derive directly
/// in `eg-types` and drops this wrapper (and the `previous_app_phase`
/// bookkeeping).
#[derive(Resource, Debug, Clone, Copy)]
pub struct AppPhaseRes {
    pub current: AppPhase,
    pub previous_app_phase: AppPhase,
}

impl AppPhaseRes {
    pub const fn new(phase: AppPhase) -> Self {
        Self {
            current: phase,
            previous_app_phase: phase,
        }
    }

    pub const fn get(&self) -> AppPhase {
        self.current
    }

    pub fn set(&mut self, phase: AppPhase) {
        self.current = phase;
    }
}

impl Default for AppPhaseRes {
    fn default() -> Self {
        Self::new(AppPhase::Menu)
    }
}

// ---------------------------------------------------------------------------
// Frame timing
// ---------------------------------------------------------------------------

/// Manual frame timing resource used while `State::update` is being
/// decomposed into scheduled systems.
///
/// REFACTOR POINT(full-bevy): replace `FrameTimeRes` with `bevy_time`
/// `Time<Virtual>` / `Time<Fixed>` resources once the custom frame
/// driver moves to the full Bevy app runner.
#[derive(Resource, Clone, Copy)]
pub struct FrameTimeRes {
    pub last_frame: web_time::Instant,
    pub delta_seconds: f32,
    pub accumulator: f32,
    pub global_time_seconds: f32,
    pub fps_smoothed: f32,
}

impl FrameTimeRes {
    pub fn advance(&mut self) {
        let now = web_time::Instant::now();
        // Treat a stalled/background frame like the accumulator clamp below
        // does: one frame may advance at most 250ms of simulation/presentation
        // time. This keeps camera transitions and fixed-step consumers from
        // completing an entire transition after a long compile/debug pause.
        let frame_dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.25);
        self.last_frame = now;
        self.delta_seconds = frame_dt;

        let instant_fps = 1.0 / frame_dt.max(1e-4);
        if self.fps_smoothed <= 0.0 {
            self.fps_smoothed = instant_fps;
        } else {
            self.fps_smoothed = self.fps_smoothed * 0.9 + instant_fps * 0.1;
        }

        self.accumulator = (self.accumulator + frame_dt).min(0.25);
        self.global_time_seconds += frame_dt.max(0.0);
    }
}

impl Default for FrameTimeRes {
    fn default() -> Self {
        Self {
            last_frame: web_time::Instant::now(),
            delta_seconds: 0.0,
            accumulator: 0.0,
            global_time_seconds: 0.0,
            fps_smoothed: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Session / menu / editor domain resources (default-constructible only)
// ---------------------------------------------------------------------------

/// `SessionSubsystem` fields mirrored as a resource. Only the
/// default-constructible subset is included in Phase 2a; the full
/// `SessionSubsystem` mirror (with `LevelImportChannelRes`, practice
/// checkpoints, etc.) lands in Phase 2b.
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
    pub practice_mode_enabled: bool,
    pub practice_checkpoints: Vec<PracticeCheckpoint>,
    /// One-shot compatibility marker for rebuilding the legacy checkpoint mesh
    /// until presentation reads the App-world checkpoint list directly.
    pub practice_checkpoints_dirty: bool,
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
        self.practice_mode_enabled = false;
        self.practice_checkpoints.clear();
        self.practice_checkpoints_dirty = true;
        self.audio_stop_pending = true;
    }
}

/// Tracks the temporary ownership handoff for the active gameplay world.
///
/// When `app_authoritative` is true, `eg_game::GameplaySchedule` advances
/// player/object ECS state and the legacy `GameState` receives only a render
/// compatibility copy. Lifecycle methods that still run on `State` set
/// `needs_legacy_seed` so the next frame can reinitialize the App world at the
/// transition boundary.
#[derive(Resource, Default)]
pub struct GameplayWorldRes {
    pub app_authoritative: bool,
    pub needs_legacy_seed: bool,
}

/// App-world marker for a real gameplay restart. The navigation message
/// records intent here and the gameplay-world system applies the entity and
/// resource reset before the next gameplay tick; it is not a Runtime/State
/// request bridge.
#[derive(Resource, Default)]
pub struct GameplayResetRes {
    pub pending: bool,
}

impl GameplayResetRes {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// `MenuState` mirrored as a resource so UI systems (Phase 5) can read
/// the menu without going through `State::ui_state_snapshot`. `MenuState`
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

/// `EditorInputState` mirrored as a resource. UI + interaction systems
/// (Phase 3+) read/write this resource instead of `state.editor.input`.
#[derive(Resource, Default)]
pub struct EditorInputRes(pub eg_types::EditorInputState);

/// `EditorSubsystem.objects` mirrored as a resource. This is the ordered
/// authored-object list used by editor export, selection, history, and render
/// mesh rebuilds while object ownership is migrating into the App world.
#[derive(Resource, Default, Clone)]
pub struct EditorObjectsRes {
    pub objects: Vec<LevelObject>,
    pub history_restore_pending: bool,
    pub object_mutation_pending: bool,
}

/// `EditorConfigState` mirrored as a resource.
#[derive(Resource, Default)]
pub struct EditorConfigRes(pub EditorConfigState);

/// `EditorCameraState` mirrored as a resource.
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

/// `EditorSubsystem.spawn: SpawnMetadata` mirrored as a resource.
/// Default value is `position = [0, 0, 0], direction = Forward`,
/// matching the in-place default that `EditorSubsystem` provides
/// when constructed through `Default::default()`. The legacy field
/// is seeded from this resource at `Runtime::new` via the
/// `set_editor_spawn_state` setter, and the resource is the
/// authoritative value of the editor spawn state during the
/// Phase 3.10 compatibility window.
///
/// Phase 3.10 carve-out: the `SetSpawnHere` and
/// `RotateSpawnDirection` messages in `events::spawn` write to
/// this resource directly instead of staging a legacy
/// `EditorCommand`. The `Runtime::sync_app_editor_spawn_state`
/// helper propagates the resource back to the legacy
/// `State.editor.spawn` field with a change guard, so the legacy
/// `editor_set_spawn_here` / `editor_rotate_spawn_direction` paths
/// remain reachable from non-message call sites.
#[derive(Resource, Default)]
pub struct EditorSpawnRes(pub SpawnMetadata);

/// `EditorRuntimeState` (dirty flags, history, interaction, gizmo) mirrored
/// as a resource. Only the default-constructible subset is exposed for
/// Phase 2a; the `FrameRuntimeState` mirror (which contains a
/// `web_time::Instant`) is added in Phase 2b.
#[derive(Resource, Default)]
pub struct EditorRuntimeRes(pub EditorRuntimeState);

/// `EditorHistoryState` separated out so it can be cloned independently
/// (and the history snapshot system in Phase 3c can lock it for the frame).
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

/// App-world request to run the remaining legacy `Escape` state machine.
///
/// The Escape keypress is a multi-step context-sensitive state machine
/// (toggle game pause → back to menu → clear selection → toggle timeline
/// playback → reset timeline time → back to menu). The editor
/// real-gameplay pause, editor selection-clearing, playback-stop, and
/// timeline-reset branches are App-world direct; back-to-menu/phase
/// fallback branches still route through this request resource so Runtime
/// can call `State::apply_editor_escape` during the compatibility window.
#[derive(Resource, Default)]
pub struct EscapeRequest {
    pub pending: bool,
}

impl EscapeRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world auth state and service sender. Phase 6e moves auth request
/// consumption and auth result application into `StateSchedule`; Runtime mirrors
/// this resource back to legacy `State` while UI still reads the compatibility
/// shell.
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

/// App-world request to quit the current real gameplay level from the
/// pause menu and return to the menu phase.
///
/// The quit side effect calls `State::back_to_menu`, which stops
/// audio, resets the gameplay state, and transitions to the menu
/// phase. The side effect stays legacy-owned for the carve-out.
/// Phase 3.48 routes the message through this request resource
/// instead of staging `AppCommand::GameQuitToMenu` in
/// legacy command bridge.
#[derive(Resource, Default)]
pub struct GameQuitToMenuRequest {
    pub pending: bool,
}

impl GameQuitToMenuRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to restart the current real gameplay level from
/// the pause menu.
///
/// The restart side effect calls `State::restart_level`, which
/// resets the gameplay clock, replayed objects, and audio. The
/// side effect stays legacy-owned for the carve-out. Phase 3.48
/// routes the message through this request resource instead of
/// staging `AppCommand::GameRestartLevel` through the legacy command bridge.
#[derive(Resource, Default)]
pub struct GameRestartLevelRequest {
    pub pending: bool,
}

impl GameRestartLevelRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to start editor playtest mode.
///
/// The playtest transition still builds gameplay state, primes audio,
/// and changes phase through legacy `State::editor_playtest`, so the
/// side effect stays legacy-owned while the App world owns the user
/// intent.
#[derive(Resource, Default)]
pub struct PlaytestRequest {
    pub pending: bool,
}

impl PlaytestRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to load a built-in level into the editor.
#[derive(Resource, Default)]
pub struct LoadLevelRequest {
    pub level_name: Option<String>,
}

impl LoadLevelRequest {
    pub fn request(&mut self, level_name: String) {
        self.level_name = Some(level_name);
    }

    pub fn take(&mut self) -> Option<String> {
        self.level_name.take()
    }
}

/// App-world request to export the current editor level.
#[derive(Resource, Default)]
pub struct ExportLevelRequest {
    pub pending: bool,
}

impl ExportLevelRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to switch the preferred audio backend.
#[derive(Resource, Default)]
pub struct SetAudioBackendRequest {
    pub backend_name: Option<String>,
}

impl SetAudioBackendRequest {
    pub fn request(&mut self, backend_name: String) {
        self.backend_name = Some(backend_name);
    }

    pub fn take(&mut self) -> Option<String> {
        self.backend_name.take()
    }
}

/// App-world request to complete a pending level import.
#[derive(Resource, Default)]
pub struct CompleteImportRequest {
    pub pending: bool,
}

impl CompleteImportRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to open the platform audio-import picker.
#[derive(Resource, Default)]
pub struct TriggerAudioImportRequest {
    pub pending: bool,
}

impl TriggerAudioImportRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request for non-editor primary start/turn behavior.
///
/// Editor placement is handled directly through App-world editor resources.
#[derive(Resource, Default)]
pub struct TurnRightRequest {
    pub pending: bool,
}

impl TurnRightRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}

/// App-world request to toggle between menu/editor/playtest editor.
#[derive(Resource, Default)]
pub struct ToggleEditorRequest {
    pub pending: bool,
}

impl ToggleEditorRequest {
    pub fn request(&mut self) {
        self.pending = true;
    }

    pub fn take(&mut self) -> bool {
        let pending = self.pending;
        self.pending = false;
        pending
    }
}
