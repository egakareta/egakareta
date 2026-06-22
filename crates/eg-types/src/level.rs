/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::{HashMap, HashSet};

use serde::Deserializer;
use serde::{Deserialize, Serialize};

use crate::block_id::{normalize_block_id, DEFAULT_BLOCK_ID};

pub const CAMERA_TRIGGER_BLOCK_ID: &str = "core/camera_trigger";
pub const TRANSFORM_TRIGGER_BLOCK_ID: &str = "core/transform_trigger";
pub const DEFAULT_CAMERA_TRIGGER_ROTATION: f32 = -std::f32::consts::FRAC_PI_4;
pub const DEFAULT_CAMERA_TRIGGER_PITCH: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_EDITOR_CAMERA_ROTATION: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_EDITOR_CAMERA_PITCH: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_PLAY_CAMERA_ROTATION: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_PLAY_CAMERA_PITCH: f32 = std::f32::consts::FRAC_PI_4;
pub const CURRENT_LEVEL_FORMAT_VERSION: u32 = 3;
pub const CAMERA_TRIGGER_VIEW_DISTANCE: f32 = 24.0;

pub type ObjectId = String;
pub const OBJECT_ID_PREFIX: &str = "obj_";

macro_rules! serde_default {
    ($default_name:ident : $ty:ty = $value:expr) => {
        #[allow(dead_code)]
        pub fn $default_name() -> $ty {
            $value
        }
        paste::paste! {
            #[allow(dead_code)]
            pub fn [<is_ $default_name>](value: &$ty) -> bool {
                *value == $value
            }
        }
    };
}

macro_rules! serde_default_float {
    ($default_name:ident : $ty:ty = $value:expr) => {
        #[allow(dead_code)]
        pub fn $default_name() -> $ty {
            $value
        }
        paste::paste! {
            #[allow(dead_code)]
            pub fn [<is_ $default_name>](value: &$ty) -> bool {
                (*value - $value).abs() <= 1e-6
            }
        }
    };
}

macro_rules! serde_default_array {
    ($default_name:ident : [$ty:ty; $n:literal] = $value:expr) => {
        #[allow(dead_code)]
        pub fn $default_name() -> [$ty; $n] {
            $value
        }
        paste::paste! {
            #[allow(dead_code)]
            pub fn [<is_ $default_name>](value: &[$ty; $n]) -> bool {
                value
                    .iter()
                    .zip($value.iter())
                    .all(|(a, b)| (*a - *b).abs() <= 1e-6)
            }
        }
    };
}

pub fn default_sky_color() -> [f32; 3] {
    [0.04, 0.07, 0.09]
}

pub fn is_default_sky_color(value: &[f32; 3]) -> bool {
    value
        .iter()
        .zip(default_sky_color())
        .all(|(component, default)| (*component - default).abs() <= f32::EPSILON)
}

serde_default!(default_level_format_version: u32 = CURRENT_LEVEL_FORMAT_VERSION);
serde_default!(default_music_source: String = "music.mp3".to_string());
serde_default_array!(default_spawn_position: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_preview_camera_position_zero: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_preview_camera_target_zero: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_float!(default_timeline_time_seconds: f32 = 0.0);
serde_default_float!(default_timeline_duration_seconds: f32 = 16.0);
serde_default!(default_transform_simulate_hitboxes: bool = false);
serde_default_array!(default_block_rotation_degrees: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_level_object_position: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_level_object_size: [f32; 3] = [1.0, 1.0, 1.0]);
serde_default!(default_level_object_block_id: String = DEFAULT_BLOCK_ID.to_string());
serde_default_array!(default_level_object_color_tint: [f32; 3] = [1.0, 1.0, 1.0]);
serde_default!(default_level_object_id: ObjectId = String::new());
serde_default_float!(default_level_stars: f32 = 0.0);
serde_default!(default_level_tags: Vec<String> = Vec::<String>::new());
serde_default!(default_time_signature_numerator: u32 = 4);
serde_default!(default_time_signature_denominator: u32 = 4);

pub fn default_camera_trigger_target_position() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

pub fn is_default_camera_trigger_target_position(value: &[f32; 3]) -> bool {
    value
        .iter()
        .zip(default_camera_trigger_target_position())
        .all(|(component, default)| (*component - default).abs() <= 1e-6)
}

pub fn default_camera_trigger_rotation() -> f32 {
    DEFAULT_CAMERA_TRIGGER_ROTATION
}

pub fn is_default_camera_trigger_rotation(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_TRIGGER_ROTATION).abs() <= 1e-6
}

