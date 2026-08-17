/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Plugin shells for the eg-state `App`.
//!
//! Each plugin owns the resources and systems for one logical domain. The
//! plugin graph establishes the App-world ownership boundaries; schedule
//! systems consume those resources directly.
//!
//! Plugin graph:
//! - [`GameplayPlugin`] — wraps `eg_game::GamePlugin` + owns
//!   gameplay-related resources (`SessionStateRes`-style
//!   fields that affect gameplay).
//! - [`EditorPlugin`] — owns the `Editor*Res` family (input, config,
//!   camera, triggers, timeline, timing, runtime, history, perf).
//! - [`SessionPlugin`] — owns `SessionStateRes` (level/music/creator
//!   metadata, settings, practice mode).
//! - [`MenuPlugin`] — owns `MenuStateRes`; top-level navigation uses Bevy's
//!   `State<AppPhase>` resource.
//! - [`AudioPlugin`] — owns the custom rodio/cpal boundary and its
//!   App-world resources.
//! - [`StateDomainPlugins`] — aggregator that registers all eg-state domains
//!   once Bevy's core plugins have been installed.
//! - [`StatePlugins`] — headless/test composition that also installs the
//!   required Bevy time and state plugins.
//!
//! Production uses `build_bevy_app()` with `DefaultPlugins` and the
//! `EgakaretaPlugin` composition. These plugin shells remain for the
//! headless `StatePlugins` test composition only.

use bevy::{
    app::{App, First, FixedUpdate, Plugin, PostUpdate, PreUpdate, Startup, Update},
    ecs::prelude::*,
    input::InputSystems,
    state::{app::AppExtStates, prelude::State},
    time::TimePlugin,
};

use super::{
    AudioBackendRes, AudioBackendSystemParam, EditorAudioCacheRes, RuntimeAudioPreloadRes,
    poll_persistence_tasks_system, poll_waveform_load_tasks_system,
    queue_pending_level_exports_system,
    render::plugin::RenderPlugin,
    state_resources::{
        AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes, AuthSubmitSignInRequest,
        BlockEffectsRes, BuiltinLevelCatalogRes, EditorCameraRes, EditorConfigRes,
        EditorHistoryRes, EditorInputRes, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes,
        EditorTimingRes, EditorTriggerRes, FrameTimeRes, GameplayResetRes, LevelExportServiceRes,
        MenuPreviewObjectsRes, PerfFrameCopyRequest, PerfRes, SessionStateRes,
    },
};

/// Registers the headless gameplay systems. Currently a thin shim around
/// [`eg_game::GamePlugin`]; in later phases the gameplay plugin will
/// also add `SessionStateRes`-style resources and domain-specific event
/// handlers.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(eg_game::GamePlugin).add_systems(
            Startup,
            super::gameplay_world::seed_default_gameplay_world_system,
        );
        app.configure_sets(
            FixedUpdate,
            (
                eg_game::systems::GameplaySet::TurnInput,
                eg_game::systems::GameplaySet::AdvancePlayer,
                eg_game::systems::GameplaySet::Collision,
                eg_game::systems::GameplaySet::ConsumeObjects,
                eg_game::systems::GameplaySet::GroundGravity,
                eg_game::systems::GameplaySet::Trail,
                eg_game::systems::GameplaySet::Completion,
                eg_game::systems::GameplaySet::Progress,
            )
                .chain()
                .run_if(super::gameplay_world::gameplay_is_active),
        );
    }
}

/// Registers the editor-domain resources. Editor mutation systems consume
/// these App-owned resources through Bevy's standard gameplay and render
/// schedules.
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInputRes>()
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
            .init_resource::<LevelExportServiceRes>();
        super::input_bridge::register_platform_input_resources(app.world_mut());
    }
}

/// Owns the custom rodio/cpal service boundary until Bevy audio replaces it.
///
/// Keeping the backend and its playback side effects in one plugin prevents
/// editor and session setup from implicitly owning native-only audio state.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorAudioCacheRes>()
            .init_resource::<RuntimeAudioPreloadRes>();
        #[cfg(not(target_arch = "wasm32"))]
        app.init_resource::<AudioBackendRes>();
        #[cfg(target_arch = "wasm32")]
        app.init_non_send::<AudioBackendRes>();
        app.add_systems(
            Update,
            consume_audio_stop_request_system.after(crate::events::CommandHandlerSet),
        )
        .add_systems(
            Update,
            sync_gameplay_audio_pause_system.after(consume_audio_stop_request_system),
        )
        .add_systems(
            Update,
            super::audio_state::poll_runtime_audio_preload_tasks_system
                .after(crate::events::CommandHandlerSet),
        )
        .add_systems(
            Update,
            poll_waveform_load_tasks_system
                .after(crate::events::CommandHandlerSet)
                .before(crate::state::async_bridge::ApplyAsyncResultsSet),
        )
        .add_systems(
            Update,
            (
                queue_pending_level_exports_system,
                poll_persistence_tasks_system,
            )
                .chain()
                .after(crate::events::CommandHandlerSet),
        )
        .add_systems(
            Update,
            (
                super::file_import_tasks::poll_level_import_tasks_system,
                super::file_import_tasks::poll_audio_import_tasks_system,
            )
                .after(crate::events::CommandHandlerSet)
                .before(crate::state::async_bridge::ApplyAsyncResultsSet),
        );
    }
}

