/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use serde::Deserializer;
use serde::{Deserialize, Serialize};

use crate::block_repository::{normalize_block_id, DEFAULT_BLOCK_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorInteractionChange {
    None,
    Hover,
    Cursor,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EditorPickResult {
    pub(crate) cursor: [f32; 3],
    pub(crate) hit_block_index: Option<usize>,
    pub(crate) hit_trigger_index: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum GizmoAxis {
    X,
    Y,
    Z,
    XNeg,
    YNeg,
    ZNeg,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum GizmoDragKind {
    Move,
    Resize,
    Rotate,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum GizmoPart {
    MoveX,
    MoveY,
    MoveZ,
    MoveXNeg,
    MoveYNeg,
    MoveZNeg,
    ResizeX,
    ResizeY,
    ResizeZ,
    ResizeXNeg,
    ResizeYNeg,
    ResizeZNeg,
    RotateX,
    RotateY,
    RotateZ,
}

impl GizmoPart {
    pub(crate) fn from_axis_kind(axis: GizmoAxis, kind: GizmoDragKind) -> Self {
        match (axis, kind) {
            (GizmoAxis::X, GizmoDragKind::Move) => GizmoPart::MoveX,
            (GizmoAxis::Y, GizmoDragKind::Move) => GizmoPart::MoveY,
            (GizmoAxis::Z, GizmoDragKind::Move) => GizmoPart::MoveZ,
            (GizmoAxis::XNeg, GizmoDragKind::Move) => GizmoPart::MoveXNeg,
            (GizmoAxis::YNeg, GizmoDragKind::Move) => GizmoPart::MoveYNeg,
            (GizmoAxis::ZNeg, GizmoDragKind::Move) => GizmoPart::MoveZNeg,
            (GizmoAxis::X, GizmoDragKind::Resize) => GizmoPart::ResizeX,
            (GizmoAxis::Y, GizmoDragKind::Resize) => GizmoPart::ResizeY,
            (GizmoAxis::Z, GizmoDragKind::Resize) => GizmoPart::ResizeZ,
            (GizmoAxis::XNeg, GizmoDragKind::Resize) => GizmoPart::ResizeXNeg,
            (GizmoAxis::YNeg, GizmoDragKind::Resize) => GizmoPart::ResizeYNeg,
            (GizmoAxis::ZNeg, GizmoDragKind::Resize) => GizmoPart::ResizeZNeg,
            (GizmoAxis::X | GizmoAxis::XNeg, GizmoDragKind::Rotate) => GizmoPart::RotateX,
            (GizmoAxis::Y | GizmoAxis::YNeg, GizmoDragKind::Rotate) => GizmoPart::RotateY,
            (GizmoAxis::Z | GizmoAxis::ZNeg, GizmoDragKind::Rotate) => GizmoPart::RotateZ,
        }
    }
}

pub(crate) const CURRENT_LEVEL_FORMAT_VERSION: u32 = 2;

pub(crate) const APP_SETTINGS_VERSION: u32 = 1;

fn default_level_format_version() -> u32 {
    CURRENT_LEVEL_FORMAT_VERSION
}

fn is_default_level_format_version(value: &u32) -> bool {
    *value == CURRENT_LEVEL_FORMAT_VERSION
}

fn default_music_source() -> String {
    "music.mp3".to_string()
}

fn is_default_music_source(value: &String) -> bool {
    value == "music.mp3"
}

fn default_spawn_position() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

fn is_default_spawn_position(value: &[f32; 3]) -> bool {
    value.iter().all(|component| component.abs() <= 1e-6)
}

fn default_timeline_time_seconds() -> f32 {
    0.0
}

fn is_default_timeline_time_seconds(value: &f32) -> bool {
    value.abs() <= 1e-6
}

fn default_timeline_duration_seconds() -> f32 {
    16.0
}

fn is_default_timeline_duration_seconds(value: &f32) -> bool {
    (value - default_timeline_duration_seconds()).abs() <= 1e-6
}

fn default_camera_trigger_target_position() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

fn is_default_camera_trigger_target_position(value: &[f32; 3]) -> bool {
    value.iter().all(|component| component.abs() <= 1e-6)
}

fn default_camera_trigger_rotation() -> f32 {
    -45.0f32.to_radians()
}

fn is_default_camera_trigger_rotation(value: &f32) -> bool {
    (*value - default_camera_trigger_rotation()).abs() <= 1e-6
}

fn default_camera_trigger_pitch() -> f32 {
    45.0f32.to_radians()
}

fn is_default_camera_trigger_pitch(value: &f32) -> bool {
    (*value - default_camera_trigger_pitch()).abs() <= 1e-6
}

fn default_camera_trigger_transition_interval_seconds() -> f32 {
    1.0
}

fn is_default_camera_trigger_transition_interval_seconds(value: &f32) -> bool {
    (*value - default_camera_trigger_transition_interval_seconds()).abs() <= 1e-6
}

fn default_camera_trigger_use_full_segment_transition() -> bool {
    false
}

fn is_default_camera_trigger_use_full_segment_transition(value: &bool) -> bool {
    !*value
}

fn default_timed_trigger_duration_seconds() -> f32 {
    0.0
}

fn is_default_timed_trigger_duration_seconds(value: &f32) -> bool {
    value.abs() <= 1e-6
}

fn default_simulate_trigger_hitboxes() -> bool {
    false
}

fn is_default_simulate_trigger_hitboxes(value: &bool) -> bool {
    !*value
}

fn default_timed_trigger_target() -> TimedTriggerTarget {
    TimedTriggerTarget::Camera
}

fn is_default_timed_trigger_target(value: &TimedTriggerTarget) -> bool {
    matches!(value, TimedTriggerTarget::Camera)
}

fn default_block_rotation_degrees() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

fn default_block_roundness() -> f32 {
    0.18
}

fn is_default_block_rotation_degrees(value: &[f32; 3]) -> bool {
    value.iter().all(|component| component.abs() <= 1e-6)
}

fn is_default_block_roundness(value: &f32) -> bool {
    (value - default_block_roundness()).abs() <= 1e-6
}

fn default_level_object_position() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

fn is_default_level_object_position(value: &[f32; 3]) -> bool {
    value.iter().all(|component| component.abs() <= 1e-6)
}

fn default_level_object_size() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn is_default_level_object_size(value: &[f32; 3]) -> bool {
    value
        .iter()
        .all(|component| (*component - 1.0).abs() <= 1e-6)
}

fn default_level_object_block_id() -> String {
    DEFAULT_BLOCK_ID.to_string()
}

fn is_default_level_object_block_id(value: &String) -> bool {
    value == DEFAULT_BLOCK_ID
}

fn default_level_object_color_tint() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn is_default_level_object_color_tint(value: &[f32; 3]) -> bool {
    value
        .iter()
        .zip(default_level_object_color_tint())
        .all(|(component, default)| (*component - default).abs() <= 1e-6)
}

fn default_app_settings_version() -> u32 {
    APP_SETTINGS_VERSION
}

fn is_default_app_settings_version(value: &u32) -> bool {
    *value == APP_SETTINGS_VERSION
}

fn default_editor_selected_block_id() -> String {
    DEFAULT_BLOCK_ID.to_string()
}

fn is_default_editor_selected_block_id(value: &String) -> bool {
    value == DEFAULT_BLOCK_ID
}

fn default_editor_snap_to_grid_setting() -> bool {
    true
}

fn is_default_editor_snap_to_grid_setting(value: &bool) -> bool {
    *value
}

fn default_editor_snap_step_setting() -> f32 {
    1.0
}

fn is_default_editor_snap_step_setting(value: &f32) -> bool {
    (*value - 1.0).abs() <= 1e-6
}

fn default_editor_rotation_snap_setting() -> bool {
    true
}

fn is_default_editor_rotation_snap_setting(value: &bool) -> bool {
    *value
}

fn default_editor_rotation_snap_step_setting() -> f32 {
    15.0
}

fn is_default_editor_rotation_snap_step_setting(value: &f32) -> bool {
    (*value - 15.0).abs() <= 1e-6
}

fn default_graphics_backend_setting() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "BrowserWebGpu".to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        "Auto".to_string()
    }
}

fn is_default_graphics_backend_setting(value: &String) -> bool {
    value == &default_graphics_backend_setting()
}

fn default_audio_backend_setting() -> String {
    "Default".to_string()
}

fn is_default_audio_backend_setting(value: &String) -> bool {
    value == "Default"
}

fn default_app_keybinds() -> Vec<KeybindBinding> {
    default_essential_keybinds()
}

fn is_default_app_keybinds(value: &[KeybindBinding]) -> bool {
    value == default_essential_keybinds().as_slice()
}

fn deserialize_level_object_block_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Ok(normalize_block_id(&raw))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Represents a vertex in 3D space with position and color.
/// Used for rendering meshes in the graphics pipeline.
pub(crate) struct Vertex {
    pub(crate) position: [f32; 3],
    pub(crate) color: [f32; 4],
    pub(crate) uv: [f32; 2],
    pub(crate) uv_norm: [f32; 2],
    pub(crate) texture_layer: f32,
    pub(crate) color_outline: [f32; 4],
}

impl Vertex {
    pub(crate) fn untextured(position: [f32; 3], color: [f32; 4]) -> Self {
        Self {
            position,
            color,
            uv: [0.0, 0.0],
            uv_norm: [0.0, 0.0],
            texture_layer: -1.0,
            color_outline: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub(crate) fn textured(
        position: [f32; 3],
        color: [f32; 4],
        uv: [f32; 2],
        texture_layer: u32,
    ) -> Self {
        Self {
            position,
            color,
            uv,
            uv_norm: [0.0, 0.0],
            texture_layer: texture_layer as f32,
            color_outline: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub(crate) fn textured_with_outline(
        position: [f32; 3],
        color: [f32; 4],
        uv: [f32; 2],
        uv_norm: [f32; 2],
        texture_layer: u32,
        color_outline: [f32; 4],
    ) -> Self {
        Self {
            position,
            color,
            uv,
            uv_norm,
            texture_layer: texture_layer as f32,
            color_outline,
        }
    }

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x4,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32,
            5 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
/// Metadata for the music track used in a level.
/// Includes source file, title, author, and any extra fields.
pub(crate) struct MusicMetadata {
    #[serde(
        default = "default_music_source",
        skip_serializing_if = "is_default_music_source"
    )]
    pub(crate) source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) author: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for MusicMetadata {
    fn default() -> Self {
        Self {
            source: default_music_source(),
            title: None,
            author: None,
            extra: serde_json::Map::new(),
        }
    }
}

fn is_default_music_metadata(value: &MusicMetadata) -> bool {
    is_default_music_source(&value.source)
        && value.title.is_none()
        && value.author.is_none()
        && value.extra.is_empty()
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
/// A timing point that defines the tempo and time signature at a specific time in the level.
/// Used for rhythm-based gameplay and music synchronization.
pub(crate) struct TimingPoint {
    pub(crate) time_seconds: f32,
    pub(crate) bpm: f32,
    #[serde(
        default = "default_time_signature_numerator",
        skip_serializing_if = "is_default_time_signature_numerator"
    )]
    pub(crate) time_signature_numerator: u32,
    #[serde(
        default = "default_time_signature_denominator",
        skip_serializing_if = "is_default_time_signature_denominator"
    )]
    pub(crate) time_signature_denominator: u32,
}

fn default_time_signature_numerator() -> u32 {
    4
}

fn default_time_signature_denominator() -> u32 {
    4
}

fn is_default_time_signature_numerator(value: &u32) -> bool {
    *value == 4
}

fn is_default_time_signature_denominator(value: &u32) -> bool {
    *value == 4
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CameraTriggerMode {
    Follow,
    #[default]
    Static,
}

fn is_default_camera_trigger_mode(value: &CameraTriggerMode) -> bool {
    matches!(value, CameraTriggerMode::Static)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimedTriggerEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

fn is_default_camera_trigger_easing(value: &TimedTriggerEasing) -> bool {
    matches!(value, TimedTriggerEasing::Linear)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct CameraTrigger {
    pub(crate) time_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_mode")]
    pub(crate) mode: CameraTriggerMode,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub(crate) easing: TimedTriggerEasing,
    #[serde(
        default = "default_camera_trigger_transition_interval_seconds",
        skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
    )]
    pub(crate) transition_interval_seconds: f32,
    #[serde(
        default = "default_camera_trigger_use_full_segment_transition",
        skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
    )]
    pub(crate) use_full_segment_transition: bool,
    #[serde(
        default = "default_camera_trigger_target_position",
        skip_serializing_if = "is_default_camera_trigger_target_position"
    )]
    pub(crate) target_position: [f32; 3],
    #[serde(
        default = "default_camera_trigger_rotation",
        skip_serializing_if = "is_default_camera_trigger_rotation"
    )]
    pub(crate) rotation: f32,
    #[serde(
        default = "default_camera_trigger_pitch",
        skip_serializing_if = "is_default_camera_trigger_pitch"
    )]
    pub(crate) pitch: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TimedTriggerTarget {
    Camera,
    Object { object_id: u32 },
    Objects { object_ids: Vec<u32> },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TimedTriggerAction {
    MoveTo {
        position: [f32; 3],
    },
    RotateTo {
        rotation_degrees: [f32; 3],
    },
    ScaleTo {
        size: [f32; 3],
    },
    CameraPose {
        #[serde(
            default = "default_camera_trigger_transition_interval_seconds",
            skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
        )]
        transition_interval_seconds: f32,
        #[serde(
            default = "default_camera_trigger_use_full_segment_transition",
            skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
        )]
        use_full_segment_transition: bool,
        #[serde(
            default = "default_camera_trigger_target_position",
            skip_serializing_if = "is_default_camera_trigger_target_position"
        )]
        target_position: [f32; 3],
        #[serde(
            default = "default_camera_trigger_rotation",
            skip_serializing_if = "is_default_camera_trigger_rotation"
        )]
        rotation: f32,
        #[serde(
            default = "default_camera_trigger_pitch",
            skip_serializing_if = "is_default_camera_trigger_pitch"
        )]
        pitch: f32,
    },
    CameraFollow {
        #[serde(
            default = "default_camera_trigger_transition_interval_seconds",
            skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
        )]
        transition_interval_seconds: f32,
        #[serde(
            default = "default_camera_trigger_use_full_segment_transition",
            skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
        )]
        use_full_segment_transition: bool,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct TimedTrigger {
    pub(crate) time_seconds: f32,
    #[serde(
        default = "default_timed_trigger_duration_seconds",
        skip_serializing_if = "is_default_timed_trigger_duration_seconds"
    )]
    pub(crate) duration_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub(crate) easing: TimedTriggerEasing,
    #[serde(
        default = "default_timed_trigger_target",
        skip_serializing_if = "is_default_timed_trigger_target"
    )]
    pub(crate) target: TimedTriggerTarget,
    pub(crate) action: TimedTriggerAction,
}

