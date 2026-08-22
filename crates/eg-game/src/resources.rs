/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS resources for eg-game.
//!
//! World-level state for the headless gameplay simulation. See
//! [`crate::components`] for per-entity data.

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use eg_types::{Direction, LevelObject, PlayerLevelProgress};

use crate::spatial::SpatialGrid;

/// World-level clock for the active level. `elapsed_seconds` advances each
/// `update`; `level_duration_seconds` is the level length (or +∞ for untimed
/// levels); `started` gates whether the simulation is running; `game_over`
/// and `level_complete` are terminal states.
///
/// The gameplay clock remains domain state; its per-frame advance is supplied
/// by the App gameplay schedule. The custom renderer does not participate in
/// gameplay ownership.
#[derive(Resource)]
pub struct GameClock {
    pub elapsed_seconds: f32,
    pub level_duration_seconds: f32,
    pub started: bool,
    pub game_over: bool,
    pub level_complete: bool,
    pub completion_hold_seconds: f32,
}

impl Default for GameClock {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            level_duration_seconds: f32::INFINITY,
            started: false,
            game_over: false,
            level_complete: false,
            completion_hold_seconds: 0.0,
        }
    }
}

impl GameClock {
    /// Resets the clock to t=0 with `started = false`. Keeps the level
    /// duration.
    pub fn reset(&mut self) {
        self.elapsed_seconds = 0.0;
        self.started = false;
        self.game_over = false;
        self.level_complete = false;
        self.completion_hold_seconds = 0.0;
    }
}

/// Player progress (UI-facing). Computed by `progress_update_system` from the
/// `GameClock` and the authored objects.
#[derive(Resource, Default, Clone, Copy)]
pub struct PlayerProgress {
    pub progress_percent: f32,
    pub completed: bool,
    pub gems_collected: u32,
    pub gems_max: u32,
}

impl PlayerProgress {
    pub fn sanitized(&self) -> PlayerLevelProgress {
        PlayerLevelProgress {
            progress_percent: self.progress_percent,
            completed: self.completed,
            gems_collected: self.gems_collected,
            gems_max: self.gems_max,
        }
    }
}

/// Spatial index over object entities. Rebuilt when the object set changes
/// (level load, level edit, play exit).
#[derive(Resource, Default)]
pub struct SpatialIndex {
    pub grid: SpatialGrid,
}

impl SpatialIndex {
    pub fn rebuild(&mut self, world: &mut World) {
        self.grid.clear();
        let entries: Vec<(Entity, LevelObject)> = {
            let mut query = world.query::<(Entity, &crate::components::ObjectData)>();
            query.iter(world).map(|(e, d)| (e, d.0.clone())).collect()
        };
        for (entity, obj) in entries {
            self.grid.insert_object(entity, &obj);
        }
    }
}

/// Pending turn inputs. `eg-state` pushes taps into this queue each frame;
/// `turn_input_system` drains them into the player direction. A small
/// `VecDeque` so we can pop from the front.
#[derive(Resource, Default)]
pub struct PendingTurnInputs {
    pub taps: VecDeque<TimedTurnInput>,
}

/// A pending tap: turn at the given `at_seconds` game-clock time.
#[derive(Clone, Copy, Debug)]
pub struct TimedTurnInput {
    pub at_seconds: f32,
    pub kind: TurnInputKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnInputKind {
    TurnRight,
}

/// Per-frame time delta. Set by the gameplay frame driver before running the
/// schedule. Read by `advance_player_system`, `ground_gravity_system`, and
/// `completion_system` to know how much wall-clock time to advance.
///
/// Production frame timing is sourced from bevy_time::Time<Virtual> by eg-state; this resource remains the gameplay-system input boundary.
#[derive(Resource, Default, Clone, Copy)]
pub struct FrameDelta(pub f32);

/// Resource set when the player dies this frame. Drained by
/// `eg-state` to play the death SFX. A `bool` (or unit) flag is enough — the
/// audio system polls it once per frame and resets it.
#[derive(Resource, Default)]
pub struct DeathSfxFlag(pub bool);

/// Constants used by the gameplay systems. Centralized as a resource so
/// tests can override them.
#[derive(Resource, Clone, Copy)]
pub struct GameRuntimeTuning {
    pub gravity: f32,
    pub max_fall_speed: f32,
    pub snap_distance: f32,
    pub death_y: f32,
    pub completion_hold_seconds: f32,
    pub player_width: f32,
    pub player_height: f32,
    pub player_footprint_tolerance: f32,
}

impl Default for GameRuntimeTuning {
    fn default() -> Self {
        const PLAYER_WIDTH: f32 = 0.8;
        const PLAYER_HEIGHT: f32 = 0.8;
        Self {
            gravity: 26.0,
            max_fall_speed: 40.0,
            snap_distance: 0.3,
            death_y: -6.0,
            completion_hold_seconds: 0.6,
            player_width: PLAYER_WIDTH,
            player_height: PLAYER_HEIGHT,
            player_footprint_tolerance: PLAYER_WIDTH * 0.05,
        }
    }
}

/// Initial level seed populated by `eg-state` on level load.
///
/// Live level objects live in the world as `ObjectData` entities. This seed is
/// retained only for atomic restart/reset transitions and is never updated by
/// gameplay consumption or entity replacement.
#[derive(Resource, Default, Clone)]
pub struct LevelMeta {
    pub initial_objects: Vec<LevelObject>,
    pub spawn_position: [f32; 3],
    pub spawn_direction: Direction,
    pub level_duration_seconds: f32,
}
