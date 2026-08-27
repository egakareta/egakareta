/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Camera / zoom / trigger selection events.
//!
//! All 5 messages in this module are **carve-outs** (same
//! pattern as `events::snap`, `events::editor_mode::handle_set_block_id`,
//! `events::session`, and `events::spawn`): the per-message handlers in
//! this file mutate App-world resources directly instead of staging a
//! the editor command bridge.
//!
//! - `AdjustZoom` / `SetCameraOrientation` mutate the authoritative
//!   `EditorCameraRes.0` camera and transition state directly.
//!
//! - `SetTriggerSelected` mutates the authoritative
//!   `EditorTriggerRes.0.selected_index` directly.
//!
//! - `AddCameraTrigger` / `AddCameraFollowTrigger` spawn generated
//!   camera trigger entities, record
//!   App-world editor history, and sync trigger selection against the
//!   App-world object list. Render extraction observes the resulting entities.
//!

use bevy_app::App;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::prelude::State;
use eg_blocks::BlockId;
use eg_editor_domain::interpolate_timeline_sample_positions;
use eg_types::{
    AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, EasingDirection, LevelObject,
    TimedTrigger, TriggerTarget,
};

use crate::state::{
    EditorAuthoredObject, EditorCameraRes, EditorConfigRes, EditorHistoryRes,
    EditorHistorySnapshot, EditorInputRes, EditorObjectOrder, EditorSpawnRes, EditorTimelineRes,
    EditorTimingRes, EditorTriggerRes, camera_trigger_object_pose, collect_editor_object_data,
    spawn_editor_object_commands, sync_camera_follow_trigger_object_pose,
};

const DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS: f32 = 1.0;

/// Adjust zoom by moving the camera along its look vector.
#[derive(Message, Debug, Clone, Copy)]
pub struct AdjustZoom(pub f32);

/// Snap the editor camera to an absolute orientation in radians.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetCameraOrientation {
    pub rotation: f32,
    pub pitch: f32,
    pub transition_seconds: Option<f32>,
}

/// Capture a new camera trigger from the current editor camera at the playhead.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraTrigger;

/// Add a camera-follow trigger at the current playhead, returning to default tracking mode.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AddCameraFollowTrigger;

/// Select or deselect a trigger in the UI.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetTriggerSelected(pub Option<usize>);

pub fn register_events(app: &mut App) {
    app.add_message::<AdjustZoom>()
        .add_message::<SetCameraOrientation>()
        .add_message::<AddCameraTrigger>()
        .add_message::<AddCameraFollowTrigger>()
        .add_message::<SetTriggerSelected>();
}

// -----------------------------------------------------------------------
// Carve-out dispatch systems
// -----------------------------------------------------------------------

/// Move the editor camera
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

/// Set the editor camera orientation (the same operation exposed by the
/// editor interaction helper):
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

/// Set the authoritative App-world selected trigger index.
pub fn handle_set_trigger_selected(
    mut events: MessageReader<SetTriggerSelected>,
    mut trigger: ResMut<EditorTriggerRes>,
) {
    for event in events.read() {
        trigger.0.selected_index = event.0;
    }
}

pub fn handle_add_camera_trigger(
    mut events: MessageReader<AddCameraTrigger>,
    mut resources: CameraResources,
) {
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    for _ in events.read() {
        add_camera_trigger(&mut resources, &mut objects);
    }
}

pub fn handle_add_camera_follow_trigger(
    mut events: MessageReader<AddCameraFollowTrigger>,
    mut resources: CameraResources,
) {
    let mut objects = collect_editor_object_data(&resources.authored_objects);
    for _ in events.read() {
        add_camera_follow_trigger(&mut resources, &mut objects);
    }
}

#[derive(SystemParam)]
pub struct CameraResources<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    authored_objects: Query<
        'w,
        's,
        (&'static EditorObjectOrder, &'static eg_game::ObjectData),
        With<EditorAuthoredObject>,
    >,
    commands: Commands<'w, 's>,
    input: Res<'w, EditorInputRes>,
    config: Res<'w, EditorConfigRes>,
    spawn: Res<'w, EditorSpawnRes>,
    camera: Res<'w, EditorCameraRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    trigger: ResMut<'w, EditorTriggerRes>,
    history: ResMut<'w, EditorHistoryRes>,
}

// -----------------------------------------------------------------------
// Local helpers
// -----------------------------------------------------------------------

fn add_camera_trigger(
    resources: &mut CameraResources<'_, '_>,
    objects: &mut Vec<LevelObject>,
) -> bool {
    if *resources.phase.get() != AppPhase::Editor {
        return false;
    }

    record_history_state(resources, objects);
    let trigger_object = capture_current_camera_trigger(
        &resources.camera.0,
        resources.timeline.0.clock.time_seconds,
    )
    .with_generated_id(objects);
    spawn_editor_object_commands(
        &mut resources.commands,
        objects.len(),
        trigger_object.clone(),
    );
    objects.push(trigger_object);
    sync_trigger_selection_from_objects(resources, objects);
    true
}