#[cfg(test)]
pub(crate) fn camera_triggers_to_timed_triggers(
    camera_triggers: &[CameraTrigger],
) -> Vec<TimedTrigger> {
    let mut triggers = Vec::with_capacity(camera_triggers.len());
    for camera_trigger in camera_triggers {
        let action = match camera_trigger.mode {
            CameraTriggerMode::Follow => TimedTriggerAction::CameraFollow {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
            },
            CameraTriggerMode::Static => TimedTriggerAction::CameraPose {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
                target_position: camera_trigger.target_position,
                rotation: camera_trigger.rotation,
                pitch: camera_trigger.pitch,
            },
        };

        triggers.push(TimedTrigger {
            time_seconds: camera_trigger.time_seconds,
            duration_seconds: 0.0,
            easing: camera_trigger.easing,
            target: TimedTriggerTarget::Camera,
            action,
        });
    }

    triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    triggers
}

pub(crate) fn timed_triggers_to_camera_triggers(triggers: &[TimedTrigger]) -> Vec<CameraTrigger> {
    let mut camera_triggers = Vec::new();

    for trigger in triggers {
        if !matches!(trigger.target, TimedTriggerTarget::Camera) {
            continue;
        }

        match trigger.action {
            TimedTriggerAction::CameraPose {
                transition_interval_seconds,
                use_full_segment_transition,
                target_position,
                rotation,
                pitch,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraTriggerMode::Static,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position,
                    rotation,
                    pitch,
                });
            }
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds,
                use_full_segment_transition,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraTriggerMode::Follow,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: default_camera_trigger_target_position(),
                    rotation: default_camera_trigger_rotation(),
                    pitch: default_camera_trigger_pitch(),
                });
            }
            TimedTriggerAction::MoveTo { .. }
            | TimedTriggerAction::RotateTo { .. }
            | TimedTriggerAction::ScaleTo { .. } => {}
        }
    }

    camera_triggers.retain(|trigger| trigger.time_seconds.is_finite());
    camera_triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    camera_triggers
}

