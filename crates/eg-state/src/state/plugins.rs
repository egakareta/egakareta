/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Plugin shells for the eg-state `App`.
//!
//! Each plugin owns the resources + systems for one logical domain. Today
//! (Phase 2a) the plugins only register the resources from
//! [`super::state_resources`]; the systems that use those resources land
//! in Phase 2c/3+. The plugin skeleton establishes the boundaries so
//! subsystems can be moved into systems one at a time.
//!
//! Plugin graph:
//! - [`GameplayPlugin`] — wraps `eg_game::GamePlugin` + owns
//!   `GameplaySubsystem`-related resources (`SessionStateRes`-style
//!   fields that affect gameplay).
//! - [`EditorPlugin`] — owns the `Editor*Res` family (input, config,
//!   camera, triggers, timeline, timing, runtime, history, perf).
//! - [`SessionPlugin`] — owns `SessionStateRes` (level/music/creator
//!   metadata, settings, practice mode).
//! - [`MenuPlugin`] — owns `MenuStateRes` + `AppPhaseRes` (top-level
//!   navigation state).
//! - [`StatePlugins`] — aggregator that registers all of the above in
//!   dependency order.
//!
//! REFACTOR POINT(full-bevy): the full-Bevy migration promotes these
//! plugin shells to the actual plugin graph (with `RenderPlugin` /
//! `AudioPlugin` / `AuthPlugin` / `UiPlugin` added), the systems they
//! register move into `bevy_render` / `bevy_egui` / `bevy_audio`, and
//! the schedule labels here are replaced by Bevy's standard
//! `Update` / `FixedUpdate` / `OnEnter` schedules.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

use super::StateSchedule;
use super::render::plugin::RenderPlugin;
use super::state_resources::{
    AppPhaseRes, AudioImportSenderRes, AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes,
    AuthSubmitSignInRequest, BlockEffectsRes, BuiltinLevelCatalogRes, EditorCameraRes,
    EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorObjectsRes, EditorRuntimeRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
    GameplayResetRes, LevelExportServiceRes, LevelImportSenderRes, MenuPreviewObjectsRes,
    PerfFrameCopyRequest, PerfRes, SessionStateRes,
};
use super::{
    AudioBackendRes, EditorAudioCacheRes, RuntimeAudioPreloadRes, RuntimeAudioPreloadSenderRes,
    WaveformLoadSenderRes,
};

/// Registers the headless gameplay systems. Currently a thin shim around
/// [`eg_game::GamePlugin`]; in later phases the gameplay plugin will
/// also add `SessionStateRes`-style resources and domain-specific event
/// handlers.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(eg_game::GamePlugin);
    }
}

/// Registers the editor-domain resources. No systems yet — the editor
/// mutation loop stays on `State` until Phase 2c, then moves into systems
/// here. The resources are registered so the resource mirror is
/// observable from the start.
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInputRes>()
            .init_resource::<EditorObjectsRes>()
            .init_resource::<EditorConfigRes>()
            .init_resource::<EditorCameraRes>()
            .init_resource::<EditorSpawnRes>()
            .init_resource::<EditorTriggerRes>()
            .init_resource::<EditorTimelineRes>()
            .init_resource::<EditorTimingRes>()
            .init_resource::<EditorRuntimeRes>()
            .init_resource::<EditorHistoryRes>()
            .init_resource::<FrameTimeRes>()
            .init_resource::<PerfRes>()
            .init_resource::<PerfFrameCopyRequest>()
            .init_resource::<AuthStateRes>()
            .init_resource::<AuthSubmitSignInRequest>()
            .init_resource::<AuthSignOutRequest>()
            .init_resource::<AuthOpenSignupRequest>()
            .init_resource::<GameplayResetRes>()
            .init_resource::<BuiltinLevelCatalogRes>()
            .init_resource::<LevelExportServiceRes>()
            .init_resource::<LevelImportSenderRes>()
            .init_resource::<AudioImportSenderRes>()
            .init_resource::<AudioBackendRes>()
            .init_resource::<EditorAudioCacheRes>()
            .init_resource::<RuntimeAudioPreloadRes>()
            .init_resource::<RuntimeAudioPreloadSenderRes>()
            .init_resource::<WaveformLoadSenderRes>();
        super::input_bridge::register_platform_input_resources(app.world_mut());
    }
}

