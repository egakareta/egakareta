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
//! Navigation messages mutate App-world phase/session/gameplay resources
//! directly. Atomic gameplay resets and the shared audio backend remain
//! App-owned resources while the compatibility `State` is still refreshed
//! for presentation.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use eg_audio::audio::runtime_asset_source_key;
use eg_blocks::BlockDefinition;
use eg_editor_domain::create_block_at_cursor;
use eg_game::{
    GameCheckpointParts, GameClock, Grounded, LevelMeta, ObjectData, ObjectId, PendingTurnInputs,
    Player, PlayerDirection, PlayerPosition, PlayerProgress, PlayerSpeed, TimedTurnInput, Trail,
    TurnInputKind, VerticalVelocity,
};
use eg_types::{AppPhase, EditorMode, LevelMetadata};

use crate::state::{
    AppPhaseRes, AudioBackendRes, BuiltinLevelCatalogRes, EditorAudioCacheRes, EditorConfigRes,
    EditorHistoryRes, EditorHistorySnapshot, EditorInputRes, EditorObjectsRes, EditorRuntimeRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, GameplayResetRes,
    GameplayResetSeed, MenuStateRes, PracticeCheckpoint, RuntimeAudioPreloadRes,
    RuntimeAudioPreloadSenderRes, SessionStateRes, apply_imported_level_to_world,
    with_audio_backend,
};

fn object_order_key(id: &str) -> (u8, usize, &str) {
    id.strip_prefix("__obj_")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .map(|index| (0, index, id))
        .unwrap_or((1, 0, id))
}

/// Primary action: in editor this places a block directly in App-world
/// resources; in menu it starts the selected level through an atomic gameplay
/// reset; in game-over it returns to menu; paused gameplay is a direct no-op.
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
// Dispatch systems. Navigation handlers mutate App-world resources directly;
// service-specific request resources are consumed by their own schedule systems.
// -----------------------------------------------------------------------

pub fn handle_turn_right(mut events: MessageReader<TurnRight>, mut resources: TurnRightResources) {
    for _ in events.read() {
        match resources.phase.current {
            AppPhase::Editor => place_editor_block_from_turn_right(&mut resources),
            AppPhase::Menu => start_gameplay_from_menu(&mut resources),
            AppPhase::GameOver => resources.phase.set(AppPhase::Menu),
            AppPhase::Playing if resources.session.game_paused => {}
            AppPhase::Playing
                if resources.clock.game_over && !resources.session.practice_mode_enabled =>
            {
                resources.gameplay_reset.request();
            }
            AppPhase::Playing
                if resources.clock.game_over && resources.session.practice_mode_enabled =>
            {
                if !respawn_from_practice_checkpoint(&mut resources) {
                    restart_practice_from_turn_right(&mut resources);
                }
            }
            AppPhase::Playing if !resources.clock.started => {
                start_gameplay_from_turn_right(&mut resources);
            }
            AppPhase::Playing
                if resources.clock.started
                    && !resources.clock.game_over
                    && !resources.clock.level_complete =>
            {
                resources.pending_turns.taps.push_back(TimedTurnInput {
                    at_seconds: resources.clock.elapsed_seconds,
                    kind: TurnInputKind::TurnRight,
                });
            }
            _ => {}
        }
    }
}

#[derive(SystemParam)]
pub struct TurnRightResources<'w> {
    phase: ResMut<'w, AppPhaseRes>,
    session: ResMut<'w, SessionStateRes>,
    menu: Option<Res<'w, MenuStateRes>>,
    catalog: Res<'w, BuiltinLevelCatalogRes>,
    gameplay_reset: ResMut<'w, GameplayResetRes>,
    #[cfg(not(target_arch = "wasm32"))]
    audio: Res<'w, AudioBackendRes>,
    #[cfg(target_arch = "wasm32")]
    audio: NonSend<'w, AudioBackendRes>,
    editor_audio: Res<'w, EditorAudioCacheRes>,
    runtime_preload: Res<'w, RuntimeAudioPreloadRes>,
    runtime_preload_sender: Res<'w, RuntimeAudioPreloadSenderRes>,
    clock: ResMut<'w, GameClock>,
    level_meta: Res<'w, LevelMeta>,
    pending_turns: ResMut<'w, PendingTurnInputs>,
    objects: ResMut<'w, EditorObjectsRes>,
    input: ResMut<'w, EditorInputRes>,
    config: Res<'w, EditorConfigRes>,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: ResMut<'w, EditorTriggerRes>,
    history: ResMut<'w, EditorHistoryRes>,
}