/// Registers session-related resources (level/music/creator metadata,
/// settings, practice mode).
pub struct SessionPlugin;

fn sync_gameplay_audio_pause_system(
    phase: Res<State<eg_types::AppPhase>>,
    session: Res<SessionStateRes>,
    audio: AudioBackendSystemParam<'_>,
    mut pending_turn_inputs: ResMut<eg_game::PendingTurnInputs>,
) {
    if !session.is_changed()
        || *phase.get() != eg_types::AppPhase::Playing
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
    audio: AudioBackendSystemParam<'_>,
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

/// Registers menu resources and preview/render defaults.
///
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuPreviewObjectsRes>()
            .init_resource::<BlockEffectsRes>()
            .init_resource::<super::MenuStateRes>();
    }
}

/// Aggregates every eg-state domain after Bevy's core plugins are present.
///
/// Production adds this after `DefaultPlugins`; headless fixtures use
/// [`StatePlugins`], which supplies the small core-plugin baseline itself.
/// Keeping the domain graph separate prevents the root app from registering
/// Bevy's unique time and state plugins twice.
pub struct StateDomainPlugins;

impl Plugin for StateDomainPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MenuPlugin,
            SessionPlugin,
            EditorPlugin,
            AudioPlugin,
            GameplayPlugin,
            // Rendering extraction plugin. Bevy's render sub-app owns the
            // GPU and surface; the domain world derives retained entities.
            RenderPlugin,
        ))
        .init_state::<eg_types::AppPhase>();
        super::transitions::register_phase_transition_hooks(app);
        configure_standard_state_schedules(app);
    }
}

/// Complete headless/test eg-state composition.
///
/// This owns the foundational plugins only when the caller has not already
/// installed `DefaultPlugins` or `MinimalPlugins`. The windowed Bevy root
/// instead adds [`StateDomainPlugins`] directly after `DefaultPlugins`.
pub struct StatePlugins;

impl Plugin for StatePlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            TimePlugin,
            bevy::state::app::StatesPlugin,
            StateDomainPlugins,
        ));
    }
}

