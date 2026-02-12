use serde::Deserialize;

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

#[derive(Deserialize, Clone)]
pub(crate) struct MusicMetadata {
    pub(crate) source: String,
}

#[derive(Deserialize, Clone)]
pub(crate) struct LevelMetadata {
    pub(crate) name: String,
    pub(crate) music: MusicMetadata,
    #[serde(default)]
    pub(crate) spawn: SpawnMetadata,
    pub(crate) objects: Vec<LevelObject>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct SpawnMetadata {
    #[serde(default)]
    pub(crate) position: [f32; 2],
    #[serde(default)]
    pub(crate) direction: SpawnDirection,
}

impl Default for SpawnMetadata {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            direction: SpawnDirection::Forward,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SpawnDirection {
    Forward,
    Right,
}

impl Default for SpawnDirection {
    fn default() -> Self {
        Self::Forward
    }
}

impl From<SpawnDirection> for Direction {
    fn from(value: SpawnDirection) -> Self {
        match value {
            SpawnDirection::Forward => Direction::Forward,
            SpawnDirection::Right => Direction::Right,
        }
    }
}

#[derive(Deserialize, Clone)]
pub(crate) struct LevelObject {
    #[serde(default)]
    pub(crate) position: [f32; 2],
    #[serde(default = "default_size")]
    pub(crate) size: [f32; 2],
}

fn default_size() -> [f32; 2] {
    [1.0, 1.0]
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
    pub(crate) cursor: [i32; 2],
    pub(crate) bounds: i32,
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self {
            cursor: [0, 0],
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