fn start_gameplay_from_turn_right(resources: &mut TurnRightResources<'_>) {
    let start_seconds = resources.clock.elapsed_seconds;
    resources.clock.started = true;

    let (level_name, start_seconds, use_editor_audio) = if resources.session.playtesting_editor {
        (
            resources
                .session
                .editor_level_name
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            resources
                .session
                .playtest_audio_start_seconds
                .unwrap_or(start_seconds),
            true,
        )
    } else if let Some(level_name) = resources.session.playing_level_name.clone() {
        (level_name, start_seconds, false)
    } else {
        return;
    };

    if use_editor_audio
        && let Some(bytes) = resources.editor_audio.bytes_for_level_audio(&level_name)
    {
        resources.audio.start_with_bytes_at(&bytes, start_seconds);
        return;
    }

    let source_key = runtime_asset_source_key(&level_name, "audio.mp3");
    if let Some(bytes) = resources.runtime_preload.bytes(&source_key) {
        resources
            .audio
            .start_preloaded_asset_at(&level_name, &bytes, start_seconds);
    } else {
        resources.audio.start_at(&level_name, start_seconds);
    }
}

fn respawn_from_practice_checkpoint(resources: &mut TurnRightResources<'_>) -> bool {
    let Some(checkpoint) = resources.session.practice_checkpoints.last().cloned() else {
        return false;
    };

    let gameplay = checkpoint.gameplay;
    resources.audio.stop();
    resources.pending_turns.taps.clear();
    resources.session.playing_trigger_base_objects = checkpoint.trigger_base_objects;
    resources
        .gameplay_reset
        .request_with_seed(GameplayResetSeed {
            objects: gameplay.objects.clone(),
            position: gameplay.position,
            direction: gameplay.direction,
            speed: gameplay.speed,
            vertical_velocity: gameplay.vertical_velocity,
            is_grounded: gameplay.is_grounded,
            elapsed_seconds: gameplay.elapsed_seconds,
            level_duration_seconds: resources.clock.level_duration_seconds,
            progress: Some(gameplay.progress()),
            preserve_practice_state: true,
        });
    true
}

fn restart_practice_from_turn_right(resources: &mut TurnRightResources<'_>) {
    resources.audio.stop();
    resources.pending_turns.taps.clear();
    resources.session.practice_checkpoints.clear();
    resources
        .gameplay_reset
        .request_with_seed(GameplayResetSeed {
            objects: resources.level_meta.objects.clone(),
            position: resources.level_meta.spawn_position,
            direction: resources.level_meta.spawn_direction,
            speed: eg_game::BASE_PLAYER_SPEED,
            vertical_velocity: 0.0,
            is_grounded: true,
            elapsed_seconds: 0.0,
            level_duration_seconds: resources.level_meta.level_duration_seconds,
            progress: None,
            preserve_practice_state: true,
        });
}

fn start_gameplay_from_menu(resources: &mut TurnRightResources<'_>) {
    let Some(menu) = resources.menu.as_ref() else {
        return;
    };
    let Some(level_name) = menu.0.levels.get(menu.0.selected_level).cloned() else {
        return;
    };
    let metadata = resources.catalog.get(&level_name).unwrap_or_else(|| {
        let mut metadata = LevelMetadata::default();
        metadata.basic.name = level_name.clone();
        metadata
    });
    let objects = metadata.objects.clone();

    resources.phase.set(AppPhase::Playing);
    resources.session.playtesting_editor = false;
    resources.session.game_paused = false;
    resources.session.playing_level_name = Some(level_name.clone());
    resources.session.playtest_audio_start_seconds = None;
    resources.session.playing_sky_color = metadata.sky_color;
    resources.session.playing_trigger_base_objects = None;
    resources.session.practice_mode_enabled = false;
    resources.session.practice_checkpoints.clear();
    resources.session.audio_stop_pending = false;

    resources.objects.objects = objects.clone();
    resources.trigger.0.selected_index = None;
    queue_runtime_audio_preload(
        &resources.runtime_preload,
        &resources.runtime_preload_sender,
        &level_name,
    );
    resources.audio.warmup_at(&level_name, 0.0);
    resources
        .gameplay_reset
        .request_with_seed(GameplayResetSeed {
            objects,
            position: metadata.spawn.position,
            direction: metadata.spawn.direction,
            speed: eg_game::BASE_PLAYER_SPEED,
            vertical_velocity: 0.0,
            is_grounded: true,
            elapsed_seconds: 0.0,
            level_duration_seconds: metadata.timeline_duration_seconds,
            progress: None,
            preserve_practice_state: false,
        });
}

