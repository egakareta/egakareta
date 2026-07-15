/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! BPM tapping events.
//!
//! These handlers mutate App-world timing resources directly (same pattern as
//! `events::snap` and `events::editor_mode::handle_set_block_id`).
//! The handlers mutate `EditorTimingRes.0.bpm_tap_times` and
//! `EditorTimingRes.0.bpm_tap_result` directly, doing the same
//! calculation as the legacy `State::editor_bpm_tap` /
//! `editor_bpm_tap_reset` paths.
//!
use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::state::EditorTimingRes;

/// Record a BPM heart-beat tap.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct BpmTap;

/// Reset the BPM tapping state.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct BpmTapReset;

pub fn register_events(app: &mut App) {
    app.add_message::<BpmTap>().add_message::<BpmTapReset>();
}

// -----------------------------------------------------------------------
// Carve-out dispatch systems: handlers mutate `EditorTimingRes` directly
// so the App world is the authoritative store for the bpm-tap state.
// The bpm-tap calculation runs in the App world: the handler reads
// wall-clock time, appends to the tap-times list, recomputes the
// running BPM (60 / average diff), and truncates the list to the
// last 16 taps. This is the same logic as
// `State::editor_bpm_tap` / `State::editor_bpm_tap_reset`, kept in
// lockstep to avoid two divergent implementations.
// -----------------------------------------------------------------------

/// Maximum number of bpm tap times retained for the running average.
/// Mirrors the cap in `EditorSubsystem::bpm_tap`.
const BPM_TAP_HISTORY_LIMIT: usize = 16;

fn wall_clock_now_seconds() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
}

pub fn handle_bpm_tap(mut events: MessageReader<BpmTap>, mut timing: ResMut<EditorTimingRes>) {
    for _ in events.read() {
        let now_secs = wall_clock_now_seconds();
        timing.0.bpm_tap_times.push(now_secs);
        if timing.0.bpm_tap_times.len() > 1 {
            let mut diffs: Vec<f64> = Vec::with_capacity(timing.0.bpm_tap_times.len() - 1);
            for i in 1..timing.0.bpm_tap_times.len() {
                diffs.push(timing.0.bpm_tap_times[i] - timing.0.bpm_tap_times[i - 1]);
            }
            let avg_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
            let bpm = (60.0 / avg_diff) as f32;
            timing.0.bpm_tap_result = Some(bpm);
        }
        if timing.0.bpm_tap_times.len() > BPM_TAP_HISTORY_LIMIT {
            let excess = timing.0.bpm_tap_times.len() - BPM_TAP_HISTORY_LIMIT;
            timing.0.bpm_tap_times.drain(0..excess);
        }
    }
}

pub fn handle_bpm_tap_reset(
    mut events: MessageReader<BpmTapReset>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for _ in events.read() {
        timing.0.bpm_tap_times.clear();
        timing.0.bpm_tap_result = None;
    }
}
