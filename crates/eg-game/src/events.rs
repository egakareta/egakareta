/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Plain Rust event types emitted by the gameplay systems.
//!
//! Bevy 0.19's `Event` trait is for fire-and-forget observer triggers; for
//! buffered per-frame events we use a regular struct + a `Resource` buffer
//! ([`ConsumedEventsBuffer`]). App-world consumers drain that buffer
//! when they need the consumed object data; gameplay systems own no
//! duplicate state mirror.
//!
//! REFACTOR POINT(full-bevy): [`ConsumedEventsBuffer`] resource becomes a
//! `Message<ConsumedObjectEvent>` in the full-Bevy migration.

use bevy_ecs::prelude::*;
use eg_types::LevelObject;

/// Emitted when an object entity is consumed (collectible or
/// portal-with-flag). Consumed by `eg-state` to play block-effect SFX /
/// particles.
#[derive(Clone, Debug)]
pub struct ConsumedObjectEvent {
    pub object: LevelObject,
}

/// Per-frame buffer of [`ConsumedObjectEvent`]s. Drained by the consuming
/// App-world schedule each tick.
#[derive(Resource, Default)]
pub struct ConsumedEventsBuffer {
    pub events: Vec<ConsumedObjectEvent>,
}
