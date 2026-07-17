/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Practice checkpoints captured from the authoritative gameplay world.

use eg_types::{Direction, LevelObject, PlayerLevelProgress};

use crate::physics::BASE_PLAYER_SPEED;

/// Snapshot of gameplay state used by practice mode checkpoints.
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

impl Default for GameCheckpointState {
    fn default() -> Self {
        Self::from_parts(GameCheckpointParts {
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
            elapsed_seconds: 0.0,
            speed: BASE_PLAYER_SPEED,
            objects: Vec::new(),
            vertical_velocity: 0.0,
            is_grounded: true,
            progress: PlayerLevelProgress::default(),
        })
    }
}

impl GameCheckpointState {
    pub fn progress(&self) -> PlayerLevelProgress {
        self.progress
    }

    pub fn from_parts(parts: GameCheckpointParts) -> Self {
        Self {
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
}

/// Values captured from an authoritative App-world gameplay state.
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
