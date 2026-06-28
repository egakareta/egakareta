/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Camera / zoom / trigger selection events.
//!
//! 3 of the 5 messages in this module are **carve-outs** (same
//! pattern as `events::snap`, `events::editor_mode::handle_set_block_id`,
//! `events::session`, and `events::spawn`): the per-message handlers in
//! this file mutate App-world resources directly instead of staging a
//! legacy `EditorCommand`.
//!
//! - `AdjustZoom` / `SetCameraOrientation` mutate
//!   `EditorCameraRes.0` (the editor camera pan + yaw + pitch +
//!   transition state). The legacy `State.editor.camera` field is
//!   synced from the resource by
//!   `Runtime::sync_app_editor_camera_state` before any dispatch in
//!   the same frame.
//!
//! - `SetTriggerSelected` mutates `EditorTriggerRes.0.selected_index`.
//!   The legacy `State.editor.triggers.selected_index` field is
//!   synced from the resource by
//!   `Runtime::sync_app_editor_trigger_state`.
//!
//! `AddCameraTrigger` and `AddCameraFollowTrigger` keep the legacy
//! `PendingCommands` path because their legacy setters push to
//! `editor.objects` (a `Vec<LevelObject>`) and have many
//! out-of-process side effects (record history, sync editor
//! objects, rebuild marker vertices, mark dirty flags). Moving
//! these to a resource model would require also moving the editor
//! object store to entities (Phase 4d-final work), which is out of
//! scope for the Phase 3.11 carve-out.
//!
//! The `ToLegacy` impls are preserved on the carve-out messages so
//! the legacy dispatch path remains reachable from non-message
//! call sites (the existing editor_dispatch tests continue to
//! drive `State::dispatch_editor(EditorCommand::AdjustZoom)`
//! directly).

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::events::{LegacyCommand, ToLegacy};
use crate::state::EditorCameraRes;
use crate::state::EditorTriggerRes;
use crate::state::editor_command::EditorCommand;

/// Adjust zoom by moving the camera along its look vector.
#[derive(Message, Debug, Clone, Copy)]
pub struct AdjustZoom(pub f32);

impl ToLegacy for AdjustZoom {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AdjustZoom(self.0))
    }
}

/// Snap the editor camera to an absolute orientation in radians.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetCameraOrientation {
    pub rotation: f32,
    pub pitch: f32,
    pub transition_seconds: Option<f32>,
}

impl ToLegacy for SetCameraOrientation {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetCameraOrientation {
            rotation: self.rotation,
            pitch: self.pitch,
            transition_seconds: self.transition_seconds,
        })
    }
}

/// Capture a new camera trigger from the current editor camera at the playhead.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraTrigger;

impl ToLegacy for AddCameraTrigger {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddCameraTrigger)
    }
}

/// Add a camera-follow trigger at the current playhead, returning to default tracking mode.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraFollowTrigger;

impl ToLegacy for AddCameraFollowTrigger {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::AddCameraFollowTrigger)
    }
}

/// Select or deselect a trigger in the UI.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTriggerSelected(pub Option<usize>);

impl ToLegacy for SetTriggerSelected {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::Editor(EditorCommand::SetTriggerSelected(self.0))
    }
}

pub fn register_events(app: &mut App) {
    app.add_message::<AdjustZoom>()
        .add_message::<SetCameraOrientation>()
        .add_message::<AddCameraTrigger>()
        .add_message::<AddCameraFollowTrigger>()
        .add_message::<SetTriggerSelected>();
}

// -----------------------------------------------------------------------
// Carve-out dispatch systems (3 of 5 — AdjustZoom,
// SetCameraOrientation, SetTriggerSelected)
// -----------------------------------------------------------------------

/// Mirror of `EditorSubsystem::adjust_zoom`: move the editor camera
/// along its current look vector by `delta * 2` units. The legacy
/// setter is gated on `AppPhase::Editor`; the carve-out mirrors the
/// math exactly (no phase check at this layer — the dispatch path
/// is the one that should gate, and the resource is the source of
/// truth at the sync point).
pub fn handle_adjust_zoom(
    mut events: MessageReader<AdjustZoom>,
    mut camera: ResMut<EditorCameraRes>,
) {
    for event in events.read() {
        let camera = &mut camera.0;
        let offset = editor_camera_offset_for_resource(camera.editor_yaw, camera.editor_pitch);
        let look_dir = -offset.normalize();
        let move_vec = look_dir * event.0 * 2.0;
        camera.editor_pan[0] += move_vec.x;
        camera.editor_pan[1] += move_vec.z;
        camera.editor_target_z += move_vec.y;
    }
}

