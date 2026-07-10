/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Navigation / phase events. One per `AppCommand` variant under
//! `AppCommand::Navigation / Phase` in the legacy enum.
//!
//! `NextLevel`/`PrevLevel` in the menu, `GameResume`, and
//! `GameSetPracticeMode` write App-world resources or typed requests directly
//! (same pattern as `events::snap`, `events::session`, ...).
//! The next/previous level handlers mutate `MenuStateRes` directly when
//! the App phase is `Menu` and `EditorInputRes.cursor` directly when the
//! App phase is `Editor`. The game pause/practice handlers mutate
//! `SessionStateRes` directly, and the legacy `State.session` fields are
//! synced from the resource in the runtime bridge.
//!
//! Multi-step navigation messages write request resources that Runtime
//! consumes through the legacy compatibility methods while gameplay,
//! phase, and audio ownership remain on `State`.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use eg_blocks::BlockDefinition;
use eg_editor_domain::create_block_at_cursor;
use eg_types::{AppPhase, EditorMode};

use crate::state::{
    AppPhaseRes, EditorConfigRes, EditorHistoryRes, EditorHistorySnapshot, EditorInputRes,
    EditorObjectsRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes,
    GameQuitToMenuRequest, GameRemovePracticeCheckpointRequest, GameRestartLevelRequest,
    GameSetPracticeCheckpointRequest, MenuStateRes, SessionStateRes, ToggleEditorRequest,
    TurnRightRequest,
};

/// Primary action: in editor this places a block directly in App-world
/// resources; in game-over it returns to menu by mutating `AppPhaseRes`.
/// Other phases still request the legacy start/turn side effect.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TurnRight;

/// Select the next level in the menu or move editor cursor +X.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct NextLevel;

/// Select the previous level in the menu or move editor cursor −X.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct PrevLevel;

/// Toggle between editor and menu/playing.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleEditor;

/// Resume real gameplay from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameResume;

/// Restart the current real gameplay level from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameRestartLevel;

/// Enable or disable practice mode for the current real gameplay level.
#[derive(Message, Debug, Clone, Copy)]
pub struct GameSetPracticeMode {
    pub enabled: bool,
}

/// Place a practice-mode checkpoint at the current gameplay position.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameSetPracticeCheckpoint;

/// Remove the latest practice-mode checkpoint.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameRemovePracticeCheckpoint;

/// Quit the current real gameplay level from the pause menu.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct GameQuitToMenu;

pub fn register_events(app: &mut App) {
    app.add_message::<TurnRight>()
        .add_message::<NextLevel>()
        .add_message::<PrevLevel>()
        .add_message::<ToggleEditor>()
        .add_message::<GameResume>()
        .add_message::<GameRestartLevel>()
        .add_message::<GameSetPracticeMode>()
        .add_message::<GameSetPracticeCheckpoint>()
        .add_message::<GameRemovePracticeCheckpoint>()
        .add_message::<GameQuitToMenu>();
}

// -----------------------------------------------------------------------
// Dispatch systems. Request-backed branches write App-world request resources;
// direct carve-outs mutate App-world resources directly.
// -----------------------------------------------------------------------

pub fn handle_turn_right(mut events: MessageReader<TurnRight>, mut resources: TurnRightResources) {
    for _ in events.read() {
        match resources.phase.current {
            AppPhase::Editor => place_editor_block_from_turn_right(&mut resources),
            AppPhase::GameOver => resources.phase.set(AppPhase::Menu),
            _ => resources.request.request(),
        }
    }
}

#[derive(SystemParam)]
pub struct TurnRightResources<'w> {
    phase: ResMut<'w, AppPhaseRes>,
    objects: ResMut<'w, EditorObjectsRes>,
    input: ResMut<'w, EditorInputRes>,
    config: Res<'w, EditorConfigRes>,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: Res<'w, EditorTriggerRes>,
    history: ResMut<'w, EditorHistoryRes>,
    request: ResMut<'w, TurnRightRequest>,
}

