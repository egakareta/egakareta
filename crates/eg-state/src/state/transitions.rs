/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy state transition wiring for the eg-state App.
//!
//! `State<AppPhase>` is authoritative. The `AppPhaseRes` resource remains a
//! compatibility mirror for handlers whose signatures have not moved to
//! `Res<State<AppPhase>>` yet.

use bevy_app::App;
use bevy_ecs::prelude::World;
use bevy_state::prelude::{NextState, OnEnter, OnExit, State};
use eg_types::AppPhase;

use super::state_resources::AppPhaseRes;

/// Queue a transition requested through the temporary compatibility mirror.
///
/// This keeps existing fixture code that sets `AppPhaseRes` working while all
/// production transitions move to `NextState<AppPhase>`.
pub(super) fn queue_compatibility_phase_transition(world: &mut World) {
    if !world.contains_resource::<State<AppPhase>>()
        || !world.contains_resource::<NextState<AppPhase>>()
        || !world.contains_resource::<AppPhaseRes>()
    {
        return;
    }

    let target = world.resource::<AppPhaseRes>().current;
    let current = *world.resource::<State<AppPhase>>().get();
    if target != current {
        world.resource_mut::<NextState<AppPhase>>().set(target);
    }
}

/// Refresh the compatibility mirror after Bevy applies a state transition.
pub(super) fn sync_compatibility_phase_from_state(world: &mut World) {
    if !world.contains_resource::<State<AppPhase>>() || !world.contains_resource::<AppPhaseRes>() {
        return;
    }

    let current = *world.resource::<State<AppPhase>>().get();
    let mut phase = world.resource_mut::<AppPhaseRes>();
    phase.current = current;
}

/// Register the small remaining phase hooks on Bevy's transition schedules.
pub(super) fn register_phase_transition_hooks(app: &mut App) {
    app.add_systems(OnEnter(AppPhase::Menu), on_enter_menu)
        .add_systems(OnEnter(AppPhase::Playing), on_enter_playing)
        .add_systems(OnEnter(AppPhase::Editor), on_enter_editor)
        .add_systems(OnEnter(AppPhase::GameOver), on_enter_game_over)
        .add_systems(OnExit(AppPhase::Menu), on_exit_menu)
        .add_systems(OnExit(AppPhase::Playing), on_exit_playing)
        .add_systems(OnExit(AppPhase::Editor), on_exit_editor)
        .add_systems(OnExit(AppPhase::GameOver), on_exit_game_over);
}

fn on_enter_menu() {
    log_phase_transition("enter", AppPhase::Menu);
}

fn on_enter_playing() {
    log_phase_transition("enter", AppPhase::Playing);
}

fn on_enter_editor() {
    log_phase_transition("enter", AppPhase::Editor);
}

fn on_enter_game_over() {
    log_phase_transition("enter", AppPhase::GameOver);
}

fn on_exit_menu() {
    log_phase_transition("exit", AppPhase::Menu);
}

fn on_exit_playing() {
    log_phase_transition("exit", AppPhase::Playing);
}

fn on_exit_editor() {
    log_phase_transition("exit", AppPhase::Editor);
}

fn on_exit_game_over() {
    log_phase_transition("exit", AppPhase::GameOver);
}

fn log_phase_transition(kind: &str, phase: AppPhase) {
    puffin::profile_scope!("PhaseTransition", format!("{kind} {phase:?}"));
    log::debug!("phase transition: {} {:?}", kind, phase);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::plugins::register_state_resources;
    use bevy_ecs::world::World;
    use bevy_state::prelude::NextState;

    #[test]
    fn compatibility_phase_queues_a_bevy_transition() {
        let mut world = World::new();
        register_state_resources(&mut world);
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Playing);

        queue_compatibility_phase_transition(&mut world);

        assert!(matches!(
            world.resource::<NextState<AppPhase>>(),
            NextState::Pending(AppPhase::Playing)
        ));
    }

    #[test]
    fn compatibility_phase_mirrors_the_authoritative_state() {
        let mut world = World::new();
        register_state_resources(&mut world);
        world.insert_resource(State::new(AppPhase::Editor));

        sync_compatibility_phase_from_state(&mut world);

        assert_eq!(world.resource::<AppPhaseRes>().current, AppPhase::Editor);
    }
}