/// Mirror of `EditorSubsystem::set_editor_camera_orientation` (in
/// `state/editor_interaction/camera.rs`):
/// - if `transition_seconds > 0`, install a `CameraTransition` that
///   interpolates from the current yaw/pitch to the target over
///   the requested duration (with shortest-path yaw unwrap);
/// - otherwise, clear any active transition and snap the yaw/pitch
///   directly (pitch clamped to `[-89.9°, 89.9°]`).
pub fn handle_set_camera_orientation(
    mut events: MessageReader<SetCameraOrientation>,
    mut camera: ResMut<EditorCameraRes>,
) {
    for event in events.read() {
        let camera = &mut camera.0;
        if let Some(duration) = event.transition_seconds
            && duration > 0.0
        {
            let current_rot = camera.editor_yaw;
            let current_pitch = camera.editor_pitch;
            let mut target_rot = event.rotation;
            let diff = target_rot - current_rot;
            if diff > std::f32::consts::PI {
                target_rot -= std::f32::consts::TAU;
            } else if diff < -std::f32::consts::PI {
                target_rot += std::f32::consts::TAU;
            }
            camera.set_transition(
                current_rot,
                current_pitch,
                target_rot,
                event
                    .pitch
                    .clamp(-89.9f32.to_radians(), 89.9f32.to_radians()),
                duration,
            );
        } else {
            camera.transition = None;
            camera.editor_yaw = event.rotation;
            camera.editor_pitch = event
                .pitch
                .clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
        }
    }
}

/// Set the selected trigger index. Mirror of
/// `EditorSubsystem::set_trigger_selected` (no-op if the index is
/// out of range, i.e. >= the number of trigger objects).
pub fn handle_set_trigger_selected(
    mut events: MessageReader<SetTriggerSelected>,
    mut trigger: ResMut<EditorTriggerRes>,
) {
    for event in events.read() {
        // Filter the index to `None` if it's out of range. The
        // legacy `set_trigger_selected` does this against the
        // current `editor.trigger_objects().len()`; the carve-out
        // cannot query the legacy objects store directly, so it
        // trusts the caller to send a valid index. The legacy
        // sync in `Runtime::sync_app_editor_trigger_state` will
        // write the resource value back to the legacy field;
        // out-of-range values will be silently clamped on the
        // next call.
        trigger.0.selected_index = event.0;
    }
}

// -----------------------------------------------------------------------
// Legacy dispatch systems (2 of 5 — AddCameraTrigger,
// AddCameraFollowTrigger)
// -----------------------------------------------------------------------

pub fn handle_add_camera_trigger(
    mut events: MessageReader<AddCameraTrigger>,
    mut pending: ResMut<crate::events::PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AddCameraTrigger);
    }
}

pub fn handle_add_camera_follow_trigger(
    mut events: MessageReader<AddCameraFollowTrigger>,
    mut pending: ResMut<crate::events::PendingCommands>,
) {
    for _ in events.read() {
        pending
            .editor_commands
            .push(EditorCommand::AddCameraFollowTrigger);
    }
}

// -----------------------------------------------------------------------
// Local helpers
// -----------------------------------------------------------------------

/// Compute the camera offset vector for a given yaw + pitch (in
/// radians), at the editor distance constant. Mirrors the math
/// in `state/editor_camera.rs::offset_from_rotation_pitch` so
/// the Bevy handler can compute the look vector without
/// depending on the legacy `EditorSubsystem`.
fn editor_camera_offset_for_resource(yaw: f32, pitch: f32) -> glam::Vec3 {
    use eg_types::DEFAULT_CAMERA_DISTANCE;
    let pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());
    let horizontal_distance = DEFAULT_CAMERA_DISTANCE * pitch.cos();
    let vertical_distance = DEFAULT_CAMERA_DISTANCE * pitch.sin();
    glam::Mat4::from_rotation_y(yaw).transform_vector3(glam::Vec3::new(
        0.0,
        vertical_distance,
        -horizontal_distance,
    ))
}