pub fn default_camera_trigger_pitch() -> f32 {
    DEFAULT_CAMERA_TRIGGER_PITCH
}

pub fn is_default_camera_trigger_pitch(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_TRIGGER_PITCH).abs() <= 1e-6
}

pub fn default_camera_trigger_distance() -> f32 {
    CAMERA_TRIGGER_VIEW_DISTANCE
}

pub fn is_default_camera_trigger_distance(value: &f32) -> bool {
    (*value - CAMERA_TRIGGER_VIEW_DISTANCE).abs() <= 1e-6
}

pub fn default_camera_trigger_transition_interval_seconds() -> f32 {
    1.0
}

pub fn is_default_camera_trigger_transition_interval_seconds(value: &f32) -> bool {
    (*value - 1.0).abs() <= 1e-6
}

pub fn default_camera_trigger_use_full_segment_transition() -> bool {
    false
}

pub fn is_default_camera_trigger_use_full_segment_transition(value: &bool) -> bool {
    !*value
}

pub fn default_timed_trigger_duration_seconds() -> f32 {
    0.0
}

pub fn is_default_timed_trigger_duration_seconds(value: &f32) -> bool {
    value.abs() <= 1e-6
}

pub fn default_timed_trigger_target() -> TimedTriggerTarget {
    TimedTriggerTarget::Camera
}

pub fn is_default_timed_trigger_target(value: &TimedTriggerTarget) -> bool {
    matches!(value, TimedTriggerTarget::Camera)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Forward,
    Right,
}

pub fn is_default_direction(value: &Direction) -> bool {
    matches!(value, Direction::Forward)
}

pub fn is_default_spawn_metadata(value: &SpawnMetadata) -> bool {
    is_default_spawn_position(&value.position) && is_default_direction(&value.direction)
}

pub fn normalize_level_block_id(block_id: &str) -> String {
    normalize_block_id(block_id)
}

pub fn deserialize_level_object_block_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Ok(normalize_level_block_id(&raw))
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct MusicMetadata {
    #[serde(
        default = "default_music_source",
        skip_serializing_if = "is_default_music_source"
    )]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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

pub fn is_default_music_metadata(value: &MusicMetadata) -> bool {
    is_default_music_source(&value.source)
        && value.title.is_none()
        && value.author.is_none()
        && value.extra.is_empty()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelCreatorMetadata {
    pub creator: Option<String>,
    pub description: Option<String>,
    pub stars: f32,
    pub tags: Vec<String>,
    pub version: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TimingPoint {
    pub time_seconds: f32,
    pub bpm: f32,
    #[serde(
        default = "default_time_signature_numerator",
        skip_serializing_if = "is_default_time_signature_numerator"
    )]
    pub time_signature_numerator: u32,
    #[serde(
        default = "default_time_signature_denominator",
        skip_serializing_if = "is_default_time_signature_denominator"
    )]
    pub time_signature_denominator: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LevelPreviewCameraMetadata {
    #[serde(
        default = "default_preview_camera_position_zero",
        skip_serializing_if = "is_default_preview_camera_position_zero"
    )]
    pub position: [f32; 3],
    #[serde(
        default = "default_preview_camera_target_zero",
        skip_serializing_if = "is_default_preview_camera_target_zero"
    )]
    pub target: [f32; 3],
}

pub struct EditorStateParams {
    pub name: String,
    pub creator_metadata: LevelCreatorMetadata,
    pub music: MusicMetadata,
    pub spawn: SpawnMetadata,
    pub sky_color: [f32; 3],
    pub tap_times: Vec<f32>,
    pub timing_points: Vec<TimingPoint>,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    pub objects: Vec<LevelObject>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LevelMetadata {
    #[serde(
        default = "default_level_format_version",
        skip_serializing_if = "is_default_level_format_version"
    )]
    pub format_version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default = "default_level_stars",
        skip_serializing_if = "is_default_level_stars"
    )]
    pub stars: f32,
    #[serde(
        default = "default_level_tags",
        skip_serializing_if = "is_default_level_tags"
    )]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_music_metadata")]
    pub music: MusicMetadata,
    #[serde(default, skip_serializing_if = "is_default_spawn_metadata")]
    pub spawn: SpawnMetadata,
    #[serde(
        default = "default_sky_color",
        skip_serializing_if = "is_default_sky_color"
    )]
    pub sky_color: [f32; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tap_times: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timing_points: Vec<TimingPoint>,
    #[serde(
        default = "default_timeline_time_seconds",
        skip_serializing_if = "is_default_timeline_time_seconds"
    )]
    pub timeline_time_seconds: f32,
    #[serde(
        default = "default_timeline_duration_seconds",
        skip_serializing_if = "is_default_timeline_duration_seconds"
    )]
    pub timeline_duration_seconds: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<LevelObject>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for LevelMetadata {
    fn default() -> Self {
        Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            name: "Untitled".to_string(),
            creator: None,
            description: None,
            stars: default_level_stars(),
            tags: default_level_tags(),
            version: None,
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: default_sky_color(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: default_timeline_time_seconds(),
            timeline_duration_seconds: default_timeline_duration_seconds(),
            menu_preview_camera: None,
            objects: Vec::new(),
            extra: serde_json::Map::new(),
        }
    }
}

