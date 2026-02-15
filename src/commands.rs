/// Application-level commands that represent user intent.
///
/// Every meaningful user action (keyboard shortcut, mouse click, UI button)
/// is captured as an `AppCommand` before being dispatched to `State`.
/// This decouples input handling from action execution, gives both
/// native and WASM targets a single code-path, and makes future
/// replay / macro / test harness support trivial.
#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    // ── Navigation / Phase ──────────────────────────────────────────
    /// Primary action: in menu → start level, in game → turn, in editor → place block
    TurnRight,
    /// Select the next level in the menu or move editor cursor +X.
    NextLevel,
    /// Select the previous level in the menu or move editor cursor −X.
    PrevLevel,
    /// Toggle between editor and menu/playing.
    ToggleEditor,
    /// Return to menu (or exit playtest back to editor).
    BackToMenu,

    // ── Gameplay ────────────────────────────────────────────────────
    /// Restart the current level.
    RestartLevel,

    // ── Editor – mode switching ─────────────────────────────────────
    /// Switch to Select mode.
    EditorModeSelect,
    /// Switch to Place mode.
    EditorModePlace,
    /// Switch to Timing mode.
    EditorModeTiming,

    // ── Editor – block ops ──────────────────────────────────────────
    /// Place or remove a block at the current cursor position.
    EditorPlaceBlock,
    /// Remove the block under cursor or selected blocks.
    EditorRemoveBlock,
    /// Duplicate selected blocks in place.
    EditorDuplicateBlock,
    /// Copy selected blocks to clipboard.
    EditorCopyBlock,
    /// Paste clipboard blocks.
    EditorPasteBlock,
    /// Switch the active block palette slot.
    EditorSetBlockId(String),

    // ── Editor – selection / transform ──────────────────────────────
    /// Nudge selected blocks by the given screen-relative offset.
    EditorNudgeSelected { dx: i32, dy: i32 },

    // ── Editor – timeline / playback ────────────────────────────────
    /// Toggle timeline playback.
    EditorToggleTimelinePlayback,
    /// Shift the timeline cursor by `delta_seconds`.
    EditorShiftTimeline(f32),
    /// Add or remove a tap at the current pointer position.
    EditorToggleTapAtPointer,
    /// Start playtesting from the current editor state.
    EditorPlaytest,

    // ── Editor – spawn ──────────────────────────────────────────────
    /// Set spawn position to current cursor.
    EditorSetSpawnHere,
    /// Rotate the spawn direction.
    EditorRotateSpawnDirection,

    // ── Editor – history ────────────────────────────────────────────
    /// Undo the last editor action.
    EditorUndo,
    /// Redo the last undone editor action.
    EditorRedo,

    // ── Editor – zoom ───────────────────────────────────────────────
    /// Adjust zoom by the given delta.
    EditorAdjustZoom(f32),

    // ── Editor – misc ───────────────────────────────────────────────
    /// Toggle the performance overlay.
    EditorTogglePerfOverlay,
    /// Export the selected block as OBJ.
    EditorExportBlockObj,

    // ── Editor – camera / input state ───────────────────────────────
    /// Escape key context-sensitive (stop playback → reset timeline → back to menu).
    EditorEscape,
}

/// Modifier key state accompanying an input event.
/// Currently tracked internally by `command_dispatch`; will be attached
/// to `InputEvent` variants once modifier-aware platform routing is complete.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Unified input event produced by platform adapters.
///
/// Both `native_runtime` and `web_runtime` translate their raw
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
