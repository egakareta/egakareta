/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Test harness for the eg-game ECS core.
//!
//! [`TestApp`] is the headless gameplay harness.
//! It builds a [`bevy_app::App`] with
//! [`GamePlugin`](crate::GamePlugin), exposes the underlying [`World`], and
//! runs the real [`GameplaySchedule`](crate::GameplaySchedule) for focused ECS
//! tests.

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::plugin::{GamePlugin, GameplaySchedule};

/// Headless Bevy app for gameplay-system tests.
pub struct TestApp {
    app: App,
}

impl TestApp {
    /// Build a gameplay test app with [`GamePlugin`] installed.
    pub fn minimal() -> Self {
        let mut app = App::new();
        app.add_plugins(GamePlugin);
        Self { app }
    }

    /// Build a harness from a preconfigured app.
    pub fn with_app(app: App) -> Self {
        Self { app }
    }

    /// Read-only access to the underlying app.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Mutable access to the underlying app.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Read-only access to the world.
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// Mutable access to the world.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// Run the default Bevy app schedules once.
    pub fn tick(&mut self) {
        self.app.update();
    }

    /// Run one headless gameplay tick.
    pub fn tick_gameplay(&mut self) {
        self.world_mut().run_schedule(GameplaySchedule);
    }

    /// Run one headless gameplay tick and report whether the schedule exists.
    pub fn try_tick_gameplay(&mut self) -> bool {
        self.world_mut().try_run_schedule(GameplaySchedule).is_ok()
    }

    /// Assert that the gameplay plugin registered a resource.
    pub fn assert_resource_exists<T: Resource>(&self) {
        assert!(
            self.world().contains_resource::<T>(),
            "expected `World` to contain resource `{}`",
            std::any::type_name::<T>(),
        );
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::minimal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{PlayerBundle, PlayerDirection, PlayerPosition, PlayerSpeed};
    use crate::physics::BASE_PLAYER_SPEED;
    use crate::resources::{FrameDelta, GameClock, PlayerProgress, SpatialIndex};
    use crate::{Grounded, Player, SpawnDirection, SpawnPosition, Trail, VerticalVelocity};
    use eg_types::Direction;

    #[test]
    fn test_app_minimal_registers_gameplay_resources() {
        let app = TestApp::minimal();

        app.assert_resource_exists::<GameClock>();
        app.assert_resource_exists::<PlayerProgress>();
        app.assert_resource_exists::<SpatialIndex>();
    }

    #[test]
    fn test_app_default_matches_minimal() {
        let default_app = TestApp::default();
        let minimal_app = TestApp::minimal();

        assert_eq!(
            default_app.world().contains_resource::<GameClock>(),
            minimal_app.world().contains_resource::<GameClock>()
        );
    }

    #[test]
    fn test_app_tick_gameplay_runs_schedule() {
        let mut app = TestApp::minimal();
        app.world_mut().spawn(PlayerBundle {
            player: Player,
            position: PlayerPosition([0.0, 1.0, 0.0]),
            direction: PlayerDirection(Direction::Right),
            speed: PlayerSpeed(BASE_PLAYER_SPEED),
            vertical_velocity: VerticalVelocity::default(),
            grounded: Grounded(true),
            trail: Trail::default(),
            spawn_position: SpawnPosition([0.0, 1.0, 0.0]),
            spawn_direction: SpawnDirection(Direction::Right),
        });
        {
            let mut clock = app.world_mut().resource_mut::<GameClock>();
            clock.started = true;
        }
        app.world_mut().resource_mut::<FrameDelta>().0 = 0.5;

        app.tick_gameplay();

        let position = app
            .world_mut()
            .query::<&PlayerPosition>()
            .single(app.world())
            .expect("player should exist");
        approx::assert_abs_diff_eq!(position.0[0], BASE_PLAYER_SPEED * 0.5, epsilon = 1e-6);
    }
}