fn queue_runtime_audio_preload(
    cache: &RuntimeAudioPreloadRes,
    sender: &RuntimeAudioPreloadSenderRes,
    level_name: &str,
) {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");
    if cache.contains_preloaded(&source_key) || cache.contains_preloading(&source_key) {
        return;
    }
    cache.mark_preloading(source_key.clone());

    #[cfg(test)]
    let _ = sender;

    #[cfg(not(test))]
    eg_audio::service::start_audio_preload(source_key, level_name.to_owned(), sender.0.clone());
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

pub fn handle_toggle_editor(world: &mut World) {
    use bevy_ecs::message::Messages;

    let events: Vec<ToggleEditor> = world
        .resource_mut::<Messages<ToggleEditor>>()
        .drain()
        .collect();
    for _ in events {
        match world.resource::<AppPhaseRes>().current {
            AppPhase::Editor => {
                world.resource_mut::<AppPhaseRes>().set(AppPhase::Menu);
                world.resource_mut::<SessionStateRes>().transition_to_menu();
                if let Some(mut menu) = world.get_resource_mut::<MenuStateRes>() {
                    menu.0.preview_level_index = None;
                }
            }
            AppPhase::Playing if world.resource::<SessionStateRes>().playtesting_editor => {
                let mut phase = world.resource_mut::<AppPhaseRes>();
                phase.set(AppPhase::Editor);
                let mut session = world.resource_mut::<SessionStateRes>();
                session.playtesting_editor = false;
                session.game_paused = false;
                session.audio_stop_pending = true;
            }
            AppPhase::Menu => enter_editor_from_menu(world),
            AppPhase::Playing | AppPhase::GameOver => {}
        }
    }
}

fn enter_editor_from_menu(world: &mut World) {
    let Some(level_name) = world
        .get_resource::<MenuStateRes>()
        .and_then(|menu| menu.0.levels.get(menu.0.selected_level).cloned())
    else {
        return;
    };

    let metadata = world
        .resource::<BuiltinLevelCatalogRes>()
        .get(&level_name)
        .unwrap_or_else(|| {
            let mut metadata = LevelMetadata::default();
            metadata.basic.name = level_name.clone();
            metadata
        });
    apply_imported_level_to_world(world, metadata);

    world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
    {
        let mut input = world.resource_mut::<EditorInputRes>();
        input.0.mode = EditorMode::Null;
        input.0.right_dragging = false;
        input.0.marquee_start_screen = None;
        input.0.marquee_current_screen = None;
    }
    {
        let mut runtime = world.resource_mut::<EditorRuntimeRes>();
        runtime.0.interaction.gizmo_drag = None;
        runtime.0.interaction.block_drag = None;
    }
    {
        let mut camera = world.resource_mut::<crate::state::EditorCameraRes>();
        camera.0.editor_yaw = eg_types::DEFAULT_CAMERA_YAW;
        camera.0.editor_pitch = eg_types::DEFAULT_CAMERA_PITCH;
        camera.0.transition = None;
    }
    world.resource_mut::<EditorTimelineRes>().0.playback = Default::default();
    {
        let mut session = world.resource_mut::<SessionStateRes>();
        session.playtesting_editor = false;
        session.game_paused = false;
        session.playtest_audio_start_seconds = None;
        session.playing_level_name = None;
        session.playing_sky_color = LevelMetadata::default().sky_color;
        session.playing_trigger_base_objects = None;
        session.practice_mode_enabled = false;
        session.practice_checkpoints.clear();
        session.audio_stop_pending = false;
    }
    if let Some(mut menu) = world.get_resource_mut::<MenuStateRes>() {
        menu.0.preview_level_index = None;
    }

    preload_editor_audio(world, &level_name);
    with_audio_backend(world, |audio| audio.warmup_at(&level_name, 0.0));
}

fn preload_editor_audio(world: &World, level_name: &str) {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");
    let cache = world.resource::<RuntimeAudioPreloadRes>().clone();
    if cache.contains_preloaded(&source_key) || cache.contains_preloading(&source_key) {
        return;
    }
    cache.mark_preloading(source_key.clone());

    #[cfg(not(test))]
    eg_audio::service::start_audio_preload(
        source_key,
        level_name.to_owned(),
        world
            .resource::<crate::state::RuntimeAudioPreloadSenderRes>()
            .0
            .clone(),
    );
}

pub fn handle_game_resume(
    mut events: MessageReader<GameResume>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.game_paused = false;
    }
}

// Active real gameplay restart is fully App-owned. Other phases and editor
// playtests ignore this pause-menu command.
pub fn handle_game_restart_level(
    mut events: MessageReader<GameRestartLevel>,
    phase: Res<AppPhaseRes>,
    session: Res<SessionStateRes>,
    mut gameplay_reset: ResMut<GameplayResetRes>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Playing || session.playtesting_editor {
            continue;
        }

        gameplay_reset.request();
    }
}