/// Install the production eg-state pipeline in Bevy's standard schedules.
///
/// The old labelled schedules remain available only for focused compatibility
/// fixtures while those tests are migrated. The runtime itself never drives
/// them: input is ingested in `PreUpdate`, domain work in `Update`, and the
/// derived render data/present bridge in `PostUpdate`.
fn configure_standard_state_schedules(app: &mut App) {
    crate::events::register_command_events(app);
    crate::state::async_bridge::register_async_result_messages(app);

    app.configure_sets(
        Update,
        (
            crate::events::CommandHandlerSet,
            crate::state::async_bridge::ApplyAsyncResultsSet,
        )
            .chain(),
    )
    .configure_sets(
        PreUpdate,
        (
            super::input_bridge::BevyInputBridgeSet::Capture,
            super::input_bridge::BevyInputBridgeSet::Translate
                .after(super::input_bridge::BevyInputBridgeSet::Capture),
        ),
    )
    .add_systems(First, crate::state::frame_time::begin_puffin_frame_system)
    .add_systems(
        PreUpdate,
        (
            super::input_bridge::drain_bevy_keyboard_input_system
                .after(InputSystems)
                .in_set(super::input_bridge::BevyInputBridgeSet::Translate),
            super::input_bridge::drain_bevy_pointer_input_system
                .after(InputSystems)
                .in_set(super::input_bridge::BevyInputBridgeSet::Translate),
            super::input_bridge::drain_bevy_mouse_motion_and_wheel_system
                .after(InputSystems)
                .in_set(super::input_bridge::BevyInputBridgeSet::Translate),
            super::input_bridge::drain_bevy_touch_input_system
                .after(InputSystems)
                .in_set(super::input_bridge::BevyInputBridgeSet::Translate),
            super::input_bridge::clear_held_input_on_bevy_focus_loss_system
                .after(InputSystems)
                .in_set(super::input_bridge::BevyInputBridgeSet::Translate),
        ),
    );
    let update = app
        .get_schedule_mut(Update)
        .expect("Bevy App must provide the Update schedule");
    update.add_systems(crate::state::frame_time::advance_frame_time_system);
    crate::events::add_command_systems(update);
    update.add_systems(bevy::ecs::schedule::ApplyDeferred.after(crate::events::CommandHandlerSet));
    update.add_systems(
        crate::state::editor_playback::update_editor_timeline_playback_system
            .after(crate::events::CommandHandlerSet),
    );
    update.add_systems(
        crate::state::editor_timeline_cache::rebuild_editor_timeline_snapshot_cache_system
            .after(crate::state::editor_playback::update_editor_timeline_playback_system),
    );
    update.add_systems(
        crate::state::menu_preview::refresh_menu_preview_system
            .after(crate::events::CommandHandlerSet),
    );
    update.add_systems(
        crate::state::visual_effects::update_block_effects_system
            .after(crate::state::editor_playback::update_editor_timeline_playback_system),
    );
    update.add_systems(
        super::gameplay_world::apply_pending_gameplay_reset_system
            .after(crate::events::navigation::handle_turn_right)
            .after(crate::events::navigation::handle_game_restart_level)
            .after(crate::events::navigation::handle_game_set_practice_mode)
            .after(crate::events::timeline::handle_playtest),
    );
    update.add_systems(
        crate::state::auth_state::process_auth_requests_system
            .after(crate::events::auth::handle_auth_submit_sign_in)
            .after(crate::events::auth::handle_auth_sign_out)
            .after(crate::events::auth::handle_auth_open_signup)
            .before(crate::state::async_bridge::ApplyAsyncResultsSet),
    );
    update.add_systems(
        crate::state::auth_state::poll_auth_tasks_system
            .after(crate::state::auth_state::process_auth_requests_system)
            .before(crate::state::async_bridge::ApplyAsyncResultsSet),
    );
    update.add_systems(
        crate::state::editor_camera::update_editor_camera_pan_system
            .after(crate::events::CommandHandlerSet),
    );
    update.add_systems(
        crate::state::perf::process_perf_frame_copy_request_system
            .after(crate::events::auth::handle_copy_latest_perf_frame)
            .before(crate::state::async_bridge::ApplyAsyncResultsSet),
    );
    update.add_systems(
        poll_waveform_load_tasks_system
            .after(crate::events::CommandHandlerSet)
            .before(crate::state::async_bridge::ApplyAsyncResultsSet),
    );
    crate::state::async_bridge::add_apply_async_results_systems(update);
    crate::state::render::plugin::add_state_render_systems(update);

    app.add_systems(
        PostUpdate,
        (
            apply_requested_phase_transition_system,
            super::gameplay_world::apply_completed_gameplay_transition_system,
            super::gameplay_world::play_death_sfx_system,
            super::gameplay_world::apply_game_over_transition_system,
            apply_terminal_phase_transition_system,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
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
        )
            .chain(),
    );
}

/// Apply transition requests emitted during `Update` before fixed gameplay.
///
/// `StatesPlugin` applies transitions at its normal frame boundary. The game
/// additionally needs commands such as Play and ToggleEditor to become
/// visible to the same frame's fixed simulation and presentation, so this is
/// the one explicit post-update transition boundary.
fn apply_requested_phase_transition_system(world: &mut World) {
    let _ = world.try_run_schedule(bevy::state::prelude::StateTransition);
}

/// Apply terminal gameplay requests created after fixed simulation.
fn apply_terminal_phase_transition_system(world: &mut World) {
    let _ = world.try_run_schedule(bevy::state::prelude::StateTransition);
}

/// One-shot setup: add all eg-state plugins + register the system set
/// ordering. Use this when constructing a `bevy_app::App` for eg-state
/// (full-ECS harness) or for tests.
///
/// This is the canonical "give me a fully wired eg-state `App`" helper.
/// Individual plugins register their systems on Bevy's standard schedules.
pub fn setup_state_app(app: &mut App) {
    app.add_plugins(StatePlugins);
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
    world.insert_resource(State::new(eg_types::AppPhase::Menu));
    world.init_resource::<bevy::state::prelude::NextState<eg_types::AppPhase>>();
    world.init_resource::<MenuPreviewObjectsRes>();
    world.init_resource::<BlockEffectsRes>();
    world.init_resource::<SessionStateRes>();
    world.init_resource::<EditorInputRes>();
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
    #[cfg(not(target_arch = "wasm32"))]
    world.init_resource::<AudioBackendRes>();
    #[cfg(target_arch = "wasm32")]
    world.init_non_send::<AudioBackendRes>();
    world.init_resource::<EditorAudioCacheRes>();
    world.init_resource::<RuntimeAudioPreloadRes>();
    super::input_bridge::register_platform_input_resources(world);
    // Bare worlds used by focused render fixtures still need the
    // default POD and placeholder mesh/surface resources.
    super::render::plugin::register_render_pod_resources(world);
}