impl LevelMetadata {
    pub fn from_editor_state(
        EditorStateParams {
            name,
            creator_metadata,
            music,
            spawn,
            sky_color,
            tap_times,
            timing_points,
            timeline_time_seconds,
            timeline_duration_seconds,
            menu_preview_camera,
            objects,
        }: EditorStateParams,
    ) -> Self {
        let mut metadata = Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            name,
            creator: creator_metadata.creator,
            description: creator_metadata.description,
            stars: creator_metadata.stars.clamp(0.0, 10.0),
            tags: creator_metadata.tags,
            version: creator_metadata.version,
            music,
            spawn,
            sky_color,
            tap_times,
            timing_points,
            timeline_time_seconds,
            timeline_duration_seconds,
            menu_preview_camera,
            objects,
            extra: serde_json::Map::new(),
        };
        metadata.normalize_object_ids();
        metadata
    }

    pub fn normalize_object_ids(&mut self) {
        self.format_version = CURRENT_LEVEL_FORMAT_VERSION;
        normalize_level_object_ids(&mut self.objects);
    }

    pub fn resolved_triggers(&self) -> Vec<TimedTrigger> {
        triggers_from_objects(&self.objects)
    }

    pub fn creator_metadata(&self) -> LevelCreatorMetadata {
        LevelCreatorMetadata {
            creator: self.creator.clone(),
            description: self.description.clone(),
            stars: self.stars.clamp(0.0, 10.0),
            tags: self.tags.clone(),
            version: self.version.clone(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct SpawnMetadata {
    #[serde(
        default = "default_spawn_position",
        skip_serializing_if = "is_default_spawn_position"
    )]
    pub position: [f32; 3],
    #[serde(default, skip_serializing_if = "is_default_direction")]
    pub direction: Direction,
}

impl Default for SpawnMetadata {
    fn default() -> Self {
        Self {
            position: default_spawn_position(),
            direction: Direction::Forward,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LevelObject {
    #[serde(
        default = "default_level_object_id",
        skip_serializing_if = "String::is_empty"
    )]
    pub id: ObjectId,
    #[serde(
        default = "default_level_object_position",
        skip_serializing_if = "is_default_level_object_position"
    )]
    pub position: [f32; 3],
    #[serde(
        default = "default_level_object_size",
        skip_serializing_if = "is_default_level_object_size"
    )]
    pub size: [f32; 3],
    #[serde(
        default = "default_block_rotation_degrees",
        skip_serializing_if = "is_default_block_rotation_degrees"
    )]
    pub rotation_degrees: [f32; 3],
    #[serde(
        default = "default_level_object_block_id",
        deserialize_with = "deserialize_level_object_block_id",
        skip_serializing_if = "is_default_level_object_block_id"
    )]
    pub block_id: String,
    #[serde(
        default = "default_level_object_color_tint",
        skip_serializing_if = "is_default_level_object_color_tint"
    )]
    pub color_tint: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<TimedTrigger>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelObjectTransformFieldVisibility {
    pub position: bool,
    pub rotation: bool,
    pub size: bool,
}

impl LevelObjectTransformFieldVisibility {
    pub const fn all() -> Self {
        Self {
            position: true,
            rotation: true,
            size: true,
        }
    }

    pub const fn none() -> Self {
        Self {
            position: false,
            rotation: false,
            size: false,
        }
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self {
            position: self.position && other.position,
            rotation: self.rotation && other.rotation,
            size: self.size && other.size,
        }
    }
}

impl LevelObject {
    pub fn with_generated_id(mut self, existing_objects: &[LevelObject]) -> Self {
        self.id = next_level_object_id(existing_objects);
        self
    }

    pub fn normalize_block_id(&mut self) {
        self.block_id = normalize_level_block_id(&self.block_id);
    }

