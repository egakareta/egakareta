/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! `GameState` — backwards-compatible wrapper around the ECS gameplay world.
//!
//! Internally the headless simulation now lives in a [`World`] + [`Schedule`]
//! populated by [`crate::plugin::GamePlugin`]. This struct exposes the same
//! `pub` field surface that the pre-ECS `GameState` had so that `eg-state`
//! and downstream crates don't need to be refactored in lockstep. Field
//! reads are served by [`GameState::sync_from_world`], which copies the
//! current ECS state into the legacy fields. Field writes go through
//! methods that update both the fields and the world.
//!
//! `TimelineSimulationRuntime` (in [`crate::simulation`]) follows the same
//! pattern with its own private world.

use std::collections::{HashMap, VecDeque};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use eg_types::{Direction, LevelObject, PlayerLevelProgress};

use super::physics::BASE_PLAYER_SPEED;
use super::spatial::SpatialGrid;
use crate::components::{
    BlockId, CachedBlockBehavior, ColorTint, Grounded, ObjectBundle, ObjectData, ObjectId,
    PlayerBundle, PlayerDirection, PlayerPosition, PlayerSpeed, Position, Rotation, Size,
    SpawnDirection, SpawnPosition, Trail, VerticalVelocity,
};
use crate::events::{ConsumedEventsBuffer, ConsumedObjectEvent};
use crate::resources::{
    DeathSfxFlag, FixedStepConfig, FrameDelta, GameClock, GameRuntimeTuning, LevelMeta,
    PendingTurnInputs, PlayerProgress, SpatialIndex,
};
use crate::systems::{self, CollisionOutcome, GameplaySet};

/// Snapshot of `GameState` for checkpoints (practice mode).
#[derive(Clone)]
pub struct GameCheckpointState {
    pub position: [f32; 3],
    pub direction: Direction,
    pub elapsed_seconds: f32,
    pub speed: f32,
    pub objects: Vec<LevelObject>,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
    progress: PlayerLevelProgress,
}

impl GameCheckpointState {
    pub fn progress(&self) -> PlayerLevelProgress {
        self.progress
    }
}

/// Values captured from an authoritative App-world gameplay state when a
/// practice checkpoint is created.
pub struct GameCheckpointParts {
    pub position: [f32; 3],
    pub direction: Direction,
    pub elapsed_seconds: f32,
    pub speed: f32,
    pub objects: Vec<LevelObject>,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
    pub progress: PlayerLevelProgress,
}

/// The core gameplay state. Backed by a [`World`] + [`Schedule`] under the
/// hood; the `pub` fields are kept in sync via [`Self::sync_from_world`].
///
/// REFACTOR POINT(full-bevy): when `eg-state` is fully ECS, this wrapper
/// goes away — `eg-state` owns the world and consumes components directly.
pub struct GameState {
    // ---- ECS core ----
    world: World,
    schedule: Schedule,
    player_entity: Option<Entity>,
    object_entities: Vec<Entity>,
    object_id_to_entity: HashMap<String, Entity>,

    // ---- Mirrors for legacy access (kept in sync via sync_from_world) ----
    pub position: [f32; 3],
    pub direction: Direction,
    pub elapsed_seconds: f32,
    pub level_duration_seconds: f32,
    pub speed: f32,
    pub trail_segments: Vec<Vec<[f32; 3]>>,
    pub objects: Vec<LevelObject>,
    cached_behaviors: Vec<CachedBlockBehavior>,
    spatial_grid: SpatialGrid,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
    pub game_over: bool,
    pub level_complete: bool,
    pub completion_hold_seconds: f32,
    pub started: bool,
    progress: PlayerLevelProgress,
    consumed_object_indices: Vec<usize>,
    consumed_object_events: Vec<ConsumedObjectEvent>,
    #[allow(dead_code)]
    query_indices: Vec<Entity>,
}

