/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use glam::{EulerRot, Mat3, Vec3};
use is_default::IsDefault;
use serde::Deserializer;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::collections::{HashMap, HashSet};

use crate::block_id::{normalize_block_id, DEFAULT_BLOCK_ID};

pub const CAMERA_TRIGGER_BLOCK_ID: &str = "core/camera_trigger";
pub const TRANSFORM_TRIGGER_BLOCK_ID: &str = "core/transform_trigger";
pub const DEFAULT_CAMERA_YAW: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_CAMERA_PITCH: f32 = std::f32::consts::FRAC_PI_4;
pub const DEFAULT_CAMERA_DISTANCE: f32 = 24.0;
pub const CURRENT_LEVEL_FORMAT_VERSION: u32 = 3;

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
serde_default!(default_level_name: String = "Untitled".to_string());
serde_default_array!(default_spawn_position: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_preview_camera_position_zero: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_preview_camera_target_zero: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_float!(default_timeline_duration_seconds: f32 = 16.0);
serde_default!(default_transform_simulate_hitboxes: bool = false);
serde_default_array!(default_block_rotation_degrees: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_level_object_position: [f32; 3] = [0.0, 0.0, 0.0]);
serde_default_array!(default_level_object_size: [f32; 3] = [1.0, 1.0, 1.0]);
serde_default!(default_level_object_block_id: String = DEFAULT_BLOCK_ID.to_string());
serde_default_array!(default_level_object_color_tint: [f32; 3] = [1.0, 1.0, 1.0]);
serde_default!(default_level_object_id: String = String::new());
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
    DEFAULT_CAMERA_YAW
}

pub fn is_default_camera_trigger_rotation(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_YAW).abs() <= 1e-6
}

pub fn default_camera_trigger_pitch() -> f32 {
    DEFAULT_CAMERA_PITCH
}

pub fn is_default_camera_trigger_pitch(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_PITCH).abs() <= 1e-6
}

pub fn default_camera_trigger_distance() -> f32 {
    DEFAULT_CAMERA_DISTANCE
}

pub fn is_default_camera_trigger_distance(value: &f32) -> bool {
    (*value - DEFAULT_CAMERA_DISTANCE).abs() <= 1e-6
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

pub fn default_timed_trigger_target() -> TriggerTarget {
    TriggerTarget::Camera
}

pub fn is_default_timed_trigger_target(value: &TriggerTarget) -> bool {
    matches!(value, TriggerTarget::Camera)
}

/// Direction the player (snake) is facing.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Facing the positive Z axis.
    #[default]
    Forward,
    /// Facing the positive X axis.
    Right,
}

pub fn deserialize_level_object_block_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Ok(normalize_block_id(&raw))
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default, IsDefault)]
pub struct MusicMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

/// Metadata that can be supplied by the level creator and should be:
/// - easily changed
/// - gameplay-irrelevant
/// - performance-insensitive
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BasicLevelMetadata {
    /// The name of the level, supplied by the creator.
    #[serde(
        default = "default_level_name",
        skip_serializing_if = "is_default_level_name"
    )]
    pub name: String,
    /// The username of level creator, None if the level is built-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// A description of the level, supplied by the creator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Difficulty of the level, from 0.0 to 10.0, supplied by the creator.
    /// TODO: subject to change.
    #[serde(
        default = "default_level_stars",
        skip_serializing_if = "is_default_level_stars"
    )]
    pub stars: f32,
    /// Tags for the level, supplied by the creator. Can be used for categorization, search, or filtering.
    #[serde(
        default = "default_level_tags",
        skip_serializing_if = "is_default_level_tags"
    )]
    pub tags: Vec<String>,
}

impl IsDefault for BasicLevelMetadata {
    fn is_default(&self) -> bool {
        is_default_level_name(&self.name)
            && self.creator.is_none()
            && self.description.is_none()
            && is_default_level_stars(&self.stars)
            && is_default_level_tags(&self.tags)
    }
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
    pub basic: BasicLevelMetadata,
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
    /// Version number for the level format. This is used to ensure compatibility between different versions of the game. Older format versions are always loadable by newer game versions, but newer format versions may not be loadable by older game versions.
    #[serde(
        default = "default_level_format_version",
        skip_serializing_if = "is_default_level_format_version"
    )]
    pub format_version: u32,
    #[serde(
        default = "Default::default",
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub basic: BasicLevelMetadata,
    /// Metadata about the music used in the level, including source, title, and author.
    #[serde(
        default = "Default::default",
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub music: MusicMetadata,
    /// Metadata about the spawn point of the level, including position and direction.
    #[serde(
        default = "Default::default",
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub spawn: SpawnMetadata,
    /// Color of the sky in the level, as RGB values from 0.0 to 1.0.
    /// TODO: will be controllable by triggers in the future.
    #[serde(
        default = "default_sky_color",
        skip_serializing_if = "is_default_sky_color"
    )]
    pub sky_color: [f32; 3],
    /// Times in seconds when the player is expected to tap, used for gameplay simulation. This is only used in the editor right now, but may be visualized in practice mode in the future.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tap_times: Vec<f32>,
    /// Timing points in the level that define the tempo and time signature of the music. This is only used in the editor to help time taps and triggers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timing_points: Vec<TimingPoint>,
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
            basic: BasicLevelMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: default_sky_color(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
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
            basic,
            music,
            spawn,
            sky_color,
            tap_times,
            timing_points,
            timeline_time_seconds: _,
            timeline_duration_seconds,
            menu_preview_camera,
            objects,
        }: EditorStateParams,
    ) -> Self {
        let mut metadata = Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            basic,
            music,
            spawn,
            sky_color,
            tap_times,
            timing_points,
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
}

