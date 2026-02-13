use std::iter;

use glam::{Mat4, Vec2, Vec3, Vec4};
use wgpu::{util::DeviceExt, SurfaceError, TextureViewDescriptor};

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};

use crate::editor_domain::{
    add_tap_step, build_editor_playtest_transition, build_playing_transition_from_metadata,
    clear_tap_steps, create_block_at_cursor, derive_timeline_position,
    editor_session_init_from_metadata, move_cursor_xy, playtest_return_objects, remove_tap_step,
    remove_topmost_block_at_cursor, toggle_spawn_direction,
};
use crate::game::{create_menu_scene, GameState};
use crate::level_repository::{
    build_ldz_archive, builtin_level_names, load_builtin_level_metadata, parse_level_metadata_json,
    read_metadata_from_ldz, serialize_level_metadata_pretty,
};
use crate::mesh::{
    build_block_vertices, build_editor_cursor_vertices, build_editor_gizmo_vertices,
    build_editor_hover_outline_vertices, build_editor_selection_outline_vertices,
    build_floor_vertices, build_grid_vertices, build_spawn_marker_vertices, build_trail_vertices,
};
use crate::platform::audio::PlatformAudio;
use crate::platform::io::{log_platform_error, read_editor_music_bytes, save_level_export};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::state_host::NativeWindow;
#[cfg(target_arch = "wasm32")]
use crate::platform::state_host::WasmCanvas;
use crate::platform::state_host::{log_backend, PlatformInstant, SurfaceHost};
use crate::types::{
    AppPhase, BlockKind, CameraUniform, ColorSpaceUniform, Direction, EditorMode, EditorState,
    LevelMetadata, LevelObject, LineUniform, MenuState, MusicMetadata, PhysicalSize,
    SpawnDirection, SpawnMetadata, Vertex,
};

use base64::Engine as _;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct State {
    surface_host: SurfaceHost,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    floor_vertex_buffer: wgpu::Buffer,
    floor_vertex_count: u32,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    trail_vertex_buffer: wgpu::Buffer,
    trail_vertex_count: u32,
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_vertex_count: u32,
    editor_cursor_vertex_buffer: Option<wgpu::Buffer>,
    editor_cursor_vertex_count: u32,
    editor_hover_outline_vertex_buffer: Option<wgpu::Buffer>,
    editor_hover_outline_vertex_count: u32,
    editor_selection_outline_vertex_buffer: Option<wgpu::Buffer>,
    editor_selection_outline_vertex_count: u32,
    editor_gizmo_vertex_buffer: Option<wgpu::Buffer>,
    editor_gizmo_vertex_count: u32,
    spawn_marker_vertex_buffer: Option<wgpu::Buffer>,
    spawn_marker_vertex_count: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    render_pipeline: wgpu::RenderPipeline,
    gizmo_overlay_pipeline: wgpu::RenderPipeline,
    line_uniform_buffer: wgpu::Buffer,
    zero_line_bind_group: wgpu::BindGroup,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    color_space_bind_group: wgpu::BindGroup,
    apply_gamma_correction: bool,
    game: GameState,
    phase: AppPhase,
    menu: MenuState,
    editor: EditorState,
    editor_selected_kind: BlockKind,
    editor_objects: Vec<LevelObject>,
    editor_spawn: SpawnMetadata,
    editor_camera_pan: [f32; 2],
    editor_camera_rotation: f32,
    editor_camera_pitch: f32,
    playing_camera_rotation: f32,
    playing_camera_pitch: f32,
    editor_zoom: f32,
    editor_timeline_step: u32,
    editor_timeline_length: u32,
    editor_tap_steps: Vec<u32>,
    editor_right_dragging: bool,
    editor_pan_up_held: bool,
    editor_pan_down_held: bool,
    editor_pan_left_held: bool,
    editor_pan_right_held: bool,
    editor_shift_held: bool,
    editor_ctrl_held: bool,
    editor_mode: EditorMode,
    editor_snap_to_grid: bool,
    editor_snap_step: f32,
    editor_selected_block_index: Option<usize>,
    editor_selected_block_indices: Vec<usize>,
    editor_hovered_block_index: Option<usize>,
    editor_gizmo_drag: Option<EditorGizmoDrag>,
    editor_block_drag: Option<EditorBlockDrag>,
    editor_pointer_screen: Option<[f64; 2]>,
    editor_clipboard_block: Option<LevelObject>,
    editor_history_undo: Vec<EditorHistorySnapshot>,
    editor_history_redo: Vec<EditorHistorySnapshot>,
    editor_level_name: Option<String>,
    editor_music_metadata: MusicMetadata,
    editor_show_metadata: bool,
    playing_level_name: Option<String>,
    editor_show_import: bool,
    editor_import_text: String,
    playtesting_editor: bool,
    line_uniform: LineUniform,
    last_frame: PlatformInstant,
    accumulator: f32,
    audio: PlatformAudio,
    local_audio_cache: std::collections::HashMap<String, Vec<u8>>,
    audio_import_channel: (
        std::sync::mpsc::Sender<AudioImportData>,
        std::sync::mpsc::Receiver<AudioImportData>,
    ),
}