    pub fn transform_field_visibility(&self) -> LevelObjectTransformFieldVisibility {
        match self.trigger.as_ref().map(|trigger| &trigger.action) {
            Some(TimedTriggerAction::CameraFollow { .. }) => {
                LevelObjectTransformFieldVisibility::none()
            }
            Some(TimedTriggerAction::CameraPose { .. }) => LevelObjectTransformFieldVisibility {
                position: true,
                rotation: true,
                size: false,
            },
            Some(TimedTriggerAction::TransformObjects { .. }) | None => {
                LevelObjectTransformFieldVisibility::all()
            }
        }
    }

    pub fn is_transform_trigger(&self) -> bool {
        if self.block_id == TRANSFORM_TRIGGER_BLOCK_ID {
            return true;
        }
        matches!(
            self.trigger.as_ref().map(|t| &t.action),
            Some(TimedTriggerAction::TransformObjects { .. })
        )
    }
}

impl Default for LevelObject {
    fn default() -> Self {
        Self {
            id: default_level_object_id(),
            position: default_level_object_position(),
            size: default_level_object_size(),
            rotation_degrees: default_block_rotation_degrees(),
            block_id: default_level_object_block_id(),
            color_tint: default_level_object_color_tint(),
            trigger: None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerLevelProgress {
    #[serde(default)]
    pub progress_percent: f32,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub gems_collected: u32,
    #[serde(default)]
    pub gems_max: u32,
}

impl PlayerLevelProgress {
    pub fn sanitized(self) -> Self {
        let progress_percent = if self.progress_percent.is_finite() {
            self.progress_percent.clamp(0.0, 100.0)
        } else {
            0.0
        };
        let gems_collected = self.gems_collected.min(self.gems_max);

        Self {
            progress_percent,
            completed: self.completed || progress_percent >= 100.0,
            gems_collected,
            gems_max: self.gems_max,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CameraTriggerMode {
    Follow,
    #[default]
    Static,
}

pub fn is_default_camera_trigger_mode(value: &CameraTriggerMode) -> bool {
    matches!(value, CameraTriggerMode::Static)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimedTriggerEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

pub fn is_default_camera_trigger_easing(value: &TimedTriggerEasing) -> bool {
    matches!(value, TimedTriggerEasing::Linear)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CameraTrigger {
    pub time_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_mode")]
    pub mode: CameraTriggerMode,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub easing: TimedTriggerEasing,
    #[serde(
        default = "default_camera_trigger_transition_interval_seconds",
        skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
    )]
    pub transition_interval_seconds: f32,
    #[serde(
        default = "default_camera_trigger_use_full_segment_transition",
        skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
    )]
    pub use_full_segment_transition: bool,
    #[serde(
        default = "default_camera_trigger_target_position",
        skip_serializing_if = "is_default_camera_trigger_target_position"
    )]
    pub target_position: [f32; 3],
    #[serde(
        default = "default_camera_trigger_distance",
        skip_serializing_if = "is_default_camera_trigger_distance"
    )]
    pub distance: f32,
    #[serde(
        default = "default_camera_trigger_rotation",
        skip_serializing_if = "is_default_camera_trigger_rotation"
    )]
    pub rotation: f32,
    #[serde(
        default = "default_camera_trigger_pitch",
        skip_serializing_if = "is_default_camera_trigger_pitch"
    )]
    pub pitch: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimedTriggerTarget {
    Camera,
    Objects {
        #[serde(deserialize_with = "deserialize_trigger_object_ids")]
        object_ids: Vec<ObjectId>,
    },
}

fn deserialize_trigger_object_ids<'de, D>(deserializer: D) -> Result<Vec<ObjectId>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|value| match value {
            serde_json::Value::String(value) => Ok(value),
            serde_json::Value::Number(value) => Ok(value.to_string()),
            other => Err(serde::de::Error::custom(format!(
                "invalid object id value {other}"
            ))),
        })
        .collect()
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimedTriggerAction {
    TransformObjects {
        position: [f32; 3],
        rotation_degrees: [f32; 3],
        size: [f32; 3],
        #[serde(
            default = "default_transform_simulate_hitboxes",
            skip_serializing_if = "is_default_transform_simulate_hitboxes"
        )]
        simulate_hitboxes: bool,
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
        #[serde(
            default = "default_camera_trigger_distance",
            skip_serializing_if = "is_default_camera_trigger_distance"
        )]
        distance: f32,
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
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TimedTrigger {
    pub time_seconds: f32,
    #[serde(
        default = "default_timed_trigger_duration_seconds",
        skip_serializing_if = "is_default_timed_trigger_duration_seconds"
    )]
    pub duration_seconds: f32,
    #[serde(default, skip_serializing_if = "is_default_camera_trigger_easing")]
    pub easing: TimedTriggerEasing,
    #[serde(
        default = "default_timed_trigger_target",
        skip_serializing_if = "is_default_timed_trigger_target"
    )]
    pub target: TimedTriggerTarget,
    pub action: TimedTriggerAction,
}

