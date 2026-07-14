/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Misc editor command events: hitbox / perf overlay / place window /
//! escape.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use super::timeline::stop_editor_timeline_playback;
use crate::state::{
    AppPhaseRes, EditorInputRes, EditorObjectsRes, EditorRuntimeRes, EditorTimelineRes,
    MenuStateRes, PerfRes, SessionStateRes,
};

/// Toggle editor hitbox visualization.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleHitboxVisualization;

/// Toggle the performance overlay.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePerfOverlay;

/// Toggle the floating place window with block catalog.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePlaceWindow;

/// Escape key context-sensitive (pause game → stop playback → reset timeline → back to menu).
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct Escape;

pub fn register_events(app: &mut App) {
    app.add_message::<ToggleHitboxVisualization>()
        .add_message::<TogglePerfOverlay>()
        .add_message::<TogglePlaceWindow>()
        .add_message::<Escape>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

// `ToggleHitboxVisualization` mutates App-world state directly (same pattern
// as `events::snap`, `events::bpm`, and `events::timing`): the handler mutates
// `EditorInputRes.0.show_hitbox_visualization` directly, and the
// legacy `State.editor.input.show_hitbox_visualization` is synced from
// the resource by `Runtime::sync_app_editor_input_state` before any
// dispatch in the same frame.
pub fn handle_toggle_hitbox_visualization(
    mut events: MessageReader<ToggleHitboxVisualization>,
    mut input: ResMut<EditorInputRes>,
) {
    for _ in events.read() {
        input.0.show_hitbox_visualization = !input.0.show_hitbox_visualization;
    }
}

// `TogglePerfOverlay` owns both the App-world value and puffin's global
// collection switch. Legacy `State` perf fields are not synchronized back
// from this handler.
pub fn handle_toggle_perf_overlay(
    mut events: MessageReader<TogglePerfOverlay>,
    mut perf: ResMut<PerfRes>,
) {
    for _ in events.read() {
        perf.0.overlay_enabled = !perf.0.overlay_enabled;
        puffin::set_scopes_on(perf.0.overlay_enabled);
    }
}

// `TogglePlaceWindow` is a carve-out: the handler mutates
// `SessionStateRes.editor_show_place_window` directly. The legacy
// `State.session.editor_show_place_window` is synced from the
// resource by `Runtime::sync_app_session_state` (which already
// handles the other carved-out session fields) before any dispatch
// in the same frame. The change-guarded sync avoids re-running
// other session-related side effects on every frame.
pub fn handle_toggle_place_window(
    mut events: MessageReader<TogglePlaceWindow>,
    mut session: ResMut<SessionStateRes>,
) {
    for _ in events.read() {
        session.editor_show_place_window = !session.editor_show_place_window;
    }
}

// `Escape` handles local branches directly in App-world resources:
// real-gameplay pause, playtest exit, editor selection clearing, editor
// timeline playback stop, editor timeline clock reset, and editor-to-menu
// transition. Menu and game-over branches are also resolved directly in the
// App world.
pub fn handle_escape(mut events: MessageReader<Escape>, mut resources: EscapeResources) {
    for _ in events.read() {
        match resources.phase.current {
            eg_types::AppPhase::Playing => {
                if resources.session.playtesting_editor {
                    resources.phase.set(eg_types::AppPhase::Editor);
                    resources.session.playtesting_editor = false;
                    resources.session.game_paused = false;
                    resources.session.audio_stop_pending = true;
                    resources.timeline.0.playback.playing = false;
                    resources.timeline.0.playback.runtime = None;
                } else {
                    resources.session.game_paused = !resources.session.game_paused;
                }
                continue;
            }
            eg_types::AppPhase::Editor => {
                if clear_editor_selection_for_escape(
                    &mut resources.input,
                    &mut resources.objects,
                    &mut resources.timeline,
                ) {
                    continue;
                }

                if stop_editor_timeline_playback(
                    &mut resources.input,
                    &mut resources.runtime,
                    &mut resources.timeline,
                ) {
                    continue;
                }

                if resources.timeline.0.clock.time_seconds > 0.001 {
                    resources.timeline.0.clock.time_seconds = 0.0;
                    continue;
                }

                if let Some(menu) = resources.menu.as_deref_mut() {
                    resources.phase.set(eg_types::AppPhase::Menu);
                    resources.session.transition_to_menu();
                    menu.0.preview_level_index = None;
                    continue;
                }
                resources.phase.set(eg_types::AppPhase::Menu);
                resources.session.transition_to_menu();
            }
            eg_types::AppPhase::GameOver => {
                resources.phase.set(eg_types::AppPhase::Menu);
                resources.session.transition_to_menu();
                if let Some(menu) = resources.menu.as_deref_mut() {
                    menu.0.preview_level_index = None;
                }
            }
            eg_types::AppPhase::Menu => {}
        }
    }
}

#[derive(SystemParam)]
pub struct EscapeResources<'w> {
    phase: ResMut<'w, AppPhaseRes>,
    input: ResMut<'w, EditorInputRes>,
    objects: ResMut<'w, EditorObjectsRes>,
    timeline: ResMut<'w, EditorTimelineRes>,
    runtime: ResMut<'w, EditorRuntimeRes>,
    session: ResMut<'w, SessionStateRes>,
    menu: Option<ResMut<'w, MenuStateRes>>,
}

fn clear_editor_selection_for_escape(
    input: &mut EditorInputRes,
    objects: &mut EditorObjectsRes,
    timeline: &mut EditorTimelineRes,
) -> bool {
    let had_block_selection = input.0.selected_block_index.is_some()
        || !input.0.selected_block_indices.is_empty()
        || input.0.hovered_block_index.is_some();
    let had_tap_selection = selected_tap_exists(timeline);

    if had_block_selection {
        input.0.selected_block_index = None;
        input.0.selected_block_indices.clear();
        input.0.hovered_block_index = None;
        objects.object_mutation_pending = true;
    }

    if had_tap_selection {
        timeline.0.taps.selected_index = None;
    }

    had_block_selection || had_tap_selection
}

fn selected_tap_exists(timeline: &EditorTimelineRes) -> bool {
    timeline.0.taps.selected_index.is_some_and(|index| {
        index < timeline.0.taps.tap_times.len()
            && index < timeline.0.taps.tap_indicator_positions.len()
    })
}
