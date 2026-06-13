/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(test)]
use super::physics::object_xz_contains;
use super::physics::{aabb_overlaps_object_xz, BASE_PLAYER_SPEED};
use super::spatial::SpatialGrid;
use crate::block_repository::{resolve_block_definition, BlockCollision};
use crate::types::{Direction, LevelObject, PlayerLevelProgress, SpawnDirection};

const PLAYER_WIDTH: f32 = 0.8;
const PLAYER_HEIGHT: f32 = 0.8;
const PLAYER_FOOTPRINT_TOLERANCE: f32 = PLAYER_WIDTH * 0.05;

/// Pre-resolved block behavior cached alongside each object to avoid
/// repeated HashMap lookups during per-frame collision/support scans.
#[derive(Clone, Copy)]
pub(crate) struct CachedBlockBehavior {
    pub(crate) collision: BlockCollision,
    pub(crate) speed_multiplier: f32,
    pub(crate) support_surface: bool,
    pub(crate) consumed_on_overlap: bool,
    pub(crate) gem_value: u32,
}

impl CachedBlockBehavior {
    pub(crate) fn from_block_id(block_id: &str) -> Self {
        let def = resolve_block_definition(block_id);
        Self {
            collision: def.behavior.collision,
            speed_multiplier: def.behavior.speed_multiplier,
            support_surface: def.behavior.support_surface,
            consumed_on_overlap: def.behavior.consumed_on_overlap,
            gem_value: def.behavior.gem_value,
        }
    }

    pub(crate) fn collectible_gem_value(self) -> u32 {
        if matches!(self.collision, BlockCollision::Collectible) {
            self.gem_value.max(1)
        } else {
            0
        }
    }
}

/// The core gameplay state containing player position, movement, and level objects.
///
/// Manages the player's position, direction, speed, trail, collision with level objects,
/// and game progression states like game over and level completion.
#[derive(Clone)]
pub(crate) struct ConsumedObjectEvent {
    pub(crate) object: LevelObject,
}

#[derive(Clone)]
pub(crate) struct GameCheckpointState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: Direction,
    pub(crate) elapsed_seconds: f32,
    pub(crate) speed: f32,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) vertical_velocity: f32,
    pub(crate) is_grounded: bool,
    progress: PlayerLevelProgress,
}

pub(crate) struct GameState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: Direction,
    pub(crate) elapsed_seconds: f32,
    pub(crate) level_duration_seconds: f32,
    pub(crate) speed: f32,
    pub(crate) trail_segments: Vec<Vec<[f32; 3]>>,
    pub(crate) objects: Vec<LevelObject>,
    cached_behaviors: Vec<CachedBlockBehavior>,
    spatial_grid: SpatialGrid,
    pub(crate) vertical_velocity: f32,
    pub(crate) is_grounded: bool,
    pub(crate) game_over: bool,
    pub(crate) level_complete: bool,
    pub(crate) completion_hold_seconds: f32,
    pub(crate) started: bool,
    progress: PlayerLevelProgress,
    consumed_object_indices: Vec<usize>,
    consumed_object_events: Vec<ConsumedObjectEvent>,
}

pub(crate) fn center_spawn_position(position: [f32; 3]) -> [f32; 3] {
    [
        position[0].floor() + 0.5,
        position[1],
        position[2].floor() + 0.5,
    ]
}

impl GameState {
    pub(crate) fn new() -> Self {
        Self {
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
        }
    }

    pub(crate) fn set_level_duration_seconds(&mut self, duration_seconds: f32) {
        self.level_duration_seconds = if duration_seconds.is_finite() && duration_seconds > 0.0 {
            duration_seconds
        } else {
            f32::INFINITY
        };
    }

    /// Rebuild the cached block behavior array from current objects.
    pub(crate) fn rebuild_behavior_cache(&mut self) {
        puffin::profile_scope!("GameRebuildBehaviorCache");
        self.cached_behaviors = self
            .objects
            .iter()
            .map(|obj| CachedBlockBehavior::from_block_id(&obj.block_id))
            .collect();

        self.rebuild_spatial_grid();
    }

    pub(crate) fn initialize_level_progress_from_objects(&mut self) {
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
    }

    pub(crate) fn level_progress(&self) -> PlayerLevelProgress {
        self.progress.sanitized()
    }

    fn rebuild_spatial_grid(&mut self) {
        puffin::profile_scope!("GameRebuildSpatialGrid");
        self.spatial_grid.clear();
        for (idx, obj) in self.objects.iter().enumerate() {
            self.spatial_grid.insert_object(idx, obj);
        }
    }

    fn apply_spawn_internal(
        &mut self,
        position: [f32; 3],
        direction: SpawnDirection,
        center_to_grid: bool,
    ) {
        let spawn_position = if center_to_grid {
            center_spawn_position(position)
        } else {
            position
        };
        self.position = spawn_position;
        self.direction = direction.into();
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
    }

    pub(crate) fn apply_spawn(&mut self, position: [f32; 3], direction: SpawnDirection) {
        self.apply_spawn_internal(position, direction, true);
    }