fn timed_trigger_eased_alpha(easing: TimedTriggerEasing, alpha: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    match easing {
        TimedTriggerEasing::Linear => alpha,
        TimedTriggerEasing::EaseIn => alpha * alpha,
        TimedTriggerEasing::EaseOut => 1.0 - (1.0 - alpha) * (1.0 - alpha),
        TimedTriggerEasing::EaseInOut => {
            if alpha < 0.5 {
                2.0 * alpha * alpha
            } else {
                1.0 - ((-2.0 * alpha + 2.0).powi(2) * 0.5)
            }
        }
    }
}

fn timed_trigger_progress(trigger: &TimedTrigger, time_seconds: f32) -> Option<f32> {
    let time_seconds = time_seconds.max(0.0);
    if !trigger.time_seconds.is_finite() {
        return None;
    }

    if trigger.duration_seconds <= 1e-6 {
        return (time_seconds + 1e-6 >= trigger.time_seconds).then_some(1.0);
    }

    let start = trigger.time_seconds;
    let end = start + trigger.duration_seconds.max(0.0);
    if time_seconds + 1e-6 < start {
        return None;
    }

    if time_seconds >= end {
        return Some(1.0);
    }

    let alpha = (time_seconds - start) / trigger.duration_seconds.max(1e-6);
    Some(timed_trigger_eased_alpha(trigger.easing, alpha))
}

fn timed_trigger_target_indices(target: &TimedTriggerTarget, object_count: usize) -> Vec<usize> {
    match target {
        TimedTriggerTarget::Camera => Vec::new(),
        TimedTriggerTarget::Object { object_id } => {
            let index = *object_id as usize;
            if index < object_count {
                vec![index]
            } else {
                Vec::new()
            }
        }
        TimedTriggerTarget::Objects { object_ids } => {
            let mut indices = Vec::new();
            for object_id in object_ids {
                let index = *object_id as usize;
                if index < object_count && !indices.contains(&index) {
                    indices.push(index);
                }
            }
            indices
        }
    }
}

pub(crate) fn apply_timed_triggers_to_objects(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    let mut objects = base_objects.to_vec();
    if objects.is_empty() || triggers.is_empty() {
        return objects;
    }

    let mut ordered_triggers = triggers
        .iter()
        .filter(|trigger| trigger.time_seconds.is_finite())
        .collect::<Vec<_>>();
    ordered_triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));

    for trigger in ordered_triggers {
        let Some(progress) = timed_trigger_progress(trigger, time_seconds) else {
            continue;
        };
        let target_indices = timed_trigger_target_indices(&trigger.target, objects.len());
        if target_indices.is_empty() {
            continue;
        }

        for index in target_indices {
            let Some(object) = objects.get_mut(index) else {
                continue;
            };

            match &trigger.action {
                TimedTriggerAction::MoveTo { position } => {
                    for (current, target) in object.position.iter_mut().zip(position.iter()) {
                        *current = *current + (*target - *current) * progress;
                    }
                }
                TimedTriggerAction::RotateTo { rotation_degrees } => {
                    for (current, target) in object
                        .rotation_degrees
                        .iter_mut()
                        .zip(rotation_degrees.iter())
                    {
                        *current = *current + (*target - *current) * progress;
                    }
                }
                TimedTriggerAction::ScaleTo { size } => {
                    for (current, target) in object.size.iter_mut().zip(size.iter()) {
                        let current_value = (*current).max(0.01);
                        let target_value = (*target).max(0.01);
                        *current = current_value + (target_value - current_value) * progress;
                    }
                }
                TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. } => {
                }
            }
        }
    }

    objects
}

