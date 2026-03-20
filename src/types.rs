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
}

impl Vertex {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
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
    pub(crate) fn keybind_for_action(&self, action: &str) -> Option<&KeyChord> {
        self.keybinds
            .iter()
            .find(|binding| binding.action == action)
            .map(|binding| &binding.chord)
    }

    pub(crate) fn set_keybind(&mut self, action: &str, chord: KeyChord) {
        let normalized = chord.normalized();
        self.keybinds
            .retain(|binding| binding.action != action && binding.chord.normalized() != normalized);
        self.keybinds.push(KeybindBinding {
            action: action.to_string(),
            chord: normalized,
        });
    }

    pub(crate) fn clear_keybind(&mut self, action: &str) {
        self.keybinds.retain(|binding| binding.action != action);
    }

    pub(crate) fn reset_essential_keybinds(&mut self) {
        let mut preserved = self.keybinds.clone();
        for (action, _) in essential_keybind_actions() {
            preserved.retain(|binding| binding.action != *action);
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

pub(crate) fn essential_keybind_actions() -> &'static [(&'static str, &'static str)] {
    &[
        ("toggle_settings", "Toggle Settings Sidebar"),
        ("toggle_timeline_playback", "Toggle Timeline Playback"),
        ("playtest", "Start Playtest"),
        ("remove_block", "Remove Block"),
        ("copy", "Copy"),
        ("paste", "Paste"),
        ("duplicate", "Duplicate"),
        ("undo", "Undo"),
        ("redo", "Redo"),
        ("escape", "Context Escape"),
    ]
}

pub(crate) fn default_essential_keybinds() -> Vec<KeybindBinding> {
    vec![
        KeybindBinding {
            action: "toggle_settings".to_string(),
            chord: KeyChord::new("o", true, false, false),
        },
        KeybindBinding {
            action: "toggle_timeline_playback".to_string(),
            chord: KeyChord::new("Space", false, false, false),
        },
        KeybindBinding {
            action: "playtest".to_string(),
            chord: KeyChord::new("Enter", false, false, false),
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
        apply_timed_triggers_to_objects, camera_triggers_to_timed_triggers,
        default_camera_trigger_pitch, default_camera_trigger_rotation,
        default_camera_trigger_transition_interval_seconds, timed_triggers_to_camera_triggers,
        CameraTrigger, CameraTriggerMode, EditorStateParams, LevelMetadata, LevelObject,
        MusicMetadata, SpawnDirection, SpawnMetadata, TimedTrigger, TimedTriggerAction,
        TimedTriggerEasing, TimedTriggerTarget,
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
            objects: Vec::new(),
            extra: serde_json::Map::new(),
        };

        let resolved = timed_triggers_to_camera_triggers(&metadata.resolved_triggers());
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].time_seconds - 1.2).abs() <= 1e-6);
    }

    #[test]
    fn applies_timed_object_triggers_with_duration_and_point_actions() {
        let base_objects = vec![
            LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                position: [3.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
        ];

        let triggers = vec![
            TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Object { object_id: 1 },
                action: TimedTriggerAction::RotateTo {
                    rotation_degrees: [0.0, 90.0, 0.0],
                },
            },
            TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 2.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Object { object_id: 0 },
                action: TimedTriggerAction::MoveTo {
                    position: [10.0, 0.0, 0.0],
                },
            },
            TimedTrigger {
                time_seconds: 2.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![0, 1],
                },
                action: TimedTriggerAction::ScaleTo {
                    size: [2.0, 2.0, 2.0],
                },
            },
        ];

        let t_half = apply_timed_triggers_to_objects(&base_objects, &triggers, 2.0);
        assert!((t_half[0].position[0] - 5.0).abs() <= 1e-5);
        assert_eq!(t_half[0].size, [2.0, 2.0, 2.0]);
        assert_eq!(t_half[1].size, [2.0, 2.0, 2.0]);
        assert!((t_half[1].rotation_degrees[1] - 90.0).abs() <= 1e-5);

        let t_done = apply_timed_triggers_to_objects(&base_objects, &triggers, 3.0);
        assert!((t_done[0].position[0] - 10.0).abs() <= 1e-5);
        assert!((t_done[1].rotation_degrees[1] - 90.0).abs() <= 1e-5);
    }
}
