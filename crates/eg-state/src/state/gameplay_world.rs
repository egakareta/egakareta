/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-world gameplay ownership handoff.
//!
//! The Runtime-owned App world is authoritative for gameplay simulation.
//! This module contains the narrow initial/transition seeding adapters
//! needed while consumers are being migrated.
//!
//!

use bevy_ecs::prelude::*;
use bevy_state::prelude::State;
use bevy_time::{Fixed, Time};

use eg_audio::audio::runtime_asset_source_key;
use eg_game::{
    BASE_PLAYER_SPEED, BlockId, CachedBlockBehavior, ColorTint, DeathSfxFlag, FrameDelta,
    GameClock, LevelMeta, Object, ObjectBundle, ObjectData, ObjectId, PendingTurnInputs, Player,
    PlayerBundle, PlayerDirection, PlayerPosition, PlayerProgress, PlayerSpeed, Position, Rotation,
    Size, SpatialIndex, SpawnDirection, SpawnPosition, TimedTurnInput, Trail, VerticalVelocity,
};
use eg_types::{Direction, LevelObject, PlayerLevelProgress};

use super::{
    EditorSpawnRes, EditorTimelineRes, GameplayResetRes, GameplayResetSeed, MenuStateRes,
    RuntimeAudioPreloadRes, SessionStateRes, with_audio_backend,
};

const DEAD_SFX: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/dead.mp3"
));

/// Keeps headless fixtures deterministic while production uses `FixedUpdate`.
#[derive(Resource, Default)]
pub(crate) struct ManualGameplayDriverRes;

pub(super) fn seed_app_gameplay_world(world: &mut World) {
    replace_app_gameplay_world(
        world,
        GameplayWorldSeed {
            objects: Vec::new(),
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
            speed: BASE_PLAYER_SPEED,
            vertical_velocity: 0.0,
            is_grounded: true,
            trail_segments: vec![vec![[0.0, 0.0, 0.0]]],
            elapsed_seconds: 0.0,
            level_duration_seconds: f32::INFINITY,
            started: false,
            game_over: false,
            level_complete: false,
            completion_hold_seconds: 0.0,
            progress: PlayerLevelProgress::default(),
            pending_turns: Vec::new(),
            death_sfx_played: false,
        },
    );
}

/// Keep fixed-step gameplay systems active only for unpaused gameplay.
pub(super) fn gameplay_is_active(
    phase: Res<State<eg_types::AppPhase>>,
    session: Res<SessionStateRes>,
    manual_driver: Option<Res<ManualGameplayDriverRes>>,
) -> bool {
    manual_driver.is_none() && *phase.get() == eg_types::AppPhase::Playing && !session.game_paused
}

/// Feed the standard `FixedUpdate` gameplay systems from Bevy's fixed delta.
pub(super) fn prepare_app_gameplay_fixed_system(
    phase: Res<State<eg_types::AppPhase>>,
    session: Res<SessionStateRes>,
    time: Res<Time<Fixed>>,
    mut delta: ResMut<FrameDelta>,
) {
    delta.0 = if *phase.get() == eg_types::AppPhase::Playing && !session.game_paused {
        // Direct FixedUpdate runs in the headless driver do not advance
        // Time<Fixed>. Use its configured fixed timestep there so the same
        // App-owned gameplay schedule remains deterministic in tests.
        let fixed_delta = time.delta_secs();
        if fixed_delta > 0.0 {
            fixed_delta
        } else {
            time.timestep().as_secs_f32()
        }
    } else {
        0.0
    };
}

