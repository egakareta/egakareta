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
use eg_types::{AppPhase, AppSettings, BasicLevelMetadata, LevelPreviewCameraMetadata, MenuState};

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
    pub practice_mode_enabled: bool,
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