/// How the spawn point is configured in the level.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, SmartDefault)]
pub struct SpawnMetadata {
    /// The position of the spawn point in world coordinates.
    #[default(default_spawn_position())]
    #[serde(
        default = "default_spawn_position",
        skip_serializing_if = "is_default_spawn_position"
    )]
    pub position: [f32; 3],
    /// The direction the player is facing at the spawn point.
    #[serde(default, skip_serializing_if = "IsDefault::is_default")]
    pub direction: Direction,
}

impl IsDefault for SpawnMetadata {
    fn is_default(&self) -> bool {
        is_default_spawn_position(&self.position) && Direction::is_default(&self.direction)
    }
}

/// An object placed in a level.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LevelObject {
    /// Unique identifier for this object, used for referencing in triggers. Must be unique within a level, but can be any string (e.g. doesn't have to be a number or follow any specific format).
    #[serde(
        default = "default_level_object_id",
        skip_serializing_if = "String::is_empty"
    )]
    pub id: String,
    /// Position of the center of the object in level coordinates.
    #[serde(
        default = "default_level_object_position",
        skip_serializing_if = "is_default_level_object_position"
    )]
    pub position: [f32; 3],
    /// Absolute size of the object.
    #[serde(
        default = "default_level_object_size",
        skip_serializing_if = "is_default_level_object_size"
    )]
    pub size: [f32; 3],
    /// Rotation of the object in degrees, applied as intrinsic XYZ = extrinsic ZYX, pivoted at the object center.
    /// X is pitch, Y is yaw, Z is roll.
    #[serde(
        default = "default_block_rotation_degrees",
        skip_serializing_if = "is_default_block_rotation_degrees"
    )]
    pub rotation_degrees: [f32; 3],
    /// ID of the block to use for this object, which determines its appearance and behavior. Can be a custom block ID or a core block ID. For example, "core/stone" or "custom/my_block".
    #[serde(
        default = "default_level_object_block_id",
        deserialize_with = "deserialize_level_object_block_id",
        skip_serializing_if = "is_default_level_object_block_id"
    )]
    pub block_id: String,
    /// Color tint to apply to the object, as RGB values from 0.0 to 1.0. This is multiplied with the block's colors, so it can be used to create variations of the same block without needing multiple block definitions. For example, a color tint of [1.0, 0.5, 0.5] would make the object appear redder, while [0.5, 1.0, 0.5] would make it appear greener.
    #[serde(
        default = "default_level_object_color_tint",
        skip_serializing_if = "is_default_level_object_color_tint"
    )]
    pub color_tint: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<TimedTrigger>,
}

impl LevelObject {
    /// Dead-center position of the object, used as the pivot point for rotation.
    pub fn center(&self) -> Vec3 {
        Vec3::from_array(self.position) + Vec3::from_array(self.size) * 0.5
    }

    /// Rotation radians matrix for the object, based on its rotation_degrees.
    pub fn rotation(&self) -> Mat3 {
        Mat3::from_euler(
            EulerRot::XYZ,
            self.rotation_degrees[0].to_radians(),
            self.rotation_degrees[1].to_radians(),
            self.rotation_degrees[2].to_radians(),
        )
    }
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
        self.block_id = normalize_block_id(&self.block_id);
    }

    pub fn transform_field_visibility(&self) -> LevelObjectTransformFieldVisibility {
        match self.trigger.as_ref().map(|trigger| &trigger.action) {
            Some(TriggerAction::CameraFollow { .. }) => LevelObjectTransformFieldVisibility::none(),
            Some(TriggerAction::CameraPose { .. }) => LevelObjectTransformFieldVisibility {
                position: true,
                rotation: true,
                size: false,
            },
            Some(TriggerAction::TransformObjects { .. }) | None => {
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
            Some(TriggerAction::TransformObjects { .. })
        )
    }
}