/// Apply the terminal gameplay transition after the App-owned simulation has
/// completed its hold timer. This removes the phase/session decision from the
/// scheduled App-world path while feeding the custom renderer
/// through the existing Runtime resource mirrors.
pub fn apply_completed_gameplay_transition_system(world: &mut World) {
    let (playtesting_editor, level_name, progress) = {
        let phase = world.resource::<State<eg_types::AppPhase>>();
        let clock = world.resource::<GameClock>();
        if *phase.get() != eg_types::AppPhase::Playing
            || !clock.level_complete
            || clock.completion_hold_seconds > 0.0
        {
            return;
        }

        (
            world.resource::<SessionStateRes>().playtesting_editor,
            world
                .resource::<SessionStateRes>()
                .playing_level_name
                .clone(),
            world.resource::<PlayerProgress>().sanitized(),
        )
    };

    if !playtesting_editor && level_name.is_some() {
        persist_level_progress(world, progress);
    }

    let (objects, position, direction, duration) = if playtesting_editor {
        let (position, direction) = {
            let spawn = &world.resource::<EditorSpawnRes>().0;
            (spawn.position, spawn.direction)
        };
        (
            {
                let mut query = world.query_filtered::<(
                    &super::editor_objects::EditorObjectOrder,
                    &eg_game::ObjectData,
                ), With<super::editor_objects::EditorAuthoredObject>>(
                );
                let mut objects = query
                    .iter(world)
                    .map(|(order, data)| (order.0, data.0.clone()))
                    .collect::<Vec<_>>();
                objects.sort_by_key(|(order, _)| *order);
                objects.into_iter().map(|(_, object)| object).collect()
            },
            position,
            direction,
            world
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .duration_seconds,
        )
    } else {
        (
            Vec::new(),
            [0.0, 0.0, 0.0],
            Direction::Forward,
            f32::INFINITY,
        )
    };

    if playtesting_editor {
        world
            .resource_mut::<bevy_state::prelude::NextState<eg_types::AppPhase>>()
            .set(eg_types::AppPhase::Editor);
        let mut session = world.resource_mut::<SessionStateRes>();
        session.playtesting_editor = false;
        session.game_paused = false;
        session.playtest_audio_start_seconds = None;
        session.playing_trigger_base_objects = None;
        session.practice_mode_enabled = false;
        session.practice_checkpoints.clear();
        session.audio_stop_pending = true;
    } else {
        world
            .resource_mut::<bevy_state::prelude::NextState<eg_types::AppPhase>>()
            .set(eg_types::AppPhase::Menu);
        world.resource_mut::<SessionStateRes>().transition_to_menu();
    }
    if let Some(mut menu) = world.get_resource_mut::<MenuStateRes>() {
        menu.0.preview_level_index = None;
    }

    replace_app_gameplay_world(
        world,
        GameplayWorldSeed {
            objects,
            position,
            direction,
            speed: BASE_PLAYER_SPEED,
            vertical_velocity: 0.0,
            is_grounded: true,
            trail_segments: vec![vec![position]],
            elapsed_seconds: 0.0,
            level_duration_seconds: duration,
            started: false,
            game_over: false,
            level_complete: false,
            completion_hold_seconds: 0.0,
            progress: eg_types::PlayerLevelProgress::default(),
            pending_turns: Vec::new(),
            death_sfx_played: false,
        },
    );
}

/// Apply the non-practice GameOver side effects after gameplay has marked the
/// App-owned clock. Practice respawn remains a compatibility path until its
/// checkpoint presentation is also world-owned.
pub fn apply_game_over_transition_system(world: &mut World) {
    let (playtesting_editor, practice_mode_enabled, progress) = {
        let phase = world.resource::<State<eg_types::AppPhase>>();
        let clock = world.resource::<GameClock>();
        if *phase.get() != eg_types::AppPhase::Playing || !clock.game_over {
            return;
        }

        (
            world.resource::<SessionStateRes>().playtesting_editor,
            world.resource::<SessionStateRes>().practice_mode_enabled,
            world.resource::<PlayerProgress>().sanitized(),
        )
    };

    let should_play_death_sfx = {
        let mut death_sfx = world.resource_mut::<DeathSfxFlag>();
        let pending = death_sfx.0;
        death_sfx.0 = false;
        pending
    };
    if should_play_death_sfx {
        with_audio_backend(world, |audio| audio.play_sfx(DEAD_SFX));
    }

    if practice_mode_enabled && !playtesting_editor {
        let (seed, trigger_base_objects) = {
            let session = world.resource::<SessionStateRes>();
            let clock = world.resource::<GameClock>();
            if let Some(checkpoint) = session.practice_checkpoints.last().cloned() {
                let gameplay = checkpoint.gameplay;
                (
                    GameplayResetSeed {
                        objects: gameplay.objects.clone(),
                        position: gameplay.position,
                        direction: gameplay.direction,
                        speed: gameplay.speed,
                        vertical_velocity: gameplay.vertical_velocity,
                        is_grounded: gameplay.is_grounded,
                        elapsed_seconds: gameplay.elapsed_seconds,
                        level_duration_seconds: clock.level_duration_seconds,
                        progress: Some(gameplay.progress()),
                        preserve_practice_state: true,
                    },
                    checkpoint.trigger_base_objects,
                )
            } else {
                let level_meta = world.resource::<LevelMeta>();
                (
                    GameplayResetSeed {
                        objects: level_meta.objects.clone(),
                        position: level_meta.spawn_position,
                        direction: level_meta.spawn_direction,
                        speed: BASE_PLAYER_SPEED,
                        vertical_velocity: 0.0,
                        is_grounded: true,
                        elapsed_seconds: 0.0,
                        level_duration_seconds: level_meta.level_duration_seconds,
                        progress: None,
                        preserve_practice_state: true,
                    },
                    None,
                )
            }
        };

        with_audio_backend(world, |audio| audio.stop());
        world.resource_mut::<PendingTurnInputs>().taps.clear();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.playing_trigger_base_objects = trigger_base_objects;
            if seed.progress.is_none() {
                session.practice_checkpoints.clear();
            }
        }
        warmup_gameplay_audio(world, seed.elapsed_seconds);
        world
            .resource_mut::<GameplayResetRes>()
            .request_with_seed(seed);
        apply_pending_gameplay_reset_system(world);
        return;
    }

    if !playtesting_editor {
        persist_level_progress(world, progress);
    }
    with_audio_backend(world, |audio| audio.stop());
    world.resource_mut::<PendingTurnInputs>().taps.clear();
}