/// Registers session-related resources (level/music/creator metadata,
/// settings, practice mode).
pub struct SessionPlugin;

fn sync_gameplay_audio_pause_system(
    phase: Res<AppPhaseRes>,
    session: Res<SessionStateRes>,
    audio: Res<AudioBackendRes>,
    mut pending_turn_inputs: ResMut<eg_game::PendingTurnInputs>,
) {
    if !session.is_changed()
        || phase.current != eg_types::AppPhase::Playing
        || session.playtesting_editor
    {
        return;
    }
    if session.game_paused {
        pending_turn_inputs.taps.clear();
        audio.pause();
    } else {
        audio.resume();
    }
}

fn consume_audio_stop_request_system(
    mut session: ResMut<SessionStateRes>,
    audio: Res<AudioBackendRes>,
) {
    if session.audio_stop_pending {
        audio.stop();
        session.audio_stop_pending = false;
    }
}

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionStateRes>();
    }
}

/// Registers menu + app-phase resources. Owns the [`AppPhaseRes`] so
/// the top-level navigation state is the first thing initialized.
///
/// `MenuStateRes` doesn't have a `Default` impl (`MenuState` itself does
/// not derive `Default`), so it must be inserted explicitly by the
/// `State` constructor. [`AppPhaseRes`] is auto-initialized to
/// `AppPhase::Menu`.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppPhaseRes>()
            .init_resource::<MenuPreviewObjectsRes>()
            .init_resource::<BlockEffectsRes>();
        // `MenuStateRes` is added by the `State` constructor because it
        // needs level names from `eg-level`.
    }
}

/// Aggregator plugin. Add this to an `App` to register every eg-state
/// plugin in dependency order. Equivalent to a hand-written
/// `app.add_plugins((MenuPlugin, SessionPlugin, EditorPlugin, GameplayPlugin))`
/// — kept as a single plugin so the order is centralized and the full
/// eg-state `App` is built with one call.
pub struct StatePlugins;

impl Plugin for StatePlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MenuPlugin,
            SessionPlugin,
            EditorPlugin,
            GameplayPlugin,
            // Rendering extraction plugin. Registers the POD
            // resources (RenderUniformsRes, RenderFrameInputRes,
            // RenderResizeRequest) and the system sets (RenderSet).
            // State construction inserts the authoritative heavy
            // resources (RenderGpuRes, RenderMeshesRes,
            // RenderSurfaceHostRes) after wgpu initialization.
            RenderPlugin,
        ));
    }
}

/// One-shot setup: add all eg-state plugins + register the system set
/// ordering. Use this when constructing a `bevy_app::App` for eg-state
/// (full-ECS harness) or for tests.
///
/// In Bevy 0.19, per-schedule `configure_sets(...)` must be called on the
/// `Schedule` itself, not on `App`. The set ordering is therefore
/// configured per-schedule by callers (see [`super::sets::StateSet`] for
/// the per-set semantics). This entry point exists as the canonical
/// "give me a fully wired eg-state `App`" helper and is where
/// `app.add_schedule(StateUpdate::default())` + the schedule's
/// `configure_sets` will be wired in Phase 2c.
pub fn setup_state_app(app: &mut App) {
    app.add_plugins(StatePlugins);
}

/// Drive the eg-state schedule on a World.
///
/// Phase 7 keeps the legacy `State` object as the compatibility owner for
/// rendering and input, but Runtime now controls the frame order explicitly:
/// `app.update()` runs App systems, then Runtime invokes this helper. The
/// `StateSchedule` owns phase-transition detection before command handlers and
/// frame/update systems run.
pub fn run_state_schedule_in_world(world: &mut World) {
    super::input_bridge::drain_platform_input_system(world);
    let _ = world.try_run_schedule(StateSchedule);
    super::gameplay_world::run_app_gameplay_schedule(world);
    super::gameplay_world::apply_completed_gameplay_transition_system(world);
    super::gameplay_world::apply_game_over_transition_system(world);
    let _ = world.try_run_schedule(crate::state::RenderExtractSchedule);
}