impl Default for LevelObject {
    /// Default object is a non-trigger unit block at the origin, with no ID and no color tint.
    /// This is guaranteed to be position (0,0,0), size (1,1,1), rotation (0,0,0).
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

/// Mode for how the camera behaves.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "lowercase")]
pub enum CameraMode {
    /// Camera follows the target, maintaining a fixed distance and orientation relative to it.
    Follow,
    #[default]
    /// Camera remains static at the target position, ignoring the target's movement.
    Static,
}

/// Direction that an easing function is applied.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default, IsDefault)]
#[serde(rename_all = "snake_case")]
pub enum EasingDirection {
    /// Linear easing, no acceleration or deceleration.
    #[default]
    Linear,
    /// Ease in, starts slow and accelerates.
    EaseIn,
    /// Ease out, starts fast and decelerates.
    EaseOut,
    /// Ease in and out, starts slow, accelerates, then decelerates.
    EaseInOut,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CameraTrigger {
    /// Time in seconds when the trigger is activated.
    pub time_seconds: f32,
    /// [`CameraMode`] for how the camera behaves during this trigger.
    #[serde(default, skip_serializing_if = "IsDefault::is_default")]
    pub mode: CameraMode,
    /// [`EasingDirection`] for how the camera transition is eased.
    #[serde(default, skip_serializing_if = "IsDefault::is_default")]
    pub easing: EasingDirection,
    /// Time in seconds for how long the camera transition takes. If 0, the transition is instant.
    #[serde(
        default = "default_camera_trigger_transition_interval_seconds",
        skip_serializing_if = "is_default_camera_trigger_transition_interval_seconds"
    )]
    pub transition_interval_seconds: f32,
    /// Decides whether the transition into a trigger spans the entire segment leading up to it, or just the `transition_interval_seconds` right before `time_seconds` when it fires.
    #[serde(
        default = "default_camera_trigger_use_full_segment_transition",
        skip_serializing_if = "is_default_camera_trigger_use_full_segment_transition"
    )]
    pub use_full_segment_transition: bool,
    /// Look-at position for the Static camera.
    #[serde(
        default = "default_camera_trigger_target_position",
        skip_serializing_if = "is_default_camera_trigger_target_position"
    )]
    pub target_position: [f32; 3],
    /// Straight-line distance from the Follow camera to the target position.
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
pub enum TriggerTarget {
    /// The player's camera.
    Camera,
    /// A list of level objects.
    Objects {
        #[serde(deserialize_with = "deserialize_trigger_object_ids")]
        object_ids: Vec<String>,
    },
}

fn deserialize_trigger_object_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
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
pub enum TriggerAction {
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
    #[serde(default, skip_serializing_if = "IsDefault::is_default")]
    pub easing: EasingDirection,
    #[serde(
        default = "default_timed_trigger_target",
        skip_serializing_if = "is_default_timed_trigger_target"
    )]
    pub target: TriggerTarget,
    pub action: TriggerAction,
}

/// The vector from the look-at target to the specified eye.
/// Roll is not considered, so the camera is always upright.
pub fn camera_offset_from_target(yaw: f32, pitch: f32, distance: f32) -> [f32; 3] {
    let horizontal_distance = distance * pitch.cos();
    [
        -yaw.sin() * horizontal_distance,
        pitch.sin() * distance,
        -yaw.cos() * horizontal_distance,
    ]
}

/// The camera eye position given a look-at target, camera yaw, pitch, and distance.
pub fn camera_eye_from_target(
    target_position: [f32; 3],
    yaw: f32,
    pitch: f32,
    distance: f32,
) -> [f32; 3] {
    let offset = camera_offset_from_target(yaw, pitch, distance);
    [
        target_position[0] + offset[0],
        target_position[1] + offset[1],
        target_position[2] + offset[2],
    ]
}

pub fn camera_trigger_eye_from_object(object: &LevelObject) -> [f32; 3] {
    object.center().into()
}

pub fn camera_trigger_target_from_eye(
    eye_position: [f32; 3],
    rotation: f32,
    pitch: f32,
) -> [f32; 3] {
    let offset = camera_offset_from_target(rotation, pitch, DEFAULT_CAMERA_DISTANCE);
    [
        eye_position[0] - offset[0],
        eye_position[1] - offset[1],
        eye_position[2] - offset[2],
    ]
}

pub fn camera_trigger_forward(rotation: f32, pitch: f32) -> [f32; 3] {
    camera_trigger_forward_with_distance(rotation, pitch, DEFAULT_CAMERA_DISTANCE)
}

