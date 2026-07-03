/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Misc editor command events: hitbox / perf overlay / place window /
//! escape.

use bevy_app::App;
use bevy_ecs::prelude::*;

/// Toggle editor hitbox visualization.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ToggleHitboxVisualization;

/// Toggle the performance overlay.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePerfOverlay;

/// Toggle the floating place window with block catalog.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct TogglePlaceWindow;

/// Escape key context-sensitive (stop playback → reset timeline → back to menu).
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

// `ToggleHitboxVisualization` is a carve-out from the `PendingCommands`
// → `State::dispatch_editor` pipeline (same pattern as `events::snap`,
// `events::bpm`, and `events::timing`): the handler mutates
// `EditorInputRes.0.show_hitbox_visualization` directly, and the
// legacy `State.editor.input.show_hitbox_visualization` is synced from
// the resource by `Runtime::sync_app_editor_input_state` before any
// dispatch in the same frame.
pub fn handle_toggle_hitbox_visualization(
    mut events: MessageReader<ToggleHitboxVisualization>,
    mut input: ResMut<crate::state::EditorInputRes>,
) {
    for _ in events.read() {
        input.0.show_hitbox_visualization = !input.0.show_hitbox_visualization;
    }
}

// `TogglePerfOverlay` is a carve-out: the handler mutates
// `PerfRes.0.overlay_enabled` directly. The legacy
// `State.editor.perf.overlay_enabled` is synced from the resource by
// `Runtime::sync_app_perf_state` before any dispatch in the same
// frame. The legacy setter also calls `puffin::set_scopes_on(...)` —
// the change-guarded sync invokes the legacy setter (which performs
// the global `puffin` side effect) when the value differs.
pub fn handle_toggle_perf_overlay(
    mut events: MessageReader<TogglePerfOverlay>,
    mut perf: ResMut<crate::state::PerfRes>,
) {
    for _ in events.read() {
        perf.0.overlay_enabled = !perf.0.overlay_enabled;
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
    mut session: ResMut<crate::state::SessionStateRes>,
) {
    for _ in events.read() {
        session.editor_show_place_window = !session.editor_show_place_window;
    }
}

// `Escape` is a carve-out from the `PendingCommands` →
// `State::dispatch_editor` pipeline (same request-resource pattern
// as `events::auth::handle_resize_surface` and
// `events::auth::handle_copy_latest_perf_frame`): the handler
// writes `EscapeRequest.pending = true`, and
// `Runtime::sync_app_escape_request` consumes the request before the
// remaining `PendingCommands` drain and calls the existing legacy
// `State::apply_editor_escape` compatibility method. This keeps
// the multi-step context-sensitive state machine in `handle_editor_escape`
// (stop playback → reset timeline → back to menu) reachable from
// the App world without staging `EditorCommand::Escape` in
// `PendingCommands`.
pub fn handle_escape(
    mut events: MessageReader<Escape>,
    mut escape_request: ResMut<crate::state::EscapeRequest>,
) {
    for _ in events.read() {
        escape_request.request();
    }
}