type AudioImportData = (String, Vec<u8>);

struct EditorPickResult {
    cursor: [i32; 3],
    hit_block_index: Option<usize>,
}

#[derive(Clone, Copy)]
enum GizmoAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy)]
enum GizmoDragKind {
    Move,
    Resize,
}

#[derive(Clone)]
struct EditorGizmoDrag {
    axis: GizmoAxis,
    kind: GizmoDragKind,
    start_mouse: [f64; 2],
    start_center_screen: [f32; 2],
    start_center_world: [f32; 3],
    start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone)]
struct EditorBlockDrag {
    start_mouse: [f64; 2],
    start_center_screen: [f32; 2],
    start_center_world: [f32; 3],
    start_blocks: Vec<EditorDragBlockStart>,
}

#[derive(Clone, Copy)]
struct EditorDragBlockStart {
    index: usize,
    position: [f32; 3],
    size: [f32; 3],
}

#[derive(Clone)]
struct EditorHistorySnapshot {
    objects: Vec<LevelObject>,
    selected_block_index: Option<usize>,
    selected_block_indices: Vec<usize>,
    cursor: [i32; 3],
    selected_kind: BlockKind,
    spawn: SpawnMetadata,
    timeline_step: u32,
    timeline_length: u32,
    tap_steps: Vec<u32>,
}

mod core;
mod editor;
mod render;

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::{LevelObject, SpawnDirection};
    use crate::editor_domain::derive_timeline_position;

    #[test]
    fn derives_position_without_taps() {
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], 3, &[]);
        assert_eq!(position, [0.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn derives_position_with_taps() {
        let taps = [2, 4];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 5, &[]);
        assert_eq!(position, [2.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn tap_at_zero_changes_direction() {
        let taps = [0];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 0, &[]);
        assert_eq!(position, [0.0, 0.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Right));
    }

    #[test]
    fn ignores_taps_after_step() {
        let taps = [5];
        let (position, direction) =
            derive_timeline_position([1.0, 1.0, 0.0], SpawnDirection::Forward, &taps, 2, &[]);
        assert_eq!(position, [1.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn supports_offset_spawn_with_tap() {
        let taps = [2];
        let (position, direction) =
            derive_timeline_position([2.0, 2.0, 0.0], SpawnDirection::Right, &taps, 3, &[]);
        assert_eq!(position, [4.0, 3.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }

    #[test]
    fn falls_from_elevated_platform() {
        let objects = [LevelObject {
            position: [0.0, 0.0, 2.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            kind: crate::types::BlockKind::Standard,
        }];
        let (position, direction) =
            derive_timeline_position([0.0, 0.0, 3.0], SpawnDirection::Forward, &[], 1, &objects);
        assert_eq!(position, [0.0, 1.0, 0.0]);
        assert!(matches!(direction, SpawnDirection::Forward));
    }
}
