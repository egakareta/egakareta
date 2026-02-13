use serde::{Deserialize, Deserializer, Serialize};

pub(crate) const CURRENT_LEVEL_FORMAT_VERSION: u32 = 1;

fn default_level_format_version() -> u32 {
    CURRENT_LEVEL_FORMAT_VERSION
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 3],
    pub(crate) color: [f32; 3],
}

impl Vertex {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct MusicMetadata {
    pub(crate) source: String,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct LevelMetadata {
    #[serde(default = "default_level_format_version")]
    pub(crate) format_version: u32,
    pub(crate) name: String,
    pub(crate) music: MusicMetadata,
    #[serde(default)]
    pub(crate) spawn: SpawnMetadata,
    #[serde(default)]
    pub(crate) taps: Vec<u32>,
    #[serde(default)]
    pub(crate) timeline_step: u32,
    pub(crate) objects: Vec<LevelObject>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

impl LevelMetadata {
    pub(crate) fn from_editor_state(
        name: String,
        music_source: String,
        spawn: SpawnMetadata,
        taps: Vec<u32>,
        timeline_step: u32,
        objects: Vec<LevelObject>,
    ) -> Self {
        Self {
            format_version: CURRENT_LEVEL_FORMAT_VERSION,
            name,
            music: MusicMetadata {
                source: music_source,
                extra: serde_json::Map::new(),
            },
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
    #[serde(default, deserialize_with = "deserialize_vec3_from_array")]
    pub(crate) position: [f32; 3],
    #[serde(default)]
    pub(crate) direction: SpawnDirection,
}

impl Default for SpawnMetadata {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            direction: SpawnDirection::Forward,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Default)]
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

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct LevelObject {
    #[serde(default, deserialize_with = "deserialize_vec3_from_array")]
    pub(crate) position: [f32; 3],
    #[serde(default = "default_size", deserialize_with = "deserialize_size_vec3")]
    pub(crate) size: [f32; 3],
    #[serde(default)]
    pub(crate) kind: BlockKind,
}

fn default_size() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn deserialize_vec3_from_array<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    let values: Vec<f32> = Vec::deserialize(deserializer)?;
    match values.as_slice() {
        [x, y] => Ok([*x, *y, 0.0]),
        [x, y, z] => Ok([*x, *y, *z]),
        _ => Err(serde::de::Error::custom(
            "expected an array with 2 or 3 numeric values",
        )),
    }
}

fn deserialize_size_vec3<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    let values: Vec<f32> = Vec::deserialize(deserializer)?;
    match values.as_slice() {
        [x, y] => Ok([*x, *y, 1.0]),
        [x, y, z] => Ok([*x, *y, *z]),
        _ => Err(serde::de::Error::custom(
            "expected an array with 2 or 3 numeric values",
        )),
    }
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

#[derive(Clone, Copy)]
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