fn place_editor_block_from_turn_right(resources: &mut TurnRightResources<'_>) {
    record_editor_history_for_placement(resources);

    let default_size = BlockDefinition::get(&resources.config.0.selected_block_id)
        .render
        .default_size;
    let placed = create_block_at_cursor(
        resources.input.0.cursor,
        &resources.config.0.selected_block_id,
        default_size,
        resources.config.0.selected_block_rotation_degrees,
    )
    .with_generated_id(&resources.objects.objects);

    let placed_index = resources.objects.objects.len();
    resources.objects.objects.push(placed);

    if resources.input.0.shift_held {
        resources.input.0.selected_block_index = None;
        resources.input.0.selected_block_indices.clear();
        resources.input.0.hovered_block_index = None;
    } else {
        resources.input.0.selected_block_index = Some(placed_index);
        resources.input.0.selected_block_indices = vec![placed_index];
        resources.input.0.hovered_block_index = Some(placed_index);
        resources.input.0.mode = EditorMode::Scale;
    }

    resources.objects.object_mutation_pending = true;
}

fn record_editor_history_for_placement(resources: &mut TurnRightResources<'_>) {
    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    resources.history.0.undo.push(EditorHistorySnapshot {
        objects: resources.objects.objects.clone(),
        selected_block_index: resources.input.0.selected_block_index,
        selected_block_indices: resources.input.0.selected_block_indices.clone(),
        cursor: resources.input.0.cursor,
        selected_block_id: resources.config.0.selected_block_id.clone(),
        selected_block_rotation_degrees: resources.config.0.selected_block_rotation_degrees,
        recent_block_ids: resources.config.0.recent_block_ids.clone(),
        spawn: resources.spawn.0.clone(),
        timeline_time_seconds: resources.timeline.0.clock.time_seconds,
        timeline_duration_seconds: resources.timeline.0.clock.duration_seconds,
        tap_times: resources.timeline.0.taps.tap_times.clone(),
        tap_indicator_positions: resources.timeline.0.taps.tap_indicator_positions.clone(),
        selected_tap_index: resources.timeline.0.taps.selected_index,
        timing_points: resources.timing.0.timing_points.clone(),
        selected_trigger_index: resources.trigger.0.selected_index,
    });
    resources.history.0.redo.clear();
}

pub fn handle_next_level(
    mut events: MessageReader<NextLevel>,
    phase: Res<AppPhaseRes>,
    config: Res<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut menu: Option<ResMut<MenuStateRes>>,
) {
    for _ in events.read() {
        match phase.current {
            AppPhase::Menu => {
                if let Some(menu) = menu.as_deref_mut() {
                    let level_count = menu.0.levels.len();
                    if level_count > 0 {
                        menu.0.selected_level = (menu.0.selected_level + 1) % level_count;
                    }
                }
            }
            AppPhase::Editor => move_editor_cursor_x(&mut input, &config, 1.0),
            _ => {}
        }
    }
}

pub fn handle_prev_level(
    mut events: MessageReader<PrevLevel>,
    phase: Res<AppPhaseRes>,
    config: Res<EditorConfigRes>,
    mut input: ResMut<EditorInputRes>,
    mut menu: Option<ResMut<MenuStateRes>>,
) {
    for _ in events.read() {
        match phase.current {
            AppPhase::Menu => {
                if let Some(menu) = menu.as_deref_mut() {
                    let level_count = menu.0.levels.len();
                    if level_count > 0 {
                        menu.0.selected_level = if menu.0.selected_level == 0 {
                            level_count - 1
                        } else {
                            menu.0.selected_level - 1
                        };
                    }
                }
            }
            AppPhase::Editor => move_editor_cursor_x(&mut input, &config, -1.0),
            _ => {}
        }
    }
}

fn move_editor_cursor_x(input: &mut EditorInputRes, config: &EditorConfigRes, direction: f32) {
    let step = if config.0.snap_to_grid && !input.0.ctrl_held {
        config.0.snap_step.max(0.05)
    } else {
        1.0
    };
    input.0.cursor[0] += direction * step;
}

