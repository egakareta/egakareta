/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Typed Bevy messages emitted by the gameplay systems.

use bevy::ecs::prelude::*;
use eg_types::LevelObject;

/// Emitted when an object entity is consumed (collectible or
/// portal-with-flag). Consumed by `eg-state` to play block-effect SFX /
/// particles.
#[derive(Clone, Debug, Message)]
pub struct ConsumedObjectEvent {
    pub object: LevelObject,
}

/// Emitted once when the active player dies during a fixed gameplay tick.
#[derive(Clone, Copy, Debug, Default, Message)]
pub struct PlayerDiedEvent;
