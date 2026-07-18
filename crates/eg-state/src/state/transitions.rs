/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy state transition wiring for the eg-state App.
//!
//! `State<AppPhase>` is authoritative. Transitions are queued through
//! `NextState<AppPhase>` and observed through Bevy's enter/exit schedules.

use bevy_app::App;
use bevy_state::prelude::{OnEnter, OnExit};
use eg_types::AppPhase;

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