    pub(crate) fn apply_spawn_exact(&mut self, position: [f32; 3], direction: SpawnDirection) {
        self.apply_spawn_internal(position, direction, false);
    }

    pub(crate) fn checkpoint_state(&self) -> GameCheckpointState {
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

    pub(crate) fn restore_checkpoint_state(&mut self, checkpoint: &GameCheckpointState) {
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
    }

    pub(crate) fn turn_right(&mut self) {
        if self.game_over || self.level_complete || !self.started || !self.is_grounded {
            return;
        }
        self.push_to_active_trail(self.position);
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    pub(crate) fn update(&mut self, dt: f32) {
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

        let remaining_level_time = self.remaining_level_time();
        if remaining_level_time <= 0.0 {
            self.complete_level();
            return;
        }
        let step = remaining_level_time.min(dt.max(0.0));

        self.elapsed_seconds += step;
        self.update_progress_percent(false);

        const GRAVITY: f32 = 26.0;
        const MAX_FALL_SPEED: f32 = 40.0;
        const SNAP_DISTANCE: f32 = 0.3;
        const DEATH_Y: f32 = -6.0;

        let delta = match self.direction {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };

        self.position[0] += delta[0] * self.speed * step;
        self.position[2] += delta[1] * self.speed * step;

        // Collision detection
        let mut hit_death = false;
        let mut hit_portals = Vec::new();
        let mut hit_collectibles = Vec::new();

        let x = self.position[0];
        let y = self.position[1];
        let z = self.position[2];

        let s_min_x = x - PLAYER_WIDTH / 2.0 + PLAYER_FOOTPRINT_TOLERANCE;
        let s_max_x = x + PLAYER_WIDTH / 2.0 - PLAYER_FOOTPRINT_TOLERANCE;
        let s_min_z = z - PLAYER_WIDTH / 2.0 + PLAYER_FOOTPRINT_TOLERANCE;
        let s_max_z = z + PLAYER_WIDTH / 2.0 - PLAYER_FOOTPRINT_TOLERANCE;
        let s_min_y = y + PLAYER_FOOTPRINT_TOLERANCE;
        let s_max_y = y + PLAYER_HEIGHT - PLAYER_FOOTPRINT_TOLERANCE;

        let query_indices = {
            puffin::profile_scope!("GameCollisionQuery");
            self.spatial_grid
                .query_aabb(s_min_x, s_max_x, s_min_z, s_max_z)
        };

        {
            puffin::profile_scope!("GameCollisionScan");
            for i in query_indices {
                let obj = &self.objects[i];
                let o_min_y = obj.position[1];
                let o_max_y = obj.position[1] + obj.size[1];
                let behavior = self
                    .cached_behaviors
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| CachedBlockBehavior::from_block_id(&obj.block_id));

                if aabb_overlaps_object_xz(s_min_x, s_max_x, s_min_z, s_max_z, obj)
                    && s_max_y > o_min_y
                    && s_min_y < o_max_y
                {
                    match behavior.collision {
                        BlockCollision::Portal => {
                            hit_portals.push(i);
                        }
                        BlockCollision::Collectible => {
                            hit_collectibles.push(i);
                        }
                        BlockCollision::Hazard => {
                            hit_death = true;
                        }
                        BlockCollision::Solid => {
                            hit_death = true;
                        }
                        BlockCollision::PassThrough => {}
                    }
                }
            }
        }

        if hit_death {
            self.game_over = true;
            return;
        }

        if !hit_portals.is_empty() || !hit_collectibles.is_empty() {
            puffin::profile_scope!("GameConsumableOverlap");
            let mut consumed_indices = Vec::new();
            let mut consumed_event_indices = Vec::new();

            for i in hit_portals {
                if let Some(behavior) = self.cached_behaviors.get(i).copied() {
                    self.speed *= behavior.speed_multiplier.max(0.1);
                    if behavior.consumed_on_overlap {
                        consumed_indices.push(i);
                    }
                } else if let Some(portal) = self.objects.get(i) {
                    let behavior = &resolve_block_definition(&portal.block_id).behavior;
                    self.speed *= behavior.speed_multiplier.max(0.1);
                    if behavior.consumed_on_overlap {
                        consumed_indices.push(i);
                    }
                }
            }

            for i in hit_collectibles {
                if let Some(behavior) = self.cached_behaviors.get(i).copied() {
                    self.progress.gems_collected = self
                        .progress
                        .gems_collected
                        .saturating_add(behavior.collectible_gem_value())
                        .min(self.progress.gems_max);
                    consumed_indices.push(i);
                    consumed_event_indices.push(i);
                } else if let Some(object) = self.objects.get(i) {
                    let behavior = CachedBlockBehavior::from_block_id(&object.block_id);
                    self.progress.gems_collected = self
                        .progress
                        .gems_collected
                        .saturating_add(behavior.collectible_gem_value())
                        .min(self.progress.gems_max);
                    consumed_indices.push(i);
                    consumed_event_indices.push(i);
                }
            }

            consumed_indices.sort_unstable();
            consumed_indices.dedup();
            consumed_event_indices.sort_unstable();
            consumed_event_indices.dedup();
            for i in consumed_indices.iter().copied().rev() {
                if i < self.objects.len() {
                    let object = self.objects.remove(i);
                    if i < self.cached_behaviors.len() {
                        self.cached_behaviors.remove(i);
                    }
                    self.consumed_object_indices.push(i);
                    if consumed_event_indices.binary_search(&i).is_ok() {
                        self.consumed_object_events
                            .push(ConsumedObjectEvent { object });
                    }
                }
            }
            if !consumed_indices.is_empty() {
                self.rebuild_spatial_grid();
            }
        }

        let was_grounded = self.is_grounded;
        let mut is_grounded = false;

        let support_height = {
            puffin::profile_scope!("GameSupportScan");
            self.top_surface_y_under_aabb(
                s_min_x,
                s_max_x,
                s_min_z,
                s_max_z,
                self.position[1] + SNAP_DISTANCE,
            )
        };

        if let Some(top) = support_height {
            let close_enough =
                self.position[1] <= top + SNAP_DISTANCE && self.position[1] >= top - SNAP_DISTANCE;
            if self.vertical_velocity <= 0.0 && close_enough {
                self.position[1] = top;
                self.vertical_velocity = 0.0;
                is_grounded = true;
            } else {
                self.vertical_velocity =
                    (self.vertical_velocity - GRAVITY * step).max(-MAX_FALL_SPEED);
                self.position[1] += self.vertical_velocity * step;
            }
        } else {
            self.vertical_velocity = (self.vertical_velocity - GRAVITY * step).max(-MAX_FALL_SPEED);
            self.position[1] += self.vertical_velocity * step;
        }

        if was_grounded && !is_grounded {
            self.push_to_active_trail(self.position);
        } else if !was_grounded && is_grounded {
            self.start_new_trail_segment(self.position);
        }

        self.is_grounded = is_grounded;

        if self.position[1] < DEATH_Y {
            self.game_over = true;
        }

        if self.elapsed_seconds >= self.level_duration_seconds {
            self.complete_level();
        }
    }

    pub(crate) fn has_animated_blocks(&self) -> bool {
        false
    }

    pub(crate) fn take_consumed_object_indices(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.consumed_object_indices)
    }

