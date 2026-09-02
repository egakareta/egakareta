/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Buffered messages emitted by the gameplay systems.
//!
//! Bevy 0.19's `Message` API gives gameplay a typed, pull-based output channel
//! without adding another manually managed resource buffer. App-world
//! consumers drain these messages at the boundary where they handle the
//! gameplay result.

use bevy_ecs::prelude::*;
use eg_types::LevelObject;

/// Emitted when an object entity is consumed (collectible or
/// portal-with-flag). Consumed by `eg-state` to play block-effect SFX /
/// particles.
#[derive(Message, Clone, Debug)]
pub struct ConsumedObjectEvent {
    pub object: LevelObject,
}

/// Emitted once when the player enters the game-over state.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct PlayerDied;