pub fn center_spawn_position(position: [f32; 3]) -> [f32; 3] {
    [
        position[0].floor() + 0.5,
        position[1],
        position[2].floor() + 0.5,
    ]
}

fn object_order_key(id: &str) -> (u8, usize, &str) {
    id.strip_prefix("__obj_")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .map(|index| (0, index, id))
        .unwrap_or((1, 0, id))
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    /// Build a fresh `GameState` with an empty world.
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(GameClock::default());
        world.insert_resource(PlayerProgress::default());
        world.insert_resource(SpatialIndex::default());
        world.insert_resource(PendingTurnInputs::default());
        world.insert_resource(FixedStepConfig::default());
        world.insert_resource(FrameDelta::default());
        world.insert_resource(DeathSfxFlag::default());
        world.insert_resource(GameRuntimeTuning::default());
        world.insert_resource(LevelMeta::default());
        world.insert_resource(CollisionOutcome::default());
        world.init_resource::<ConsumedEventsBuffer>();

        Self {
            world,
            schedule: build_schedule(),
            player_entity: None,
            object_entities: Vec::new(),
            object_id_to_entity: HashMap::new(),

            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
            elapsed_seconds: 0.0,
            level_duration_seconds: f32::INFINITY,
            speed: BASE_PLAYER_SPEED,
            trail_segments: vec![vec![[0.0, 0.0, 0.0]]],
            objects: Vec::new(),
            cached_behaviors: Vec::new(),
            spatial_grid: SpatialGrid::default(),
            vertical_velocity: 0.0,
            is_grounded: true,
            game_over: false,
            level_complete: false,
            completion_hold_seconds: 0.0,
            started: false,
            progress: PlayerLevelProgress::default(),
            consumed_object_indices: Vec::new(),
            consumed_object_events: Vec::new(),
            query_indices: Vec::new(),
        }
    }

    pub fn set_level_duration_seconds(&mut self, duration_seconds: f32) {
        self.level_duration_seconds = if duration_seconds.is_finite() && duration_seconds > 0.0 {
            duration_seconds
        } else {
            f32::INFINITY
        };
        if let Some(mut meta) = self.world.get_resource_mut::<LevelMeta>() {
            meta.level_duration_seconds = self.level_duration_seconds;
        }
    }

    /// Rebuild the cached block behavior array from current objects, and
    /// rebuild the spatial grid + world entities.
    pub fn rebuild_behavior_cache(&mut self) {
        puffin::profile_scope!("GameRebuildBehaviorCache");
        self.cached_behaviors = self
            .objects
            .iter()
            .map(|obj| CachedBlockBehavior::from_block_id(&obj.block_id))
            .collect();
        self.respawn_all_object_entities();
        self.rebuild_world_spatial_index();
        self.rebuild_spatial_grid();
    }

    pub fn initialize_level_progress_from_objects(&mut self) {
        let gems_max = self
            .objects
            .iter()
            .map(|object| {
                CachedBlockBehavior::from_block_id(&object.block_id).collectible_gem_value()
            })
            .sum();
        self.progress = PlayerLevelProgress {
            progress_percent: 0.0,
            completed: false,
            gems_collected: 0,
            gems_max,
        };
        if let Some(mut prog) = self.world.get_resource_mut::<PlayerProgress>() {
            prog.gems_max = gems_max;
            prog.gems_collected = 0;
            prog.progress_percent = 0.0;
            prog.completed = false;
        }
    }

    pub fn level_progress(&self) -> PlayerLevelProgress {
        self.progress.sanitized()
    }

    fn rebuild_spatial_grid(&mut self) {
        puffin::profile_scope!("GameRebuildSpatialGrid");
        self.spatial_grid.clear();
        for (idx, obj) in self.objects.iter().enumerate() {
            self.spatial_grid
                .insert_object(self.entity_for_index(idx), obj);
        }
    }

    /// Replace the entire object list and rebuild the ECS entities + the
    /// spatial index. Returns the previous list (so callers can diff).
    pub fn replace_objects_for_transform_hitboxes(
        &mut self,
        mut objects: Vec<LevelObject>,
    ) -> Vec<LevelObject> {
        let can_reuse_behaviors = self.cached_behaviors.len() == objects.len()
            && self
                .objects
                .iter()
                .zip(objects.iter())
                .all(|(current, next)| current.block_id == next.block_id);

        std::mem::swap(&mut self.objects, &mut objects);
        if can_reuse_behaviors {
            self.respawn_all_object_entities();
            self.rebuild_world_spatial_index();
            self.rebuild_spatial_grid();
        } else {
            self.rebuild_behavior_cache();
        }
        objects
    }

    fn respawn_all_object_entities(&mut self) {
        for entity in self.object_entities.drain(..) {
            if let Ok(e) = self.world.get_entity_mut(entity) {
                e.despawn();
            }
        }
        self.object_id_to_entity.clear();
        for (idx, obj) in self.objects.iter().enumerate() {
            let entity = self
                .world
                .spawn(ObjectBundle {
                    object: crate::components::Object,
                    id: ObjectId(format!("__obj_{idx}")),
                    data: ObjectData(obj.clone()),
                    position: Position(obj.position),
                    rotation: Rotation(obj.rotation),
                    size: Size(obj.size),
                    block_id: BlockId(obj.block_id.clone()),
                    color_tint: ColorTint(obj.color_tint),
                    cached_behavior: self
                        .cached_behaviors
                        .get(idx)
                        .copied()
                        .unwrap_or_else(|| CachedBlockBehavior::from_block_id(&obj.block_id)),
                })
                .id();
            self.object_entities.push(entity);
            self.object_id_to_entity
                .insert(format!("__obj_{idx}"), entity);
        }
    }

    fn rebuild_world_spatial_index(&mut self) {
        // Collect (entity, object) pairs first to avoid borrow conflicts
        // between `get_resource_mut` and `query`.
        let entries: Vec<(Entity, LevelObject)> = {
            let mut query = self.world.query::<(Entity, &ObjectData)>();
            query
                .iter(&self.world)
                .map(|(e, d)| (e, d.0.clone()))
                .collect()
        };
        let mut index = self
            .world
            .get_resource_mut::<SpatialIndex>()
            .expect("SpatialIndex resource missing");
        index.grid.clear();
        for (entity, obj) in entries {
            index.grid.insert_object(entity, &obj);
        }
    }

    fn entity_for_index(&self, idx: usize) -> Entity {
        self.object_entities
            .get(idx)
            .copied()
            .unwrap_or(Entity::PLACEHOLDER)
    }

    fn apply_spawn_internal(
        &mut self,
        position: [f32; 3],
        direction: Direction,
        center_to_grid: bool,
    ) {
        let spawn_position = if center_to_grid {
            center_spawn_position(position)
        } else {
            position
        };
        self.position = spawn_position;
        self.direction = direction;
        self.elapsed_seconds = 0.0;
        self.speed = BASE_PLAYER_SPEED;
        self.vertical_velocity = 0.0;
        self.is_grounded = true;
        self.game_over = false;
        self.level_complete = false;
        self.completion_hold_seconds = 0.0;
        self.progress.progress_percent = 0.0;
        self.progress.completed = false;
        self.progress.gems_collected = 0;
        self.consumed_object_indices.clear();
        self.consumed_object_events.clear();
        self.trail_segments = vec![vec![spawn_position]];

        if let Some(mut clock) = self.world.get_resource_mut::<GameClock>() {
            clock.reset();
        }
        if let Some(mut meta) = self.world.get_resource_mut::<LevelMeta>() {
            meta.spawn_position = spawn_position;
            meta.spawn_direction = direction;
        }
        if let Some(mut prog) = self.world.get_resource_mut::<PlayerProgress>() {
            prog.gems_collected = 0;
            prog.progress_percent = 0.0;
            prog.completed = false;
        }

        self.spawn_player_entity(spawn_position, direction);
    }

    /// Spawn a fresh player entity in the world using the given position +
    /// direction. Does NOT mutate any legacy field — call
    /// [`Self::apply_spawn`] for that. Used by [`Self::update`] when a test
    /// sets `started = true` and `position` without calling `apply_spawn`.
    fn spawn_player_entity(&mut self, position: [f32; 3], direction: Direction) {
        if let Some(prev) = self.player_entity.take()
            && let Ok(e) = self.world.get_entity_mut(prev)
        {
            e.despawn();
        }
        let entity = self
            .world
            .spawn(PlayerBundle {
                player: crate::components::Player,
                position: PlayerPosition(position),
                direction: PlayerDirection(direction),
                speed: PlayerSpeed(self.speed),
                vertical_velocity: VerticalVelocity(self.vertical_velocity),
                grounded: Grounded(self.is_grounded),
                trail: Trail {
                    segments: self.trail_segments.clone(),
                },
                spawn_position: SpawnPosition(position),
                spawn_direction: SpawnDirection(direction),
            })
            .id();
        self.player_entity = Some(entity);
    }

    pub fn apply_spawn(&mut self, position: [f32; 3], direction: Direction) {
        self.apply_spawn_internal(position, direction, true);
    }

    pub fn apply_spawn_exact(&mut self, position: [f32; 3], direction: Direction) {
        self.apply_spawn_internal(position, direction, false);
    }

    pub fn checkpoint_state(&self) -> GameCheckpointState {
        GameCheckpointState {
            position: self.position,
            direction: self.direction,
            elapsed_seconds: self.elapsed_seconds,
            speed: self.speed,
            objects: self.objects.clone(),
            vertical_velocity: self.vertical_velocity,
            is_grounded: self.is_grounded,
            progress: self.progress,
        }
    }

    /// Build a checkpoint from the authoritative App-world gameplay values.
    ///
    /// `eg-state` uses this constructor from a normal Bevy system so practice
    /// checkpoints do not need to pass through the compatibility `GameState`
    /// wrapper. Object ordering is supplied by the caller because authored
    /// order is an editor concern rather than an ECS query guarantee.
    pub fn checkpoint_state_from_parts(parts: GameCheckpointParts) -> GameCheckpointState {
        GameCheckpointState {
            position: parts.position,
            direction: parts.direction,
            elapsed_seconds: parts.elapsed_seconds,
            speed: parts.speed,
            objects: parts.objects,
            vertical_velocity: parts.vertical_velocity,
            is_grounded: parts.is_grounded,
            progress: parts.progress,
        }
    }

    pub fn restore_checkpoint_state(&mut self, checkpoint: &GameCheckpointState) {
        self.position = checkpoint.position;
        self.direction = checkpoint.direction;
        self.elapsed_seconds = checkpoint.elapsed_seconds.max(0.0);
        self.speed = checkpoint.speed.max(0.1);
        self.objects = checkpoint.objects.clone();
        self.rebuild_behavior_cache();
        self.vertical_velocity = checkpoint.vertical_velocity;
        self.is_grounded = checkpoint.is_grounded;
        self.game_over = false;
        self.level_complete = false;
        self.completion_hold_seconds = 0.0;
        self.started = false;
        self.progress = checkpoint.progress;
        self.consumed_object_indices.clear();
        self.consumed_object_events.clear();
        self.trail_segments = vec![vec![checkpoint.position]];

        if let Some(mut clock) = self.world.get_resource_mut::<GameClock>() {
            clock.reset();
            clock.elapsed_seconds = self.elapsed_seconds;
        }
        if let Some(mut prog) = self.world.get_resource_mut::<PlayerProgress>() {
            prog.gems_collected = checkpoint.progress.gems_collected;
            prog.gems_max = checkpoint.progress.gems_max;
            prog.progress_percent = checkpoint.progress.progress_percent;
            prog.completed = false;
        }
        if let Some(mut meta) = self.world.get_resource_mut::<LevelMeta>() {
            meta.spawn_position = checkpoint.position;
            meta.spawn_direction = checkpoint.direction;
        }
        if let Some(prev) = self.player_entity.take()
            && let Ok(e) = self.world.get_entity_mut(prev)
        {
            e.despawn();
        }
        let entity = self
            .world
            .spawn(PlayerBundle {
                player: crate::components::Player,
                position: PlayerPosition(checkpoint.position),
                direction: PlayerDirection(checkpoint.direction),
                speed: PlayerSpeed(self.speed),
                vertical_velocity: VerticalVelocity(self.vertical_velocity),
                grounded: Grounded(self.is_grounded),
                trail: Trail {
                    segments: vec![vec![checkpoint.position]],
                },
                spawn_position: SpawnPosition(checkpoint.position),
                spawn_direction: SpawnDirection(checkpoint.direction),
            })
            .id();
        self.player_entity = Some(entity);
    }

    /// Apply a turn at the current `elapsed_seconds`. Mutates the legacy
    /// `direction` field immediately (matches pre-ECS behavior) and mirrors
    /// into the world `PlayerDirection` component. The schedule's
    /// [`crate::systems::turn_input_system`] is unused by the wrapper for
    /// now — it was originally added for input recording, but the legacy
    /// semantics are "tap = immediate toggle" so we don't double-apply.
    pub fn turn_right(&mut self) {
        if self.game_over || self.level_complete || !self.started || !self.is_grounded {
            return;
        }
        self.push_to_active_trail(self.position);
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
        if let Some(entity) = self.player_entity
            && let Some(mut dir) = self.world.get_mut::<PlayerDirection>(entity)
        {
            dir.0 = self.direction;
        }
    }

    /// Run one simulation tick.
    pub fn update(&mut self, dt: f32) {
        puffin::profile_scope!("GameUpdate");
        self.consumed_object_indices.clear();

        if self.level_complete {
            self.completion_hold_seconds = (self.completion_hold_seconds - dt.max(0.0)).max(0.0);
            return;
        }
        if self.game_over {
            return;
        }
        if !self.started {
            return;
        }

        // Ensure a player entity exists. Tests that set `position` and
        // `started` directly without calling `apply_spawn` rely on this.
        // We do NOT mutate the legacy fields — that would clobber
        // `self.speed`, `self.position`, etc. that the test set up.
        if self.player_entity.is_none() {
            self.spawn_player_entity(self.position, self.direction);
        }

        // Sync mutable per-frame inputs to the world before ticking.
        if let Some(mut clock) = self.world.get_resource_mut::<GameClock>() {
            clock.level_duration_seconds = self.level_duration_seconds;
            clock.started = self.started;
        }
        if let Some(mut prog) = self.world.get_resource_mut::<PlayerProgress>() {
            prog.gems_collected = self.progress.gems_collected;
            prog.gems_max = self.progress.gems_max;
        }
        if let Some(mut meta) = self.world.get_resource_mut::<LevelMeta>() {
            meta.spawn_position = self.position;
            meta.spawn_direction = self.direction;
        }
        // Mirror runtime player state into the world components.
        if let Some(entity) = self.player_entity {
            if let Some(mut pos) = self.world.get_mut::<PlayerPosition>(entity) {
                pos.0 = self.position;
            }
            if let Some(mut dir) = self.world.get_mut::<PlayerDirection>(entity) {
                dir.0 = self.direction;
            }
            if let Some(mut speed) = self.world.get_mut::<PlayerSpeed>(entity) {
                speed.0 = self.speed;
            }
            if let Some(mut vvel) = self.world.get_mut::<VerticalVelocity>(entity) {
                vvel.0 = self.vertical_velocity;
            }
            if let Some(mut ground) = self.world.get_mut::<Grounded>(entity) {
                ground.0 = self.is_grounded;
            }
        }

        // Set the per-frame dt for the schedule's systems to read.
        if let Some(mut frame) = self.world.get_resource_mut::<FrameDelta>() {
            frame.0 = dt;
        }

        // Run the schedule once. The systems use `FrameDelta` for time
        // advancement; collision and consumption still run when dt == 0.
        self.schedule.run(&mut self.world);

        // Sync world -> fields.
        self.sync_from_world();
    }

    /// Copy current world state into the legacy `pub` fields.
    pub fn sync_from_world(&mut self) {
        if let Some(clock) = self.world.get_resource::<GameClock>() {
            self.elapsed_seconds = clock.elapsed_seconds;
            self.game_over = clock.game_over;
            self.level_complete = clock.level_complete;
            self.completion_hold_seconds = clock.completion_hold_seconds;
            self.started = clock.started;
        }
        if let Some(entity) = self.player_entity {
            let mut query = self.world.query::<(
                &PlayerPosition,
                &PlayerDirection,
                &PlayerSpeed,
                &VerticalVelocity,
                &Grounded,
                &Trail,
            )>();
            if let Ok((pos, dir, speed, vv, g, trail)) = query.get(&self.world, entity) {
                self.position = pos.0;
                self.direction = dir.0;
                self.speed = speed.0;
                self.vertical_velocity = vv.0;
                self.is_grounded = g.0;
                self.trail_segments = trail.segments.clone();
            }
        }
        if let Some(prog) = self.world.get_resource::<PlayerProgress>() {
            self.progress = PlayerLevelProgress {
                progress_percent: prog.progress_percent,
                completed: prog.completed,
                gems_collected: prog.gems_collected,
                gems_max: prog.gems_max,
            };
        }
        self.sync_objects_from_world();

        self.consumed_object_events.clear();
        if let Some(mut events) = self.world.get_resource_mut::<ConsumedEventsBuffer>() {
            for event in events.events.drain(..) {
                self.consumed_object_events.push(event);
            }
        }
    }

    /// Copy gameplay values from an external App world into the legacy field
    /// surface used by the remaining renderer and compatibility helpers.
    ///
    /// The external world is authoritative; this method deliberately does
    /// not rebuild the wrapper's private ECS world. It is a presentation
    /// compatibility adapter while `eg-state` finishes moving render and
    /// lifecycle consumers onto the App world.
    pub fn sync_from_external_world(&mut self, world: &mut World) {
        if let Some(clock) = world.get_resource::<GameClock>() {
            self.elapsed_seconds = clock.elapsed_seconds;
            self.level_duration_seconds = clock.level_duration_seconds;
            self.game_over = clock.game_over;
            self.level_complete = clock.level_complete;
            self.completion_hold_seconds = clock.completion_hold_seconds;
            self.started = clock.started;
        }

        let player = {
            let mut query = world.query::<(
                &PlayerPosition,
                &PlayerDirection,
                &PlayerSpeed,
                &VerticalVelocity,
                &Grounded,
                &Trail,
            )>();
            query.iter(world).next().map(
                |(position, direction, speed, vertical_velocity, grounded, trail)| {
                    (
                        position.0,
                        direction.0,
                        speed.0,
                        vertical_velocity.0,
                        grounded.0,
                        trail.segments.clone(),
                    )
                },
            )
        };
        if let Some((position, direction, speed, vertical_velocity, grounded, trail)) = player {
            self.position = position;
            self.direction = direction;
            self.speed = speed;
            self.vertical_velocity = vertical_velocity;
            self.is_grounded = grounded;
            self.trail_segments = trail;
        }

        if let Some(progress) = world.get_resource::<PlayerProgress>() {
            self.progress = PlayerLevelProgress {
                progress_percent: progress.progress_percent,
                completed: progress.completed,
                gems_collected: progress.gems_collected,
                gems_max: progress.gems_max,
            };
        }

        let mut objects = {
            let mut query = world.query::<(&ObjectId, &ObjectData)>();
            query
                .iter(world)
                .map(|(id, data)| (id.0.clone(), data.0.clone()))
                .collect::<Vec<_>>()
        };
        objects
            .sort_by(|(left, _), (right, _)| object_order_key(left).cmp(&object_order_key(right)));
        self.objects = objects.into_iter().map(|(_, object)| object).collect();
    }

    fn sync_objects_from_world(&mut self) {
        let mut world_objects: Vec<LevelObject> = Vec::with_capacity(self.object_entities.len());
        let mut new_id_to_entity: HashMap<String, Entity> = HashMap::new();
        let mut new_entities: Vec<Entity> = Vec::with_capacity(self.object_entities.len());
        let mut new_behaviors: Vec<CachedBlockBehavior> = Vec::new();
        let mut query = self.world.query::<(&ObjectData, &CachedBlockBehavior)>();
        for (idx, entity) in self.object_entities.iter().enumerate() {
            if let Ok((data, behavior)) = query.get(&self.world, *entity) {
                world_objects.push(data.0.clone());
                new_id_to_entity.insert(format!("__obj_{idx}"), *entity);
                new_entities.push(*entity);
                new_behaviors.push(*behavior);
            }
        }
        self.objects = world_objects;
        self.object_entities = new_entities;
        self.object_id_to_entity = new_id_to_entity;
        self.cached_behaviors = new_behaviors;
        if self.cached_behaviors.is_empty() && !self.objects.is_empty() {
            self.cached_behaviors = self
                .objects
                .iter()
                .map(|obj| CachedBlockBehavior::from_block_id(&obj.block_id))
                .collect();
        }
        self.spatial_grid.clear();
        for (idx, obj) in self.objects.iter().enumerate() {
            self.spatial_grid
                .insert_object(self.entity_for_index(idx), obj);
        }
    }

    pub fn has_animated_blocks(&self) -> bool {
        false
    }

    pub fn take_consumed_object_indices(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.consumed_object_indices)
    }

    pub fn take_consumed_object_events(&mut self) -> Vec<ConsumedObjectEvent> {
        std::mem::take(&mut self.consumed_object_events)
    }

    pub fn prune_consumed_indices_from_objects(
        objects: &mut Vec<LevelObject>,
        consumed_indices: Vec<usize>,
    ) {
        let mut indices = consumed_indices;
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            if index < objects.len() {
                objects.remove(index);
            }
        }
    }

    #[allow(dead_code)]
    fn complete_level(&mut self) {
        self.level_complete = true;
        self.completion_hold_seconds = 0.6;
        self.started = false;
        if let Some(clock) = self.world.get_resource::<GameClock>() {
            let elapsed = clock.elapsed_seconds;
            let dur = clock.level_duration_seconds;
            if let Some(mut prog) = self.world.get_resource_mut::<PlayerProgress>() {
                if dur.is_finite() && dur > 0.0 {
                    prog.progress_percent = (elapsed / dur * 100.0).clamp(0.0, 100.0);
                }
                prog.progress_percent = 100.0;
                prog.completed = true;
            }
        }
    }

    #[allow(dead_code)]
    fn update_progress_percent(&mut self, completed: bool) {
        if self.level_duration_seconds.is_finite() && self.level_duration_seconds > 0.0 {
            self.progress.progress_percent =
                (self.elapsed_seconds / self.level_duration_seconds * 100.0).clamp(0.0, 100.0);
        }
        if completed {
            self.progress.progress_percent = 100.0;
            self.progress.completed = true;
        }
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    fn start_new_trail_segment(&mut self, point: [f32; 3]) {
        self.trail_segments.push(vec![point]);
        if let Some(entity) = self.player_entity
            && let Some(mut t) = self.world.get_mut::<Trail>(entity)
        {
            t.segments.push(vec![point]);
        }
    }

    fn push_to_active_trail(&mut self, point: [f32; 3]) {
        const MIN_DELTA: f32 = 0.001;
        if let Some(segment) = self.trail_segments.last_mut() {
            if let Some(last) = segment.last()
                && (last[0] - point[0]).abs() < MIN_DELTA
                && (last[1] - point[1]).abs() < MIN_DELTA
                && (last[2] - point[2]).abs() < MIN_DELTA
            {
                return;
            }
            segment.push(point);
        } else {
            self.trail_segments.push(vec![point]);
        }
        if let Some(entity) = self.player_entity
            && let Some(mut t) = self.world.get_mut::<Trail>(entity)
        {
            t.segments = self.trail_segments.clone();
        }
    }

    #[cfg(test)]
    pub fn top_surface_y_at(&self, x: f32, z: f32, max_y: f32) -> Option<f32> {
        let mut top_surface: Option<f32> = Some(0.0);
        let query_entities = self.spatial_grid.query_point(x, z);

        for &entity in query_entities {
            let i = self
                .object_entities
                .iter()
                .position(|e| *e == entity)
                .unwrap_or(usize::MAX);
            if i == usize::MAX {
                continue;
            }
            let obj = &self.objects[i];
            let is_support = self
                .cached_behaviors
                .get(i)
                .map(|b| b.support_surface)
                .unwrap_or_else(|| {
                    crate::components::CachedBlockBehavior::from_block_id(&obj.block_id)
                        .support_surface
                });
            if !is_support {
                continue;
            }
            if super::physics::object_xz_contains(obj, x, z) {
                let top = obj.position[1] + obj.size[1];
                if top <= max_y {
                    top_surface = match top_surface {
                        Some(existing) if existing > top => Some(existing),
                        _ => Some(top),
                    };
                }
            }
        }

        top_surface
    }

    /// Borrowed read access to the underlying world (for tests / advanced
    /// callers). Not part of the legacy API.
    #[allow(dead_code)]
    pub fn world(&self) -> &World {
        &self.world
    }
    #[allow(dead_code)]
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Borrowed read access to the underlying schedule.
    #[allow(dead_code)]
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }
    #[allow(dead_code)]
    pub fn schedule_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }
}

