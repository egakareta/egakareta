/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Misc editor command events: hitbox / perf overlay / place window /
//! escape.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_state::prelude::{NextState, State};
use eg_types::EditorMode;

use crate::state::{
    EditorInputRes, EditorRuntimeRes, EditorTimelineRes, MenuStateRes, PerfRes, SessionStateRes,
    replace_editor_objects,
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

// `ToggleHitboxVisualization` writes the authoritative App-world input state.
// Render extraction reads this resource directly, so no renderer invalidation
// bit or Runtime synchronization is required.

pub fn handle_toggle_hitbox_visualization(
    mut events: MessageReader<ToggleHitboxVisualization>,
    mut input: ResMut<EditorInputRes>,
) {
    for _ in events.read() {
        input.0.show_hitbox_visualization = !input.0.show_hitbox_visualization;
    }
}

// `TogglePerfOverlay` owns both the App-world value and puffin's global
// collection switch.
pub fn handle_toggle_perf_overlay(
    mut events: MessageReader<TogglePerfOverlay>,
    mut perf: ResMut<PerfRes>,
) {
    for _ in events.read() {
        perf.0.overlay_enabled = !perf.0.overlay_enabled;
        puffin::set_scopes_on(perf.0.overlay_enabled);
    }
}

// `TogglePlaceWindow` mutates the authoritative App-owned session directly.
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
pub fn handle_escape(world: &mut World) {
    use bevy_ecs::message::Messages;

    let events: Vec<Escape> = world.resource_mut::<Messages<Escape>>().drain().collect();
    for _ in events {
        let phase = *world.resource::<State<eg_types::AppPhase>>().get();
        match phase {
            eg_types::AppPhase::Playing => {
                let playtesting_editor = world.resource::<SessionStateRes>().playtesting_editor;
                if playtesting_editor {
                    exit_playtest_restore_editor(world);
                } else {
                    let mut session = world.resource_mut::<SessionStateRes>();
                    session.game_paused = !session.game_paused;
                }
                continue;
            }
            eg_types::AppPhase::Editor => {
                // Inline the selection-clear and timeline-stop logic with
                // scoped blocks so every `resource_mut` borrow is dropped
                // before the next one. (Holding two `Mut`s simultaneously is
                // not possible from a `&mut World` exclusive-system entry
                // point without going through `SystemParam`.)
                let had_block_selection = {
                    let mut input = world.resource_mut::<EditorInputRes>();
                    let had = input.0.selected_block_index.is_some()
                        || !input.0.selected_block_indices.is_empty()
                        || input.0.hovered_block_index.is_some();
                    if had {
                        input.0.selected_block_index = None;
                        input.0.selected_block_indices.clear();
                        input.0.hovered_block_index = None;
                    }
                    had
                };
                let had_tap_selection = {
                    let mut timeline = world.resource_mut::<EditorTimelineRes>();
                    let had = selected_tap_exists(&timeline);
                    if had {
                        timeline.0.taps.selected_index = None;
                    }
                    had
                };
                if had_block_selection || had_tap_selection {
                    continue;
                }

                // Stop active editor timeline playback if it's running. The
                // helper takes three resources, so we read the `last_mode`
                // out of the runtime and pass it as plain data so the three
                // resources can be borrowed one at a time.
                let was_playing = {
                    let timeline = world.resource::<EditorTimelineRes>();
                    timeline.0.playback.playing
                };
                if was_playing {
                    let next_mode = {
                        let runtime = world.resource::<EditorRuntimeRes>();
                        runtime.0.interaction.last_mode.unwrap_or(EditorMode::Place)
                    };
                    {
                        let mut timeline = world.resource_mut::<EditorTimelineRes>();
                        timeline.0.playback.playing = false;
                        timeline.0.playback.runtime = None;
                        timeline.0.playback.pending_seek_time_seconds = None;
                        timeline.0.playback.seek_resync_cooldown_seconds = 0.0;
                    }
                    {
                        let mut input = world.resource_mut::<EditorInputRes>();
                        input.0.mode = next_mode;
                    }
                    {
                        let mut runtime = world.resource_mut::<EditorRuntimeRes>();
                        runtime.0.interaction.last_mode = None;
                    }
                    continue;
                }

                {
                    let mut timeline = world.resource_mut::<EditorTimelineRes>();
                    if timeline.0.clock.time_seconds > 0.001 {
                        timeline.0.clock.time_seconds = 0.0;
                        continue;
                    }
                }

                world
                    .resource_mut::<NextState<eg_types::AppPhase>>()
                    .set(eg_types::AppPhase::Menu);
                world.resource_mut::<SessionStateRes>().transition_to_menu();
                if let Some(mut menu) = world.get_resource_mut::<MenuStateRes>() {
                    menu.0.preview_level_index = None;
                }
            }
            eg_types::AppPhase::GameOver => {
                world
                    .resource_mut::<NextState<eg_types::AppPhase>>()
                    .set(eg_types::AppPhase::Menu);
                world.resource_mut::<SessionStateRes>().transition_to_menu();
                if let Some(mut menu) = world.get_resource_mut::<MenuStateRes>() {
                    menu.0.preview_level_index = None;
                }
            }
            eg_types::AppPhase::Menu => {}
        }
    }
}

/// Apply the playtest-exit side effects: stop the playtest audio, transition
/// to the editor, and restore the editor's authored block list from the
/// snapshot captured at playtest start.
fn exit_playtest_restore_editor(world: &mut World) {
    world
        .resource_mut::<NextState<eg_types::AppPhase>>()
        .set(eg_types::AppPhase::Editor);
    {
        let mut session = world.resource_mut::<SessionStateRes>();
        session.playtesting_editor = false;
        session.game_paused = false;
        session.audio_stop_pending = true;
    }
    {
        let mut timeline = world.resource_mut::<EditorTimelineRes>();
        timeline.0.playback.playing = false;
        timeline.0.playback.runtime = None;
    }
    restore_editor_objects_from_session(world);
}

/// Repopulate the editor's authored block list from
/// `SessionStateRes::playtest_editor_objects` and clear the snapshot.
pub(crate) fn restore_editor_objects_from_session(world: &mut World) {
    let snapshot = world
        .resource_mut::<SessionStateRes>()
        .playtest_editor_objects
        .take();
    if let Some(objects) = snapshot {
        replace_editor_objects(world, objects);
    }
}

fn selected_tap_exists(timeline: &EditorTimelineRes) -> bool {
    timeline.0.taps.selected_index.is_some_and(|index| {
        index < timeline.0.taps.tap_times.len()
            && index < timeline.0.taps.tap_indicator_positions.len()
    })
}