fn warmup_gameplay_audio(world: &World, start_seconds: f32) {
    let Some(level_name) = world
        .resource::<SessionStateRes>()
        .playing_level_name
        .as_deref()
        .map(str::to_owned)
    else {
        return;
    };
    let source_key = runtime_asset_source_key(&level_name, "audio.mp3");
    if let Some(bytes) = world
        .resource::<RuntimeAudioPreloadRes>()
        .bytes(&source_key)
    {
        with_audio_backend(world, |audio| {
            audio.warmup_preloaded_asset_at(&level_name, &bytes, start_seconds);
        });
    } else {
        with_audio_backend(world, |audio| audio.warmup_at(&level_name, start_seconds));
    }
}

fn persist_level_progress(world: &mut World, progress: PlayerLevelProgress) {
    let Some(level_name) = world
        .resource::<SessionStateRes>()
        .playing_level_name
        .clone()
    else {
        return;
    };

    let settings_to_persist = {
        let mut session = world.resource_mut::<SessionStateRes>();
        if session
            .app_settings
            .update_level_progress(&level_name, progress)
        {
            Some(session.app_settings.clone())
        } else {
            None
        }
    };

    #[cfg(not(test))]
    if let Some(settings) = settings_to_persist {
        eg_platform::task::spawn_background(async move {
            if let Err(error) = eg_platform::io::save_app_settings_to_storage(&settings).await {
                log::warn!("Failed to persist gameplay progress: {error}");
            }
        });
    }
    #[cfg(test)]
    let _ = settings_to_persist;
}

/// Apply a real-gameplay restart recorded by the navigation message handler.
/// This exclusive system is the only place that needs to replace the object
/// entity set, so the reset remains one atomic App-world operation.
pub(crate) fn apply_pending_gameplay_reset_system(world: &mut World) {
    let transition_seed = {
        let mut reset = world.resource_mut::<GameplayResetRes>();
        if !reset.pending {
            return;
        }
        reset.pending = false;
        reset.seed.take()
    };
    let preserve_practice_state = transition_seed
        .as_ref()
        .is_some_and(|seed| seed.preserve_practice_state);
    let progress = transition_seed.as_ref().and_then(|seed| seed.progress);
    let (
        objects,
        position,
        direction,
        speed,
        vertical_velocity,
        is_grounded,
        elapsed_seconds,
        duration,
    ) = if let Some(seed) = transition_seed {
        (
            seed.objects,
            seed.position,
            seed.direction,
            seed.speed,
            seed.vertical_velocity,
            seed.is_grounded,
            seed.elapsed_seconds,
            seed.level_duration_seconds,
        )
    } else {
        let meta = world.resource::<LevelMeta>();
        (
            meta.objects.clone(),
            meta.spawn_position,
            meta.spawn_direction,
            BASE_PLAYER_SPEED,
            0.0,
            true,
            0.0,
            meta.level_duration_seconds,
        )
    };
    {
        let mut session = world.resource_mut::<SessionStateRes>();
        session.game_paused = false;
        session.practice_mode_enabled = preserve_practice_state;
        if !preserve_practice_state {
            session.practice_checkpoints.clear();
        }
    }
    replace_app_gameplay_world(
        world,
        GameplayWorldSeed {
            objects,
            position,
            direction,
            speed,
            vertical_velocity,
            is_grounded,
            trail_segments: vec![vec![position]],
            elapsed_seconds,
            level_duration_seconds: duration,
            started: false,
            game_over: false,
            level_complete: false,
            completion_hold_seconds: 0.0,
            progress: progress.unwrap_or_default(),
            pending_turns: Vec::new(),
            death_sfx_played: false,
        },
    );
}

