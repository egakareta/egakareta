/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorInteractionChange {
    None,
    Hover,
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameCursor {
    Default,
    Hidden,
}

impl GameCursor {
    pub fn hides_system_cursor(self) -> bool {
        matches!(self, Self::Hidden)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EditorPickResult {
    pub cursor: [f32; 3],
    pub hit_block_index: Option<usize>,
    pub hit_trigger_index: Option<usize>,
    pub hit_tap_index: Option<usize>,
    pub hit_tap_division: Option<EditorTapDivisionPick>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorTapDivisionPick {
    pub time_seconds: f32,
    pub indicator_position: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPlaceMode {
    SelectPlaced,
    Stamp,
}

pub const DEFAULT_MENU_PREVIEW_CAMERA_POSITION: [f32; 3] = [22.657694, 15.0, -10.565456];
pub const DEFAULT_MENU_PREVIEW_CAMERA_TARGET: [f32; 3] = [0.0, 0.0, 0.0];

#[derive(Debug, PartialEq, Clone, Copy)]
/// The current phase of the application.
/// Determines which UI and logic to run.
pub enum AppPhase {
    Menu,
    Playing,
    Editor,
    GameOver,
}

/// State for the main menu screen.
/// Manages level selection and available levels list.
/// State for the main menu.
/// Tracks the currently selected level and the list of available levels.
pub struct MenuState {
    pub selected_level: usize,
    pub levels: Vec<String>,
    pub preview_level_index: Option<usize>,
    pub preview_sky_color: [f32; 3],
    pub preview_camera_position: [f32; 3],
    pub preview_camera_target: [f32; 3],
}

/// State for the level editor.
/// Manages cursor position, mode, and other editor-specific settings.
pub struct EditorState {
    pub cursor: [f32; 3],
    pub mode: EditorMode,
    pub left_mouse_down: bool,
    pub right_dragging: bool,
    pub pan_up_held: bool,
    pub pan_down_held: bool,
    pub pan_left_held: bool,
    pub pan_right_held: bool,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub alt_held: bool,
    pub selected_block_index: Option<usize>,
    pub selected_block_indices: Vec<usize>,
    pub hovered_block_index: Option<usize>,
    pub pointer_screen: Option<[f64; 2]>,
    pub ui_input_blocking_rects: Vec<[f32; 4]>,
    pub ui_input_pixels_per_point: f32,
    pub marquee_start_screen: Option<[f64; 2]>,
    pub marquee_current_screen: Option<[f64; 2]>,
    pub show_hitbox_visualization: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// The current mode of the level editor.
/// Determines what actions are available and how interactions behave.
pub enum EditorMode {
    Select,
    Move,
    Scale,
    Rotate,
    #[default]
    Place,
    Tapping,
    Timing,
    Null,
}

impl EditorMode {
    pub fn is_selection_mode(self) -> bool {
        matches!(self, Self::Select | Self::Move | Self::Scale | Self::Rotate)
    }

    pub fn is_compose_mode(self) -> bool {
        matches!(
            self,
            Self::Select | Self::Move | Self::Scale | Self::Rotate | Self::Place | Self::Null
        )
    }

    pub fn shows_move_gizmo(self) -> bool {
        self == Self::Move
    }

    pub fn shows_scale_gizmo(self) -> bool {
        self == Self::Scale
    }

    pub fn shows_rotate_gizmo(self) -> bool {
        self == Self::Rotate
    }

    pub fn shows_gizmo(self) -> bool {
        self.shows_move_gizmo() || self.shows_scale_gizmo() || self.shows_rotate_gizmo()
    }

    pub fn can_select(self) -> bool {
        self != Self::Null && self != Self::Timing && self != Self::Tapping
    }
}

impl EditorState {
    pub fn new() -> Self {
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
            ui_input_blocking_rects: Vec::new(),
            ui_input_pixels_per_point: 1.0,
            marquee_start_screen: None,
            marquee_current_screen: None,
            show_hitbox_visualization: false,
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// A generic size structure with width and height components.
/// Used for representing physical dimensions like window size or surface size.
#[derive(Clone, Copy)]
pub struct PhysicalSize<T> {
    pub width: T,
    pub height: T,
}

impl<T> PhysicalSize<T> {
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for line rendering parameters.
/// Contains offset and rotation for positioning lines in screen space.
pub struct LineUniform {
    pub offset: [f32; 2],
    pub rotation: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for camera transformation matrix.
/// Contains the view-projection matrix for 3D rendering.
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_position: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for color space settings.
/// Controls gamma correction application in the shader.
pub struct ColorSpaceUniform {
    pub apply_gamma_correction: f32,
    pub time_seconds: f32,
    pub viewport_size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// Uniform buffer for the analytic world grid.
pub struct GridUniform {
    pub center: [f32; 2],
    pub half_extent: f32,
    pub darkening: f32,
}