#[derive(Deserialize, Serialize, Clone)]
/// Represents the metadata for a level, including music, spawn, timing, and objects.
/// This struct is serialized to/from JSON for level files.
pub(crate) struct LevelMetadata {
    #[serde(
        default = "default_level_format_version",
        skip_serializing_if = "is_default_level_format_version"
    )]
    pub(crate) format_version: u32,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "is_default_music_metadata")]
    pub(crate) music: MusicMetadata,
    #[serde(default, skip_serializing_if = "is_default_spawn_metadata")]
    pub(crate) spawn: SpawnMetadata,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tap_times: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) timing_points: Vec<TimingPoint>,
    #[serde(
        default = "default_timeline_time_seconds",
        skip_serializing_if = "is_default_timeline_time_seconds"
    )]
    pub(crate) timeline_time_seconds: f32,
    #[serde(
        default = "default_timeline_duration_seconds",
        skip_serializing_if = "is_default_timeline_duration_seconds"
    )]
    pub(crate) timeline_duration_seconds: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) triggers: Vec<TimedTrigger>,
    #[serde(
        default = "default_simulate_trigger_hitboxes",
        skip_serializing_if = "is_default_simulate_trigger_hitboxes"
    )]
    pub(crate) simulate_trigger_hitboxes: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) objects: Vec<LevelObject>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

pub(crate) struct EditorStateParams {
    pub name: String,
    pub music: MusicMetadata,
    pub spawn: SpawnMetadata,
    pub tap_times: Vec<f32>,
    pub timing_points: Vec<TimingPoint>,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub triggers: Vec<TimedTrigger>,
    pub simulate_trigger_hitboxes: bool,
    pub objects: Vec<LevelObject>,
}

impl LevelMetadata {
    pub(crate) fn from_editor_state(
        EditorStateParams {
            name,
            music,
            spawn,
            tap_times,
            timing_points,
            timeline_time_seconds,
            timeline_duration_seconds,
            triggers,
            simulate_trigger_hitboxes,
            objects,
        }: EditorStateParams,
    ) -> Self {
        Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            name,
            music,
            spawn,
            tap_times,
            timing_points,
            timeline_time_seconds,
            timeline_duration_seconds,
            triggers,
            simulate_trigger_hitboxes,
            objects,
            extra: serde_json::Map::new(),
        }
    }

    pub(crate) fn resolved_triggers(&self) -> Vec<TimedTrigger> {
        self.triggers.clone()
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
/// Metadata for the player's spawn point in a level.
/// Defines the initial position and facing direction.
pub(crate) struct SpawnMetadata {
    #[serde(
        default = "default_spawn_position",
        skip_serializing_if = "is_default_spawn_position"
    )]
    pub(crate) position: [f32; 3],
    #[serde(default, skip_serializing_if = "is_default_spawn_direction")]
    pub(crate) direction: SpawnDirection,
}

impl Default for SpawnMetadata {
    fn default() -> Self {
        Self {
            position: default_spawn_position(),
            direction: SpawnDirection::Forward,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
/// The direction the player faces when spawning.
/// Used to orient the camera and movement.
pub(crate) enum SpawnDirection {
    #[default]
    Forward,
    Right,
}

impl From<SpawnDirection> for Direction {
    fn from(value: SpawnDirection) -> Self {
        match value {
            SpawnDirection::Forward => Direction::Forward,
            SpawnDirection::Right => Direction::Right,
        }
    }
}

fn is_default_spawn_direction(value: &SpawnDirection) -> bool {
    matches!(value, SpawnDirection::Forward)
}

fn is_default_spawn_metadata(value: &SpawnMetadata) -> bool {
    is_default_spawn_position(&value.position) && is_default_spawn_direction(&value.direction)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
/// Represents an object in a level, such as a block or obstacle.
/// Includes position, size, rotation, and block type.
pub(crate) struct LevelObject {
    #[serde(
        default = "default_level_object_position",
        skip_serializing_if = "is_default_level_object_position"
    )]
    pub(crate) position: [f32; 3],
    #[serde(
        default = "default_level_object_size",
        skip_serializing_if = "is_default_level_object_size"
    )]
    pub(crate) size: [f32; 3],
    #[serde(
        default = "default_block_rotation_degrees",
        skip_serializing_if = "is_default_block_rotation_degrees"
    )]
    pub(crate) rotation_degrees: [f32; 3],
    #[serde(
        default = "default_block_roundness",
        skip_serializing_if = "is_default_block_roundness"
    )]
    pub(crate) roundness: f32,
    #[serde(
        default = "default_level_object_block_id",
        alias = "kind",
        deserialize_with = "deserialize_level_object_block_id",
        skip_serializing_if = "is_default_level_object_block_id"
    )]
    pub(crate) block_id: String,
    #[serde(
        default = "default_level_object_color_tint",
        skip_serializing_if = "is_default_level_object_color_tint"
    )]
    pub(crate) color_tint: [f32; 3],
}

impl LevelObject {
    pub(crate) fn normalize_block_id(&mut self) {
        self.block_id = normalize_block_id(&self.block_id);
    }
}

