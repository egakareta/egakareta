/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS components for eg-game.
//!
//! These are the building blocks of the headless gameplay simulation. The
//! `eg-game` crate stays free of wgpu/egui/I/O — it only operates on these
//! components + the resources in [`crate::resources`].

use bevy_ecs::prelude::*;
use eg_blocks::BlockCollision;
use eg_types::{Direction, LevelObject};

/// Pre-resolved block behavior cached alongside each object to avoid
/// repeated HashMap lookups during per-frame collision/support scans.
#[derive(Clone, Copy, Component)]
pub struct CachedBlockBehavior {
    pub collision: BlockCollision,
    pub speed_multiplier: f32,
    pub support_surface: bool,
    pub consumed_on_overlap: bool,
    pub gem_value: u32,
}

impl CachedBlockBehavior {
    pub fn from_block_id(block_id: &str) -> Self {
        let def = eg_blocks::BlockDefinition::get(block_id);
        Self {
            collision: def.behavior.collision,
            speed_multiplier: def.behavior.speed_multiplier,
            support_surface: def.behavior.support_surface,
            consumed_on_overlap: def.behavior.consumed_on_overlap,
            gem_value: def.behavior.gem_value,
        }
    }

    pub fn collectible_gem_value(self) -> u32 {
        if matches!(self.collision, BlockCollision::Collectible) {
            self.gem_value.max(1)
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Player components (single Player entity, runtime state)
// ---------------------------------------------------------------------------

/// Marker for the player entity. There is at most one Player.
#[derive(Component)]
pub struct Player;

/// Player world-space position. Updated by `advance_player_system`.
#[derive(Component, Default)]
pub struct PlayerPosition(pub [f32; 3]);

/// Player facing direction. Mutated by the turn input handler.
#[derive(Component, Default)]
pub struct PlayerDirection(pub Direction);

/// Current movement speed. Modified by portals (speed multipliers).
#[derive(Component)]
pub struct PlayerSpeed(pub f32);

/// Vertical velocity for gravity integration. Negative when falling.
#[derive(Component, Default)]
pub struct VerticalVelocity(pub f32);

/// Whether the player is on a support surface. Updated by `ground_gravity_system`.
#[derive(Component)]
pub struct Grounded(pub bool);

/// Trail line segments behind the player. The outer Vec is segments; the
/// inner Vec is points along the active segment.
#[derive(Component, Default)]
pub struct Trail {
    pub segments: Vec<Vec<[f32; 3]>>,
}

/// Where the player spawns when play begins. Authored (immutable).
#[derive(Component)]
pub struct SpawnPosition(pub [f32; 3]);

/// Which way the player faces when play begins. Authored (immutable).
#[derive(Component)]
pub struct SpawnDirection(pub Direction);

// ---------------------------------------------------------------------------
// Object components (one per authored object entity, immutable during gameplay)
// ---------------------------------------------------------------------------

/// Marker for authored level-object entities. There is one of these per
/// `LevelObject` in the level.
#[derive(Component)]
pub struct Object;

/// Stable string id for editor lookup. Authored (immutable).
#[derive(Component)]
pub struct ObjectId(pub String);

/// The full authored object data. Treated as immutable during gameplay; the
/// runtime state lives in [`PlayState`] and the player components instead.
#[derive(Component)]
pub struct ObjectData(pub LevelObject);

// ---------------------------------------------------------------------------
// Runtime + editor markers
// ---------------------------------------------------------------------------

/// Runtime state added to object entities on play-enter, removed on play-exit.
/// This keeps authored `ObjectData` immutable so undo/redo + play/restore
/// are reversible.
#[derive(Component, Default)]
pub struct PlayState {
    pub consumed: bool,
    pub effect_age: Option<f32>,
}

/// Editor marker: object is part of the current selection set.
#[derive(Component)]
pub struct Selected;

/// Editor marker: object is hovered by the cursor.
#[derive(Component)]
pub struct Hovered;

// ---------------------------------------------------------------------------
// Bundles — convenience for `world.spawn` call sites
// ---------------------------------------------------------------------------

/// Bundle to spawn a new player entity. The runtime state defaults to
/// `Direction::Forward`, `speed = BASE_PLAYER_SPEED`, `vertical_velocity = 0`,
/// `grounded = true`, and a single-point trail at the spawn.
#[derive(Bundle)]
pub struct PlayerBundle {
    pub player: Player,
    pub position: PlayerPosition,
    pub direction: PlayerDirection,
    pub speed: PlayerSpeed,
    pub vertical_velocity: VerticalVelocity,
    pub grounded: Grounded,
    pub trail: Trail,
    pub spawn_position: SpawnPosition,
    pub spawn_direction: SpawnDirection,
}

/// Bundle to spawn a new authored object entity.
#[derive(Bundle)]
pub struct ObjectBundle {
    pub object: Object,
    pub id: ObjectId,
    pub data: ObjectData,
    pub cached_behavior: CachedBlockBehavior,
}