/// `GameSetPracticeMode` mutates the App-owned practice session directly.
/// Disabling real gameplay practice mode also queues the existing
/// App-world gameplay reset, matching the legacy restart behavior.
pub fn handle_game_set_practice_mode(
    mut events: MessageReader<GameSetPracticeMode>,
    phase: Res<AppPhaseRes>,
    mut session: ResMut<SessionStateRes>,
    mut gameplay_reset: ResMut<GameplayResetRes>,
) {
    for event in events.read() {
        if session.practice_mode_enabled != event.enabled {
            session.practice_mode_enabled = event.enabled;
            session.practice_checkpoints.clear();
            session.game_paused = false;
            if phase.current == AppPhase::Playing && !session.playtesting_editor && !event.enabled {
                gameplay_reset.request();
            }
        }
    }
}

#[derive(SystemParam)]
pub struct PracticeCheckpointResources<'w, 's> {
    phase: Res<'w, AppPhaseRes>,
    clock: Res<'w, GameClock>,
    progress: Res<'w, PlayerProgress>,
    players: Query<
        'w,
        's,
        (
            &'static PlayerPosition,
            &'static PlayerDirection,
            &'static PlayerSpeed,
            &'static VerticalVelocity,
            &'static Grounded,
        ),
        With<Player>,
    >,
    trails: Query<'w, 's, &'static Trail, With<Player>>,
    objects: Query<'w, 's, (&'static ObjectId, &'static ObjectData)>,
    session: ResMut<'w, SessionStateRes>,
}

// `GameSetPracticeCheckpoint` is an App-world gameplay carve-out. The
// handler captures the current player/object ECS state directly and builds
// an independent checkpoint snapshot for App-owned practice state.
pub fn handle_game_set_practice_checkpoint(
    mut events: MessageReader<GameSetPracticeCheckpoint>,
    mut resources: PracticeCheckpointResources,
) {
    for _ in events.read() {
        if resources.phase.current != AppPhase::Playing
            || resources.session.playtesting_editor
            || !resources.session.practice_mode_enabled
            || resources.session.game_paused
            || !resources.clock.started
            || resources.clock.game_over
            || resources.clock.level_complete
        {
            continue;
        }

        let Some((position, direction, speed, vertical_velocity, grounded)) =
            resources.players.iter().next()
        else {
            continue;
        };
        let trail_exists = resources.trails.iter().next().is_some();
        if !trail_exists {
            continue;
        }

        let mut ordered_objects = resources
            .objects
            .iter()
            .map(|(id, data)| (id.0.clone(), data.0.clone()))
            .collect::<Vec<_>>();
        ordered_objects
            .sort_by(|(left, _), (right, _)| object_order_key(left).cmp(&object_order_key(right)));
        let ordered_objects = ordered_objects
            .into_iter()
            .map(|(_, object)| object)
            .collect();
        let checkpoint = eg_game::GameCheckpointState::from_parts(GameCheckpointParts {
            position: position.0,
            direction: direction.0,
            elapsed_seconds: resources.clock.elapsed_seconds,
            speed: speed.0,
            objects: ordered_objects,
            vertical_velocity: vertical_velocity.0,
            is_grounded: grounded.0,
            progress: resources.progress.sanitized(),
        });
        let trigger_base_objects = resources.session.playing_trigger_base_objects.clone();
        resources
            .session
            .practice_checkpoints
            .push(PracticeCheckpoint {
                gameplay: checkpoint,
                trigger_base_objects,
            });
    }
}

/// Removes the latest practice checkpoint directly from `SessionStateRes`.
/// The compatibility sync rebuilds the legacy mesh once while rendering still
/// reads the old presentation path.
pub fn handle_game_remove_practice_checkpoint(
    mut events: MessageReader<GameRemovePracticeCheckpoint>,
    phase: Res<AppPhaseRes>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        if phase.current == AppPhase::Playing
            && !session.playtesting_editor
            && session.practice_mode_enabled
            && !session.game_paused
        {
            let _ = session.practice_checkpoints.pop();
        }
    }
}

// Active real gameplay quit-to-menu is fully App-owned. Other phases and
// editor playtests ignore this pause-menu command.
pub fn handle_game_quit_to_menu(
    mut events: MessageReader<GameQuitToMenu>,
    mut phase: ResMut<AppPhaseRes>,
    mut session: ResMut<SessionStateRes>,
    mut menu: Option<ResMut<MenuStateRes>>,
) {
    for _ in events.read() {
        if phase.current != AppPhase::Playing || session.playtesting_editor {
            continue;
        }

        phase.set(AppPhase::Menu);
        session.transition_to_menu();
        if let Some(menu) = menu.as_deref_mut() {
            menu.0.preview_level_index = None;
        }
    }
}