impl Default for LevelObject {
    fn default() -> Self {
        Self {
            position: default_level_object_position(),
            size: default_level_object_size(),
            rotation_degrees: default_block_rotation_degrees(),
            roundness: default_block_roundness(),
            block_id: default_level_object_block_id(),
            color_tint: default_level_object_color_tint(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
/// The current phase of the application.
/// Determines which UI and logic to run.
pub(crate) enum AppPhase {
    Splash,
    Menu,
    Playing,
    Editor,
    GameOver,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum SettingsSection {
    #[default]
    Backends,
    Keybinds,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyChord {
    pub(crate) key: String,
    #[serde(default)]
    pub(crate) ctrl: bool,
    #[serde(default)]
    pub(crate) shift: bool,
    #[serde(default)]
    pub(crate) alt: bool,
}

impl KeyChord {
    pub(crate) fn new(key: impl Into<String>, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key: normalize_binding_key(&key.into()),
            ctrl,
            shift,
            alt,
        }
    }

    pub(crate) fn normalized(&self) -> Self {
        Self {
            key: normalize_binding_key(&self.key),
            ctrl: self.ctrl,
            shift: self.shift,
            alt: self.alt,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeybindBinding {
    pub(crate) action: String,
    pub(crate) chord: KeyChord,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct AppSettings {
    #[serde(
        default = "default_app_settings_version",
        skip_serializing_if = "is_default_app_settings_version"
    )]
    pub(crate) version: u32,
    #[serde(
        default = "default_editor_selected_block_id",
        skip_serializing_if = "is_default_editor_selected_block_id"
    )]
    pub(crate) editor_selected_block_id: String,
    #[serde(
        default = "default_editor_snap_to_grid_setting",
        skip_serializing_if = "is_default_editor_snap_to_grid_setting"
    )]
    pub(crate) editor_snap_to_grid: bool,
    #[serde(
        default = "default_editor_snap_step_setting",
        skip_serializing_if = "is_default_editor_snap_step_setting"
    )]
    pub(crate) editor_snap_step: f32,
    #[serde(
        default = "default_editor_rotation_snap_setting",
        skip_serializing_if = "is_default_editor_rotation_snap_setting"
    )]
    pub(crate) editor_rotation_snap: bool,
    #[serde(
        default = "default_editor_rotation_snap_step_setting",
        skip_serializing_if = "is_default_editor_rotation_snap_step_setting"
    )]
    pub(crate) editor_rotation_snap_step: f32,
    #[serde(
        default = "default_graphics_backend_setting",
        skip_serializing_if = "is_default_graphics_backend_setting"
    )]
    pub(crate) graphics_backend: String,
    #[serde(
        default = "default_audio_backend_setting",
        skip_serializing_if = "is_default_audio_backend_setting"
    )]
    pub(crate) audio_backend: String,
    #[serde(
        default = "default_app_keybinds",
        skip_serializing_if = "is_default_app_keybinds"
    )]
    pub(crate) keybinds: Vec<KeybindBinding>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: APP_SETTINGS_VERSION,
            editor_selected_block_id: default_editor_selected_block_id(),
            editor_snap_to_grid: default_editor_snap_to_grid_setting(),
            editor_snap_step: default_editor_snap_step_setting(),
            editor_rotation_snap: default_editor_rotation_snap_setting(),
            editor_rotation_snap_step: default_editor_rotation_snap_step_setting(),
            graphics_backend: default_graphics_backend_setting(),
            audio_backend: default_audio_backend_setting(),
            keybinds: default_app_keybinds(),
        }
    }
}

impl AppSettings {
    pub(crate) fn keybinds_for_action(&self, action: &str) -> Vec<&KeyChord> {
        self.keybinds
            .iter()
            .filter(|binding| binding.action == action)
            .map(|binding| &binding.chord)
            .collect()
    }

    pub(crate) fn set_keybind_at_slot(&mut self, action: &str, slot: usize, chord: KeyChord) {
        let normalized = chord.normalized();

        // Prevent duplicate chord for SAME action
        if self
            .keybinds
            .iter()
            .any(|b| b.action == action && b.chord == normalized)
        {
            return;
        }

        let existing_indices: Vec<usize> = self
            .keybinds
            .iter()
            .enumerate()
            .filter(|(_, b)| b.action == action)
            .map(|(i, _)| i)
            .collect();

        if slot < existing_indices.len() {
            self.keybinds[existing_indices[slot]].chord = normalized;
        } else {
            let capacity = essential_keybind_actions()
                .iter()
                .find(|m| m.action == action)
                .map(|m| m.capacity)
                .unwrap_or(1);
            if existing_indices.len() < capacity {
                self.keybinds.push(KeybindBinding {
                    action: action.to_string(),
                    chord: normalized,
                });
            }
        }
    }

    pub(crate) fn clear_keybind_slot(&mut self, action: &str, slot: usize) {
        let existing_indices: Vec<usize> = self
            .keybinds
            .iter()
            .enumerate()
            .filter(|(_, b)| b.action == action)
            .map(|(i, _)| i)
            .collect();

        if slot < existing_indices.len() {
            self.keybinds.remove(existing_indices[slot]);
        }
    }

    pub(crate) fn reset_keybind(&mut self, action: &str) {
        self.keybinds.retain(|b| b.action != action);
        for default_binding in default_essential_keybinds() {
            if default_binding.action == action {
                self.keybinds.push(default_binding);
            }
        }
    }

    pub(crate) fn reset_essential_keybinds(&mut self) {
        let mut preserved = self.keybinds.clone();
        for metadata in essential_keybind_actions() {
            preserved.retain(|binding| binding.action != metadata.action);
        }
        preserved.extend(default_essential_keybinds());
        self.keybinds = preserved;
    }
}

pub(crate) fn normalize_binding_key(key: &str) -> String {
    match key {
        " " => "Space".to_string(),
        "ControlLeft" | "ControlRight" => "Control".to_string(),
        "AltLeft" | "AltRight" => "Alt".to_string(),
        _ => {
            if key.len() == 1 {
                key.to_ascii_lowercase()
            } else {
                key.to_string()
            }
        }
    }
}

pub(crate) fn format_key_chord(chord: &KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push("Ctrl".to_string());
    }
    if chord.shift {
        parts.push("Shift".to_string());
    }
    if chord.alt {
        parts.push("Alt".to_string());
    }

    let key = if chord.key.len() == 1 {
        chord.key.to_ascii_uppercase()
    } else {
        chord.key.clone()
    };
    parts.push(key);
    parts.join("+")
}

pub(crate) struct KeybindActionMetadata {
    pub(crate) group: &'static str,
    pub(crate) action: &'static str,
    pub(crate) label: &'static str,
    pub(crate) capacity: usize,
}

pub(crate) fn essential_keybind_actions() -> &'static [KeybindActionMetadata] {
    &[
        KeybindActionMetadata {
            group: "General",
            action: "toggle_settings",
            label: "Toggle Settings Sidebar",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "toggle_editor",
            label: "Toggle Editor",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "game_turn",
            label: "Turn / Action",
            capacity: 3,
        },
        KeybindActionMetadata {
            group: "General",
            action: "menu_prev_level",
            label: "Previous Level",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "General",
            action: "menu_next_level",
            label: "Next Level",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "toggle_timeline_playback",
            label: "Toggle Timeline Playback",
            capacity: 4,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "playtest",
            label: "Start Playtest",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "toggle_tap_timing",
            label: "Toggle Tap/Timing Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "remove_block",
            label: "Remove Block",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "copy",
            label: "Copy",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "paste",
            label: "Paste",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "duplicate",
            label: "Duplicate",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "undo",
            label: "Undo",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "redo",
            label: "Redo",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_up",
            label: "Pan Up",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_down",
            label: "Pan Down",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_left",
            label: "Pan Left",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "pan_right",
            label: "Pan Right",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_up",
            label: "Nudge Block Up",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_down",
            label: "Nudge Block Down",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_left",
            label: "Nudge Block Left",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "nudge_right",
            label: "Nudge Block Right",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "spawn_set",
            label: "Set Spawn Position",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "spawn_rotate",
            label: "Rotate Spawn Direction",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "add_camera_trigger",
            label: "Add Camera Trigger",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "timeline_forward",
            label: "Shift Timeline Forward",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Timeline",
            action: "timeline_backward",
            label: "Shift Timeline Backward",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "zoom_in",
            label: "Zoom In",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Editor",
            action: "zoom_out",
            label: "Zoom Out",
            capacity: 2,
        },
        KeybindActionMetadata {
            group: "Debug",
            action: "export_obj",
            label: "Export Block as OBJ",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Debug",
            action: "toggle_perf_overlay",
            label: "Toggle Performance Overlay",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_select",
            label: "Select Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_move",
            label: "Move Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_scale",
            label: "Scale Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_rotate",
            label: "Rotate Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_place",
            label: "Place Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "Modes",
            action: "mode_trigger",
            label: "Trigger Mode",
            capacity: 1,
        },
        KeybindActionMetadata {
            group: "General",
            action: "escape",
            label: "Context Escape",
            capacity: 1,
        },
    ]
}