pub fn camera_trigger_offset(rotation: f32, pitch: f32) -> [f32; 3] {
    camera_trigger_offset_with_distance(rotation, pitch, CAMERA_TRIGGER_VIEW_DISTANCE)
}

pub fn camera_trigger_offset_with_distance(rotation: f32, pitch: f32, distance: f32) -> [f32; 3] {
    let horizontal_distance = distance * pitch.cos();
    [
        -rotation.sin() * horizontal_distance,
        pitch.sin() * distance,
        -rotation.cos() * horizontal_distance,
    ]
}

pub fn camera_trigger_eye_from_target(
    target_position: [f32; 3],
    rotation: f32,
    pitch: f32,
) -> [f32; 3] {
    let offset = camera_trigger_offset(rotation, pitch);
    [
        target_position[0] + offset[0],
        target_position[1] + offset[1],
        target_position[2] + offset[2],
    ]
}

pub fn camera_trigger_eye_from_target_with_distance(
    target_position: [f32; 3],
    rotation: f32,
    pitch: f32,
    distance: f32,
) -> [f32; 3] {
    let offset = camera_trigger_offset_with_distance(rotation, pitch, distance);
    [
        target_position[0] + offset[0],
        target_position[1] + offset[1],
        target_position[2] + offset[2],
    ]
}

pub fn camera_trigger_eye_from_object(object: &LevelObject) -> [f32; 3] {
    [
        object.position[0] + object.size[0] * 0.5,
        object.position[1] + object.size[1] * 0.5,
        object.position[2] + object.size[2] * 0.5,
    ]
}

pub fn camera_trigger_target_from_eye(
    eye_position: [f32; 3],
    rotation: f32,
    pitch: f32,
) -> [f32; 3] {
    let offset = camera_trigger_offset(rotation, pitch);
    [
        eye_position[0] - offset[0],
        eye_position[1] - offset[1],
        eye_position[2] - offset[2],
    ]
}

pub fn camera_trigger_forward(rotation: f32, pitch: f32) -> [f32; 3] {
    camera_trigger_forward_with_distance(rotation, pitch, CAMERA_TRIGGER_VIEW_DISTANCE)
}

pub fn camera_trigger_forward_with_distance(rotation: f32, pitch: f32, distance: f32) -> [f32; 3] {
    let distance = distance.max(0.001);
    let offset = camera_trigger_offset_with_distance(rotation, pitch, distance);
    [
        -offset[0] / distance,
        -offset[1] / distance,
        -offset[2] / distance,
    ]
}

pub fn camera_trigger_forward_from_rotation_degrees(rotation_degrees: [f32; 3]) -> [f32; 3] {
    let (rotation, pitch) = camera_trigger_rotation_pitch_from_rotation_degrees(rotation_degrees);
    camera_trigger_forward(rotation, pitch)
}

pub fn camera_trigger_rotation_pitch_from_rotation_degrees(
    rotation_degrees: [f32; 3],
) -> (f32, f32) {
    let mut pitch_degrees = normalize_degrees(rotation_degrees[0]);
    let mut rotation_degrees_y = normalize_degrees(rotation_degrees[1]);
    if pitch_degrees > 90.0 {
        pitch_degrees = 180.0 - pitch_degrees;
        rotation_degrees_y += 180.0;
    } else if pitch_degrees < -90.0 {
        pitch_degrees += 180.0;
        rotation_degrees_y += 180.0;
    }
    (
        normalize_degrees(rotation_degrees_y).to_radians(),
        pitch_degrees.to_radians(),
    )
}

fn normalize_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

pub fn apply_timed_triggers_to_objects(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    let mut objects = base_objects.to_vec();
    apply_timed_triggers_to_objects_in_place(&mut objects, triggers, time_seconds);
    objects
}

/// Returns `true` when the trigger is an object-transform trigger that moves
/// gameplay hitboxes during play (i.e. a `TransformObjects` trigger with
/// `simulate_hitboxes` enabled that targets objects).
pub fn trigger_simulates_hitboxes(trigger: &TimedTrigger) -> bool {
    !matches!(trigger.target, TimedTriggerTarget::Camera)
        && matches!(
            trigger.action,
            TimedTriggerAction::TransformObjects {
                simulate_hitboxes: true,
                ..
            }
        )
}

/// Returns `true` when any of the provided triggers moves gameplay hitboxes
/// during play.
pub fn has_simulate_hitboxes_triggers(triggers: &[TimedTrigger]) -> bool {
    triggers.iter().any(trigger_simulates_hitboxes)
}