struct GameplayWorldSeed {
    objects: Vec<LevelObject>,
    position: [f32; 3],
    direction: Direction,
    speed: f32,
    vertical_velocity: f32,
    is_grounded: bool,
    trail_segments: Vec<Vec<[f32; 3]>>,
    elapsed_seconds: f32,
    level_duration_seconds: f32,
    started: bool,
    game_over: bool,
    level_complete: bool,
    completion_hold_seconds: f32,
    progress: eg_types::PlayerLevelProgress,
    pending_turns: Vec<TimedTurnInput>,
    death_sfx_played: bool,
}

fn replace_app_gameplay_world(world: &mut World, seed: GameplayWorldSeed) {
    let GameplayWorldSeed {
        objects,
        position,
        direction,
        speed,
        vertical_velocity,
        is_grounded,
        trail_segments,
        elapsed_seconds,
        level_duration_seconds,
        started,
        game_over,
        level_complete,
        completion_hold_seconds,
        progress,
        pending_turns,
        death_sfx_played,
    } = seed;

    let entities = {
        let mut query = world.query_filtered::<Entity, Or<(With<Player>, With<Object>)>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in entities {
        if let Ok(entity) = world.get_entity_mut(entity) {
            entity.despawn();
        }
    }

    let gems_max = objects
        .iter()
        .map(|object| CachedBlockBehavior::from_block_id(&object.block_id).collectible_gem_value())
        .sum();
    for (index, object) in objects.iter().enumerate() {
        world.spawn(ObjectBundle {
            object: Object,
            id: ObjectId(format!("__obj_{index}")),
            data: ObjectData(object.clone()),
            position: Position(object.position),
            rotation: Rotation(object.rotation),
            size: Size(object.size),
            block_id: BlockId(object.block_id.clone()),
            color_tint: ColorTint(object.color_tint),
            cached_behavior: CachedBlockBehavior::from_block_id(&object.block_id),
        });
    }
    world.spawn(PlayerBundle {
        player: Player,
        position: PlayerPosition(position),
        direction: PlayerDirection(direction),
        speed: PlayerSpeed(speed.max(0.1)),
        vertical_velocity: VerticalVelocity(vertical_velocity),
        grounded: eg_game::Grounded(is_grounded),
        trail: Trail {
            segments: trail_segments,
        },
        spawn_position: SpawnPosition(position),
        spawn_direction: SpawnDirection(direction),
    });

    {
        let mut clock = world.resource_mut::<GameClock>();
        clock.elapsed_seconds = elapsed_seconds.max(0.0);
        clock.level_duration_seconds =
            if level_duration_seconds.is_finite() && level_duration_seconds > 0.0 {
                level_duration_seconds
            } else {
                f32::INFINITY
            };
        clock.started = started;
        clock.game_over = game_over;
        clock.level_complete = level_complete;
        clock.completion_hold_seconds = completion_hold_seconds.max(0.0);
    }
    {
        let mut player_progress = world.resource_mut::<PlayerProgress>();
        player_progress.progress_percent = progress.progress_percent;
        player_progress.completed = progress.completed;
        player_progress.gems_collected = progress.gems_collected;
        player_progress.gems_max = progress.gems_max.max(gems_max);
    }
    let normalized_duration = world.resource::<GameClock>().level_duration_seconds;
    {
        let mut level_meta = world.resource_mut::<LevelMeta>();
        level_meta.objects = objects;
        level_meta.spawn_position = position;
        level_meta.spawn_direction = direction;
        level_meta.level_duration_seconds = normalized_duration;
    }
    world.resource_mut::<PendingTurnInputs>().taps = pending_turns.into();
    world.resource_mut::<FrameDelta>().0 = 0.0;

    world.resource_mut::<DeathSfxFlag>().0 = death_sfx_played;
    rebuild_spatial_index(world);
}

fn rebuild_spatial_index(world: &mut World) {
    let entries = {
        let mut query = world.query::<(Entity, &ObjectData)>();
        query
            .iter(world)
            .map(|(entity, object)| (entity, object.0.clone()))
            .collect::<Vec<_>>()
    };
    let mut spatial = world.resource_mut::<SpatialIndex>();
    spatial.grid.clear();
    for (entity, object) in entries {
        spatial.grid.insert_object(entity, &object);
    }
}
