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

    // ── Editor – mode switching ─────────────────────────────────────
    /// Switch the editor mode.
    EditorSetMode(crate::types::EditorMode),
    /// Set the current block ID for placement.
    EditorSetBlockId(String),
    /// Set whether to snap to grid.
    EditorSetSnapToGrid(bool),
    /// Set the grid snap step.
    EditorSetSnapStep(f32),

    // ── Editor – block ops ──────────────────────────────────────────
    /// Remove the block under cursor or selected blocks.
    EditorRemoveBlock,
    /// Duplicate selected blocks in place.
    EditorDuplicateBlock,
    /// Copy selected blocks to clipboard.
    EditorCopyBlock,
    /// Paste clipboard blocks.
    EditorPasteBlock,
    /// Update the properties of the currently selected block.
    EditorUpdateSelectedBlock(crate::types::LevelObject),

    // ── Editor – selection / transform ──────────────────────────────
    /// Nudge selected blocks by the given screen-relative offset.
    EditorNudgeSelected { dx: i32, dy: i32 },

    // ── Editor – timeline / playback ────────────────────────────────
    /// Toggle timeline playback.
    EditorToggleTimelinePlayback,
    /// Shift the timeline cursor by `delta_seconds`.
    EditorShiftTimeline(f32),
    /// Set the timeline cursor to an absolute time in seconds.
    EditorSetTimelineTime(f32),
    /// Set the total duration of the timeline (seconds).
    EditorSetTimelineDuration(f32),
    /// Add or remove a tap at the current pointer position.
    EditorToggleTapAtPointer,
    /// Add a tap at the current timeline position.
    EditorAddTap,
    /// Remove a tap at the current timeline position.
    EditorRemoveTap,
    /// Remove all taps from the level.
    EditorClearTaps,
    /// Update the playback speed multiplier.
    EditorSetPlaybackSpeed(f32),
    /// Set waveform view zoom.
    EditorSetWaveformZoom(f32),
    /// Set waveform view scroll offset.
    EditorSetWaveformScroll(f32),
    /// Start playtesting from the current editor state.
    EditorPlaytest,

    // ── Editor – timing points ──────────────────────────────────────
    /// Add a new timing point.
    EditorAddTimingPoint { time_seconds: f32, bpm: f32 },
    /// Remove an existing timing point by index.
    EditorRemoveTimingPoint(usize),
    /// Update an existing timing point's timestamp.
    EditorSetTimingPointTime(usize, f32),
    /// Update an existing timing point's BPM.
    EditorSetTimingPointBpm(usize, f32),
    /// Update an existing timing point's time signature.
    EditorSetTimingPointTimeSignature(usize, u32, u32),
    /// Select or deselect a timing point in the UI.
    EditorSetTimingSelected(Option<usize>),

    // ── Editor – BPM tapping ────────────────────────────────────────
    /// Record a BPM heart-beat tap.
    EditorBpmTap,
    /// Reset the BPM tapping state.
    EditorBpmTapReset,

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

    // ── Editor – UI / Session ───────────────────────────────────────
    /// Load a level from a builtin resource name.
    EditorLoadLevel(String),
    /// Rename the current level.
    EditorRenameLevel(String),
    /// Start the level export process.
    EditorExportLevel,
    /// Open or close the metadata editing window.
    EditorSetShowMetadata(bool),
    /// Open or close the import/export raw data window.
    EditorSetShowImport(bool),
    /// Update the text in the raw import field.
    EditorSetImportText(String),
    /// Parse and apply the raw import text.
    EditorCompleteImport,
    /// Update music information for the level.
    EditorUpdateMusic(crate::types::MusicMetadata),
    /// Trigger the platform audio import dialog.
    EditorTriggerAudioImport,

    // ── Editor – escape context ─────────────────────────────
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