/// Register all default-constructible eg-state resources on a `World`.
///
/// Use this helper to bring a bare world up to the same state as
/// [`setup_state_app`] without going through an `App` first. Note: this
/// only handles default-constructible resources.
/// `MenuStateRes` (which needs level names) is not registered here and
/// must be inserted by the caller with [`MenuStateRes::new`].
#[allow(dead_code)]
pub fn register_state_resources(world: &mut World) {
    world.init_resource::<AppPhaseRes>();
    world.init_resource::<MenuPreviewObjectsRes>();
    world.init_resource::<BlockEffectsRes>();
    world.init_resource::<SessionStateRes>();
    world.init_resource::<EditorInputRes>();
    world.init_resource::<EditorObjectsRes>();
    world.init_resource::<EditorConfigRes>();
    world.init_resource::<EditorCameraRes>();
    world.init_resource::<EditorSpawnRes>();
    world.init_resource::<EditorTriggerRes>();
    world.init_resource::<EditorTimelineRes>();
    world.init_resource::<EditorTimingRes>();
    world.init_resource::<EditorRuntimeRes>();
    world.init_resource::<EditorHistoryRes>();
    world.init_resource::<FrameTimeRes>();
    world.init_resource::<PerfRes>();
    world.init_resource::<PerfFrameCopyRequest>();
    world.init_resource::<AuthStateRes>();
    world.init_resource::<AuthSubmitSignInRequest>();
    world.init_resource::<AuthSignOutRequest>();
    world.init_resource::<AuthOpenSignupRequest>();
    world.init_resource::<GameplayResetRes>();
    world.init_resource::<BuiltinLevelCatalogRes>();
    world.init_resource::<LevelExportServiceRes>();
    world.init_resource::<LevelImportSenderRes>();
    world.init_resource::<AudioImportSenderRes>();
    world.init_resource::<AudioBackendRes>();
    world.init_resource::<EditorAudioCacheRes>();
    world.init_resource::<RuntimeAudioPreloadRes>();
    world.init_resource::<RuntimeAudioPreloadSenderRes>();
    world.init_resource::<WaveformLoadSenderRes>();
    super::input_bridge::register_platform_input_resources(world);
    // Phase 4a: rendering extraction plugin's POD resources. The
    // The heavy GPU resource is not default-constructible. Production
    // Runtime construction transfers all heavy resources into the App world;
    // bare test worlds keep only the default mesh/surface placeholders.
    super::render::plugin::register_render_pod_resources(world);
}

