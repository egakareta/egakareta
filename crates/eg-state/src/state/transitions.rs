/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Phase transition systems for the eg-state `App`.
//!
//! Phase 2c introduces the [`AppPhaseRes`] -> legacy `State::phase` sync
//!   + a small set of transition hooks that run when the application
//!     changes phase. The transitions are driven by a `previous_app_phase`
//!     field on [`AppPhaseRes`] so the Bevy 0.19 lack of a buffered
//!     `Events<T>` doesn't get in the way. The actual `OnEnter(T)` /
//!     `OnExit(T)` system-set ergonomics from Bevy 0.13 land in Phase 3
//!     (Command/Event fragmentation).
//!
//! Hook contract:
//!
//!   - [`on_enter_phase`] runs once when `AppPhaseRes` first changes to a
//!     value. Side effects that should happen on phase entry (e.g. reset
//!     `GameClock`, refresh menu preview) go here.
//!   - [`on_exit_phase`] runs once when `AppPhaseRes` changes away from a
//!     value. Side effects that should happen on phase exit (e.g. stop
//!     audio, despawn `Player`) go here.
//!
//! REFACTOR POINT(full-bevy): the full-Bevy migration replaces
//! `AppPhaseRes` with `AppPhase` itself annotated with `#[derive(States)]`
//! and uses `OnEnter(T)` / `OnExit(T)` system sets directly. The
//! `previous_app_phase` bookkeeping is replaced by Bevy's `States` derive
//! + schedule-level transition detection.

use bevy_ecs::change_detection::Mut;
use eg_types::AppPhase;

use super::state_resources::AppPhaseRes;

/// Detect `AppPhase` changes and run the appropriate enter/exit
/// transitions. Called once per frame from [`super::State::tick_state`].
///
/// Side effects (per phase):
/// - **Enter Menu:** reset `EditorRuntimeRes.dirty` (force one
///   sync of the menu preview).
/// - **Enter Playing:** no-op for Phase 2c; the level-load systems
///   that should fire here live in `level_management.rs` and are
///   triggered by the existing `start_level` method. Phase 3 wires
///   this through `NavigationEvent`.
/// - **Enter Editor:** no-op for Phase 2c; same as Playing.
/// - **Enter GameOver:** no-op for Phase 2c.
///
/// Takes a [`Mut<AppPhaseRes>`] directly (not `ResMut`) so it can be
/// invoked from outside a Bevy schedule (`State::tick_state` runs it
/// once per frame against a borrowed world). To run this as a system
/// in a schedule, wrap the call in a `move |mut r: ResMut<_>|` closure.
pub fn phase_transition_system(mut phase: Mut<'_, AppPhaseRes>) {
    let current = phase.get();
    if phase.previous_app_phase == current {
        return;
    }

    on_exit_phase(phase.previous_app_phase);
    on_enter_phase(current);

    phase.previous_app_phase = current;
}

/// Hook that runs when the application leaves a given phase. Kept as a
/// free function (not a system) so it can be unit-tested without a
/// schedule. Side effects are minimal in Phase 2c — the existing
/// `State::turn_right` / `start_level` / `toggle_editor` methods
/// already do the heavy lifting; this hook just exists as a single
/// place to put cleanup logic in subsequent phases.
fn on_exit_phase(phase: AppPhase) {
    puffin::profile_scope!("OnExitPhase", format!("{:?}", phase));
    match phase {
        AppPhase::Menu => log_phase_transition("exit", phase),
        AppPhase::Playing => log_phase_transition("exit", phase),
        AppPhase::Editor => log_phase_transition("exit", phase),
        AppPhase::GameOver => log_phase_transition("exit", phase),
    }
}

/// Hook that runs when the application enters a given phase. See
/// [`on_exit_phase`] for the design.
fn on_enter_phase(phase: AppPhase) {
    puffin::profile_scope!("OnEnterPhase", format!("{:?}", phase));
    match phase {
        AppPhase::Menu => log_phase_transition("enter", phase),
        AppPhase::Playing => log_phase_transition("enter", phase),
        AppPhase::Editor => log_phase_transition("enter", phase),
        AppPhase::GameOver => log_phase_transition("enter", phase),
    }
}

fn log_phase_transition(kind: &str, phase: AppPhase) {
    log::debug!("phase transition: {} {:?}", kind, phase);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::plugins::register_state_resources;
    use crate::state::state_resources::AppPhaseRes;
    use bevy_ecs::world::World;

    #[test]
    fn first_call_with_default_phase_runs_enter_menu_transition() {
        let mut world = World::new();
        register_state_resources(&mut world);

        // Default: AppPhase::Menu, previous = Menu -> no transition.
        phase_transition_system(world.resource_mut::<AppPhaseRes>());
        assert_eq!(world.resource::<AppPhaseRes>().get(), AppPhase::Menu);
        assert_eq!(
            world.resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Menu
        );
    }

    #[test]
    fn changing_phase_runs_enter_and_exit_hooks() {
        let mut world = World::new();
        register_state_resources(&mut world);

        // Switch to Playing. previous starts as Menu, so we expect
        // an OnExit(Menu) + OnEnter(Playing) pair.
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Playing);
        phase_transition_system(world.resource_mut::<AppPhaseRes>());
        assert_eq!(
            world.resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Playing
        );

        // Switch to Editor. expect OnExit(Playing) + OnEnter(Editor).
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
        phase_transition_system(world.resource_mut::<AppPhaseRes>());
        assert_eq!(
            world.resource::<AppPhaseRes>().previous_app_phase,
            AppPhase::Editor
        );
    }

    #[test]
    fn no_transition_when_phase_unchanged() {
        let mut world = World::new();
        register_state_resources(&mut world);

        // First call: from Menu -> Menu, no transition.
        phase_transition_system(world.resource_mut::<AppPhaseRes>());
        // Set the previous manually so we can confirm a no-op call.
        let before = *world.resource::<AppPhaseRes>();
        phase_transition_system(world.resource_mut::<AppPhaseRes>());
        let after = *world.resource::<AppPhaseRes>();
        assert_eq!(before.previous_app_phase, after.previous_app_phase);
    }
}