fn add_camera_follow_trigger(
    resources: &mut CameraResources<'_, '_>,
    objects: &mut Vec<LevelObject>,
) -> bool {
    if *resources.phase.get() != AppPhase::Editor {
        return false;
    }

    record_history_state(resources, objects);
    let camera = CameraTrigger {
        mode: CameraMode::Follow,
        ..CameraTrigger::default()
    };
    let mut trigger_object = LevelObject {
        block_id: BlockId::new(CAMERA_TRIGGER_BLOCK_ID),
        trigger: Some(TimedTrigger {
            time_seconds: resources.timeline.0.clock.time_seconds,
            target: TriggerTarget::Camera,
            camera: Some(camera),
            ..TimedTrigger::default()
        }),
        ..LevelObject::default()
    }
    .with_generated_id(objects);
    if let Some(trigger) = trigger_object.trigger.clone() {
        let target_position =
            timeline_position_at_seconds(&resources.timeline.0, trigger.time_seconds);
        sync_camera_follow_trigger_object_pose(&mut trigger_object, &trigger, target_position);
    }

    spawn_editor_object_commands(
        &mut resources.commands,
        objects.len(),
        trigger_object.clone(),
    );
    objects.push(trigger_object);
    sync_trigger_selection_from_objects(resources, objects);
    true
}

fn capture_current_camera_trigger(
    camera_state: &crate::state::EditorCameraState,
    time_seconds: f32,
) -> LevelObject {
    let camera = CameraTrigger {
        mode: CameraMode::Static,
        transition_interval_seconds: DEFAULT_CAMERA_TRIGGER_TRANSITION_INTERVAL_SECONDS,
        use_full_segment_transition: false,
        target_position: camera_state.editor_camera_target().to_array(),
        yaw: camera_state.editor_yaw,
        pitch: camera_state.editor_pitch,
        ..CameraTrigger::default()
    };
    let (position, rotation) = camera_trigger_object_pose(&camera);
    LevelObject {
        position,
        rotation,
        size: [1.0, 1.0, 1.0],
        block_id: BlockId::new(CAMERA_TRIGGER_BLOCK_ID),
        trigger: Some(TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: EasingDirection::Linear,
            target: TriggerTarget::Camera,
            camera: Some(camera),
            ..TimedTrigger::default()
        }),
        ..LevelObject::default()
    }
}

fn timeline_position_at_seconds(
    timeline: &crate::state::EditorTimelineState,
    time_seconds: f32,
) -> [f32; 3] {
    let cache = &timeline.snapshot_cache;
    if cache.is_empty() || !timeline.snapshot_cache_complete {
        return timeline.preview.position;
    }

    let step_seconds = timeline.snapshot_cache_step_seconds.max(1.0 / 480.0);
    let target_time = time_seconds.clamp(0.0, timeline.clock.duration_seconds);
    let max_index = cache.len().saturating_sub(1);
    let sample_position = (target_time / step_seconds).clamp(0.0, max_index as f32);
    let lower_index = sample_position.floor() as usize;
    let upper_index = (lower_index + 1).min(max_index);
    let alpha = (sample_position - lower_index as f32).clamp(0.0, 1.0);
    let lower = &cache[lower_index];
    let upper = &cache[upper_index];
    interpolate_timeline_sample_positions(
        lower.position,
        lower.direction,
        upper.position,
        upper.direction,
        alpha,
    )
}

fn sync_trigger_selection_from_objects(
    resources: &mut CameraResources<'_, '_>,
    objects: &[LevelObject],
) {
    let trigger_count = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .count();
    resources.trigger.0.selected_index = resources
        .trigger
        .0
        .selected_index
        .filter(|index| *index < trigger_count);
}

fn record_history_state(resources: &mut CameraResources<'_, '_>, objects: &[LevelObject]) {
    const MAX_HISTORY: usize = 256;
    if resources.history.0.undo.len() >= MAX_HISTORY {
        resources.history.0.undo.remove(0);
    }
    let snapshot = history_snapshot_from_resources(resources, objects);
    resources.history.0.undo.push(snapshot);
    resources.history.0.redo.clear();
}

fn history_snapshot_from_resources(
    resources: &CameraResources<'_, '_>,
    objects: &[LevelObject],
) -> EditorHistorySnapshot {
    EditorHistorySnapshot {
        objects: objects.to_vec(),
        selected_block_index: resources.input.0.selected_block_index,
        selected_block_indices: resources.input.0.selected_block_indices.clone(),
        cursor: resources.input.0.cursor,
        selected_block_id: resources.config.0.selected_block_id.clone(),
        selected_block_rotation_degrees: resources.config.0.selected_block_rotation_degrees,
        recent_block_ids: resources.config.0.recent_block_ids.clone(),
        spawn: resources.spawn.0.clone(),
        timeline_time_seconds: resources.timeline.0.clock.time_seconds,
        timeline_duration_seconds: resources.timeline.0.clock.duration_seconds,
        tap_times: resources.timeline.0.taps.tap_times.clone(),
        tap_indicator_positions: resources.timeline.0.taps.tap_indicator_positions.clone(),
        selected_tap_index: resources.timeline.0.taps.selected_index,
        timing_points: resources.timing.0.timing_points.clone(),
        selected_trigger_index: resources.trigger.0.selected_index,
    }
}

/// Compute the camera offset vector for a given yaw + pitch (in
/// radians), at the editor distance constant. Mirrors the math
/// in `state/editor_camera.rs::offset_from_rotation_pitch` so
/// the Bevy handler can compute the look vector without
/// depending on an owned editor fixture.
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