pub fn handle_toggle_editor(
    mut events: MessageReader<ToggleEditor>,
    mut request: ResMut<ToggleEditorRequest>,
) {
    for _ in events.read() {
        request.request();
    }
}

pub fn handle_game_resume(
    mut events: MessageReader<GameResume>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.game_paused = false;
    }
}

// `GameRestartLevel` writes a typed request resource (same
// request-resource pattern as
// `events::auth::handle_resize_surface` and
// `events::misc::handle_escape`): the handler writes
// `GameRestartLevelRequest.pending = true` and
// `Runtime::sync_app_game_restart_level_request` consumes the request
// during the typed request sync pass and calls the
// existing legacy `State::apply_game_restart_level` compatibility
// method. The restart side effect (gameplay clock reset, audio
// restart) stays legacy-owned.
pub fn handle_game_restart_level(
    mut events: MessageReader<GameRestartLevel>,
    mut restart_request: ResMut<GameRestartLevelRequest>,
) {
    for _ in events.read() {
        restart_request.request();
    }
}

/// `GameSetPracticeMode` is a carve-out: the handler writes the
/// enabled flag directly to `SessionStateRes.practice_mode_enabled`
/// instead of staging a legacy `AppCommand`. The legacy
/// `set_practice_mode_enabled` compatibility setter has
/// heavy side effects (clears `practice_checkpoints`, calls
/// `rebuild_practice_checkpoint_vertices`, `restart_level`, and
/// `resume_game`); the change-guarded sync in
/// `Runtime::sync_app_session_state` invokes the legacy setter only
/// on real user-driven changes.
pub fn handle_game_set_practice_mode(
    mut events: MessageReader<GameSetPracticeMode>,
    mut session: ResMut<SessionStateRes>,
) {
    for event in events.read() {
        session.practice_mode_enabled = event.enabled;
    }
}

// `GameSetPracticeCheckpoint` is a Phase 3.48 carve-out: same
// request-resource pattern as `GameRestartLevel`. The handler
// writes `GameSetPracticeCheckpointRequest.pending = true` and
// `Runtime::sync_app_game_set_practice_checkpoint_request` consumes
// the request and calls the existing legacy
// `State::apply_game_set_practice_checkpoint` compatibility method.
// The checkpoint side effect (session.practice_checkpoints.push +
// vertex rebuild) stays legacy-owned.
pub fn handle_game_set_practice_checkpoint(
    mut events: MessageReader<GameSetPracticeCheckpoint>,
    mut checkpoint_request: ResMut<GameSetPracticeCheckpointRequest>,
) {
    for _ in events.read() {
        checkpoint_request.request();
    }
}

// `GameRemovePracticeCheckpoint` is a Phase 3.48 carve-out: same
// request-resource pattern as `GameRestartLevel`. The handler
// writes `GameRemovePracticeCheckpointRequest.pending = true` and
// `Runtime::sync_app_game_remove_practice_checkpoint_request`
// consumes the request and calls the existing legacy
// `State::apply_game_remove_practice_checkpoint` compatibility
// method. The pop + vertex rebuild side effect stays legacy-owned.
pub fn handle_game_remove_practice_checkpoint(
    mut events: MessageReader<GameRemovePracticeCheckpoint>,
    mut remove_request: ResMut<GameRemovePracticeCheckpointRequest>,
) {
    for _ in events.read() {
        remove_request.request();
    }
}

// `GameQuitToMenu` is a Phase 3.48 carve-out: same request-resource
// pattern as `GameRestartLevel`. The handler writes
// `GameQuitToMenuRequest.pending = true` and
// `Runtime::sync_app_game_quit_to_menu_request` consumes the
// request and calls the existing legacy
// `State::apply_game_quit_to_menu` compatibility method. The
// `back_to_menu` side effect (phase change + audio stop + gameplay
// state reset) stays legacy-owned.
pub fn handle_game_quit_to_menu(
    mut events: MessageReader<GameQuitToMenu>,
    mut quit_request: ResMut<GameQuitToMenuRequest>,
) {
    for _ in events.read() {
        quit_request.request();
    }
}
