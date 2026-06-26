/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! `GamePlugin` — the headless gameplay Bevy plugin.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;

use crate::events::ConsumedEventsBuffer;
use crate::resources::{
    DeathSfxFlag, FixedStepConfig, FrameDelta, GameClock, GameRuntimeTuning, PendingTurnInputs,
    PlayerProgress, SpatialIndex,
};
use crate::systems::{self, CollisionOutcome, GameplaySet};

/// `ScheduleLabel` for the headless gameplay tick.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplaySchedule;

/// Plugin that registers the headless gameplay systems + resources.
///
/// REFACTOR POINT(full-bevy): the [`Schedule`] + [`GameplaySet`] defined here
/// will be replaced by `bevy_render` + `bevy_time` `Time<Fixed>` + Bevy's
/// render graph in the full-Bevy migration. The `App` lives in `eg-state`
/// then, and the eg-state plugin just composes this with editor + render
/// plugins.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameClock>()
            .init_resource::<PlayerProgress>()
            .init_resource::<SpatialIndex>()
            .init_resource::<PendingTurnInputs>()
            .init_resource::<FixedStepConfig>()
            .init_resource::<FrameDelta>()
            .init_resource::<DeathSfxFlag>()
            .init_resource::<GameRuntimeTuning>()
            .init_resource::<CollisionOutcome>()
            .init_resource::<ConsumedEventsBuffer>();

        let mut schedule = Schedule::new(GameplaySchedule);
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
        app.add_schedule(schedule);
    }
}

/// Run a single gameplay tick on the given `World`.
pub fn tick_gameplay(world: &mut World) {
    world.run_schedule(GameplaySchedule);
}
