/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// Application-level commands that represent user intent.
///
/// Every meaningful user action (keyboard shortcut, mouse click, UI button)
/// is captured as an `AppCommand` before being dispatched to `State`.
/// This decouples input handling from action execution, gives both
/// native and WASM targets a single code-path, and makes future
/// replay / macro / test harness support trivial.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AppCommand {
    // ── Navigation / Phase ──────────────────────────────────────────
    /// Primary action: in menu → start level, in game → turn, in editor → place block
    TurnRight,
    /// Select the next level in the menu or move editor cursor +X.
    NextLevel,
    /// Select the previous level in the menu or move editor cursor −X.
    PrevLevel,
    /// Toggle between editor and menu/playing.
    ToggleEditor,
    /// Resume real gameplay from the pause menu.
    GameResume,
    /// Restart the current real gameplay level from the pause menu.
    GameRestartLevel,
    /// Enable or disable practice mode for the current real gameplay level.
    GameSetPracticeMode(bool),
    /// Place a practice-mode checkpoint at the current gameplay position.
    GameSetPracticeCheckpoint,
    /// Remove the latest practice-mode checkpoint.
    GameRemovePracticeCheckpoint,
    /// Quit the current real gameplay level from the pause menu.
    GameQuitToMenu,

    // ── Auth ───────────────────────────────────────────────────────
    /// Start the browser sign-in flow.
    AuthSubmitSignIn,
    /// Sign out of the current account.
    AuthSignOut,
    /// Open the web signup page.
    AuthOpenSignup,
    /// Resize render surface to the specified dimensions.
    ResizeSurface { width: u32, height: u32 },

    // ── Editor ───────────────────────────────────────────────────
    /// Editor-specific command routed through `EditorCommand` sub-enum.
    /// New editor commands should use this variant with `EditorCommand`.
    Editor(crate::state::editor_command::EditorCommand),
}

/// Unified input event produced by platform adapters.
///
/// Platform runtimes translate their raw
/// platform events into `InputEvent`s, which `State` processes
/// through a single code-path.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// A keyboard key was pressed or released.
    Key {
        key: String,
        pressed: bool,
        just_pressed: bool,
    },
    /// A mouse button was pressed or released.
    MouseButton { button: u32, pressed: bool },
    /// Primary (left) mouse click at screen coordinates.
    PrimaryClick { x: f64, y: f64 },
    /// Mouse / pointer moved to screen coordinates.
    PointerMoved { x: f64, y: f64 },
    /// Drag camera by pixel delta.  
    CameraDrag { dx: f64, dy: f64 },
    /// Zoom by delta value.
    Zoom(f32),
    /// Surface was resized.
    Resize { width: u32, height: u32 },
}
