/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Timing point events.
//!
//! All 6 messages in this module are **carve-outs** (same pattern as
//! `events::snap`, `events::editor_mode::handle_set_block_id`, and
//! `events::session`): the per-message handlers in this file mutate
//! `EditorTimingRes.0` directly instead of staging a legacy
//! `EditorCommand`. The legacy `State.editor.timing.*` fields are synced
//! from the resource by
//! `Runtime::sync_app_editor_timing_state` before any dispatch in the
//! same frame.
//!
//! The carve-out covers `timing_points` and `timing_selected_index`.
//! The bpm-tap fields are carved out in `events::bpm` (separate
//! handlers but the same shared `EditorTimingRes` resource + the same
//! change-guarded `sync_app_editor_timing_state` helper).

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::state::EditorTimingRes;

/// Add a new timing point.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddTimingPoint {
    pub time_seconds: f32,
    pub bpm: f32,
}

/// Remove an existing timing point by index.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemoveTimingPoint(pub usize);

/// Update an existing timing point's timestamp.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointTime {
    pub index: usize,
    pub time: f32,
}

/// Update an existing timing point's BPM.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointBpm {
    pub index: usize,
    pub bpm: f32,
}

/// Update an existing timing point's time signature.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingPointTimeSignature {
    pub index: usize,
    pub numerator: u32,
    pub denominator: u32,
}

/// Select or deselect a timing point in the UI.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTimingSelected(pub Option<usize>);

pub fn register_events(app: &mut App) {
    app.add_message::<AddTimingPoint>()
        .add_message::<RemoveTimingPoint>()
        .add_message::<SetTimingPointTime>()
        .add_message::<SetTimingPointBpm>()
        .add_message::<SetTimingPointTimeSignature>()
        .add_message::<SetTimingSelected>();
}

// -----------------------------------------------------------------------
// Dispatch systems (all carve-outs: write directly to EditorTimingRes)
// -----------------------------------------------------------------------

/// Insert a new timing point and re-sort the list by time. Mirrors
/// `State::editor_add_timing_point` / `EditorSubsystem::add_timing_point`
/// (which sorts ascending by `time_seconds` and bumps the revision).
pub fn handle_add_timing_point(
    mut events: MessageReader<AddTimingPoint>,
    mut timing: ResMut<EditorTimingRes>,
) {
    use eg_types::TimingPoint;
    for event in events.read() {
        timing.0.timing_points.push(TimingPoint {
            time_seconds: event.time_seconds,
            bpm: event.bpm,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        });
        timing
            .0
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        timing.0.revision = timing.0.revision.wrapping_add(1);
    }
}

/// Remove the timing point at `index` (no-op if out of range) and
/// bump the revision. Mirrors
/// `EditorSubsystem::remove_timing_point`.
pub fn handle_remove_timing_point(
    mut events: MessageReader<RemoveTimingPoint>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        if event.0 < timing.0.timing_points.len() {
            timing.0.timing_points.remove(event.0);
            timing.0.revision = timing.0.revision.wrapping_add(1);
        }
    }
}

/// Update the timestamp of the timing point at `index` and re-sort
/// the list. Mirrors `EditorSubsystem::update_timing_point_time`.
pub fn handle_set_timing_point_time(
    mut events: MessageReader<SetTimingPointTime>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        if let Some(tp) = timing.0.timing_points.get_mut(event.index) {
            tp.time_seconds = event.time.max(0.0);
        }
        timing
            .0
            .timing_points
            .sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        timing.0.revision = timing.0.revision.wrapping_add(1);
    }
}

/// Update the BPM of the timing point at `index`. Mirrors
/// `EditorSubsystem::update_timing_point_bpm` (clamped to >= 1.0).
pub fn handle_set_timing_point_bpm(
    mut events: MessageReader<SetTimingPointBpm>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        if let Some(tp) = timing.0.timing_points.get_mut(event.index) {
            tp.bpm = event.bpm.max(1.0);
            timing.0.revision = timing.0.revision.wrapping_add(1);
        }
    }
}

/// Update the time signature of the timing point at `index`.
/// Mirrors `EditorSubsystem::update_timing_point_time_signature`
/// (numerator and denominator clamped to >= 1).
pub fn handle_set_timing_point_time_signature(
    mut events: MessageReader<SetTimingPointTimeSignature>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        if let Some(tp) = timing.0.timing_points.get_mut(event.index) {
            tp.time_signature_numerator = event.numerator.max(1);
            tp.time_signature_denominator = event.denominator.max(1);
            timing.0.revision = timing.0.revision.wrapping_add(1);
        }
    }
}

/// Select or deselect a timing point. Mirrors
/// `EditorSubsystem::set_timing_selected_index`.
pub fn handle_set_timing_selected(
    mut events: MessageReader<SetTimingSelected>,
    mut timing: ResMut<EditorTimingRes>,
) {
    for event in events.read() {
        timing.0.timing_selected_index = event.0;
    }
}