/// Returns `true` when at least one simulate-hitboxes trigger is actively
/// interpolating its targets at `time_seconds` (progress strictly between 0
/// and 1). While this is true the transformed hitbox pose changes every step,
/// so callers cannot reuse a previously computed pose.
pub fn any_simulate_hitboxes_trigger_interpolating(
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> bool {
    triggers
        .iter()
        .filter(|trigger| trigger_simulates_hitboxes(trigger))
        .any(
            |trigger| match timed_trigger_progress(trigger, time_seconds) {
                Some(progress) => progress > 0.0 && progress < 1.0,
                None => false,
            },
        )
}

/// Returns a compact signature describing the discrete pose state of every
/// simulate-hitboxes trigger at `time_seconds` (before its window / fully
/// applied). Two equal signatures mean the transformed hitbox pose is
/// identical, as long as no trigger is currently interpolating (see
/// [`any_simulate_hitboxes_trigger_interpolating`]). Used to skip redundant
/// hitbox rebuilds during the stable stretches between trigger windows.
pub fn simulate_hitboxes_trigger_pose_signature(
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> u64 {
    let mut signature: u64 = 0;
    let mut shift: u32 = 0;
    for trigger in triggers
        .iter()
        .filter(|trigger| trigger_simulates_hitboxes(trigger))
    {
        let state: u64 = match timed_trigger_progress(trigger, time_seconds) {
            None => 0,
            Some(progress) if progress >= 1.0 => 2,
            Some(progress) if progress <= 0.0 => 0,
            Some(_) => 1,
        };
        signature |= state << shift;
        shift = shift.saturating_add(2);
    }
    signature
}

pub fn apply_simulate_hitboxes_triggers_to_objects(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    let mut objects = base_objects.to_vec();
    apply_simulate_hitboxes_triggers_to_objects_in_place(&mut objects, triggers, time_seconds);
    objects
}

pub fn apply_simulate_hitboxes_triggers_to_objects_into(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
    objects: &mut Vec<LevelObject>,
) {
    if objects.len() == base_objects.len() {
        objects.clone_from_slice(base_objects);
    } else {
        objects.clear();
        objects.extend_from_slice(base_objects);
    }
    apply_simulate_hitboxes_triggers_to_objects_in_place(objects, triggers, time_seconds);
}

fn apply_simulate_hitboxes_triggers_to_objects_in_place(
    objects: &mut [LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) {
    if objects.is_empty() || triggers.is_empty() {
        return;
    }

    let simulating: Vec<&TimedTrigger> = triggers
        .iter()
        .filter(|trigger| trigger_simulates_hitboxes(trigger))
        .collect();
    if simulating.is_empty() {
        return;
    }

    let object_index_by_id = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| !object.id.is_empty())
        .map(|(index, object)| (object.id.clone(), index))
        .collect::<HashMap<_, _>>();

    for trigger in simulating {
        if !trigger.time_seconds.is_finite() {
            continue;
        }

        let Some(progress) = timed_trigger_progress(trigger, time_seconds) else {
            continue;
        };
        let object_count = objects.len();

        if let TimedTriggerTarget::Objects { object_ids } = &trigger.target {
            let mut seen: Vec<ObjectId> = Vec::new();
            for object_id in object_ids.iter().cloned() {
                let Some(object_index) = object_index_by_id.get(&object_id).copied() else {
                    continue;
                };
                if object_index >= object_count || seen.contains(&object_id) {
                    continue;
                }
                seen.push(object_id);
                apply_trigger_action_to_object(
                    &mut objects[object_index],
                    &trigger.action,
                    progress,
                );
            }
        }
    }
}

pub fn apply_timed_triggers_to_objects_into(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
    objects: &mut Vec<LevelObject>,
) {
    if objects.len() == base_objects.len() {
        objects.clone_from_slice(base_objects);
    } else {
        objects.clear();
        objects.extend_from_slice(base_objects);
    }
    apply_timed_triggers_to_objects_in_place(objects, triggers, time_seconds);
}

pub fn next_level_object_id(objects: &[LevelObject]) -> ObjectId {
    let mut existing = HashSet::new();
    for object in objects {
        if object.id.is_empty() {
            continue;
        }
        existing.insert(object.id.as_str());
    }

    let mut next_suffix = next_level_object_suffix_after(objects.iter().map(|object| &object.id));
    allocate_next_level_object_id(&existing, &mut next_suffix)
}

pub fn normalize_level_object_ids(objects: &mut [LevelObject]) {
    let original_ids = objects
        .iter()
        .map(|object| object.id.clone())
        .collect::<Vec<_>>();
    let legacy_compact_object_indices = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.trigger.is_none())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    let mut used_ids = HashSet::<ObjectId>::new();
    let mut remapped_ids = HashMap::<ObjectId, ObjectId>::new();
    let mut next_suffix = next_level_object_suffix_after(original_ids.iter());
    for index in 0..objects.len() {
        let original_id = original_ids[index].clone();
        let needs_fresh_id = original_id.is_empty() || used_ids.contains(&original_id);
        if needs_fresh_id {
            let next_id = allocate_next_level_object_id(&used_ids, &mut next_suffix);
            objects[index].id = next_id.clone();
            if !original_id.is_empty() {
                remapped_ids
                    .entry(original_id)
                    .or_insert_with(|| next_id.clone());
            }
            used_ids.insert(next_id);
        } else {
            used_ids.insert(original_id);
        }
    }

    let current_ids = objects
        .iter()
        .map(|object| object.id.clone())
        .collect::<HashSet<_>>();
    let legacy_compact_ids = legacy_compact_object_indices
        .iter()
        .map(|index| objects[*index].id.clone())
        .collect::<Vec<_>>();

    for object in objects {
        let Some(trigger) = object.trigger.as_mut() else {
            continue;
        };
        let TimedTriggerTarget::Objects { object_ids } = &mut trigger.target else {
            continue;
        };
        let mut normalized_ids = object_ids
            .iter()
            .filter_map(|object_id| {
                remapped_ids
                    .get(object_id)
                    .cloned()
                    .or_else(|| legacy_object_id_to_current_id(object_id, &legacy_compact_ids))
                    .or_else(|| current_ids.contains(object_id).then(|| object_id.clone()))
            })
            .collect::<Vec<_>>();
        normalized_ids.sort_unstable();
        normalized_ids.dedup();
        *object_ids = normalized_ids;
    }
}

fn next_level_object_suffix_after<'a>(object_ids: impl Iterator<Item = &'a ObjectId>) -> u64 {
    let mut max_suffix = 0_u64;
    for object_id in object_ids {
        if let Some(suffix) = object_id.strip_prefix(OBJECT_ID_PREFIX) {
            if let Ok(value) = suffix.parse::<u64>() {
                max_suffix = max_suffix.max(value);
            }
        }
    }

    max_suffix.saturating_add(1)
}