pub fn camera_trigger_forward_with_distance(rotation: f32, pitch: f32, distance: f32) -> [f32; 3] {
    let distance = distance.max(0.001);
    let offset = camera_offset_from_target(rotation, pitch, distance);
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
    !matches!(trigger.target, TriggerTarget::Camera)
        && matches!(
            trigger.action,
            TriggerAction::TransformObjects {
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

        if let TriggerTarget::Objects { object_ids } = &trigger.target {
            let mut seen: Vec<String> = Vec::new();
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

pub fn next_level_object_id(objects: &[LevelObject]) -> String {
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

    let mut used_ids = HashSet::<String>::new();
    let mut remapped_ids = HashMap::<String, String>::new();
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
        let TriggerTarget::Objects { object_ids } = &mut trigger.target else {
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

fn next_level_object_suffix_after<'a>(object_ids: impl Iterator<Item = &'a String>) -> u64 {
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

fn allocate_next_level_object_id<T>(used_ids: &HashSet<T>, next_suffix: &mut u64) -> String
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
    object_id: &String,
    legacy_compact_ids: &[String],
) -> Option<String> {
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
            CameraMode::Follow => TriggerAction::CameraFollow {
                transition_interval_seconds: camera_trigger.transition_interval_seconds,
                use_full_segment_transition: camera_trigger.use_full_segment_transition,
                distance: camera_trigger.distance,
                rotation: camera_trigger.rotation,
                pitch: camera_trigger.pitch,
            },
            CameraMode::Static => TriggerAction::CameraPose {
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
            target: TriggerTarget::Camera,
            action,
        });
    }

    triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    triggers
}

pub fn timed_triggers_to_camera_triggers(triggers: &[TimedTrigger]) -> Vec<CameraTrigger> {
    let mut camera_triggers = Vec::new();

    for trigger in triggers {
        if !matches!(trigger.target, TriggerTarget::Camera) {
            continue;
        }

        match trigger.action {
            TriggerAction::CameraPose {
                transition_interval_seconds,
                use_full_segment_transition,
                target_position,
                rotation,
                pitch,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraMode::Static,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position,
                    distance: default_camera_trigger_distance(),
                    rotation,
                    pitch,
                });
            }
            TriggerAction::CameraFollow {
                transition_interval_seconds,
                use_full_segment_transition,
                distance,
                rotation,
                pitch,
            } => {
                camera_triggers.push(CameraTrigger {
                    time_seconds: trigger.time_seconds,
                    mode: CameraMode::Follow,
                    easing: trigger.easing,
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: default_camera_trigger_target_position(),
                    distance,
                    rotation,
                    pitch,
                });
            }
            TriggerAction::TransformObjects { .. } => {}
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
            TriggerTarget::Camera => {}
            TriggerTarget::Objects { object_ids } => {
                let mut seen: Vec<&String> = Vec::new();
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

fn apply_trigger_action_to_object(object: &mut LevelObject, action: &TriggerAction, progress: f32) {
    match action {
        TriggerAction::TransformObjects {
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
        TriggerAction::CameraPose { .. } | TriggerAction::CameraFollow { .. } => {}
    }
}

pub fn triggers_from_objects(objects: &[LevelObject]) -> Vec<TimedTrigger> {
    let mut triggers = objects
        .iter()
        .filter_map(|object| {
            let mut trigger = object.trigger.clone()?;
            trigger.action = match trigger.action {
                TriggerAction::TransformObjects {
                    position: _,
                    rotation_degrees: _,
                    size: _,
                    simulate_hitboxes,
                } => TriggerAction::TransformObjects {
                    position: object.position,
                    rotation_degrees: object.rotation_degrees,
                    size: object.size,
                    simulate_hitboxes,
                },
                TriggerAction::CameraPose {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    target_position: _,
                    rotation: _,
                    pitch: _,
                } => {
                    let (rotation, pitch) = camera_trigger_rotation_pitch_from_rotation_degrees(
                        object.rotation_degrees,
                    );
                    TriggerAction::CameraPose {
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
                TriggerAction::CameraFollow {
                    transition_interval_seconds,
                    use_full_segment_transition,
                    distance,
                    rotation,
                    pitch,
                } => TriggerAction::CameraFollow {
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

fn timed_trigger_eased_alpha(easing: EasingDirection, alpha: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    match easing {
        EasingDirection::Linear => alpha,
        EasingDirection::EaseIn => alpha * alpha,
        EasingDirection::EaseOut => 1.0 - (1.0 - alpha) * (1.0 - alpha),
        EasingDirection::EaseInOut => {
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
                    easing: EasingDirection::Linear,
                    target: TriggerTarget::Objects {
                        object_ids: vec!["0".to_string()],
                    },
                    action: TriggerAction::TransformObjects {
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
        let TriggerTarget::Objects { object_ids } = &trigger.target else {
            panic!("expected object trigger target");
        };
        assert_eq!(object_ids, &["obj_000001"]);
    }
}
