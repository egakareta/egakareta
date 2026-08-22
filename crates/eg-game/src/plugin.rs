/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! `GamePlugin` - the headless gameplay Bevy plugin.

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{prelude::*, schedule::Schedule};

use crate::{
    events::ConsumedEventsBuffer,
    resources::{
        DeathSfxFlag, FrameDelta, GameClock, GameRuntimeTuning, LevelMeta, PendingTurnInputs,
        PlayerProgress, SpatialIndex,
    },
    systems::{self, CollisionOutcome, GameplaySet},
};

/// Plugin that registers the headless gameplay systems + resources.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        initialize_gameplay_world(app.world_mut());
        app.add_systems(
            FixedUpdate,
            (
                systems::turn_input_system.in_set(GameplaySet::TurnInput),
                systems::advance_player_system.in_set(GameplaySet::AdvancePlayer),
                systems::collision_query_system.in_set(GameplaySet::Collision),
                systems::consume_objects_system.in_set(GameplaySet::ConsumeObjects),
                systems::ground_gravity_system.in_set(GameplaySet::GroundGravity),
                systems::trail_system.in_set(GameplaySet::Trail),
                systems::completion_system.in_set(GameplaySet::Completion),
                systems::progress_update_system.in_set(GameplaySet::Progress),
            ),
        );
        app.configure_sets(
            FixedUpdate,
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
    }
}

pub fn initialize_gameplay_world(world: &mut World) {
    world.init_resource::<GameClock>();
    world.init_resource::<PlayerProgress>();
    world.init_resource::<SpatialIndex>();
    world.init_resource::<LevelMeta>();
    world.init_resource::<PendingTurnInputs>();

    world.init_resource::<FrameDelta>();
    world.init_resource::<DeathSfxFlag>();
    world.init_resource::<GameRuntimeTuning>();
    world.init_resource::<CollisionOutcome>();
    world.init_resource::<ConsumedEventsBuffer>();
}

/// Build the same standard Bevy fixed-time schedule for an isolated gameplay
/// fixture such as the editor timeline preview.
pub fn build_fixed_update_schedule() -> Schedule {
    let mut schedule = Schedule::new(FixedUpdate);
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
    schedule.add_systems((
        systems::turn_input_system.in_set(GameplaySet::TurnInput),
        systems::advance_player_system.in_set(GameplaySet::AdvancePlayer),
        systems::collision_query_system.in_set(GameplaySet::Collision),
        systems::consume_objects_system.in_set(GameplaySet::ConsumeObjects),
        systems::ground_gravity_system.in_set(GameplaySet::GroundGravity),
        systems::trail_system.in_set(GameplaySet::Trail),
        systems::completion_system.in_set(GameplaySet::Completion),
        systems::progress_update_system.in_set(GameplaySet::Progress),
    ));
    schedule
}

/// Run one standard Bevy fixed-time gameplay tick on the given world.
pub fn tick_gameplay(world: &mut World) {
    world.run_schedule(FixedUpdate);
}