fn allocate_next_level_object_id<T>(used_ids: &HashSet<T>, next_suffix: &mut u64) -> ObjectId
where
    T: std::borrow::Borrow<str> + Eq + std::hash::Hash,
{
    loop {
        let candidate = format!("{OBJECT_ID_PREFIX}{:06}", *next_suffix);
        *next_suffix = next_suffix.checked_add(1).unwrap_or(1);
        if !used_ids.contains(candidate.as_str()) {
            return candidate;
        }
    }
}

fn legacy_object_id_to_current_id(
    object_id: &ObjectId,
    legacy_compact_ids: &[ObjectId],
) -> Option<ObjectId> {
    let index = object_id.parse::<usize>().ok()?;
    legacy_compact_ids
        .get(index)
        .filter(|id| !id.is_empty())
        .cloned()
}

pub fn camera_triggers_to_timed_triggers(camera_triggers: &[CameraTrigger]) -> Vec<TimedTrigger> {
    let mut triggers = Vec::with_capacity(camera_triggers.len());
    for camera_trigger in camera_triggers {
        let action = match camera_trigger.mode {
            CameraTriggerMode::Follow => TimedTriggerAction::CameraFollow {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
                distance: camera_trigger.distance,
                rotation: camera_trigger.rotation,
                pitch: camera_trigger.pitch,
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

pub fn timed_triggers_to_camera_triggers(triggers: &[TimedTrigger]) -> Vec<CameraTrigger> {
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
                    distance: default_camera_trigger_distance(),
                    rotation,
                    pitch,
                });
            }
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds,
                use_full_segment_transition,
                distance,
                rotation,
                pitch,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraTriggerMode::Follow,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: default_camera_trigger_target_position(),
                    distance,
                    rotation,
                    pitch,
                });
            }
            TimedTriggerAction::TransformObjects { .. } => {}
        }
    }

    camera_triggers.retain(|trigger| trigger.time_seconds.is_finite());
    camera_triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    camera_triggers
}