    pub(crate) fn take_consumed_object_events(&mut self) -> Vec<ConsumedObjectEvent> {
        std::mem::take(&mut self.consumed_object_events)
    }

    pub(crate) fn prune_consumed_indices_from_objects(
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

    fn remaining_level_time(&self) -> f32 {
        if self.level_duration_seconds.is_finite() {
            (self.level_duration_seconds - self.elapsed_seconds).max(0.0)
        } else {
            f32::INFINITY
        }
    }

    fn complete_level(&mut self) {
        self.level_complete = true;
        self.completion_hold_seconds = 0.6;
        self.started = false;
        self.update_progress_percent(true);
    }

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

    fn start_new_trail_segment(&mut self, point: [f32; 3]) {
        self.trail_segments.push(vec![point]);
    }

    fn push_to_active_trail(&mut self, point: [f32; 3]) {
        const MIN_DELTA: f32 = 0.001;
        if let Some(segment) = self.trail_segments.last_mut() {
            if let Some(last) = segment.last() {
                if (last[0] - point[0]).abs() < MIN_DELTA
                    && (last[1] - point[1]).abs() < MIN_DELTA
                    && (last[2] - point[2]).abs() < MIN_DELTA
                {
                    return;
                }
            }
            segment.push(point);
        } else {
            self.trail_segments.push(vec![point]);
        }
    }

    #[cfg(test)]
    pub(crate) fn top_surface_y_at(&self, x: f32, z: f32, max_y: f32) -> Option<f32> {
        let mut top_surface: Option<f32> = Some(0.0);
        let query_indices = self.spatial_grid.query_point(x, z);

        for &i in query_indices {
            let obj = &self.objects[i];
            let is_support = self
                .cached_behaviors
                .get(i)
                .map(|b| b.support_surface)
                .unwrap_or_else(|| {
                    resolve_block_definition(&obj.block_id)
                        .behavior
                        .support_surface
                });
            if !is_support {
                continue;
            }
            if object_xz_contains(obj, x, z) {
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

    pub(crate) fn top_surface_y_under_aabb(
        &self,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        max_y: f32,
    ) -> Option<f32> {
        let mut top_surface: Option<f32> = Some(0.0);
        let query_indices = self.spatial_grid.query_aabb(min_x, max_x, min_z, max_z);

        for i in query_indices {
            let obj = &self.objects[i];
            let is_support = self
                .cached_behaviors
                .get(i)
                .map(|b| b.support_surface)
                .unwrap_or_else(|| {
                    resolve_block_definition(&obj.block_id)
                        .behavior
                        .support_surface
                });
            if !is_support {
                continue;
            }
            if aabb_overlaps_object_xz(min_x, max_x, min_z, max_z, obj) {
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
}
