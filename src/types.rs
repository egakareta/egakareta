use serde::Deserializer;
use serde::{Deserialize, Serialize};

use crate::block_repository::{normalize_block_id, DEFAULT_BLOCK_ID};

pub(crate) const CURRENT_LEVEL_FORMAT_VERSION: u32 = 1;

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

fn default_block_rotation_degrees() -> f32 {
    0.0
}

fn default_block_roundness() -> f32 {
    0.18
}

fn is_default_block_rotation_degrees(value: &f32) -> bool {
    value.abs() <= 1e-6
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
    #[serde(default, rename = "taps", skip_serializing)]
    pub(crate) legacy_taps: Vec<u32>,
    #[serde(default, rename = "timeline_step", skip_serializing)]
    pub(crate) legacy_timeline_step: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) objects: Vec<LevelObject>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

impl LevelMetadata {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_editor_state(
        name: String,
        music: MusicMetadata,
        spawn: SpawnMetadata,
        tap_times: Vec<f32>,
        timing_points: Vec<TimingPoint>,
        timeline_time_seconds: f32,
        timeline_duration_seconds: f32,
        objects: Vec<LevelObject>,
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
            legacy_taps: Vec::new(),
            legacy_timeline_step: 0,
            objects,
            extra: serde_json::Map::new(),
        }
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
    pub(crate) rotation_degrees: f32,
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
}

impl LevelObject {
    pub(crate) fn normalize_block_id(&mut self) {
        self.block_id = normalize_block_id(&self.block_id);
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
    pub(crate) bounds: i32,
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
    #[default]
    Place,
    Timing,
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self {
            cursor: [0.0, 0.0, 0.0],
            bounds: 55,
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
    use super::{LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata};
    use serde_json::json;

    #[test]
    fn level_object_rotation_defaults_when_missing() {
        let json = r#"{
            "position":[1.0,2.0,3.0],
            "size":[4.0,5.0,6.0],
            "kind":"standard"
        }"#;

        let object: LevelObject = serde_json::from_str(json).expect("valid level object");
        assert_eq!(object.rotation_degrees, 0.0);
        assert_eq!(object.roundness, 0.18);
        assert_eq!(object.block_id, "core/standard");
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
        assert_eq!(metadata.objects[0].rotation_degrees, 0.0);
        assert_eq!(metadata.objects[0].roundness, 0.18);
        assert!(matches!(metadata.spawn.direction, SpawnDirection::Forward));
    }

    #[test]
    fn level_object_serialization_omits_default_rotation_and_roundness() {
        let object = LevelObject {
            position: [1.0, 2.0, 3.0],
            size: [4.0, 5.0, 6.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/grass".to_string(),
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
        let metadata = LevelMetadata::from_editor_state(
            "Minimal".to_string(),
            MusicMetadata::default(),
            SpawnMetadata::default(),
            Vec::new(),
            Vec::new(),
            0.0,
            16.0,
            vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
            }],
        );

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value["name"], "Minimal");
        assert!(value.get("format_version").is_none());
        assert!(value.get("music").is_none());
        assert!(value.get("spawn").is_none());
        assert!(value.get("tap_times").is_none());
        assert!(value.get("timeline_time_seconds").is_none());
        assert!(value.get("timeline_duration_seconds").is_none());
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
}