fn apply_timed_triggers_to_objects_in_place(
    objects: &mut [LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) {
    if objects.is_empty() || triggers.is_empty() {
        return;
    }

    let object_index_by_id = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| !object.id.is_empty())
        .map(|(index, object)| (object.id.clone(), index))
        .collect::<HashMap<_, _>>();

    for trigger in triggers {
        if !trigger.time_seconds.is_finite() {
            continue;
        }

        let Some(progress) = timed_trigger_progress(trigger, time_seconds) else {
            continue;
        };
        let object_count = objects.len();

        match &trigger.target {
            TimedTriggerTarget::Camera => {}
            TimedTriggerTarget::Objects { object_ids } => {
                let mut seen: Vec<&ObjectId> = Vec::new();
                for object_id in object_ids {
                    let Some(object_index) = object_index_by_id.get(object_id).copied() else {
                        continue;
                    };
                    if object_index >= object_count || seen.contains(&object_id) {
                        continue;
                    }
                    seen.push(object_id);
                    apply_trigger_action_to_object(
                        &mut objects[object_index],
                        &trigger.action,
                        progress,
                    );
                }
            }
        }
    }
}

fn apply_trigger_action_to_object(
    object: &mut LevelObject,
    action: &TimedTriggerAction,
    progress: f32,
) {
    match action {
        TimedTriggerAction::TransformObjects {
            position,
            rotation_degrees,
            size,
            ..
        } => {
            for (current, target) in object.position.iter_mut().zip(position.iter()) {
                *current += (*target - *current) * progress;
            }
            for (current, target) in object
                .rotation_degrees
                .iter_mut()
                .zip(rotation_degrees.iter())
            {
                *current += (*target - *current) * progress;
            }
            for (current, target) in object.size.iter_mut().zip(size.iter()) {
                let current_value = (*current).max(0.01);
                let target_value = (*target).max(0.01);
                *current = current_value + (target_value - current_value) * progress;
            }
        }
        TimedTriggerAction::CameraPose { .. } | TimedTriggerAction::CameraFollow { .. } => {}
    }
}

pub fn triggers_from_objects(objects: &[LevelObject]) -> Vec<TimedTrigger> {
    let mut triggers = objects
        .iter()
        .filter_map(|object| {
            let mut trigger = object.trigger.clone()?;
            trigger.action = match trigger.action {
                TimedTriggerAction::TransformObjects {
                    position: _,
                    rotation_degrees: _,
                    size: _,
                    simulate_hitboxes,
                } => TimedTriggerAction::TransformObjects {
                    position: object.position,
                    rotation_degrees: object.rotation_degrees,
                    size: object.size,
                    simulate_hitboxes,
                },
                TimedTriggerAction::CameraPose {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: _,
                    rotation: _,
                    pitch: _,
                } => {
                    let (rotation, pitch) = camera_trigger_rotation_pitch_from_rotation_degrees(
                        object.rotation_degrees,
                    );
                    TimedTriggerAction::CameraPose {
                        transition_interval_seconds,
                        use_full_segment_transition,
                        target_position: camera_trigger_target_from_eye(
                            camera_trigger_eye_from_object(object),
                            rotation,
                            pitch,
                        ),
                        rotation,
                        pitch,
                    }
                }
                TimedTriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    distance,
                    rotation,
                    pitch,
                } => TimedTriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    distance,
                    rotation,
                    pitch,
                },
            };
            Some(trigger)
        })
        .filter(|trigger| trigger.time_seconds.is_finite())
        .collect::<Vec<_>>();
    triggers.sort_by(|left, right| f32::total_cmp(&left.time_seconds, &right.time_seconds));
    triggers
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_level_object_ids_maps_legacy_numeric_targets_to_generated_ids() {
        let mut objects = vec![
            LevelObject::default(),
            LevelObject {
                trigger: Some(TimedTrigger {
                    time_seconds: 1.0,
                    duration_seconds: 1.0,
                    easing: TimedTriggerEasing::Linear,
                    target: TimedTriggerTarget::Objects {
                        object_ids: vec!["0".to_string()],
                    },
                    action: TimedTriggerAction::TransformObjects {
                        position: [4.0, 0.0, 0.0],
                        rotation_degrees: [0.0, 0.0, 0.0],
                        size: [1.0, 1.0, 1.0],
                        simulate_hitboxes: false,
                    },
                }),
                ..LevelObject::default()
            },
        ];

        normalize_level_object_ids(&mut objects);

        assert_eq!(objects[0].id, "obj_000001");
        let Some(trigger) = objects[1].trigger.as_ref() else {
            panic!("expected trigger payload");
        };
        let TimedTriggerTarget::Objects { object_ids } = &trigger.target else {
            panic!("expected object trigger target");
        };
        assert_eq!(object_ids, &["obj_000001"]);
    }
}
