use serde::{Deserialize, Serialize};

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

fn default_timeline_step() -> u32 {
    0
}

fn is_default_timeline_step(value: &u32) -> bool {
    *value == 0
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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
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

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Clone)]
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
    pub(crate) taps: Vec<u32>,
    #[serde(
        default = "default_timeline_step",
        skip_serializing_if = "is_default_timeline_step"
    )]
    pub(crate) timeline_step: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) objects: Vec<LevelObject>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

impl LevelMetadata {
    pub(crate) fn from_editor_state(
        name: String,
        music: MusicMetadata,
        spawn: SpawnMetadata,
        taps: Vec<u32>,
        timeline_step: u32,
        objects: Vec<LevelObject>,
    ) -> Self {
        Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            name,
            music,
            spawn,
            taps,
            timeline_step,
            objects,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
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

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    #[default]
    Standard,
    Grass,
    Dirt,
    Void,
    SpeedPortal,
}

fn is_default_block_kind(value: &BlockKind) -> bool {
    *value == BlockKind::Standard
}

#[derive(Deserialize, Serialize, Clone)]
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
    #[serde(default, skip_serializing_if = "is_default_block_kind")]
    pub(crate) kind: BlockKind,
}

#[derive(PartialEq)]
pub(crate) enum AppPhase {
    Menu,
    Playing,
    Editor,
    GameOver,
}

pub(crate) struct MenuState {
    pub(crate) selected_level: usize,
    pub(crate) levels: Vec<String>,
}

pub(crate) struct EditorState {
    pub(crate) cursor: [i32; 3],
    pub(crate) bounds: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum EditorMode {
    Select,
    #[default]
    Place,
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self {
            cursor: [0, 0, 0],
            bounds: 55,
        }
    }
}

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
pub(crate) enum Direction {
    Forward,
    Right,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LineUniform {
    pub(crate) offset: [f32; 2],
    pub(crate) rotation: f32,
    pub(crate) _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ColorSpaceUniform {
    pub(crate) apply_gamma_correction: f32,
    pub(crate) _pad: [f32; 3],
}

#[cfg(test)]
mod tests {
    use super::{
        BlockKind, LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata,
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
        assert_eq!(object.rotation_degrees, 0.0);
        assert_eq!(object.roundness, 0.18);
        assert!(matches!(object.kind, BlockKind::Standard));
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
            kind: BlockKind::Grass,
        };

        let value = serde_json::to_value(&object).expect("serialize object");
        let expected = json!({
            "position": [1.0, 2.0, 3.0],
            "size": [4.0, 5.0, 6.0],
            "kind": "grass"
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
            0,
            vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                kind: BlockKind::Standard,
            }],
        );

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value["name"], "Minimal");
        assert!(value.get("format_version").is_none());
        assert!(value.get("music").is_none());
        assert!(value.get("spawn").is_none());
        assert!(value.get("taps").is_none());
        assert!(value.get("timeline_step").is_none());
        assert_eq!(value["objects"].as_array().map(|v| v.len()), Some(1));
        let object = &value["objects"][0];
        assert!(object.get("position").is_none());
        assert!(object.get("size").is_none());
        assert!(object.get("rotation_degrees").is_none());
        assert!(object.get("roundness").is_none());
        assert!(object.get("kind").is_none());
    }
}