pub(crate) fn default_essential_keybinds() -> Vec<KeybindBinding> {
    vec![
        KeybindBinding {
            action: "toggle_settings".to_string(),
            chord: KeyChord::new("o", true, false, false),
        },
        KeybindBinding {
            action: "toggle_editor".to_string(),
            chord: KeyChord::new("e", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("w", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("Space", false, false, false),
        },
        KeybindBinding {
            action: "game_turn".to_string(),
            chord: KeyChord::new("ArrowUp", false, false, false),
        },
        KeybindBinding {
            action: "menu_prev_level".to_string(),
            chord: KeyChord::new("a", false, false, false),
        },
        KeybindBinding {
            action: "menu_prev_level".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "menu_next_level".to_string(),
            chord: KeyChord::new("d", false, false, false),
        },
        KeybindBinding {
            action: "menu_next_level".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("Space", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPlayPause", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPlay", false, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("MediaPause", false, false, false),
        },
        KeybindBinding {
            action: "playtest".to_string(),
            chord: KeyChord::new("Enter", false, false, false),
        },
        KeybindBinding {
            action: "toggle_tap_timing".to_string(),
            chord: KeyChord::new("t", false, false, false),
        },
        KeybindBinding {
            action: "remove_block".to_string(),
            chord: KeyChord::new("Delete", false, false, false),
        },
        KeybindBinding {
            action: "copy".to_string(),
            chord: KeyChord::new("c", true, false, false),
        },
        KeybindBinding {
            action: "paste".to_string(),
            chord: KeyChord::new("v", true, false, false),
        },
        KeybindBinding {
            action: "duplicate".to_string(),
            chord: KeyChord::new("d", true, false, false),
        },
        KeybindBinding {
            action: "undo".to_string(),
            chord: KeyChord::new("z", true, false, false),
        },
        KeybindBinding {
            action: "redo".to_string(),
            chord: KeyChord::new("y", true, false, false),
        },
        KeybindBinding {
            action: "redo".to_string(),
            chord: KeyChord::new("z", true, true, false),
        },
        KeybindBinding {
            action: "pan_up".to_string(),
            chord: KeyChord::new("w", false, false, false),
        },
        KeybindBinding {
            action: "pan_down".to_string(),
            chord: KeyChord::new("s", false, false, false),
        },
        KeybindBinding {
            action: "pan_left".to_string(),
            chord: KeyChord::new("a", false, false, false),
        },
        KeybindBinding {
            action: "pan_right".to_string(),
            chord: KeyChord::new("d", false, false, false),
        },
        KeybindBinding {
            action: "nudge_up".to_string(),
            chord: KeyChord::new("ArrowUp", false, false, false),
        },
        KeybindBinding {
            action: "nudge_down".to_string(),
            chord: KeyChord::new("ArrowDown", false, false, false),
        },
        KeybindBinding {
            action: "nudge_left".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "nudge_right".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "spawn_set".to_string(),
            chord: KeyChord::new("p", false, false, false),
        },
        KeybindBinding {
            action: "spawn_rotate".to_string(),
            chord: KeyChord::new("r", false, false, false),
        },
        KeybindBinding {
            action: "add_camera_trigger".to_string(),
            chord: KeyChord::new("k", false, true, false),
        },
        KeybindBinding {
            action: "timeline_forward".to_string(),
            chord: KeyChord::new("ArrowRight", false, false, false),
        },
        KeybindBinding {
            action: "timeline_backward".to_string(),
            chord: KeyChord::new("ArrowLeft", false, false, false),
        },
        KeybindBinding {
            action: "zoom_in".to_string(),
            chord: KeyChord::new("=", false, false, false),
        },
        KeybindBinding {
            action: "zoom_in".to_string(),
            chord: KeyChord::new("+", false, false, false),
        },
        KeybindBinding {
            action: "zoom_out".to_string(),
            chord: KeyChord::new("-", false, false, false),
        },
        KeybindBinding {
            action: "zoom_out".to_string(),
            chord: KeyChord::new("_", false, false, false),
        },
        KeybindBinding {
            action: "export_obj".to_string(),
            chord: KeyChord::new("o", true, true, true),
        },
        KeybindBinding {
            action: "toggle_perf_overlay".to_string(),
            chord: KeyChord::new("F12", true, true, true),
        },
        KeybindBinding {
            action: "mode_select".to_string(),
            chord: KeyChord::new("1", false, false, false),
        },
        KeybindBinding {
            action: "mode_move".to_string(),
            chord: KeyChord::new("2", false, false, false),
        },
        KeybindBinding {
            action: "mode_scale".to_string(),
            chord: KeyChord::new("3", false, false, false),
        },
        KeybindBinding {
            action: "mode_rotate".to_string(),
            chord: KeyChord::new("4", false, false, false),
        },
        KeybindBinding {
            action: "mode_place".to_string(),
            chord: KeyChord::new("5", false, false, false),
        },
        KeybindBinding {
            action: "mode_trigger".to_string(),
            chord: KeyChord::new("6", false, false, false),
        },
        KeybindBinding {
            action: "escape".to_string(),
            chord: KeyChord::new("Escape", false, false, false),
        },
    ]
}

/// State for the main menu screen.
/// Manages level selection and available levels list.
/// State for the main menu.
/// Tracks the currently selected level and the list of available levels.
pub(crate) struct MenuState {
    pub(crate) selected_level: usize,
    pub(crate) levels: Vec<String>,
}

/// State for the level editor.
/// Manages cursor position, mode, and other editor-specific settings.
pub(crate) struct EditorState {
    pub(crate) cursor: [f32; 3],
    pub(crate) mode: EditorMode,
    pub(crate) left_mouse_down: bool,
    pub(crate) right_dragging: bool,
    pub(crate) pan_up_held: bool,
    pub(crate) pan_down_held: bool,
    pub(crate) pan_left_held: bool,
    pub(crate) pan_right_held: bool,
    pub(crate) shift_held: bool,
    pub(crate) ctrl_held: bool,
    pub(crate) alt_held: bool,
    pub(crate) selected_block_index: Option<usize>,
    pub(crate) selected_block_indices: Vec<usize>,
    pub(crate) hovered_block_index: Option<usize>,
    pub(crate) pointer_screen: Option<[f64; 2]>,
    pub(crate) marquee_start_screen: Option<[f64; 2]>,
    pub(crate) marquee_current_screen: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// The current mode of the level editor.
/// Determines what actions are available and how interactions behave.
pub(crate) enum EditorMode {
    Select,
    Move,
    Scale,
    Rotate,
    #[default]
    Place,
    Trigger,
    Timing,
    Null,
}

impl EditorMode {
    pub(crate) fn is_selection_mode(self) -> bool {
        matches!(self, Self::Select | Self::Move | Self::Scale | Self::Rotate)
    }

    pub(crate) fn is_compose_mode(self) -> bool {
        matches!(
            self,
            Self::Select
                | Self::Move
                | Self::Scale
                | Self::Rotate
                | Self::Place
                | Self::Trigger
                | Self::Null
        )
    }

    pub(crate) fn shows_move_gizmo(self) -> bool {
        self == Self::Move
    }

    pub(crate) fn shows_scale_gizmo(self) -> bool {
        self == Self::Scale
    }

    pub(crate) fn shows_rotate_gizmo(self) -> bool {
        self == Self::Rotate
    }

    pub(crate) fn shows_gizmo(self) -> bool {
        self.shows_move_gizmo() || self.shows_scale_gizmo() || self.shows_rotate_gizmo()
    }

    pub(crate) fn can_select(self) -> bool {
        self != Self::Null && self != Self::Timing && self != Self::Trigger
    }
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self {
            cursor: [0.0, 0.0, 0.0],
            mode: EditorMode::Place,
            left_mouse_down: false,
            right_dragging: false,
            pan_up_held: false,
            pan_down_held: false,
            pan_left_held: false,
            pan_right_held: false,
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
            selected_block_index: None,
            selected_block_indices: Vec::new(),
            hovered_block_index: None,
            pointer_screen: None,
            marquee_start_screen: None,
            marquee_current_screen: None,
        }
    }
}

/// A generic size structure with width and height components.
/// Used for representing physical dimensions like window size or surface size.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalSize<T> {
    pub(crate) width: T,
    pub(crate) height: T,
}

impl<T> PhysicalSize<T> {
    pub(crate) fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Cardinal directions for movement and orientation.
pub(crate) enum Direction {
    Forward,
    Right,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for line rendering parameters.
/// Contains offset and rotation for positioning lines in screen space.
pub(crate) struct LineUniform {
    pub(crate) offset: [f32; 2],
    pub(crate) rotation: f32,
    pub(crate) _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for camera transformation matrix.
/// Contains the view-projection matrix for 3D rendering.
pub(crate) struct CameraUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for color space settings.
/// Controls gamma correction application in the shader.
pub(crate) struct ColorSpaceUniform {
    pub(crate) apply_gamma_correction: f32,
    pub(crate) _pad: [f32; 3],
}

#[cfg(test)]
mod tests {
    use super::{
        camera_triggers_to_timed_triggers, default_camera_trigger_pitch,
        default_camera_trigger_rotation, default_camera_trigger_transition_interval_seconds,
        timed_triggers_to_camera_triggers, CameraTrigger, CameraTriggerMode, EditorStateParams,
        LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata,
        TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget, Vertex,
    };
    use serde_json::json;

    #[test]
    fn level_object_rotation_defaults_when_missing() {
        let json = r#"{
            "position":[1.0,2.0,3.0],
            "size":[4.0,5.0,6.0],
            "kind":"standard"
        }"#;

        let object: LevelObject = serde_json::from_str(json).expect("valid level object");
        assert_eq!(object.rotation_degrees, [0.0, 0.0, 0.0]);
        assert_eq!(object.roundness, 0.18);
        assert_eq!(object.block_id, "core/stone");
        assert_eq!(object.color_tint, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn level_metadata_parses_objects_without_rotation_field() {
        let json = r#"{
            "name":"Compat",
            "music":{"source":"music.mp3"},
            "spawn":{"position":[0.0,0.0,0.0],"direction":"forward"},
            "objects":[
                {"position":[0.0,0.0,0.0],"size":[1.0,1.0,1.0],"kind":"grass"}
            ]
        }"#;

        let metadata: LevelMetadata = serde_json::from_str(json).expect("valid metadata");
        assert_eq!(metadata.objects.len(), 1);
        assert_eq!(metadata.objects[0].rotation_degrees, [0.0, 0.0, 0.0]);
        assert_eq!(metadata.objects[0].roundness, 0.18);
        assert!(matches!(metadata.spawn.direction, SpawnDirection::Forward));
    }

    #[test]
    fn level_object_serialization_omits_default_rotation_and_roundness() {
        let object = LevelObject {
            position: [1.0, 2.0, 3.0],
            size: [4.0, 5.0, 6.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/grass".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        };

        let value = serde_json::to_value(&object).expect("serialize object");
        let expected = json!({
            "position": [1.0, 2.0, 3.0],
            "size": [4.0, 5.0, 6.0],
            "block_id": "core/grass"
        });

        assert_eq!(value, expected);
    }

    #[test]
    fn level_metadata_serialization_omits_default_fields() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "Minimal".to_string(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            triggers: Vec::new(),
            simulate_trigger_hitboxes: false,
            objects: vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            }],
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value["name"], "Minimal");
        assert!(value.get("format_version").is_none());
        assert!(value.get("music").is_none());
        assert!(value.get("spawn").is_none());
        assert!(value.get("tap_times").is_none());
        assert!(value.get("timeline_time_seconds").is_none());
        assert!(value.get("timeline_duration_seconds").is_none());
        assert!(value.get("simulate_trigger_hitboxes").is_none());
        assert!(value.get("camera_triggers").is_none());
        assert!(value.get("taps").is_none());
        assert!(value.get("timeline_step").is_none());
        assert_eq!(value["objects"].as_array().map(|v| v.len()), Some(1));
        let object = &value["objects"][0];
        assert!(object.get("position").is_none());
        assert!(object.get("size").is_none());
        assert!(object.get("rotation_degrees").is_none());
        assert!(object.get("roundness").is_none());
        assert!(object.get("block_id").is_none());
    }

    #[test]
    fn level_metadata_serialization_includes_simulate_trigger_hitboxes_when_enabled() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "HitboxPolicy".to_string(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            triggers: Vec::new(),
            simulate_trigger_hitboxes: true,
            objects: Vec::new(),
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value.get("simulate_trigger_hitboxes"), Some(&json!(true)));
    }

    #[test]
    fn camera_trigger_serialization_omits_default_pose_fields() {
        let camera_trigger = CameraTrigger {
            time_seconds: 2.5,
            mode: CameraTriggerMode::Follow,
            easing: TimedTriggerEasing::EaseInOut,
            transition_interval_seconds: default_camera_trigger_transition_interval_seconds(),
            use_full_segment_transition: false,
            target_position: [0.0, 0.0, 0.0],
            rotation: default_camera_trigger_rotation(),
            pitch: default_camera_trigger_pitch(),
        };

        let value = serde_json::to_value(&camera_trigger).expect("serialize camera trigger");
        let expected = json!({
            "time_seconds": 2.5,
            "mode": "follow",
            "easing": "ease_in_out"
        });

        assert_eq!(value, expected);
    }

    #[test]
    fn converts_camera_triggers_to_triggers_and_back() {
        let camera_triggers = vec![
            CameraTrigger {
                time_seconds: 0.5,
                mode: CameraTriggerMode::Follow,
                easing: TimedTriggerEasing::EaseIn,
                transition_interval_seconds: 0.4,
                use_full_segment_transition: true,
                target_position: [0.0, 0.0, 0.0],
                rotation: default_camera_trigger_rotation(),
                pitch: default_camera_trigger_pitch(),
            },
            CameraTrigger {
                time_seconds: 1.0,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::EaseOut,
                transition_interval_seconds: 0.25,
                use_full_segment_transition: false,
                target_position: [1.0, 2.0, 3.0],
                rotation: 0.8,
                pitch: 0.2,
            },
        ];

        let triggers = camera_triggers_to_timed_triggers(&camera_triggers);
        assert_eq!(triggers.len(), 2);
        assert!(matches!(triggers[0].target, TimedTriggerTarget::Camera));
        assert!(matches!(
            triggers[0].action,
            TimedTriggerAction::CameraFollow { .. }
        ));

        let restored = timed_triggers_to_camera_triggers(&triggers);
        assert_eq!(restored, camera_triggers);
    }

    #[test]
    fn resolved_triggers_return_trigger_data_for_camera_conversion() {
        let metadata = LevelMetadata {
            format_version: 1,
            name: "Bridge".to_string(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            triggers: camera_triggers_to_timed_triggers(&[CameraTrigger {
                time_seconds: 1.2,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::Linear,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [2.0, 3.0, 4.0],
                rotation: 0.4,
                pitch: 0.6,
            }]),
            simulate_trigger_hitboxes: false,
            objects: Vec::new(),
            extra: serde_json::Map::new(),
        };

        let resolved = timed_triggers_to_camera_triggers(&metadata.resolved_triggers());
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].time_seconds - 1.2).abs() <= 1e-6);
    }

    #[test]
    fn test_essential_keybind_groups() {
        let actions = super::essential_keybind_actions();
        assert!(!actions.is_empty());
        for metadata in actions {
            assert!(!metadata.group.is_empty());
            assert!(!metadata.action.is_empty());
        }
    }

    #[test]
    fn test_app_settings_keybind_management() {
        let mut settings = super::AppSettings::default();
        let action = "zoom_in"; // capacity 2
        let chord1 = super::KeyChord::new("=", false, false, false);
        let chord2 = super::KeyChord::new("+", false, false, false);
        let chord3 = super::KeyChord::new("k", true, false, false);

        // Clear defaults first to be sure
        settings.keybinds.retain(|b| b.action != action);
        assert_eq!(settings.keybinds_for_action(action).len(), 0);

        // Test multi-slot append
        settings.set_keybind_at_slot(action, 0, chord1.clone());
        settings.set_keybind_at_slot(action, 1, chord2.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Test duplicate chord prevention SAME action
        settings.set_keybind_at_slot(action, 1, chord1.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);
        assert_eq!(settings.keybinds_for_action(action)[0], &chord1);
        assert_eq!(settings.keybinds_for_action(action)[1], &chord2);

        // Test capacity enforcement (max 2)
        settings.set_keybind_at_slot(action, 2, chord3.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Test slot replacement
        settings.set_keybind_at_slot(action, 0, chord3.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);
        assert!(settings.keybinds_for_action(action).contains(&&chord3));

        // Test single-slot replacement
        let single_action = "undo";
        let u_chord = super::KeyChord::new("u", false, false, false);
        settings.set_keybind_at_slot(single_action, 0, u_chord.clone());
        assert_eq!(settings.keybinds_for_action(single_action).len(), 1);
        assert_eq!(
            settings.keybinds_for_action(single_action).first().copied(),
            Some(&u_chord)
        );

        let u_chord2 = super::KeyChord::new("z", true, false, false);
        settings.set_keybind_at_slot(single_action, 0, u_chord2.clone());
        assert_eq!(settings.keybinds_for_action(single_action).len(), 1);
        assert_eq!(
            settings.keybinds_for_action(single_action).first().copied(),
            Some(&u_chord2)
        );
    }

    #[test]
    fn untextured_vertex_uses_texture_bypass_layer() {
        let vertex = Vertex::untextured([1.0, 2.0, 3.0], [0.2, 0.3, 0.4, 0.5]);
        assert!(
            vertex.texture_layer < 0.0,
            "untextured vertices should bypass texture sampling"
        );
    }

    #[test]
    fn test_reset_essential_keybinds_with_groups() {
        let mut settings = super::AppSettings::default();
        let action = "toggle_settings";
        let custom_chord = super::KeyChord::new("k", true, false, false);

        // Change a default keybind
        settings.set_keybind_at_slot(action, 0, custom_chord.clone());
        assert_eq!(
            settings.keybinds_for_action(action).first().copied(),
            Some(&custom_chord)
        );

        // Reset
        settings.reset_essential_keybinds();

        // Should be back to default
        let defaults = super::default_essential_keybinds();
        let default_chord = defaults
            .iter()
            .find(|b| b.action == action)
            .map(|b| &b.chord);
        assert_eq!(
            settings.keybinds_for_action(action).first().copied(),
            default_chord
        );
    }

    #[test]
    fn test_reset_single_multi_slot_keybind() {
        let mut settings = super::AppSettings::default();
        let action = "zoom_in"; // Capacity 2, defaults are = and +

        // Custom bindings
        settings.set_keybind_at_slot(action, 0, super::KeyChord::new("1", false, false, false));
        settings.set_keybind_at_slot(action, 1, super::KeyChord::new("2", false, false, false));
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Reset
        settings.reset_keybind(action);

        // Should be back to BOTH defaults
        let chords = settings.keybinds_for_action(action);
        assert_eq!(chords.len(), 2);

        let defaults = super::default_essential_keybinds();
        let expected: Vec<_> = defaults
            .iter()
            .filter(|b| b.action == action)
            .map(|b| &b.chord)
            .collect();
        assert_eq!(chords, expected);
    }
}