/// Build the [`Schedule`] for the gameplay tick.
pub fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.configure_sets(
        (
            GameplaySet::TurnInput,
            GameplaySet::AdvancePlayer,
            GameplaySet::Collision,
            GameplaySet::ConsumeObjects,
            GameplaySet::GroundGravity,
            GameplaySet::Trail,
            GameplaySet::Completion,
            GameplaySet::Progress,
        )
            .chain(),
    );
    schedule
        .add_systems(systems::turn_input_system.in_set(GameplaySet::TurnInput))
        .add_systems(systems::advance_player_system.in_set(GameplaySet::AdvancePlayer))
        .add_systems(systems::collision_query_system.in_set(GameplaySet::Collision))
        .add_systems(systems::consume_objects_system.in_set(GameplaySet::ConsumeObjects))
        .add_systems(systems::ground_gravity_system.in_set(GameplaySet::GroundGravity))
        .add_systems(systems::trail_system.in_set(GameplaySet::Trail))
        .add_systems(systems::completion_system.in_set(GameplaySet::Completion))
        .add_systems(systems::progress_update_system.in_set(GameplaySet::Progress));
    schedule
}

// Silence "unused import" warnings for items kept for downstream callers
// (eg-state, tests).
#[allow(dead_code)]
fn _silence_unused(
    _aabb_overlaps_cuboid_xz: fn(
        f32,
        f32,
        f32,
        f32,
        &LevelObject,
        eg_block_geometry::WorldCuboid,
    ) -> bool,
    _hitbox_cuboids: fn(&LevelObject) -> Vec<eg_block_geometry::WorldCuboid>,
    _vecdeque: fn() -> VecDeque<u8>,
) {
}
