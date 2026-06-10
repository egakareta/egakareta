/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Editor-specific command sub-enum.
//!
//! New editor commands should be added here instead of in `AppCommand`.
//! The `State::dispatch_editor()` method routes these to the appropriate
//! `EditorSubsystem` / `State` methods, keeping editor feature additions
//! self-contained within the `state/` module.

use crate::types::{
    EditorMode, KeyChord, LevelCreatorMetadata, LevelObject, MusicMetadata, SettingsSection,
};

/// Editor-specific commands. Grouped by domain for clarity.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EditorCommand {
    // ── Mode switching ──────────────────────────────────────────────
    /// Switch the editor mode.
    SetMode(EditorMode),
    /// Set the current block ID for placement.
    SetBlockId(String),
    /// Select the Nth recent placeable block and switch to Place mode.
    SelectRecentBlock(usize),
    /// Pick the selected block and enter place mode with that block type.
    PickSelectedBlock,
    /// Pick the block under the pointer and enter place mode with that block type.
    PickBlockAt { x: f64, y: f64 },

    // ── Snap ────────────────────────────────────────────────────────
    /// Set whether to snap to grid.
    SetSnapToGrid(bool),
    /// Set the grid snap step.
    SetSnapStep(f32),
    /// Set whether rotation uses snapping.
    SetSnapRotation(bool),
    /// Set the rotation snap step in degrees.
    SetSnapRotationStep(f32),

    // ── Block ops ───────────────────────────────────────────────────
    /// Remove the block under cursor or selected blocks.
    RemoveBlock,
    /// Duplicate selected blocks in place.
    DuplicateBlock,
    /// Copy selected blocks to clipboard.
    CopyBlock,
    /// Paste clipboard blocks.
    PasteBlock,
    /// Update the properties of the currently selected block.
    UpdateSelectedBlock(LevelObject),

    // ── Selection / Transform ───────────────────────────────────────
    /// Nudge selected blocks by the given screen-relative offset.
    NudgeSelected { dx: i32, dy: i32 },
    /// Snap selected blocks or tap to the nearest grid cell.
    SnapSelectionToGrid,
    /// Focus the editor camera on the selected or preview target.
    FocusCameraTarget,
    /// Begin authoring transform-object triggers from the selected blocks.
    BeginTransformTriggerCapture,
    /// Commit the active transform-object trigger capture.
    CommitTransformTriggerCapture,
    /// Cancel the active transform-object trigger capture.
    CancelTransformTriggerCapture,

    // ── Timeline / Playback ─────────────────────────────────────────
    /// Toggle timeline playback.
    ToggleTimelinePlayback,
    /// Shift the timeline cursor by `delta_seconds`.
    ShiftTimeline(f32),
    /// Set the timeline cursor to an absolute time in seconds.
    SetTimelineTime(f32),
    /// Set the total duration of the timeline (seconds).
    SetTimelineDuration(f32),
    /// Add a tap at the current timeline position.
    AddTap,
    /// Remove a tap at the current timeline position.
    RemoveTap,
    /// Remove a tap at a specific timeline position.
    RemoveTapAt(f32),
    /// Select or deselect a tap by index.
    SetSelectedTap(Option<usize>),
    /// Update the selected tap's timestamp.
    SetSelectedTapTime(f32),
    /// Remove all taps from the level.
    ClearTaps,
    /// Update the playback speed multiplier.
    SetPlaybackSpeed(f32),
    /// Set waveform view zoom.
    SetWaveformZoom(f32),
    /// Set waveform view scroll offset.
    SetWaveformScroll(f32),
    /// Start playtesting from the current editor state.
    Playtest,

    // ── Timing Points ───────────────────────────────────────────────
    /// Add a new timing point.
    AddTimingPoint { time_seconds: f32, bpm: f32 },
    /// Remove an existing timing point by index.
    RemoveTimingPoint(usize),
    /// Update an existing timing point's timestamp.
    SetTimingPointTime(usize, f32),
    /// Update an existing timing point's BPM.
    SetTimingPointBpm(usize, f32),
    /// Update an existing timing point's time signature.
    SetTimingPointTimeSignature(usize, u32, u32),
    /// Select or deselect a timing point in the UI.
    SetTimingSelected(Option<usize>),

    // ── BPM Tapping ─────────────────────────────────────────────────
    /// Record a BPM heart-beat tap.
    BpmTap,
    /// Reset the BPM tapping state.
    BpmTapReset,

    // ── Spawn ───────────────────────────────────────────────────────
    /// Set spawn position to current cursor.
    SetSpawnHere,
    /// Rotate the spawn direction.
    RotateSpawnDirection,
    /// Rotate the block placement preview.
    RotatePlacementPreview,

    // ── History ─────────────────────────────────────────────────────
    /// Undo the last editor action.
    Undo,
    /// Redo the last undone editor action.
    Redo,

    // ── Zoom / Camera ───────────────────────────────────────────────
    /// Adjust zoom by moving the camera along its look vector.
    AdjustZoom(f32),
    /// Snap the editor camera to an absolute orientation in radians.
    SetCameraOrientation {
        rotation: f32,
        pitch: f32,
        transition_seconds: Option<f32>,
    },
    /// Capture a new camera trigger from the current editor camera at the playhead.
    AddCameraTrigger,
    /// Select or deselect a trigger in the UI.
    SetTriggerSelected(Option<usize>),
    /// Set whether timed object triggers move gameplay hitboxes during play.
    SetSimulateTriggerHitboxes(bool),

    // ── Misc ────────────────────────────────────────────────────────
    /// Toggle editor hitbox visualization.
    ToggleHitboxVisualization,
    /// Toggle the performance overlay.
    TogglePerfOverlay,
    /// Export the selected block as OBJ.
    ExportBlockObj,

    // ── UI / Session ────────────────────────────────────────────────
    /// Load a level from a builtin resource name.
    LoadLevel(String),
    /// Rename the current level.
    RenameLevel(String),
    /// Start the level export process.
    ExportLevel,
    /// Open or close the metadata editing window.
    SetShowMetadata(bool),
    /// Toggle the settings sidebar visibility.
    ToggleSettings,
    /// Open or close the settings sidebar.
    SetShowSettings(bool),
    /// Select which settings section is displayed in the sidebar.
    SetSettingsSection(SettingsSection),
    /// Set preferred graphics backend for subsequent launches.
    SetGraphicsBackend(String),
    /// Set preferred audio backend and apply immediately when possible.
    SetAudioBackend(String),
    /// Set UI scale multiplier used with responsive UI scaling.
    SetUiScaleMultiplier(f32),
    /// Start or cancel keybind capture for an action slot.
    SetKeybindCapture(Option<(String, usize)>),
    /// Set a keybind mapping for an action at a specific slot.
    SetKeybind {
        action: String,
        slot: usize,
        chord: KeyChord,
    },
    /// Clear the keybind mapping for an action at a specific slot.
    ClearKeybindSlot { action: String, slot: usize },
    /// Reset a single keybind action to its default values.
    ResetKeybind(String),
    /// Reset all keybinds to defaults.
    ResetKeybinds,
    /// Trigger level import via platform file picker.
    CompleteImport,
    /// Update music information for the level.
    UpdateMusic(MusicMetadata),
    /// Update creator-facing metadata for the level.
    UpdateCreatorMetadata(LevelCreatorMetadata),
    /// Update the level sky clear color.
    UpdateSkyColor([f32; 3]),
    /// Trigger the platform audio import dialog.
    TriggerAudioImport,
    /// Store the current editor camera as menu preview camera metadata.
    CaptureMenuPreviewCamera,
    /// Remove manual menu preview camera metadata and use automatic camera.
    UseAutoMenuPreviewCamera,

    // ── Keyboard State Routing ──────────────────────────────────────
    /// Track Shift held state for editor shortcuts.
    SetShiftHeld(bool),
    /// Track Ctrl held state for editor shortcuts.
    SetCtrlHeld(bool),
    /// Track Alt held state for editor shortcuts.
    SetAltHeld(bool),
    /// Track W-pan held state in editor.
    SetPanUpHeld(bool),
    /// Track S-pan held state in editor.
    SetPanDownHeld(bool),
    /// Track A-pan held state in editor.
    SetPanLeftHeld(bool),
    /// Track D-pan held state in editor.
    SetPanRightHeld(bool),

    // ── Pointer / Input Routing ─────────────────────────────────────
    /// Mouse button state update for editor interaction routing.
    MouseButton { button: u32, pressed: bool },
    /// Primary click action at screen coordinates.
    PrimaryClick { x: f64, y: f64 },
    /// Pointer moved to screen coordinates.
    PointerMoved { x: f64, y: f64 },
    /// Update the editor cursor from screen coordinates without triggering a click.
    UpdateCursorFromScreen { x: f64, y: f64 },
    /// Camera drag delta in screen pixels.
    CameraDrag { dx: f64, dy: f64 },

    // ── Place Window ────────────────────────────────────────────────
    /// Toggle the floating place window with block catalog.
    TogglePlaceWindow,

    // ── Escape Context ──────────────────────────────────────────────
    /// Escape key context-sensitive (stop playback → reset timeline → back to menu).
    Escape,
}