/// Register the per-message command-pipeline resources + the
/// [`StateSchedule`] on a `World`.
///
/// Phase 3: alongside the eg-state resources, this also initializes
/// the `Messages<T>` buffers for every event type, then adds the
/// [`StateSchedule`] to the world. The schedule contains per-message
/// handlers that mutate App-world resources directly or write typed request
/// resources for Runtime to consume while legacy side effects still live on
/// `State`.
///
/// Phase 6a: the schedule also contains the
/// `poll_channels_system` from `state::async_bridge` that drains
/// the five legacy `mpsc` receivers (auth, audio import,
/// waveform, audio preload, level import) and emits the
/// corresponding async-result events into the World. The channel
/// wrappers themselves are inserted lazily by
/// `install_async_bridge_from_state` (when the legacy `State`
/// fields are wired into the world).
///
/// Runtime and tests call this for the App-owned world before running
/// [`StateSchedule`].
pub fn register_state_command_pipeline(world: &mut World) {
    // Message resources for every command event type. The
    // `register_command_events` helper takes an `App`; we use the
    // `World`-flavoured helper to keep this function working on a
    // bare world (no `App`).
    crate::events::register_command_events_world(world);

    // Phase 6a: register the async-bridge `Messages<T>` resources
    // (the channel wrappers are inserted lazily by
    // `install_async_bridge_from_state`).
    crate::state::async_bridge::register_state_async_bridge_world(world);

    // Build the per-frame schedule that handles the per-event
    // message resources, updates App-world resources / typed
    // request resources, then runs the render bookkeeping systems,
    // Phase 6a channel poll systems, and Phase 6b result handler systems.
    // The schedule is added to the world's
    // scheduler so runtime and test harnesses can invoke it.
    let mut schedule = bevy_ecs::schedule::Schedule::new(crate::state::StateSchedule);
    schedule.configure_sets(
        (
            crate::events::CommandHandlerSet,
            crate::state::async_bridge::PollChannelsSet,
            crate::state::async_bridge::ApplyAsyncResultsSet,
        )
            .chain(),
    );
    schedule.add_systems(
        (
            crate::state::transitions::phase_transition_schedule_system,
            crate::state::frame_time::advance_frame_time_system,
        )
            .chain(),
    );
    crate::events::add_command_systems(&mut schedule);
    schedule.add_systems(consume_audio_stop_request_system.after(crate::events::CommandHandlerSet));
    schedule.add_systems(sync_gameplay_audio_pause_system.after(consume_audio_stop_request_system));
    schedule.add_systems(
        crate::state::editor_playback::update_editor_timeline_playback_system
            .after(crate::events::CommandHandlerSet),
    );
    schedule.add_systems(
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_system
            .after(crate::state::editor_playback::update_editor_timeline_playback_system),
    );
    schedule.add_systems(
        crate::state::menu_preview::refresh_menu_preview_system
            .after(crate::events::CommandHandlerSet),
    );
    schedule.add_systems(
        crate::state::visual_effects::update_block_effects_system
            .after(crate::state::editor_playback::update_editor_timeline_playback_system),
    );
    schedule.add_systems(
        super::gameplay_world::apply_pending_gameplay_reset_system
            .after(crate::events::navigation::handle_turn_right)
            .after(crate::events::navigation::handle_game_restart_level)
            .after(crate::events::navigation::handle_game_set_practice_mode)
            .after(crate::events::timeline::handle_playtest),
    );
    schedule.add_systems(
        crate::state::auth_state::process_auth_requests_system
            .after(crate::events::auth::handle_auth_submit_sign_in)
            .after(crate::events::auth::handle_auth_sign_out)
            .after(crate::events::auth::handle_auth_open_signup)
            .before(crate::state::async_bridge::PollChannelsSet),
    );
    schedule.add_systems(
        crate::state::editor_camera::update_editor_camera_pan_system
            .after(crate::events::CommandHandlerSet),
    );
    schedule.add_systems(
        crate::state::perf::process_perf_frame_copy_request_system
            .after(crate::events::auth::handle_copy_latest_perf_frame)
            .before(crate::state::async_bridge::PollChannelsSet),
    );
    crate::state::async_bridge::add_poll_channels_systems(&mut schedule);
    crate::state::async_bridge::add_apply_async_results_systems(&mut schedule);
    crate::state::render::plugin::add_state_render_systems(&mut schedule);
    world.add_schedule(schedule);

    let mut render_extract_schedule =
        bevy_ecs::schedule::Schedule::new(crate::state::RenderExtractSchedule);
    render_extract_schedule.add_systems(
        (
            crate::state::render::plugin::extract_trail_vertices_system,
            crate::state::render::plugin::extract_block_effect_vertices_system,
            crate::state::render::plugin::extract_practice_checkpoint_vertices_system,
            crate::state::render::plugin::extract_playing_block_meshes_system,
            crate::state::render::plugin::extract_menu_preview_block_mesh_system,
            crate::state::render::plugin::extract_editor_block_meshes_system,
            crate::state::render::plugin::extract_editor_spawn_marker_vertices_system,
            crate::state::render::plugin::extract_editor_camera_arrow_vertices_system,
            crate::state::render::plugin::extract_editor_cursor_vertices_system,
            crate::state::render::plugin::extract_editor_hitbox_vertices_system,
            crate::state::render::plugin::extract_editor_tap_indicator_vertices_system,
            crate::state::render::plugin::extract_editor_selection_overlays_system,
            crate::state::render::plugin::extract_editor_transform_trigger_marker_vertices_system,
            crate::state::render::plugin::update_render_frame_input_system,
            crate::state::render::plugin::update_render_uniforms_system,
            crate::state::render::plugin::extract_editor_gizmo_vertices_system
                .after(crate::state::render::plugin::update_render_uniforms_system),
            crate::state::render::plugin::upload_render_uniforms_system
                .after(crate::state::render::plugin::update_render_uniforms_system),
            crate::state::render::plugin::clear_editor_dirty_flags_system,
        )
            .chain(),
    );
    render_extract_schedule.add_systems(
        crate::state::render::plugin::upload_extracted_render_meshes_system
            .after(crate::state::render::plugin::clear_editor_dirty_flags_system)
            .in_set(crate::state::render::plugin::RenderSet::Present),
    );
    render_extract_schedule.add_systems(
        crate::state::render::plugin::present_render_frame_system
            .after(crate::state::render::plugin::upload_extracted_render_meshes_system)
            .in_set(crate::state::render::plugin::RenderSet::Present),
    );
    world.add_schedule(render_extract_schedule);
}
