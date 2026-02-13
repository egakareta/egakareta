use serde::{Deserialize, Serialize};

pub(crate) const CURRENT_LEVEL_FORMAT_VERSION: u32 = 1;

fn default_level_format_version() -> u32 {
    CURRENT_LEVEL_FORMAT_VERSION
}

fn default_block_rotation_degrees() -> f32 {
    0.0
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
    pub(crate) source: String,
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for MusicMetadata {
    fn default() -> Self {
        Self {
            source: "music.mp3".to_string(),
            title: None,
            author: None,
            extra: serde_json::Map::new(),
        }
    }
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
    pub(crate) position: [f32; 3],
    pub(crate) size: [f32; 3],
    #[serde(default = "default_block_rotation_degrees")]
    pub(crate) rotation_degrees: f32,
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
