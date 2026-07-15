/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy `Message` taxonomy for the eg-state `App`.
//!
//! Phase 3 fragments the monolithic `AppCommand` / `EditorCommand` enums
//! into a per-semantic Bevy `Message` type. During the Phase 7
//! compatibility window, messages mutate App-world resources directly or
//! write typed request resources that `Runtime` consumes through legacy
//! compatibility methods.
//!
//! Bevy 0.19 uses `Message` (not `Event`) for buffered per-frame
//! payloads; observer-based `Event`s fire immediately. We use `Message`
//! here because we want pull-based consumption by the per-message
//! handler systems in the [`CommandHandlerSet`] system set.
//!
//! The per-message systems are wired into the `StateSchedule` so each
//! message type has one explicit handler. Runtime runs the schedule on
//! the App world, then consumes request resources for side effects that
//! still live on legacy `State`. Input-state and direct command messages
//! update App-world resources directly.
//!
//! ## Why messages?
//!
//! - **One reader per message type** — distinct `Message<T>` types give
//!   each handler its own `MessageReader<T>`, so multiple readers of
//!   the same message can be added in any order without cross-reader
//!   nondeterminism.
//! - **`Changed<T>` change detection** — distinct messages unlock
//!   component-level change detection (e.g. a "rebuild mesh" system
//!   can `Query<Changed<Position>, Changed<Rotation>, ...>`) and let
//!   us eventually delete `EditorDirtyFlags` entirely.
//! - **UI / winit decoupling** — UI / winit code can write
//!   `Messages<T>::write(event)` from anywhere; no need to thread a
//!   `&mut State` through every UI closure.
//!
//! ## Input-state carve-out
//!
//! Per the migration plan, the following 12 editor command variants are
//! **not** commands — they are *input state*. Modifier/pan held state now
//! writes directly to `EditorInputRes`; pointer/camera drag messages now
//! write directly to App-world editor input/camera resources during the
//! compatibility window.
//!
//! - `SetShiftHeld` / `SetCtrlHeld` / `SetAltHeld` (direct resource writes)
//! - `SetPan{Up,Down,Left,Right}Held` (direct resource writes)
//! - `MouseButton` / `PrimaryClick` / `PointerMoved` /
//!   `UpdateCursorFromScreen` / `CameraDrag` (direct resource writes)
//!
//! REFACTOR POINT(full-bevy): the messages defined here migrate to
//! `bevy_input` `Input<KeyCode>` / `ButtonInput<MouseButton>` in the
//! full-Bevy migration. The Bevy `Message` types themselves stay.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;

use crate::commands::AppCommand;
use crate::state::editor_command::EditorCommand;

pub mod async_results;
pub mod auth;
pub mod block;
pub mod bpm;
pub mod camera;
pub mod editor_mode;
pub mod history;
pub mod input_state;
pub mod mesh_rebuild;
pub mod misc;
pub mod navigation;
pub mod pointer;
pub mod selection;
pub mod session;
pub mod snap;
pub mod spawn;
pub mod timeline;
pub mod timing;

// Note: we intentionally do NOT re-export submodule items here. Each
// submodule defines its own `register_events` (and `handle_*`) function
// with the same name; glob re-exports would create an ambiguous-glob
// lint and the names would shadow. The top-level helpers below
// (`register_command_events` / `add_command_systems`) call each
// submodule's `register_events` / `handle_*` directly through explicit
// module paths. Public message *types* remain accessible via their
// submodule path (e.g. `events::navigation::TurnRight`).

/// System set that contains the per-message command handlers.
///
/// All systems in this set read a single `MessageReader<T>`. Handlers mutate
/// App-world resources directly or write typed request resources consumed by
/// Runtime. They run as part of the `StateSchedule`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandHandlerSet;

/// Register every eg-state message type on the given `App`.
///
/// The schedule itself is owned by `State` (Phase 2c / 7), so this
/// helper does not add a `Schedule` — only the `Messages<T>` resources.
/// Use [`add_command_systems`] to add the per-message command handlers
/// to a `Schedule`.
pub fn register_command_events(app: &mut App) {
    // Add the message resources so `Messages<T>` is available.
    navigation::register_events(app);
    auth::register_events(app);
    editor_mode::register_events(app);
    snap::register_events(app);
    block::register_events(app);
    selection::register_events(app);
    timeline::register_events(app);
    timing::register_events(app);
    bpm::register_events(app);
    spawn::register_events(app);
    history::register_events(app);
    camera::register_events(app);
    misc::register_events(app);
    session::register_events(app);
    input_state::register_events(app);
    pointer::register_events(app);
}

/// World-flavoured variant of [`register_command_events`] for callers
/// that work with a bare `World` (e.g. `State` constructs its `World`
/// directly without going through an `App`). Initializes the
/// `Messages<T>` resource for every event type; does **not** add a
/// schedule — see `register_state_command_pipeline` for the full
/// `World` setup.
pub fn register_command_events_world(world: &mut World) {
    use bevy_ecs::message::Messages;
    // Navigation
    world.init_resource::<Messages<navigation::TurnRight>>();
    world.init_resource::<Messages<navigation::NextLevel>>();
    world.init_resource::<Messages<navigation::PrevLevel>>();
    world.init_resource::<Messages<navigation::ToggleEditor>>();
    world.init_resource::<Messages<navigation::GameResume>>();
    world.init_resource::<Messages<navigation::GameRestartLevel>>();
    world.init_resource::<Messages<navigation::GameSetPracticeMode>>();
    world.init_resource::<Messages<navigation::GameSetPracticeCheckpoint>>();
    world.init_resource::<Messages<navigation::GameRemovePracticeCheckpoint>>();
    world.init_resource::<Messages<navigation::GameQuitToMenu>>();
    // Auth
    world.init_resource::<Messages<auth::AuthSubmitSignIn>>();
    world.init_resource::<Messages<auth::AuthSignOut>>();
    world.init_resource::<Messages<auth::AuthOpenSignup>>();
    world.init_resource::<Messages<auth::CopyLatestPerfFrame>>();
    world.init_resource::<Messages<auth::ResizeSurface>>();
    // Editor mode
    world.init_resource::<Messages<editor_mode::SetEditorMode>>();
    world.init_resource::<Messages<editor_mode::SetBlockId>>();
    world.init_resource::<Messages<editor_mode::SelectRecentBlock>>();
    world.init_resource::<Messages<editor_mode::PickSelectedBlock>>();
    world.init_resource::<Messages<editor_mode::PickBlockAt>>();
    // Snap
    world.init_resource::<Messages<snap::SetSnapToGrid>>();
    world.init_resource::<Messages<snap::SetSnapStep>>();
    world.init_resource::<Messages<snap::SetSnapRotation>>();
    world.init_resource::<Messages<snap::SetSnapRotationStep>>();
    // Block
    world.init_resource::<Messages<block::RemoveBlock>>();
    world.init_resource::<Messages<block::DuplicateBlock>>();
    world.init_resource::<Messages<block::CopyBlock>>();
    world.init_resource::<Messages<block::PasteBlock>>();
    world.init_resource::<Messages<block::UpdateSelectedBlock>>();
    // Selection
    world.init_resource::<Messages<selection::SnapSelectionToGrid>>();
    world.init_resource::<Messages<selection::FocusCameraTarget>>();
    world.init_resource::<Messages<selection::AddTransformTrigger>>();
    // Timeline
    world.init_resource::<Messages<timeline::ToggleTimelinePlayback>>();
    world.init_resource::<Messages<timeline::ShiftTimeline>>();
    world.init_resource::<Messages<timeline::SetTimelineTime>>();
    world.init_resource::<Messages<timeline::SetTimelineDuration>>();
    world.init_resource::<Messages<timeline::AddTap>>();
    world.init_resource::<Messages<timeline::RemoveTap>>();
    world.init_resource::<Messages<timeline::RemoveTapAt>>();
    world.init_resource::<Messages<timeline::SetSelectedTap>>();
    world.init_resource::<Messages<timeline::SetSelectedTapTime>>();
    world.init_resource::<Messages<timeline::ClearTaps>>();
    world.init_resource::<Messages<timeline::SetPlaybackSpeed>>();
    world.init_resource::<Messages<timeline::SetWaveformZoom>>();
    world.init_resource::<Messages<timeline::SetWaveformScroll>>();
    world.init_resource::<Messages<timeline::Playtest>>();
    // Timing
    world.init_resource::<Messages<timing::AddTimingPoint>>();
    world.init_resource::<Messages<timing::RemoveTimingPoint>>();
    world.init_resource::<Messages<timing::SetTimingPointTime>>();
    world.init_resource::<Messages<timing::SetTimingPointBpm>>();
    world.init_resource::<Messages<timing::SetTimingPointTimeSignature>>();
    world.init_resource::<Messages<timing::SetTimingSelected>>();
    // BPM
    world.init_resource::<Messages<bpm::BpmTap>>();
    world.init_resource::<Messages<bpm::BpmTapReset>>();
    // Spawn
    world.init_resource::<Messages<spawn::SetSpawnHere>>();
    world.init_resource::<Messages<spawn::RotateSpawnDirection>>();
    world.init_resource::<Messages<spawn::RotatePlacementPreview>>();
    // History
    world.init_resource::<Messages<history::Undo>>();
    world.init_resource::<Messages<history::Redo>>();
    world.init_resource::<Messages<history::HistorySnapshot>>();
    // Camera
    world.init_resource::<Messages<camera::AdjustZoom>>();
    world.init_resource::<Messages<camera::SetCameraOrientation>>();
    world.init_resource::<Messages<camera::AddCameraTrigger>>();
    world.init_resource::<Messages<camera::AddCameraFollowTrigger>>();
    world.init_resource::<Messages<camera::SetTriggerSelected>>();
    // Misc
    world.init_resource::<Messages<misc::ToggleHitboxVisualization>>();
    world.init_resource::<Messages<misc::TogglePerfOverlay>>();
    world.init_resource::<Messages<misc::TogglePlaceWindow>>();
    world.init_resource::<Messages<misc::Escape>>();
    // Session
    world.init_resource::<Messages<session::LoadLevel>>();
    world.init_resource::<Messages<session::RenameLevel>>();
    world.init_resource::<Messages<session::ExportLevel>>();
    world.init_resource::<Messages<session::SetShowMetadata>>();
    world.init_resource::<Messages<session::ToggleSettings>>();
    world.init_resource::<Messages<session::SetShowSettings>>();
    world.init_resource::<Messages<session::SetSettingsSection>>();
    world.init_resource::<Messages<session::SetGraphicsBackend>>();
    world.init_resource::<Messages<session::SetAudioBackend>>();
    world.init_resource::<Messages<session::SetUiScaleMultiplier>>();
    world.init_resource::<Messages<session::SetKeybindCapture>>();
    world.init_resource::<Messages<session::SetKeybind>>();
    world.init_resource::<Messages<session::ClearKeybindSlot>>();
    world.init_resource::<Messages<session::ResetKeybind>>();
    world.init_resource::<Messages<session::ResetKeybinds>>();
    world.init_resource::<Messages<session::CompleteImport>>();
    world.init_resource::<Messages<session::UpdateMusic>>();
    world.init_resource::<Messages<session::UpdateCreatorMetadata>>();
    world.init_resource::<Messages<session::UpdateSkyColor>>();
    world.init_resource::<Messages<session::TriggerAudioImport>>();
    world.init_resource::<Messages<session::CaptureMenuPreviewCamera>>();
    world.init_resource::<Messages<session::UseAutoMenuPreviewCamera>>();
    // Input state (direct resource writes for held state; see module docs).
    world.init_resource::<Messages<input_state::SetShiftHeld>>();
    world.init_resource::<Messages<input_state::SetCtrlHeld>>();
    world.init_resource::<Messages<input_state::SetAltHeld>>();
    world.init_resource::<Messages<input_state::SetPanUpHeld>>();
    world.init_resource::<Messages<input_state::SetPanDownHeld>>();
    world.init_resource::<Messages<input_state::SetPanLeftHeld>>();
    world.init_resource::<Messages<input_state::SetPanRightHeld>>();
    // Pointer / drag.
    world.init_resource::<Messages<pointer::MouseButton>>();
    world.init_resource::<Messages<pointer::PrimaryClick>>();
    world.init_resource::<Messages<pointer::PointerMoved>>();
    world.init_resource::<Messages<pointer::UpdateCursorFromScreen>>();
    world.init_resource::<Messages<pointer::CameraDrag>>();
}

/// Write a legacy app command into the Bevy message bus.
///
/// UI and compatibility callers use this as the single command-ingress
/// adapter while the public UI surface still returns `AppCommand` values.
pub fn write_app_command_message(world: &mut World, command: AppCommand) {
    match command {
        AppCommand::TurnRight => {
            world.write_message(navigation::TurnRight);
        }
        AppCommand::NextLevel => {
            world.write_message(navigation::NextLevel);
        }
        AppCommand::PrevLevel => {
            world.write_message(navigation::PrevLevel);
        }
        AppCommand::ToggleEditor => {
            world.write_message(navigation::ToggleEditor);
        }
        AppCommand::GameResume => {
            world.write_message(navigation::GameResume);
        }
        AppCommand::GameRestartLevel => {
            world.write_message(navigation::GameRestartLevel);
        }
        AppCommand::GameSetPracticeMode(enabled) => {
            world.write_message(navigation::GameSetPracticeMode { enabled });
        }
        AppCommand::GameSetPracticeCheckpoint => {
            world.write_message(navigation::GameSetPracticeCheckpoint);
        }
        AppCommand::GameRemovePracticeCheckpoint => {
            world.write_message(navigation::GameRemovePracticeCheckpoint);
        }
        AppCommand::GameQuitToMenu => {
            world.write_message(navigation::GameQuitToMenu);
        }
        AppCommand::AuthSubmitSignIn => {
            world.write_message(auth::AuthSubmitSignIn);
        }
        AppCommand::AuthSignOut => {
            world.write_message(auth::AuthSignOut);
        }
        AppCommand::AuthOpenSignup => {
            world.write_message(auth::AuthOpenSignup);
        }
        AppCommand::CopyLatestPerfFrame => {
            world.write_message(auth::CopyLatestPerfFrame);
        }
        AppCommand::ResizeSurface { width, height } => {
            world.write_message(auth::ResizeSurface { width, height });
        }
        AppCommand::Editor(command) => write_editor_command_message(world, command),
    }
}

/// Write multiple legacy app commands into the Bevy message bus.
pub fn write_app_command_messages(
    world: &mut World,
    commands: impl IntoIterator<Item = AppCommand>,
) {
    for command in commands {
        write_app_command_message(world, command);
    }
}

/// Write a legacy editor command into the Bevy message bus.
pub fn write_editor_command_message(world: &mut World, command: EditorCommand) {
    match command {
        EditorCommand::SetMode(mode) => {
            world.write_message(editor_mode::SetEditorMode(mode));
        }
        EditorCommand::SetBlockId(block_id) => {
            world.write_message(editor_mode::SetBlockId(block_id));
        }
        EditorCommand::SelectRecentBlock(index) => {
            world.write_message(editor_mode::SelectRecentBlock(index));
        }
        EditorCommand::PickSelectedBlock => {
            world.write_message(editor_mode::PickSelectedBlock);
        }
        EditorCommand::PickBlockAt { x, y } => {
            world.write_message(editor_mode::PickBlockAt { x, y });
        }
        EditorCommand::SetSnapToGrid(enabled) => {
            world.write_message(snap::SetSnapToGrid(enabled));
        }
        EditorCommand::SetSnapStep(step) => {
            world.write_message(snap::SetSnapStep(step));
        }
        EditorCommand::SetSnapRotation(enabled) => {
            world.write_message(snap::SetSnapRotation(enabled));
        }
        EditorCommand::SetSnapRotationStep(step) => {
            world.write_message(snap::SetSnapRotationStep(step));
        }
        EditorCommand::RemoveBlock => {
            world.write_message(block::RemoveBlock);
        }
        EditorCommand::DuplicateBlock => {
            world.write_message(block::DuplicateBlock);
        }
        EditorCommand::CopyBlock => {
            world.write_message(block::CopyBlock);
        }
        EditorCommand::PasteBlock => {
            world.write_message(block::PasteBlock);
        }
        EditorCommand::UpdateSelectedBlock(object) => {
            world.write_message(block::UpdateSelectedBlock(object));
        }
        EditorCommand::SnapSelectionToGrid => {
            world.write_message(selection::SnapSelectionToGrid);
        }
        EditorCommand::FocusCameraTarget => {
            world.write_message(selection::FocusCameraTarget);
        }
        EditorCommand::AddTransformTrigger => {
            world.write_message(selection::AddTransformTrigger);
        }
        EditorCommand::ToggleTimelinePlayback => {
            world.write_message(timeline::ToggleTimelinePlayback);
        }
        EditorCommand::ShiftTimeline(delta_seconds) => {
            world.write_message(timeline::ShiftTimeline(delta_seconds));
        }
        EditorCommand::SetTimelineTime(time_seconds) => {
            world.write_message(timeline::SetTimelineTime(time_seconds));
        }
        EditorCommand::SetTimelineDuration(duration_seconds) => {
            world.write_message(timeline::SetTimelineDuration(duration_seconds));
        }
        EditorCommand::AddTap => {
            world.write_message(timeline::AddTap);
        }
        EditorCommand::RemoveTap => {
            world.write_message(timeline::RemoveTap);
        }
        EditorCommand::RemoveTapAt(time_seconds) => {
            world.write_message(timeline::RemoveTapAt(time_seconds));
        }
        EditorCommand::SetSelectedTap(index) => {
            world.write_message(timeline::SetSelectedTap(index));
        }
        EditorCommand::SetSelectedTapTime(time_seconds) => {
            world.write_message(timeline::SetSelectedTapTime(time_seconds));
        }
        EditorCommand::ClearTaps => {
            world.write_message(timeline::ClearTaps);
        }
        EditorCommand::SetPlaybackSpeed(speed) => {
            world.write_message(timeline::SetPlaybackSpeed(speed));
        }
        EditorCommand::SetWaveformZoom(zoom) => {
            world.write_message(timeline::SetWaveformZoom(zoom));
        }
        EditorCommand::SetWaveformScroll(scroll) => {
            world.write_message(timeline::SetWaveformScroll(scroll));
        }
        EditorCommand::Playtest => {
            world.write_message(timeline::Playtest);
        }
        EditorCommand::AddTimingPoint { time_seconds, bpm } => {
            world.write_message(timing::AddTimingPoint { time_seconds, bpm });
        }
        EditorCommand::RemoveTimingPoint(index) => {
            world.write_message(timing::RemoveTimingPoint(index));
        }
        EditorCommand::SetTimingPointTime(index, time) => {
            world.write_message(timing::SetTimingPointTime { index, time });
        }
        EditorCommand::SetTimingPointBpm(index, bpm) => {
            world.write_message(timing::SetTimingPointBpm { index, bpm });
        }
        EditorCommand::SetTimingPointTimeSignature(index, numerator, denominator) => {
            world.write_message(timing::SetTimingPointTimeSignature {
                index,
                numerator,
                denominator,
            });
        }
        EditorCommand::SetTimingSelected(index) => {
            world.write_message(timing::SetTimingSelected(index));
        }
        EditorCommand::BpmTap => {
            world.write_message(bpm::BpmTap);
        }
        EditorCommand::BpmTapReset => {
            world.write_message(bpm::BpmTapReset);
        }
        EditorCommand::SetSpawnHere => {
            world.write_message(spawn::SetSpawnHere);
        }
        EditorCommand::RotateSpawnDirection => {
            world.write_message(spawn::RotateSpawnDirection);
        }
        EditorCommand::RotatePlacementPreview => {
            world.write_message(spawn::RotatePlacementPreview);
        }
        EditorCommand::Undo => {
            world.write_message(history::Undo);
        }
        EditorCommand::Redo => {
            world.write_message(history::Redo);
        }
        EditorCommand::AdjustZoom(delta) => {
            world.write_message(camera::AdjustZoom(delta));
        }
        EditorCommand::SetCameraOrientation {
            rotation,
            pitch,
            transition_seconds,
        } => {
            world.write_message(camera::SetCameraOrientation {
                rotation,
                pitch,
                transition_seconds,
            });
        }
        EditorCommand::AddCameraTrigger => {
            world.write_message(camera::AddCameraTrigger);
        }
        EditorCommand::AddCameraFollowTrigger => {
            world.write_message(camera::AddCameraFollowTrigger);
        }
        EditorCommand::SetTriggerSelected(index) => {
            world.write_message(camera::SetTriggerSelected(index));
        }
        EditorCommand::ToggleHitboxVisualization => {
            world.write_message(misc::ToggleHitboxVisualization);
        }
        EditorCommand::TogglePerfOverlay => {
            world.write_message(misc::TogglePerfOverlay);
        }
        EditorCommand::LoadLevel(name) => {
            world.write_message(session::LoadLevel(name));
        }
        EditorCommand::RenameLevel(name) => {
            world.write_message(session::RenameLevel(name));
        }
        EditorCommand::ExportLevel => {
            world.write_message(session::ExportLevel);
        }
        EditorCommand::SetShowMetadata(show) => {
            world.write_message(session::SetShowMetadata(show));
        }
        EditorCommand::ToggleSettings => {
            world.write_message(session::ToggleSettings);
        }
        EditorCommand::SetShowSettings(show) => {
            world.write_message(session::SetShowSettings(show));
        }
        EditorCommand::SetSettingsSection(section) => {
            world.write_message(session::SetSettingsSection(section));
        }
        EditorCommand::SetGraphicsBackend(backend) => {
            world.write_message(session::SetGraphicsBackend(backend));
        }
        EditorCommand::SetAudioBackend(backend) => {
            world.write_message(session::SetAudioBackend(backend));
        }
        EditorCommand::SetUiScaleMultiplier(scale) => {
            world.write_message(session::SetUiScaleMultiplier(scale));
        }
        EditorCommand::SetKeybindCapture(capture) => {
            world.write_message(session::SetKeybindCapture(capture));
        }
        EditorCommand::SetKeybind {
            action,
            slot,
            chord,
        } => {
            world.write_message(session::SetKeybind {
                action,
                slot,
                chord,
            });
        }
        EditorCommand::ClearKeybindSlot { action, slot } => {
            world.write_message(session::ClearKeybindSlot { action, slot });
        }
        EditorCommand::ResetKeybind(action) => {
            world.write_message(session::ResetKeybind(action));
        }
        EditorCommand::ResetKeybinds => {
            world.write_message(session::ResetKeybinds);
        }
        EditorCommand::CompleteImport => {
            world.write_message(session::CompleteImport);
        }
        EditorCommand::UpdateMusic(metadata) => {
            world.write_message(session::UpdateMusic(metadata));
        }
        EditorCommand::UpdateCreatorMetadata(metadata) => {
            world.write_message(session::UpdateCreatorMetadata(metadata));
        }
        EditorCommand::UpdateSkyColor(color) => {
            world.write_message(session::UpdateSkyColor(color));
        }
        EditorCommand::TriggerAudioImport => {
            world.write_message(session::TriggerAudioImport);
        }
        EditorCommand::CaptureMenuPreviewCamera => {
            world.write_message(session::CaptureMenuPreviewCamera);
        }
        EditorCommand::UseAutoMenuPreviewCamera => {
            world.write_message(session::UseAutoMenuPreviewCamera);
        }
        EditorCommand::SetShiftHeld(held) => {
            world.write_message(input_state::SetShiftHeld(held));
        }
        EditorCommand::SetCtrlHeld(held) => {
            world.write_message(input_state::SetCtrlHeld(held));
        }
        EditorCommand::SetAltHeld(held) => {
            world.write_message(input_state::SetAltHeld(held));
        }
        EditorCommand::SetPanUpHeld(held) => {
            world.write_message(input_state::SetPanUpHeld(held));
        }
        EditorCommand::SetPanDownHeld(held) => {
            world.write_message(input_state::SetPanDownHeld(held));
        }
        EditorCommand::SetPanLeftHeld(held) => {
            world.write_message(input_state::SetPanLeftHeld(held));
        }
        EditorCommand::SetPanRightHeld(held) => {
            world.write_message(input_state::SetPanRightHeld(held));
        }
        EditorCommand::MouseButton { button, pressed } => {
            world.write_message(pointer::MouseButton { button, pressed });
        }
        EditorCommand::PrimaryClick { x, y } => {
            world.write_message(pointer::PrimaryClick { x, y });
        }
        EditorCommand::PointerMoved { x, y } => {
            world.write_message(pointer::PointerMoved { x, y });
        }
        EditorCommand::UpdateCursorFromScreen { x, y } => {
            world.write_message(pointer::UpdateCursorFromScreen { x, y });
        }
        EditorCommand::CameraDrag { dx, dy } => {
            world.write_message(pointer::CameraDrag { dx, dy });
        }
        EditorCommand::TogglePlaceWindow => {
            world.write_message(misc::TogglePlaceWindow);
        }
        EditorCommand::Escape => {
            world.write_message(misc::Escape);
        }
    }
}

/// Add every per-message drain system to a `Schedule`.
///
/// Runtime runs the schedule once per frame, so these systems execute
/// in the order they're added (Bevy schedules are not parallel-by-default
/// for unconstrained systems, so this is deterministic). All systems
/// target the [`CommandHandlerSet`].
pub fn add_command_systems(schedule: &mut Schedule) {
    // Bevy's `IntoScheduleConfigs` only goes up to 16 systems in a flat
    // tuple, so we add each domain's systems in its own `add_systems`
    // call. Every group is tagged with `CommandHandlerSet`, giving the
    // StateSchedule an explicit command boundary for downstream systems.
    schedule
        // Navigation
        .add_systems(
            (
                navigation::handle_turn_right,
                navigation::handle_next_level,
                navigation::handle_prev_level,
                navigation::handle_toggle_editor,
                navigation::handle_game_resume,
                navigation::handle_game_restart_level,
                navigation::handle_game_set_practice_mode,
                navigation::handle_game_set_practice_checkpoint,
                navigation::handle_game_remove_practice_checkpoint,
                navigation::handle_game_quit_to_menu,
            )
                .after(pointer::handle_mouse_button)
                .in_set(CommandHandlerSet),
        )
        // Auth
        .add_systems(
            (
                auth::handle_auth_submit_sign_in,
                auth::handle_auth_sign_out,
                auth::handle_auth_open_signup,
                auth::handle_copy_latest_perf_frame,
                auth::handle_resize_surface,
            )
                .in_set(CommandHandlerSet),
        )
        // Editor mode
        .add_systems(
            (
                editor_mode::handle_set_editor_mode,
                editor_mode::handle_set_block_id,
                editor_mode::handle_select_recent_block,
                editor_mode::handle_pick_selected_block,
                editor_mode::handle_pick_block_at,
            )
                .in_set(CommandHandlerSet),
        )
        // Snap
        .add_systems(
            (
                snap::handle_set_snap_to_grid,
                snap::handle_set_snap_step,
                snap::handle_set_snap_rotation,
                snap::handle_set_snap_rotation_step,
            )
                .in_set(CommandHandlerSet),
        )
        // Block ops
        .add_systems(
            (
                block::handle_remove_block,
                block::handle_duplicate_block,
                block::handle_copy_block,
                block::handle_paste_block,
                block::handle_update_selected_block,
            )
                .in_set(CommandHandlerSet),
        )
        // Selection / transform
        .add_systems(
            (
                selection::handle_snap_selection_to_grid,
                selection::handle_focus_camera_target,
                selection::handle_add_transform_trigger,
            )
                .in_set(CommandHandlerSet),
        )
        // Timeline
        .add_systems(
            (
                timeline::handle_toggle_timeline_playback,
                timeline::handle_shift_timeline,
                timeline::handle_set_timeline_time,
                timeline::handle_set_timeline_duration,
                timeline::handle_add_tap,
                timeline::handle_remove_tap,
                timeline::handle_remove_tap_at,
                timeline::handle_set_selected_tap,
                timeline::handle_set_selected_tap_time,
                timeline::handle_clear_taps,
                timeline::handle_set_playback_speed,
                timeline::handle_set_waveform_zoom,
                timeline::handle_set_waveform_scroll,
                timeline::handle_playtest,
            )
                .in_set(CommandHandlerSet),
        )
        // Timing
        .add_systems(
            (
                timing::handle_add_timing_point,
                timing::handle_remove_timing_point,
                timing::handle_set_timing_point_time,
                timing::handle_set_timing_point_bpm,
                timing::handle_set_timing_point_time_signature,
                timing::handle_set_timing_selected,
            )
                .in_set(CommandHandlerSet),
        )
        // BPM
        .add_systems((bpm::handle_bpm_tap, bpm::handle_bpm_tap_reset).in_set(CommandHandlerSet))
        // Spawn
        .add_systems(
            (
                spawn::handle_set_spawn_here,
                spawn::handle_rotate_spawn_direction,
                spawn::handle_rotate_placement_preview,
            )
                .in_set(CommandHandlerSet),
        )
        // History
        .add_systems(
            (
                history::handle_undo,
                history::handle_redo,
                history::handle_history_snapshot,
            )
                .in_set(CommandHandlerSet),
        )
        // Camera
        .add_systems(
            (
                camera::handle_adjust_zoom,
                camera::handle_set_camera_orientation,
                camera::handle_add_camera_trigger,
                camera::handle_add_camera_follow_trigger,
                camera::handle_set_trigger_selected,
            )
                .in_set(CommandHandlerSet),
        )
        // Misc
        .add_systems(
            (
                misc::handle_toggle_hitbox_visualization,
                misc::handle_toggle_perf_overlay,
                misc::handle_toggle_place_window,
                misc::handle_escape,
            )
                .in_set(CommandHandlerSet),
        )
        // Session
        .add_systems(
            (
                session::handle_load_level.before(session::handle_export_level),
                session::handle_rename_level,
                session::handle_export_level,
                session::handle_set_show_metadata,
                session::handle_toggle_settings,
                session::handle_set_show_settings,
                session::handle_set_settings_section,
                session::handle_set_graphics_backend,
                session::handle_set_audio_backend,
                session::handle_set_ui_scale_multiplier,
                session::handle_set_keybind_capture,
                session::handle_set_keybind,
                session::handle_clear_keybind_slot,
                session::handle_reset_keybind,
                session::handle_reset_keybinds,
            )
                .in_set(CommandHandlerSet),
        )
        .add_systems(
            (
                session::handle_complete_import,
                session::handle_update_music,
                session::handle_update_creator_metadata,
                session::handle_update_sky_color,
                session::handle_trigger_audio_import,
                session::handle_capture_menu_preview_camera,
                session::handle_use_auto_menu_preview_camera,
            )
                .in_set(CommandHandlerSet),
        )
        // Input state (modifier / pan held). These write directly to
        // EditorInputRes instead of staging legacy commands.
        .add_systems(
            (
                input_state::handle_set_shift_held,
                input_state::handle_set_ctrl_held,
                input_state::handle_set_alt_held,
                input_state::handle_set_pan_up_held,
                input_state::handle_set_pan_down_held,
                input_state::handle_set_pan_left_held,
                input_state::handle_set_pan_right_held,
            )
                .in_set(CommandHandlerSet),
        )
        // Pointer / drag.
        .add_systems(
            (
                pointer::handle_mouse_button,
                pointer::handle_pointer_moved,
                pointer::handle_update_cursor_from_screen,
                pointer::handle_camera_drag,
                pointer::handle_primary_click,
            )
                .chain()
                .in_set(CommandHandlerSet),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::auth::{
        AuthOpenSignup, AuthSignOut, AuthSubmitSignIn, CopyLatestPerfFrame, ResizeSurface,
    };
    use crate::events::editor_mode::SetEditorMode;
    use crate::events::history::{HistorySnapshot, Redo, Undo};
    use crate::events::navigation::TurnRight;
    use crate::events::snap::SetSnapToGrid;
    use bevy_ecs::message::Messages;
    use eg_types::{
        DEFAULT_MENU_PREVIEW_CAMERA_POSITION, DEFAULT_MENU_PREVIEW_CAMERA_TARGET, EditorMode,
        LevelMetadata, LevelObject, MenuState,
    };

    /// Build a `World` that has the message resources + the per-message
    /// drain schedule.
    fn build_event_world() -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        // All per-message drain systems run in the same schedule, so the
        // helper must register every resource any of them reads. The
        // carve-out handlers (input_state, pointer) reach for
        // `EditorInputRes`, `EditorCameraRes`, and `AppPhaseRes`; the
        // `snap` carve-out reaches for `EditorConfigRes`; the `history`
        // carve-out reaches for `EditorObjectsRes` + the editor mirrors; the `timeline`
        // carve-out reaches for `EditorTimelineRes` + `EditorRuntimeRes`; the `bpm`
        // carve-out reaches for `EditorTimingRes`; the `session`
        // carve-out (SetShowMetadata, ToggleSettings, ...) reaches for
        // `SessionStateRes`; the `timing` carve-out reaches for
        // `EditorTimingRes`; the `spawn` carve-out reaches for
        // `EditorInputRes` (cursor) + `EditorSpawnRes` +
        // `EditorConfigRes`; the `PickBlockAt` carve-out reaches for
        // `RenderUniformsRes` viewport size; the `camera` carve-out reaches for
        // `EditorCameraRes` + `EditorTriggerRes`; the resize carve-out
        // reaches for `RenderResizeRequest`; the perf-frame copy carve-out
        // reaches for `PerfFrameCopyRequest`. The rest of the schedule only
        // reads message buffers and App-world resources.
        world.init_resource::<crate::state::EditorInputRes>();
        world.init_resource::<crate::state::EditorObjectsRes>();
        world.init_resource::<crate::state::EditorCameraRes>();
        world.init_resource::<crate::state::EditorConfigRes>();
        world.init_resource::<crate::state::EditorSpawnRes>();
        world.init_resource::<crate::state::EditorTriggerRes>();
        world.init_resource::<crate::state::EditorTimelineRes>();
        world.init_resource::<crate::state::EditorTimingRes>();
        world.init_resource::<crate::state::EditorRuntimeRes>();
        world.init_resource::<crate::state::EditorHistoryRes>();
        world.init_resource::<crate::state::SessionStateRes>();
        world.init_resource::<eg_game::GameClock>();
        world.init_resource::<eg_game::PendingTurnInputs>();
        world.init_resource::<eg_game::PlayerProgress>();
        world.init_resource::<eg_game::FrameDelta>();
        world.init_resource::<eg_game::FixedStepConfig>();
        world.init_resource::<eg_game::DeathSfxFlag>();
        world.init_resource::<eg_game::SpatialIndex>();
        world.init_resource::<eg_game::GameRuntimeTuning>();
        world.init_resource::<eg_game::LevelMeta>();
        world.init_resource::<crate::state::PerfRes>();
        world.init_resource::<crate::state::PerfFrameCopyRequest>();
        world.init_resource::<crate::state::AppPhaseRes>();
        world.init_resource::<crate::state::RenderUniformsRes>();
        world.init_resource::<crate::state::RenderResizeRequest>();
        world.init_resource::<crate::state::AuthSubmitSignInRequest>();
        world.init_resource::<crate::state::AuthSignOutRequest>();
        world.init_resource::<crate::state::AuthOpenSignupRequest>();
        world.init_resource::<crate::state::GameplayResetRes>();
        world.init_resource::<crate::state::BuiltinLevelCatalogRes>();
        world.init_resource::<crate::state::LevelExportServiceRes>();
        world.init_resource::<crate::state::LevelImportSenderRes>();
        world.init_resource::<crate::state::AudioImportSenderRes>();
        world.init_resource::<crate::state::AudioBackendRes>();
        world.init_resource::<crate::state::EditorAudioCacheRes>();
        world.init_resource::<crate::state::RuntimeAudioPreloadRes>();
        world.init_resource::<crate::state::RuntimeAudioPreloadSenderRes>();
        world.init_resource::<crate::state::WaveformLoadSenderRes>();
        world.insert_resource(crate::state::MenuStateRes::new(MenuState {
            selected_level: 0,
            levels: vec![
                "NavigationLevelA".to_string(),
                "NavigationLevelB".to_string(),
                "NavigationLevelC".to_string(),
            ],
            preview_level_index: None,
            preview_sky_color: LevelMetadata::default().sky_color,
            preview_camera_position: DEFAULT_MENU_PREVIEW_CAMERA_POSITION,
            preview_camera_target: DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
        }));
        register_command_events_world(&mut world);
        let mut schedule = bevy_ecs::schedule::Schedule::new(crate::state::StateSchedule);
        add_command_systems(&mut schedule);
        schedule.add_systems(
            crate::state::gameplay_world::apply_pending_gameplay_reset_system
                .after(crate::events::navigation::handle_turn_right)
                .after(crate::events::navigation::handle_game_restart_level)
                .after(crate::events::timeline::handle_playtest),
        );
        world.add_schedule(schedule);
        world
    }

    #[test]
    fn turn_right_from_menu_starts_app_owned_gameplay() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Playing
        );
        assert_eq!(
            world
                .resource::<crate::state::SessionStateRes>()
                .playing_level_name
                .as_deref(),
            Some("NavigationLevelA")
        );
        assert!(!world.resource::<crate::state::GameplayResetRes>().pending);
    }

    #[test]
    fn turn_right_starts_unstarted_app_gameplay_without_request() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .playing_level_name = Some("NavigationLevelA".to_string());

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert!(world.resource::<eg_game::GameClock>().started);
    }

    #[test]
    fn carve_out_turn_right_in_game_over_returns_to_menu_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::GameOver);

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Menu,
            "GameOver TurnRight must return to menu through AppPhaseRes"
        );
    }

    #[test]
    fn carve_out_turn_right_in_paused_gameplay_is_a_direct_noop() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .game_paused = true;

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Playing,
            "paused gameplay TurnRight must not change the App phase"
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .game_paused,
            "paused gameplay TurnRight must preserve the App-world pause flag"
        );
    }

    #[test]
    fn carve_out_turn_right_in_non_practice_game_over_requests_app_reset() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.resource_mut::<eg_game::GameClock>().game_over = true;

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert!(!world.resource::<crate::state::GameplayResetRes>().pending);
        assert!(!world.resource::<eg_game::GameClock>().game_over);
    }

    #[test]
    fn turn_right_respawns_from_app_practice_checkpoint_without_request() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.resource_mut::<eg_game::GameClock>().game_over = true;
        let checkpoint =
            eg_game::GameState::checkpoint_state_from_parts(eg_game::GameCheckpointParts {
                position: [2.0, 0.0, 3.0],
                direction: eg_types::Direction::Right,
                elapsed_seconds: 3.5,
                speed: 2.0,
                objects: Vec::new(),
                vertical_velocity: 0.25,
                is_grounded: false,
                progress: eg_types::PlayerLevelProgress {
                    progress_percent: 42.0,
                    completed: false,
                    gems_collected: 2,
                    gems_max: 5,
                },
            });
        let mut session = world.resource_mut::<crate::state::SessionStateRes>();
        session.practice_mode_enabled = true;
        session
            .practice_checkpoints
            .push(crate::state::PracticeCheckpoint {
                gameplay: checkpoint,
                trigger_base_objects: None,
            });

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert!(!world.resource::<eg_game::GameClock>().game_over);
        assert_eq!(world.resource::<eg_game::GameClock>().elapsed_seconds, 3.5);
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .practice_mode_enabled
        );
        assert_eq!(
            world
                .resource::<crate::state::SessionStateRes>()
                .practice_checkpoints
                .len(),
            1
        );
    }

    #[test]
    fn turn_right_restarts_practice_without_checkpoint_in_app_world() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.resource_mut::<eg_game::GameClock>().game_over = true;
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .practice_mode_enabled = true;

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert!(!world.resource::<eg_game::GameClock>().game_over);
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .practice_mode_enabled
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .practice_checkpoints
                .is_empty()
        );
    }

    #[test]
    fn carve_out_turn_right_in_active_gameplay_queues_app_turn() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.resource_mut::<eg_game::GameClock>().elapsed_seconds = 1.25;
        world.spawn(eg_game::PlayerBundle {
            player: eg_game::Player,
            position: eg_game::PlayerPosition([0.0, 0.0, 0.0]),
            direction: eg_game::PlayerDirection(eg_types::Direction::Forward),
            speed: eg_game::PlayerSpeed(1.0),
            vertical_velocity: eg_game::VerticalVelocity(0.0),
            grounded: eg_game::Grounded(true),
            trail: eg_game::Trail {
                segments: vec![vec![[0.0, 0.0, 0.0]]],
            },
            spawn_position: eg_game::SpawnPosition([0.0, 0.0, 0.0]),
            spawn_direction: eg_game::SpawnDirection(eg_types::Direction::Forward),
        });

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        let pending = world.resource::<eg_game::PendingTurnInputs>();
        assert_eq!(pending.taps.len(), 1);
        assert_eq!(pending.taps[0].at_seconds, 1.25);
    }

    #[test]
    fn carve_out_turn_right_in_airborne_gameplay_queues_app_turn() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.spawn(eg_game::PlayerBundle {
            player: eg_game::Player,
            position: eg_game::PlayerPosition([0.0, 0.0, 0.0]),
            direction: eg_game::PlayerDirection(eg_types::Direction::Forward),
            speed: eg_game::PlayerSpeed(1.0),
            vertical_velocity: eg_game::VerticalVelocity(1.0),
            grounded: eg_game::Grounded(false),
            trail: eg_game::Trail {
                segments: vec![vec![[0.0, 0.0, 0.0]]],
            },
            spawn_position: eg_game::SpawnPosition([0.0, 0.0, 0.0]),
            spawn_direction: eg_game::SpawnDirection(eg_types::Direction::Forward),
        });

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(world.resource::<eg_game::PendingTurnInputs>().taps.len(), 1);
    }

    #[test]
    fn carve_out_turn_right_in_editor_places_block_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorInputRes>()
            .0
            .cursor = [2.0, 0.0, 3.0];

        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);

        let objects = world.resource::<crate::state::EditorObjectsRes>();
        assert_eq!(objects.objects.len(), 1);
        assert_eq!(objects.objects[0].position, [2.0, 0.0, 3.0]);
        let input = &world.resource::<crate::state::EditorInputRes>().0;
        assert_eq!(input.selected_block_index, Some(0));
        assert_eq!(input.selected_block_indices, vec![0]);
        assert_eq!(input.mode, EditorMode::Scale);
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }

    #[test]
    fn toggle_editor_from_menu_enters_app_owned_editor() {
        let mut world = build_event_world();
        world.write_message(navigation::ToggleEditor);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Editor
        );
        assert_eq!(
            world
                .resource::<crate::state::SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("NavigationLevelA")
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Null
        );
    }

    #[test]
    fn carve_out_toggle_editor_from_editor_returns_to_menu_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world.write_message(navigation::ToggleEditor);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Menu
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .audio_stop_pending
        );
    }

    #[test]
    fn toggle_editor_returns_from_playtest_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .playtesting_editor = true;

        world.write_message(navigation::ToggleEditor);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Editor
        );
        let session = world.resource::<crate::state::SessionStateRes>();
        assert!(!session.playtesting_editor);
        assert!(!session.game_paused);
        assert!(session.audio_stop_pending);
    }

    #[test]
    fn carve_out_resize_surface_writes_render_resize_request() {
        let mut world = build_event_world();
        world.write_message(ResizeSurface {
            width: 1920,
            height: 1080,
        });
        world.run_schedule(crate::state::StateSchedule);
        let resize = world.resource::<crate::state::RenderResizeRequest>();
        assert!(resize.pending);
        assert_eq!(resize.size.width, 1920);
        assert_eq!(resize.size.height, 1080);
    }

    #[test]
    fn carve_out_copy_latest_perf_frame_writes_copy_request() {
        let mut world = build_event_world();
        world.write_message(CopyLatestPerfFrame);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::PerfFrameCopyRequest>()
                .pending,
            "CopyLatestPerfFrame handler must request an App-world perf-frame copy"
        );
    }

    /// `AuthSubmitSignIn` is a carve-out: the handler writes
    /// `AuthSubmitSignInRequest.pending = true`. Pin the request
    /// is set and no legacy command is staged.
    #[test]
    fn carve_out_auth_submit_sign_in_writes_request() {
        let mut world = build_event_world();
        world.write_message(AuthSubmitSignIn);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::AuthSubmitSignInRequest>()
                .pending,
            "AuthSubmitSignIn handler must request an App-world sign-in"
        );
    }

    /// `AuthSignOut` is a carve-out: the handler writes
    /// `AuthSignOutRequest.pending = true`. Pin the request is set
    /// and no legacy command is staged.
    #[test]
    fn carve_out_auth_sign_out_writes_request() {
        let mut world = build_event_world();
        world.write_message(AuthSignOut);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world.resource::<crate::state::AuthSignOutRequest>().pending,
            "AuthSignOut handler must request an App-world sign-out"
        );
    }

    /// `AuthOpenSignup` is a carve-out: the handler writes
    /// `AuthOpenSignupRequest.pending = true`. Pin the request is
    /// set and no legacy command is staged.
    #[test]
    fn carve_out_auth_open_signup_writes_request() {
        let mut world = build_event_world();
        world.write_message(AuthOpenSignup);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::AuthOpenSignupRequest>()
                .pending,
            "AuthOpenSignup handler must request an App-world signup-page open"
        );
    }

    // --- Phase 3.48 / Navigation request carve-out tests ---
    //
    // The navigation handlers mutate App resources directly where the
    // lifecycle side effects have moved; remaining menu/playtest branches
    // still write dedicated request resources instead of staging
    // `AppCommand`s through the legacy command bridge.

    use crate::events::navigation::{
        GameQuitToMenu, GameRemovePracticeCheckpoint, GameRestartLevel, GameSetPracticeCheckpoint,
    };

    /// Quit-to-menu is ignored outside active real gameplay.
    #[test]
    fn game_quit_to_menu_is_noop_outside_playing() {
        let mut world = build_event_world();
        world.write_message(GameQuitToMenu);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Menu
        );
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .audio_stop_pending
        );
    }

    /// Real gameplay quit-to-menu mutates App phase/session/menu resources
    /// directly and leaves only the one-shot external-audio intent.
    #[test]
    fn carve_out_game_quit_to_menu_mutates_app_resources() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .playtesting_editor = false;

        world.write_message(GameQuitToMenu);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Menu
        );
        let session = world.resource::<crate::state::SessionStateRes>();
        assert!(!session.game_paused);
        assert!(!session.practice_mode_enabled);
        assert!(session.practice_checkpoints.is_empty());
        assert!(session.audio_stop_pending);
    }

    /// Restart is ignored outside active real gameplay.
    #[test]
    fn game_restart_level_is_noop_outside_playing() {
        let mut world = build_event_world();
        world.write_message(GameRestartLevel);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            !world.resource::<crate::state::GameplayResetRes>().pending,
            "menu-phase restart must not request an App-world gameplay reset"
        );
    }

    /// Real gameplay restart is applied by the App-world reset system and
    /// does not leave a Runtime request behind.
    #[test]
    fn carve_out_game_restart_level_resets_app_gameplay_world() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        {
            let mut session = world.resource_mut::<crate::state::SessionStateRes>();
            session.game_paused = true;
            session.practice_mode_enabled = true;
        }
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.init_resource::<eg_game::LevelMeta>();
        {
            let mut meta = world.resource_mut::<eg_game::LevelMeta>();
            meta.objects = vec![LevelObject::new("core/stone")];
            meta.spawn_position = [2.0, 0.0, 3.0];
            meta.spawn_direction = eg_types::Direction::Right;
            meta.level_duration_seconds = 9.0;
        }
        world.spawn(eg_game::PlayerBundle {
            player: eg_game::Player,
            position: eg_game::PlayerPosition([99.0, 0.0, 99.0]),
            direction: eg_game::PlayerDirection(eg_types::Direction::Forward),
            speed: eg_game::PlayerSpeed(4.0),
            vertical_velocity: eg_game::VerticalVelocity(-2.0),
            grounded: eg_game::Grounded(false),
            trail: eg_game::Trail {
                segments: vec![vec![[99.0, 0.0, 99.0]]],
            },
            spawn_position: eg_game::SpawnPosition([99.0, 0.0, 99.0]),
            spawn_direction: eg_game::SpawnDirection(eg_types::Direction::Forward),
        });

        world.write_message(GameRestartLevel);
        world.run_schedule(crate::state::StateSchedule);

        let player = {
            let mut query = world.query::<(
                &eg_game::PlayerPosition,
                &eg_game::PlayerDirection,
                &eg_game::Grounded,
            )>();
            query
                .iter(&world)
                .next()
                .map(|(position, direction, grounded)| (position.0, direction.0, grounded.0))
                .expect("restart must respawn the App-world player")
        };
        assert_eq!(player.0, [2.0, 0.0, 3.0]);
        assert_eq!(player.1, eg_types::Direction::Right);
        assert!(player.2);
        assert!(!world.resource::<eg_game::GameClock>().started);
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .game_paused
        );
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .practice_mode_enabled
        );
    }

    /// `GameSetPracticeCheckpoint` captures the App-world gameplay state
    /// directly into `SessionStateRes` without a request bridge.
    #[test]
    fn carve_out_game_set_practice_checkpoint_captures_app_world_state() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .practice_mode_enabled = true;
        world.resource_mut::<eg_game::GameClock>().started = true;
        world.spawn(eg_game::PlayerBundle {
            player: eg_game::Player,
            position: eg_game::PlayerPosition([1.0, 0.0, 2.0]),
            direction: eg_game::PlayerDirection(eg_types::Direction::Forward),
            speed: eg_game::PlayerSpeed(4.0),
            vertical_velocity: eg_game::VerticalVelocity(0.0),
            grounded: eg_game::Grounded(true),
            trail: eg_game::Trail {
                segments: vec![vec![[1.0, 0.0, 2.0]]],
            },
            spawn_position: eg_game::SpawnPosition([1.0, 0.0, 2.0]),
            spawn_direction: eg_game::SpawnDirection(eg_types::Direction::Forward),
        });
        world.write_message(GameSetPracticeCheckpoint);
        world.run_schedule(crate::state::StateSchedule);

        let checkpoints = &world
            .resource::<crate::state::SessionStateRes>()
            .practice_checkpoints;
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].gameplay.position, [1.0, 0.0, 2.0]);
    }

    /// `GameRemovePracticeCheckpoint` directly removes the newest App-world
    /// checkpoint and marks the temporary presentation sync dirty.
    #[test]
    fn carve_out_game_remove_practice_checkpoint_mutates_session_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Playing);
        {
            let mut session = world.resource_mut::<crate::state::SessionStateRes>();
            session.practice_mode_enabled = true;
            session
                .practice_checkpoints
                .push(crate::state::PracticeCheckpoint {
                    gameplay: eg_game::GameState::new().checkpoint_state(),
                    trigger_base_objects: None,
                });
        }

        world.write_message(GameRemovePracticeCheckpoint);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .practice_checkpoints
                .is_empty(),
            "GameRemovePracticeCheckpoint must remove the newest App-world checkpoint"
        );
    }

    #[test]
    fn history_snapshot_drains_without_legacy_command_bridge() {
        let mut world = build_event_world();
        world.write_message(HistorySnapshot {
            label: "drag_release".to_string(),
        });
        world.run_schedule(crate::state::StateSchedule);
    }

    #[test]
    fn carve_out_undo_redo_mutates_editor_history_resources() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);

        let current_object = LevelObject {
            id: "current".to_string(),
            position: [8.0, 0.0, 8.0],
            ..LevelObject::default()
        };
        let undo_object = LevelObject {
            id: "undo".to_string(),
            position: [3.0, 0.0, 4.0],
            ..LevelObject::default()
        };
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![current_object.clone()];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.cursor = [8.0, 0.0, 8.0];
        }
        world
            .resource_mut::<crate::state::EditorHistoryRes>()
            .0
            .undo
            .push(crate::state::EditorHistorySnapshot {
                objects: vec![undo_object.clone()],
                selected_block_index: Some(0),
                selected_block_indices: vec![0],
                cursor: [3.0, 0.0, 4.0],
                selected_block_id: eg_blocks::BlockId::new("core/stone"),
                selected_block_rotation_degrees: [0.0, 90.0, 0.0],
                recent_block_ids: vec![eg_blocks::BlockId::new("core/stone")],
                spawn: eg_types::SpawnMetadata::default(),
                timeline_time_seconds: 0.25,
                timeline_duration_seconds: 2.0,
                tap_times: vec![0.25],
                tap_indicator_positions: vec![[3.0, 0.0, 4.0]],
                selected_tap_index: Some(0),
                timing_points: Vec::new(),
                selected_trigger_index: None,
            });

        world.write_message(Undo);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorObjectsRes>().objects,
            vec![undo_object.clone()]
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [3.0, 0.0, 4.0]
        );
        assert!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .is_empty()
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .redo
                .len(),
            1
        );

        world.write_message(Redo);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorObjectsRes>().objects,
            vec![current_object]
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [8.0, 0.0, 8.0]
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
        assert!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .redo
                .is_empty()
        );
    }

    #[test]
    fn carve_out_remove_block_mutates_editor_object_resources() {
        use crate::events::block::RemoveBlock;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![
            LevelObject {
                id: "removed".to_string(),
                position: [0.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                id: "kept".to_string(),
                position: [2.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }

        world.write_message(RemoveBlock);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "kept");
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }

    #[test]
    fn carve_out_copy_duplicate_and_paste_use_app_clipboard() {
        use crate::events::block::{CopyBlock, DuplicateBlock, PasteBlock};

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            id: "source".to_string(),
            position: [1.0, 0.0, 1.0],
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.cursor = [6.0, 0.0, 6.0];
        }

        world.write_message(CopyBlock);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world
                .resource::<crate::state::EditorRuntimeRes>()
                .0
                .interaction
                .clipboard
                .is_some()
        );

        world.write_message(DuplicateBlock);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorObjectsRes>()
                .objects
                .len(),
            2
        );

        world.write_message(PasteBlock);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects.len(), 3);
        assert_eq!(objects[2].position, [6.0, 0.0, 6.0]);
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            2
        );
    }

    #[test]
    fn carve_out_update_selected_block_mutates_object_resource_once() {
        use crate::events::block::UpdateSelectedBlock;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            id: "selected".to_string(),
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }
        {
            let mut config = world.resource_mut::<crate::state::EditorConfigRes>();
            config.0.snap_to_grid = false;
            config.0.snap_rotation = false;
        }

        world.write_message(UpdateSelectedBlock(LevelObject {
            id: "selected".to_string(),
            position: [1.5, -2.0, 3.0],
            size: [0.1, 2.0, 3.0],
            rotation: [4.0, 5.0, 6.0],
            color_tint: [-1.0, 0.5, 2.0],
            block_id: eg_blocks::BlockId::new("core/lava"),
            ..LevelObject::default()
        }));
        world.run_schedule(crate::state::StateSchedule);

        let object = &world.resource::<crate::state::EditorObjectsRes>().objects[0];
        assert_eq!(object.position, [1.5, 0.0, 3.0]);
        assert_eq!(object.size, [0.25, 2.0, 3.0]);
        assert_eq!(object.rotation, [4.0, 5.0, 6.0]);
        assert_eq!(object.color_tint, [0.0, 0.5, 1.0]);
        assert_eq!(object.block_id, eg_blocks::BlockId::new("core/lava"));
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }

    #[test]
    fn carve_out_snap_selection_to_grid_mutates_selected_block_resources() {
        use crate::events::selection::SnapSelectionToGrid;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .snap_step = 1.0;
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![
            LevelObject {
                id: "first".to_string(),
                position: [0.2, 0.0, 0.7],
                ..LevelObject::default()
            },
            LevelObject {
                id: "second".to_string(),
                position: [3.1, 0.0, 3.6],
                ..LevelObject::default()
            },
        ];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0, 1];
            input.0.cursor = [9.0, 0.0, 9.0];
        }

        world.write_message(SnapSelectionToGrid);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects[0].position, [0.0, 0.0, 1.0]);
        assert_eq!(objects[1].position, [3.0, 0.0, 4.0]);
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [0.0, 0.0, 1.0],
            "block snap must move the cursor to the primary selected block"
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }

    #[test]
    fn carve_out_snap_selection_to_grid_mutates_selected_tap_resources() {
        use crate::events::selection::SnapSelectionToGrid;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .snap_step = 1.0;
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 8.0;
            timeline.0.clock.time_seconds = 1.5;
            timeline.0.taps.tap_times = vec![0.5, 1.5];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.2, 0.0, 1.7]];
            timeline.0.taps.selected_index = Some(1);
        }

        world.write_message(SnapSelectionToGrid);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(
            timeline.taps.tap_indicator_positions[timeline.taps.selected_index.unwrap()],
            [1.0, 0.0, 2.0]
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.0],
            "tap snap must move the cursor to the snapped selected tap"
        );
        assert!(
            timeline
                .taps
                .tap_times
                .iter()
                .all(|time_seconds| time_seconds.is_finite())
        );
    }

    #[test]
    fn carve_out_focus_camera_target_mutates_editor_camera_resource() {
        use crate::events::selection::FocusCameraTarget;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![
            LevelObject {
                position: [10.0, 2.0, 30.0],
                size: [2.0, 4.0, 6.0],
                ..LevelObject::default()
            },
            LevelObject {
                position: [50.0, 4.0, 70.0],
                size: [4.0, 2.0, 8.0],
                ..LevelObject::new("core/grass")
            },
        ];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
            input.0.hovered_block_index = Some(1);
        }

        world.write_message(FocusCameraTarget);
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<crate::state::EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [11.0, 33.0]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 4.0, epsilon = 1e-6);
    }

    #[test]
    fn carve_out_focus_camera_target_prefers_selected_tap_in_tapping_mode() {
        use crate::events::selection::FocusCameraTarget;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            position: [10.0, 2.0, 30.0],
            size: [2.0, 4.0, 6.0],
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.mode = EditorMode::Tapping;
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.taps.tap_times = vec![1.25];
            timeline.0.taps.tap_indicator_positions = vec![[5.0, 1.0, 6.0]];
            timeline.0.taps.selected_index = Some(0);
        }

        world.write_message(FocusCameraTarget);
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<crate::state::EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [5.5, 6.5]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn carve_out_focus_camera_target_uses_camera_track_fallback() {
        use crate::events::selection::FocusCameraTarget;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 0.5;
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            trigger: Some(eg_types::TimedTrigger {
                time_seconds: 0.0,
                target: eg_types::TriggerTarget::Camera,
                camera: Some(eg_types::CameraTrigger {
                    mode: eg_types::CameraMode::Static,
                    target_position: [3.0, 4.0, 5.0],
                    yaw: 0.4,
                    pitch: 0.2,
                    ..eg_types::CameraTrigger::default()
                }),
                ..eg_types::TimedTrigger::default()
            }),
            ..LevelObject::new(eg_types::CAMERA_TRIGGER_BLOCK_ID)
        }];

        world.write_message(FocusCameraTarget);
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<crate::state::EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [3.0, 5.0]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 4.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_yaw, 0.4, epsilon = 1e-5);
        approx::assert_abs_diff_eq!(camera.editor_pitch, 0.2, epsilon = 1e-5);
    }

    #[test]
    fn carve_out_add_transform_trigger_mutates_editor_object_resources() {
        use crate::events::selection::AddTransformTrigger;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 2.5;
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .editor_show_place_window = true;
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![
            LevelObject {
                id: "object_0".to_string(),
                position: [0.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                id: "object_4".to_string(),
                position: [4.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                id: "existing_trigger".to_string(),
                trigger: Some(eg_types::TimedTrigger {
                    time_seconds: 2.5,
                    target: eg_types::TriggerTarget::Objects {
                        object_ids: vec!["object_0".to_string(), "object_8".to_string()],
                    },
                    transform: true,
                    ..eg_types::TimedTrigger::default()
                }),
                ..LevelObject::new(eg_types::TRANSFORM_TRIGGER_BLOCK_ID)
            },
        ];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0, 1];
            input.0.hovered_block_index = Some(1);
        }

        world.write_message(AddTransformTrigger);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects.len(), 5);
        assert_eq!(objects[3].block_id, eg_types::TRANSFORM_TRIGGER_BLOCK_ID);
        assert_eq!(objects[3].position, [0.0, 0.0, 0.0]);
        assert_eq!(objects[4].block_id, eg_types::TRANSFORM_TRIGGER_BLOCK_ID);
        assert_eq!(objects[4].position, [4.0, 0.0, 0.0]);
        assert_eq!(
            objects[2].trigger.as_ref().unwrap().target,
            eg_types::TriggerTarget::Objects {
                object_ids: vec!["object_8".to_string()],
            }
        );
        for (object, expected_id) in objects[3..].iter().zip(["object_0", "object_4"]) {
            let trigger = object.trigger.as_ref().expect("new transform trigger");
            approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
            approx::assert_abs_diff_eq!(trigger.duration_seconds, 1.0, epsilon = 1e-6);
            assert_eq!(trigger.easing, eg_types::EasingDirection::EaseInOut);
            assert!(trigger.transform);
            assert_eq!(
                trigger.target,
                eg_types::TriggerTarget::Objects {
                    object_ids: vec![expected_id.to_string()],
                }
            );
        }
        assert_eq!(
            world
                .resource::<crate::state::EditorInputRes>()
                .0
                .selected_block_indices,
            vec![3, 4]
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorInputRes>()
                .0
                .hovered_block_index,
            Some(3)
        );
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .editor_show_place_window
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }

    #[test]
    fn schedule_drains_set_editor_mode_into_editor_command() {
        // `SetEditorMode` is a Phase 3.13 carve-out: the handler
        // mutates `EditorInputRes.0.mode` directly.
        let mut world = build_event_world();
        world.write_message(SetEditorMode(EditorMode::Tapping));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Tapping,
            "SetEditorMode handler must mutate EditorInputRes.mode"
        );
    }

    #[test]
    fn set_snap_to_grid_updates_settings_and_resnaps_selected_object() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        {
            let mut config = world.resource_mut::<crate::state::EditorConfigRes>();
            config.0.snap_to_grid = false;
            config.0.snap_step = 0.5;
        }
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .app_settings
            .editor_snap_to_grid = false;
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            position: [0.26, 0.2, 0.74],
            size: [0.2, 0.7, 0.76],
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }

        world.write_message(SetSnapToGrid(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .snap_to_grid
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .app_settings
                .editor_snap_to_grid
        );
        let object = &world.resource::<crate::state::EditorObjectsRes>().objects[0];
        assert_eq!(object.position, [0.5, 0.0, 0.5]);
        assert_eq!(object.size, [0.5, 0.5, 1.0]);
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            object.position
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1
        );
    }
    #[test]
    fn multiple_turn_right_messages_are_handled_without_legacy_request() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Playing
        );
    }

    #[test]
    fn empty_message_buffer_is_noop() {
        let mut world = build_event_world();
        world.run_schedule(crate::state::StateSchedule);
    }

    #[test]
    fn mixed_message_types_write_direct_resources() {
        // Menu `TurnRight` and `SetSnapToGrid` both resolve in App-world
        // handlers without staging a legacy command.
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(SetSnapToGrid(true));
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .snap_to_grid
        );
    }

    /// Each `Messages<T>` is a `Default` + `Resource`, so registering the
    /// resource twice (e.g. by `register_command_events_world` on a world
    /// that already has it) should not panic — `init_resource` returns
    /// the existing resource in that case.
    #[test]
    fn register_command_events_world_is_idempotent() {
        let mut world = build_event_world();
        register_command_events_world(&mut world);
        // No panic = pass.
        assert!(world.contains_resource::<Messages<TurnRight>>());
    }

    // --- Carve-out pattern: handlers that mutate App-world resources
    // directly. Pin the contract: the message reaches the schedule
    // and the handler mutates the resource.

    use crate::events::input_state::{SetPanRightHeld, SetShiftHeld};
    use crate::state::EditorInputRes;

    /// `SetShiftHeld` is a carve-out: the handler writes the held state
    /// directly to `EditorInputRes` instead of staging a legacy
    /// `EditorCommand`. Pin the resource is updated and no legacy
    /// command is queued.
    #[test]
    fn carve_out_set_shift_held_mutates_editor_input_resource() {
        let mut world = build_event_world();

        // Start from a known state.
        world.resource_mut::<EditorInputRes>().0.shift_held = false;
        assert!(!world.resource::<EditorInputRes>().0.shift_held);

        world.write_message(SetShiftHeld(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world.resource::<EditorInputRes>().0.shift_held,
            "SetShiftHeld handler must mutate EditorInputRes"
        );
        // Carve-out: the message must not be staged as a legacy command.
    }

    /// Repeated messages of the same carve-out type should leave the
    /// resource in the last-written state. This catches accidental
    /// reset-to-default logic and confirms Bevy's `MessageReader` reads
    /// the full buffer (not just the first event).
    #[test]
    fn carve_out_last_written_state_wins() {
        let mut world = build_event_world();
        world.resource_mut::<EditorInputRes>().0.pan_right_held = false;

        world.write_message(SetPanRightHeld(true));
        world.write_message(SetPanRightHeld(false));
        world.write_message(SetPanRightHeld(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(world.resource::<EditorInputRes>().0.pan_right_held);
    }

    /// `SetBlockId` is a carve-out: the handler writes the selected
    /// block id directly to `EditorConfigRes.0.selected_block_id`
    /// instead of staging a legacy `EditorCommand::SetBlockId`. Pin
    /// the resource is updated and no legacy command is queued. The
    /// legacy `State` sync (and the recent-blocks side effect) is
    /// exercised end-to-end in `eg-app` runtime tests.
    #[test]
    fn carve_out_set_block_id_mutates_editor_config_resource() {
        use crate::events::editor_mode::SetBlockId;

        let mut world = build_event_world();
        let initial = world
            .resource::<crate::state::EditorConfigRes>()
            .0
            .selected_block_id
            .clone();
        let new_id = eg_blocks::BlockId::new("core/lava");
        assert_ne!(initial, new_id, "test precondition: new id must differ");

        world.write_message(SetBlockId(new_id.clone()));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            new_id,
            "SetBlockId handler must mutate EditorConfigRes.selected_block_id"
        );
    }

    // --- BPM carve-out pattern ---

    use crate::events::bpm::{BpmTap, BpmTapReset};
    use crate::state::EditorTimingRes;

    /// `BpmTap` is a carve-out: the handler reads wall-clock time,
    /// appends to `EditorTimingRes.0.bpm_tap_times`, recomputes the
    /// running BPM, and writes the resource directly. Pin the
    /// resource is updated and no legacy command is queued.
    #[test]
    fn carve_out_bpm_tap_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        // Start from a known empty state.
        world
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_times
            .clear();
        world.resource_mut::<EditorTimingRes>().0.bpm_tap_result = None;
        assert!(
            world
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .is_empty(),
            "test precondition: empty tap history"
        );

        world.write_message(BpmTap);
        world.run_schedule(crate::state::StateSchedule);

        // First tap: list grows by 1, no result yet (needs >= 2 taps).
        assert_eq!(
            world.resource::<EditorTimingRes>().0.bpm_tap_times.len(),
            1,
            "BpmTap handler must append to EditorTimingRes.bpm_tap_times"
        );
        assert!(
            world
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_none(),
            "BpmTap with a single tap must not produce a result"
        );
        // Carve-out: the message must not be staged as a legacy command.
    }

    /// `BpmTapReset` is a carve-out: the handler clears the
    /// tap-history list and result on the resource directly.
    #[test]
    fn carve_out_bpm_tap_reset_clears_editor_timing_resource() {
        let mut world = build_event_world();
        // Pre-populate the resource with fake tap history so we can
        // verify the reset actually clears it.
        let mut res = world.resource_mut::<EditorTimingRes>();
        res.0.bpm_tap_times = vec![1.0, 2.0, 3.0];
        res.0.bpm_tap_result = Some(120.0);
        drop(res);

        world.write_message(BpmTapReset);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .is_empty(),
            "BpmTapReset must clear bpm_tap_times"
        );
        assert!(
            world
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_none(),
            "BpmTapReset must clear bpm_tap_result"
        );
    }

    // --- Session carve-out pattern ---
    //
    // `RenameLevel`, `SetShowMetadata`, `SetShowSettings`, `ToggleSettings`,
    // `SetSettingsSection`, `SetGraphicsBackend`, `SetUiScaleMultiplier`,
    // `UpdateMusic`, `UpdateCreatorMetadata`, `UpdateSkyColor`,
    // `SetKeybindCapture`, the keybind mapping messages, and the
    // menu-preview camera messages are carve-outs: their handlers
    // mutate `SessionStateRes` directly. Pin the resource is updated
    // and no legacy command is queued.

    use crate::events::session::{
        CaptureMenuPreviewCamera, ClearKeybindSlot, RenameLevel, ResetKeybind, ResetKeybinds,
        SetGraphicsBackend, SetKeybind, SetKeybindCapture, SetSettingsSection, SetShowMetadata,
        SetShowSettings, SetUiScaleMultiplier, ToggleSettings, UpdateCreatorMetadata, UpdateMusic,
        UpdateSkyColor, UseAutoMenuPreviewCamera,
    };
    use crate::state::{AppPhaseRes, EditorCameraRes, SessionStateRes};
    use eg_types::{
        AppPhase, BasicLevelMetadata, KeyChord, KeybindBinding, LevelPreviewCameraMetadata,
        MusicMetadata, SettingsSection, default_essential_keybinds,
    };

    #[test]
    fn carve_out_rename_level_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<SessionStateRes>().editor_level_name = None;

        world.write_message(RenameLevel("AppWorldName".to_string()));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("AppWorldName"),
            "RenameLevel handler must mutate SessionStateRes.editor_level_name"
        );
    }

    #[test]
    fn carve_out_set_show_metadata_mutates_session_resource() {
        let mut world = build_event_world();
        // Start from a known state.
        world.resource_mut::<SessionStateRes>().editor_show_metadata = false;
        assert!(!world.resource::<SessionStateRes>().editor_show_metadata);

        world.write_message(SetShowMetadata(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world.resource::<SessionStateRes>().editor_show_metadata,
            "SetShowMetadata handler must mutate SessionStateRes.editor_show_metadata"
        );
    }

    #[test]
    fn carve_out_set_show_settings_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<SessionStateRes>().editor_show_settings = false;

        world.write_message(SetShowSettings(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(world.resource::<SessionStateRes>().editor_show_settings);
    }

    #[test]
    fn carve_out_toggle_settings_flips_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<SessionStateRes>().editor_show_settings = false;

        // First toggle: false -> true.
        world.write_message(ToggleSettings);
        world.run_schedule(crate::state::StateSchedule);
        assert!(world.resource::<SessionStateRes>().editor_show_settings);

        // Second toggle: true -> false.
        world.write_message(ToggleSettings);
        world.run_schedule(crate::state::StateSchedule);
        assert!(!world.resource::<SessionStateRes>().editor_show_settings);
    }

    #[test]
    fn carve_out_set_settings_section_mutates_session_resource() {
        let mut world = build_event_world();
        let new_section = SettingsSection::Keybinds;
        world
            .resource_mut::<SessionStateRes>()
            .editor_settings_section = SettingsSection::Backends;

        world.write_message(SetSettingsSection(new_section));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<SessionStateRes>().editor_settings_section,
            new_section
        );
    }

    #[test]
    fn carve_out_set_keybind_capture_mutates_session_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<SessionStateRes>()
            .editor_keybind_capture_action = None;

        let capture = Some(("editor.place".to_string(), 0));
        world.write_message(SetKeybindCapture(capture.clone()));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_keybind_capture_action,
            capture
        );
    }

    #[test]
    fn carve_out_set_ui_scale_multiplier_mutates_session_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<SessionStateRes>()
            .app_settings
            .ui_scale_multiplier = 1.0;

        world.write_message(SetUiScaleMultiplier(8.0));
        world.run_schedule(crate::state::StateSchedule);

        approx::assert_abs_diff_eq!(
            world
                .resource::<SessionStateRes>()
                .app_settings
                .ui_scale_multiplier,
            3.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_graphics_backend_mutates_session_resource() {
        let mut world = build_event_world();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.graphics_backend_locked = false;
            session.settings_restart_required = false;
            session.app_settings.graphics_backend = "Auto".to_string();
        }

        world.write_message(SetGraphicsBackend("ManualBackend".to_string()));
        world.run_schedule(crate::state::StateSchedule);

        let session = world.resource::<SessionStateRes>();
        assert_eq!(session.app_settings.graphics_backend, "ManualBackend");
        assert!(
            session.settings_restart_required,
            "SetGraphicsBackend must mark restart required when it changes the setting"
        );
    }

    #[test]
    fn carve_out_set_graphics_backend_respects_locked_resource() {
        let mut world = build_event_world();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.graphics_backend_locked = true;
            session.settings_restart_required = false;
            session.app_settings.graphics_backend = "LockedBackend".to_string();
        }

        world.write_message(SetGraphicsBackend("IgnoredBackend".to_string()));
        world.run_schedule(crate::state::StateSchedule);

        let session = world.resource::<SessionStateRes>();
        assert_eq!(session.app_settings.graphics_backend, "LockedBackend");
        assert!(
            !session.settings_restart_required,
            "locked SetGraphicsBackend must not mark restart required"
        );
    }

    #[test]
    fn carve_out_set_keybind_mutates_session_resource() {
        let mut world = build_event_world();
        let chord = KeyChord::new("K", true, false, false);
        world.write_message(SetKeybind {
            action: "copy".to_string(),
            slot: 0,
            chord: chord.clone(),
        });
        world.run_schedule(crate::state::StateSchedule);

        let session = world.resource::<SessionStateRes>();
        assert!(
            session
                .app_settings
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == chord),
            "SetKeybind must write the normalized chord into SessionStateRes.app_settings"
        );
    }

    #[test]
    fn carve_out_clear_keybind_slot_mutates_session_resource() {
        let mut world = build_event_world();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.app_settings.keybinds = vec![KeybindBinding {
                action: "copy".to_string(),
                chord: KeyChord::new("k", true, false, false),
            }];
        }

        world.write_message(ClearKeybindSlot {
            action: "copy".to_string(),
            slot: 0,
        });
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<SessionStateRes>()
                .app_settings
                .keybinds_for_action("copy")
                .is_empty(),
            "ClearKeybindSlot must remove the requested slot from SessionStateRes.app_settings"
        );
    }

    #[test]
    fn carve_out_reset_keybind_mutates_session_resource() {
        let mut world = build_event_world();
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.app_settings.keybinds = vec![KeybindBinding {
                action: "copy".to_string(),
                chord: KeyChord::new("k", true, false, false),
            }];
        }

        world.write_message(ResetKeybind("copy".to_string()));
        world.run_schedule(crate::state::StateSchedule);

        let expected_defaults: Vec<_> = default_essential_keybinds()
            .into_iter()
            .filter(|binding| binding.action == "copy")
            .collect();
        let session = world.resource::<SessionStateRes>();
        let actual: Vec<_> = session
            .app_settings
            .keybinds
            .iter()
            .filter(|binding| binding.action == "copy")
            .cloned()
            .collect();
        assert_eq!(
            actual, expected_defaults,
            "ResetKeybind must restore default bindings for the requested action"
        );
    }

    #[test]
    fn carve_out_reset_keybinds_mutates_session_resource() {
        let mut world = build_event_world();
        let custom = KeybindBinding {
            action: "custom.action".to_string(),
            chord: KeyChord::new("x", false, false, false),
        };
        {
            let mut session = world.resource_mut::<SessionStateRes>();
            session.app_settings.keybinds = vec![
                KeybindBinding {
                    action: "copy".to_string(),
                    chord: KeyChord::new("k", true, false, false),
                },
                custom.clone(),
            ];
        }

        world.write_message(ResetKeybinds);
        world.run_schedule(crate::state::StateSchedule);

        let session = world.resource::<SessionStateRes>();
        assert!(
            session.app_settings.keybinds.contains(&custom),
            "ResetKeybinds must preserve non-essential custom keybinds"
        );
        for default_binding in default_essential_keybinds() {
            assert!(
                session.app_settings.keybinds.contains(&default_binding),
                "ResetKeybinds must restore essential default {default_binding:?}"
            );
        }
    }

    #[test]
    fn carve_out_update_creator_metadata_mutates_session_resource() {
        let mut world = build_event_world();
        let metadata = BasicLevelMetadata {
            name: "Metadata Carveout".to_string(),
            creator: Some("codex".to_string()),
            description: Some("direct session resource write".to_string()),
            stars: 6.25,
            tags: vec!["ecs".to_string(), "metadata".to_string()],
        };

        world.write_message(UpdateCreatorMetadata(metadata.clone()));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            &world.resource::<SessionStateRes>().editor_creator_metadata,
            &metadata,
            "UpdateCreatorMetadata handler must mutate SessionStateRes.editor_creator_metadata"
        );
    }

    #[test]
    fn carve_out_update_music_mutates_session_resource() {
        let mut world = build_event_world();
        let metadata = MusicMetadata {
            title: Some("Direct Track".to_string()),
            author: Some("App World".to_string()),
        };

        world.write_message(UpdateMusic(metadata.clone()));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            &world.resource::<SessionStateRes>().editor_music_metadata,
            &metadata,
            "UpdateMusic handler must mutate SessionStateRes.editor_music_metadata"
        );
    }

    #[test]
    fn carve_out_update_sky_color_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<SessionStateRes>().editor_sky_color = [0.0, 0.0, 0.0];

        world.write_message(UpdateSkyColor([-1.0, 0.5, 2.0]));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<SessionStateRes>().editor_sky_color,
            [0.0, 0.5, 1.0],
            "UpdateSkyColor handler must clamp and mutate SessionStateRes.editor_sky_color"
        );
    }

    #[test]
    fn carve_out_capture_menu_preview_camera_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
        {
            let mut camera = world.resource_mut::<EditorCameraRes>();
            camera.0.editor_pan = [2.0, -4.0];
            camera.0.editor_target_z = 3.0;
            camera.0.editor_yaw = 30.0f32.to_radians();
            camera.0.editor_pitch = 40.0f32.to_radians();
        }
        let expected = {
            let camera = &world.resource::<EditorCameraRes>().0;
            let target = camera.editor_camera_target();
            LevelPreviewCameraMetadata {
                position: (target + camera.editor_camera_offset()).to_array(),
                target: target.to_array(),
            }
        };

        world.write_message(CaptureMenuPreviewCamera);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            Some(expected)
        );
    }

    #[test]
    fn carve_out_use_auto_menu_preview_camera_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
        world
            .resource_mut::<SessionStateRes>()
            .editor_menu_preview_camera = Some(LevelPreviewCameraMetadata {
            position: [9.0, 8.0, 7.0],
            target: [6.0, 5.0, 4.0],
        });

        world.write_message(UseAutoMenuPreviewCamera);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            None
        );
    }

    #[test]
    fn carve_out_menu_preview_camera_messages_respect_editor_phase() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Menu);
        let existing = Some(LevelPreviewCameraMetadata {
            position: [9.0, 8.0, 7.0],
            target: [6.0, 5.0, 4.0],
        });
        world
            .resource_mut::<SessionStateRes>()
            .editor_menu_preview_camera = existing.clone();

        world.write_message(CaptureMenuPreviewCamera);
        world.write_message(UseAutoMenuPreviewCamera);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            existing
        );
    }

    // --- Timing carve-out pattern ---
    //
    // All 6 `events::timing::*` messages are carve-outs: their handlers
    // mutate `EditorTimingRes.0.{timing_points,timing_selected_index}`
    // directly. Pin the resource is updated and no legacy command is
    // queued.

    use crate::events::timing::{
        AddTimingPoint, RemoveTimingPoint, SetTimingPointBpm, SetTimingPointTime,
        SetTimingPointTimeSignature, SetTimingSelected,
    };

    /// `AddTimingPoint` appends to the list and sorts ascending by
    /// `time_seconds` (mirroring `EditorSubsystem::add_timing_point`).
    /// Pin the list grows, the revision bumps, and no legacy command
    /// is staged.
    #[test]
    fn carve_out_add_timing_point_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        assert!(
            world
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .is_empty()
        );
        let prev_revision = world.resource::<EditorTimingRes>().0.revision;

        // Add out of order to confirm the handler re-sorts.
        world.write_message(AddTimingPoint {
            time_seconds: 2.0,
            bpm: 120.0,
        });
        world.write_message(AddTimingPoint {
            time_seconds: 0.5,
            bpm: 90.0,
        });
        world.run_schedule(crate::state::StateSchedule);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points.len(), 2);
        assert_eq!(timing.timing_points[0].time_seconds, 0.5);
        assert_eq!(timing.timing_points[1].time_seconds, 2.0);
        assert!(
            timing.revision > prev_revision,
            "AddTimingPoint handler must bump the revision"
        );
    }

    /// `RemoveTimingPoint` removes the entry at the given index (or
    /// no-ops if out of range) and bumps the revision. Pin the list
    /// shrinks and no legacy command is staged.
    #[test]
    fn carve_out_remove_timing_point_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        // Pre-populate the resource with two timing points.
        {
            let mut res = world.resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![
                eg_types::TimingPoint {
                    time_seconds: 0.0,
                    bpm: 120.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
                eg_types::TimingPoint {
                    time_seconds: 1.0,
                    bpm: 120.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
            ];
        }
        let prev_revision = world.resource::<EditorTimingRes>().0.revision;

        world.write_message(RemoveTimingPoint(0));
        world.run_schedule(crate::state::StateSchedule);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points.len(), 1);
        assert_eq!(timing.timing_points[0].time_seconds, 1.0);
        assert!(timing.revision > prev_revision);
    }

    /// `RemoveTimingPoint` with an out-of-range index is a no-op and
    /// must not bump the revision.
    #[test]
    fn carve_out_remove_timing_point_out_of_range_is_noop() {
        let mut world = build_event_world();
        {
            let mut res = world.resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![eg_types::TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];
        }
        let prev_revision = world.resource::<EditorTimingRes>().0.revision;

        world.write_message(RemoveTimingPoint(99));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(world.resource::<EditorTimingRes>().0.timing_points.len(), 1);
        assert_eq!(
            world.resource::<EditorTimingRes>().0.revision,
            prev_revision
        );
    }

    /// `SetTimingPointTime` updates the time of the point at `index`
    /// and re-sorts the list ascending by `time_seconds`.
    #[test]
    fn carve_out_set_timing_point_time_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        {
            let mut res = world.resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![
                eg_types::TimingPoint {
                    time_seconds: 0.0,
                    bpm: 120.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
                eg_types::TimingPoint {
                    time_seconds: 1.0,
                    bpm: 120.0,
                    time_signature_numerator: 4,
                    time_signature_denominator: 4,
                },
            ];
        }

        // Move index 0 to time 5.0 — the new order should be 1.0, 5.0.
        world.write_message(SetTimingPointTime {
            index: 0,
            time: 5.0,
        });
        world.run_schedule(crate::state::StateSchedule);

        let timing = &world.resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points.len(), 2);
        assert_eq!(timing.timing_points[0].time_seconds, 1.0);
        assert_eq!(timing.timing_points[1].time_seconds, 5.0);
    }

    /// `SetTimingPointBpm` updates the BPM (clamped to >= 1.0).
    #[test]
    fn carve_out_set_timing_point_bpm_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        {
            let mut res = world.resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![eg_types::TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];
        }

        world.write_message(SetTimingPointBpm { index: 0, bpm: 0.5 });
        world.run_schedule(crate::state::StateSchedule);

        // Clamped to >= 1.0.
        assert!(
            (world.resource::<EditorTimingRes>().0.timing_points[0].bpm - 1.0).abs() < 1e-6,
            "SetTimingPointBpm must clamp BPM to >= 1.0"
        );
    }

    /// `SetTimingPointTimeSignature` updates the numerator/denominator
    /// (clamped to >= 1) and bumps the revision.
    #[test]
    fn carve_out_set_timing_point_time_signature_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        {
            let mut res = world.resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![eg_types::TimingPoint {
                time_seconds: 0.0,
                bpm: 120.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            }];
        }

        world.write_message(SetTimingPointTimeSignature {
            index: 0,
            numerator: 0,
            denominator: 0,
        });
        world.run_schedule(crate::state::StateSchedule);

        let tp = &world.resource::<EditorTimingRes>().0.timing_points[0];
        assert_eq!(tp.time_signature_numerator, 1, "clamped to >= 1");
        assert_eq!(tp.time_signature_denominator, 1, "clamped to >= 1");
    }

    /// `SetTimingSelected` updates the selected timing-point index.
    #[test]
    fn carve_out_set_timing_selected_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        assert_eq!(
            world.resource::<EditorTimingRes>().0.timing_selected_index,
            None
        );

        world.write_message(SetTimingSelected(Some(2)));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<EditorTimingRes>().0.timing_selected_index,
            Some(2)
        );

        world.write_message(SetTimingSelected(None));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<EditorTimingRes>().0.timing_selected_index,
            None
        );
    }

    // --- Spawn carve-out pattern ---
    //
    // All 3 `events::spawn::*` messages are carve-outs: their handlers
    // mutate `EditorSpawnRes.0` (SetSpawnHere, RotateSpawnDirection)
    // or `EditorConfigRes.0.selected_block_rotation_degrees[1]`
    // (RotatePlacementPreview) directly. Pin the resource is
    // updated and no legacy command is queued.

    use crate::events::spawn::{RotatePlacementPreview, RotateSpawnDirection, SetSpawnHere};
    use crate::state::EditorSpawnRes;
    use eg_types::Direction;

    /// `SetSpawnHere` mutates `EditorSpawnRes.0.position` to the
    /// current editor cursor (read from `EditorInputRes.0.cursor`).
    /// Pin the resource is updated and no legacy command is staged.
    #[test]
    fn carve_out_set_spawn_here_mutates_editor_spawn_resource() {
        let mut world = build_event_world();
        // Seed the resource with an initial spawn position and the
        // input with a different cursor.
        world.resource_mut::<EditorSpawnRes>().0.position = [0.0, 0.0, 0.0];
        world.resource_mut::<EditorInputRes>().0.cursor = [4.0, 8.0, 12.0];

        world.write_message(SetSpawnHere);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<EditorSpawnRes>().0.position,
            [4.0, 8.0, 12.0],
            "SetSpawnHere handler must copy cursor into EditorSpawnRes.position"
        );
    }

    /// `RotateSpawnDirection` cycles `EditorSpawnRes.0.direction`
    /// Forward → Right → Forward.
    #[test]
    fn carve_out_rotate_spawn_direction_cycles_editor_spawn_resource() {
        let mut world = build_event_world();
        world.resource_mut::<EditorSpawnRes>().0.direction = Direction::Forward;

        world.write_message(RotateSpawnDirection);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );

        world.write_message(RotateSpawnDirection);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<EditorSpawnRes>().0.direction,
            Direction::Forward
        );
    }

    /// `RotatePlacementPreview` increments
    /// `EditorConfigRes.0.selected_block_rotation_degrees[1]` by 90,
    /// wrapping into `0..360`.
    #[test]
    fn carve_out_rotate_placement_preview_mutates_editor_config_resource() {
        let mut world = build_event_world();
        // Start at 0; the first rotate should produce 90.
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .selected_block_rotation_degrees = [0.0, 0.0, 0.0];

        for expected in [90.0, 180.0, 270.0, 0.0] {
            world.write_message(RotatePlacementPreview);
            world.run_schedule(crate::state::StateSchedule);
            assert!(
                (world
                    .resource::<crate::state::EditorConfigRes>()
                    .0
                    .selected_block_rotation_degrees[1]
                    - expected)
                    .abs()
                    < 1e-6,
                "RotatePlacementPreview handler must rotate yaw by 90 (got {})",
                world
                    .resource::<crate::state::EditorConfigRes>()
                    .0
                    .selected_block_rotation_degrees[1]
            );
        }
    }

    // --- Camera + TriggerSelected carve-out pattern ---
    //
    // All 5 `events::camera::*` messages are carve-outs: their handlers
    // mutate `EditorCameraRes.0`, `EditorTriggerRes.0.selected_index`, or
    // `EditorObjectsRes` directly instead of staging legacy editor commands.

    use crate::events::camera::{
        AddCameraFollowTrigger, AddCameraTrigger, AdjustZoom, SetCameraOrientation,
        SetTriggerSelected,
    };
    use crate::state::EditorTriggerRes;

    /// `AdjustZoom(delta)` moves the editor camera along its look
    /// vector by `delta * 2` units. Pin the resource pan /
    /// target_z update and no legacy command is staged.
    #[test]
    fn carve_out_adjust_zoom_mutates_editor_camera_resource() {
        let mut world = build_event_world();
        world.resource_mut::<EditorCameraRes>().0.editor_pan = [0.0, 0.0];
        world.resource_mut::<EditorCameraRes>().0.editor_target_z = 0.0;

        // Positive zoom should move the pan by some non-zero amount.
        world.write_message(AdjustZoom(1.0));
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<EditorCameraRes>().0;
        assert!(
            camera.editor_pan[0] != 0.0
                || camera.editor_pan[1] != 0.0
                || camera.editor_target_z != 0.0,
            "AdjustZoom handler must move the editor camera"
        );
    }

    /// `SetCameraOrientation` (no transition) sets the yaw +
    /// pitch directly, clamping pitch to `[-89.9°, 89.9°]` and
    /// clearing any active transition. Pin the resource update
    /// and no legacy command is staged.
    #[test]
    fn carve_out_set_camera_orientation_mutates_editor_camera_resource() {
        let mut world = build_event_world();
        world.resource_mut::<EditorCameraRes>().0.editor_yaw = 0.0;
        world.resource_mut::<EditorCameraRes>().0.editor_pitch = 0.0;
        world.resource_mut::<EditorCameraRes>().0.transition = None;

        world.write_message(SetCameraOrientation {
            rotation: 1.0,
            pitch: 0.5,
            transition_seconds: None,
        });
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<EditorCameraRes>().0;
        approx::assert_abs_diff_eq!(camera.editor_yaw, 1.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_pitch, 0.5, epsilon = 1e-6);
        assert!(camera.transition.is_none());
    }

    /// `SetCameraOrientation` with a positive `transition_seconds`
    /// installs a `CameraTransition` instead of snapping directly.
    #[test]
    fn carve_out_set_camera_orientation_with_transition_installs_transition() {
        let mut world = build_event_world();
        world.resource_mut::<EditorCameraRes>().0.editor_yaw = 0.0;
        world.resource_mut::<EditorCameraRes>().0.editor_pitch = 0.0;
        world.resource_mut::<EditorCameraRes>().0.transition = None;

        world.write_message(SetCameraOrientation {
            rotation: 2.0,
            pitch: 0.3,
            transition_seconds: Some(0.5),
        });
        world.run_schedule(crate::state::StateSchedule);

        let camera = &world.resource::<EditorCameraRes>().0;
        // Yaw/pitch are NOT directly updated when a transition is
        // requested — they interpolate over the transition.
        assert!(camera.transition.is_some());
        approx::assert_abs_diff_eq!(
            camera.transition.as_ref().unwrap().elapsed,
            0.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            camera.transition.as_ref().unwrap().duration,
            0.5,
            epsilon = 1e-6
        );
    }

    /// `SetTriggerSelected` mutates
    /// `EditorTriggerRes.0.selected_index`. Pin the resource
    /// update and no legacy command is staged.
    #[test]
    fn carve_out_set_trigger_selected_mutates_editor_trigger_resource() {
        let mut world = build_event_world();
        world.resource_mut::<EditorTriggerRes>().0.selected_index = None;

        world.write_message(SetTriggerSelected(Some(2)));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<EditorTriggerRes>().0.selected_index,
            Some(2)
        );

        world.write_message(SetTriggerSelected(None));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(world.resource::<EditorTriggerRes>().0.selected_index, None);
    }

    /// `AddCameraTrigger` captures the App-world editor camera pose as a
    /// static camera trigger object, records App-world history, and avoids
    /// staging a legacy command.
    #[test]
    fn carve_out_add_camera_trigger_mutates_editor_objects_resource() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
        world.resource_mut::<EditorCameraRes>().0.editor_pan = [12.0, -7.0];
        world.resource_mut::<EditorCameraRes>().0.editor_target_z = 4.5;
        world.resource_mut::<EditorCameraRes>().0.editor_yaw = 0.9;
        world.resource_mut::<EditorCameraRes>().0.editor_pitch = 0.3;
        world
            .resource_mut::<crate::state::EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 8.0;

        world.write_message(AddCameraTrigger);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].block_id, eg_types::CAMERA_TRIGGER_BLOCK_ID);
        let trigger = objects[0].trigger.as_ref().expect("camera trigger");
        approx::assert_abs_diff_eq!(trigger.time_seconds, 8.0, epsilon = 1e-6);
        assert_eq!(trigger.target, eg_types::TriggerTarget::Camera);
        let camera = trigger.camera.as_ref().expect("camera payload");
        assert_eq!(camera.mode, eg_types::CameraMode::Static);
        assert_eq!(camera.target_position, [12.0, 4.5, -7.0]);
        approx::assert_abs_diff_eq!(camera.yaw, 0.9, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.pitch, 0.3, epsilon = 1e-6);
        assert_eq!(
            world
                .resource::<crate::state::EditorHistoryRes>()
                .0
                .undo
                .len(),
            1,
            "AddCameraTrigger must record App-world editor history"
        );
    }

    /// `AddCameraFollowTrigger` appends a follow-mode camera trigger and uses
    /// the App-world timeline sample cache to derive the visible trigger pose.
    #[test]
    fn carve_out_add_camera_follow_trigger_mutates_editor_objects_resource() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Editor);
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.time_seconds = 0.5;
            timeline.0.clock.duration_seconds = 2.0;
            timeline.0.snapshot_cache_step_seconds = 0.5;
            timeline.0.snapshot_cache = vec![
                crate::state::EditorTimelineSnapshot {
                    position: [0.0, 0.0, 0.0],
                    direction: eg_types::Direction::Forward,
                    is_grounded: true,
                    game_over: false,
                    trail_segment_count: 0,
                    trail_point_count: 0,
                },
                crate::state::EditorTimelineSnapshot {
                    position: [10.0, 2.0, 4.0],
                    direction: eg_types::Direction::Forward,
                    is_grounded: true,
                    game_over: false,
                    trail_segment_count: 0,
                    trail_point_count: 0,
                },
            ];
        }

        world.write_message(AddCameraFollowTrigger);
        world.run_schedule(crate::state::StateSchedule);

        let objects = &world.resource::<crate::state::EditorObjectsRes>().objects;
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].block_id, eg_types::CAMERA_TRIGGER_BLOCK_ID);
        assert_ne!(
            objects[0].position,
            [0.0, 0.0, 0.0],
            "follow trigger object pose must be derived from the timeline sample"
        );
        let trigger = objects[0].trigger.as_ref().expect("camera follow trigger");
        let camera = trigger.camera.as_ref().expect("camera payload");
        assert_eq!(camera.mode, eg_types::CameraMode::Follow);
        assert_eq!(trigger.target, eg_types::TriggerTarget::Camera);
    }

    #[test]
    fn carve_out_camera_trigger_creation_respects_editor_phase() {
        let mut world = build_event_world();
        world.resource_mut::<AppPhaseRes>().set(AppPhase::Menu);

        world.write_message(AddCameraTrigger);
        world.write_message(AddCameraFollowTrigger);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::EditorObjectsRes>()
                .objects
                .is_empty(),
            "camera trigger creation must no-op outside editor phase"
        );
    }

    // --- Phase 3.12 / Misc carve-out tests ---
    //
    // The `misc` event handlers for `ToggleHitboxVisualization`,
    // `TogglePerfOverlay`, `TogglePlaceWindow`, and `Escape` write
    // directly to App-world resources or typed request resources.

    use crate::events::misc::{
        Escape, ToggleHitboxVisualization, TogglePerfOverlay, TogglePlaceWindow,
    };

    /// `ToggleHitboxVisualization` is a carve-out: the handler
    /// mutates `EditorInputRes.0.show_hitbox_visualization`
    /// directly. Pin the resource is updated and no legacy
    /// command is queued.
    #[test]
    fn carve_out_toggle_hitbox_visualization_mutates_editor_input_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<EditorInputRes>()
            .0
            .show_hitbox_visualization = false;
        assert!(
            !world
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization
        );

        world.write_message(ToggleHitboxVisualization);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization,
            "ToggleHitboxVisualization handler must mutate EditorInputRes"
        );

        // Toggle back.
        world.write_message(ToggleHitboxVisualization);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            !world
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization
        );
    }

    /// `TogglePerfOverlay` is a carve-out: the handler mutates
    /// `PerfRes.0.overlay_enabled` directly. Pin the resource is
    /// updated and no legacy command is queued.
    #[test]
    fn carve_out_toggle_perf_overlay_mutates_perf_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::PerfRes>()
            .0
            .overlay_enabled = false;
        assert!(!world.resource::<crate::state::PerfRes>().0.overlay_enabled);

        world.write_message(TogglePerfOverlay);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world.resource::<crate::state::PerfRes>().0.overlay_enabled,
            "TogglePerfOverlay handler must mutate PerfRes"
        );

        // Toggle back.
        world.write_message(TogglePerfOverlay);
        world.run_schedule(crate::state::StateSchedule);
        assert!(!world.resource::<crate::state::PerfRes>().0.overlay_enabled);
    }

    /// `TogglePlaceWindow` is a carve-out: the handler mutates
    /// `SessionStateRes.editor_show_place_window` directly. Pin
    /// the resource is updated and no legacy command is queued.
    #[test]
    fn carve_out_toggle_place_window_mutates_session_state_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .editor_show_place_window = false;
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .editor_show_place_window
        );

        world.write_message(TogglePlaceWindow);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .editor_show_place_window,
            "TogglePlaceWindow handler must mutate SessionStateRes"
        );

        // Toggle back.
        world.write_message(TogglePlaceWindow);
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .editor_show_place_window
        );
    }

    /// `Escape` in the menu is an App-owned no-op and does not stage a
    /// legacy command or request.
    #[test]
    fn escape_in_menu_is_app_owned_noop() {
        let mut world = build_event_world();

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            eg_types::AppPhase::Menu
        );
    }

    #[test]
    fn carve_out_escape_in_real_gameplay_toggles_pause_resource_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);
        {
            let mut session = world.resource_mut::<crate::state::SessionStateRes>();
            session.playtesting_editor = false;
            session.game_paused = false;
        }

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .game_paused,
            "real-gameplay Escape must pause through SessionStateRes"
        );
        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .game_paused,
            "real-gameplay Escape must toggle pause back off"
        );
    }

    #[test]
    fn escape_in_playtest_returns_to_editor_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);
        {
            let mut session = world.resource_mut::<crate::state::SessionStateRes>();
            session.playtesting_editor = true;
            session.game_paused = false;
        }

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .game_paused,
            "playtest Escape must not mutate the real-game pause flag"
        );
        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            eg_types::AppPhase::Editor
        );
        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .playtesting_editor
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .audio_stop_pending
        );
    }

    /// In editor, Escape first clears block/tap selection directly in
    /// App-world resources. That branch must not fall back to the Runtime
    /// escape request.
    #[test]
    fn carve_out_escape_with_editor_selection_clears_resources_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0, 2];
            input.0.hovered_block_index = Some(2);
        }
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.taps.tap_times = vec![0.5];
            timeline.0.taps.tap_indicator_positions = vec![[1.0, 0.0, 0.0]];
            timeline.0.taps.selected_index = Some(0);
        }

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        let input = &world.resource::<crate::state::EditorInputRes>().0;
        assert_eq!(input.selected_block_index, None);
        assert!(input.selected_block_indices.is_empty());
        assert_eq!(input.hovered_block_index, None);
        assert_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .taps
                .selected_index,
            None
        );
    }

    #[test]
    fn carve_out_escape_with_editor_playback_stops_resources_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world.resource_mut::<crate::state::EditorInputRes>().0.mode = eg_types::EditorMode::Null;
        world
            .resource_mut::<crate::state::EditorRuntimeRes>()
            .0
            .interaction
            .last_mode = Some(eg_types::EditorMode::Timing);
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.playback.playing = true;
            timeline.0.playback.pending_seek_time_seconds = Some(1.25);
            timeline.0.playback.seek_resync_cooldown_seconds = 0.12;
        }

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert!(!timeline.playback.playing);
        assert!(timeline.playback.runtime.is_none());
        assert!(timeline.playback.pending_seek_time_seconds.is_none());
        approx::assert_abs_diff_eq!(
            timeline.playback.seek_resync_cooldown_seconds,
            0.0,
            epsilon = 1e-6
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            eg_types::EditorMode::Timing
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            None
        );
    }

    #[test]
    fn carve_out_escape_with_editor_timeline_time_resets_resource_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 4.0;
            timeline.0.clock.time_seconds = 2.5;
        }

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            0.0,
            epsilon = 1e-6
        );
    }

    /// Editor Escape at the timeline origin transitions directly to the
    /// App-owned menu phase instead of requesting the legacy state machine.
    #[test]
    fn carve_out_escape_at_editor_origin_returns_to_menu_directly() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            eg_types::AppPhase::Menu
        );
        assert!(
            world
                .resource::<crate::state::SessionStateRes>()
                .audio_stop_pending
        );
    }

    // --- Phase 3.13 / Editor-mode carve-out test ---
    //
    // The handler writes the authoritative `EditorInputRes.0.mode`
    // directly; no legacy command or Runtime mirror participates.
    /// `SetEditorMode` is a carve-out: the handler mutates
    /// `EditorInputRes.0.mode` directly. Pin the resource is
    /// updated and no legacy command is queued.
    #[test]
    fn carve_out_set_editor_mode_mutates_editor_input_resource() {
        let mut world = build_event_world();
        world.resource_mut::<crate::state::EditorInputRes>().0.mode = EditorMode::Place;
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Place
        );

        world.write_message(SetEditorMode(EditorMode::Tapping));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Tapping,
            "SetEditorMode handler must mutate EditorInputRes.mode"
        );

        // Switch back.
        world.write_message(SetEditorMode(EditorMode::Select));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Select
        );
    }

    /// `SelectRecentBlock` is a carve-out: the handler reads
    /// `EditorConfigRes.0.recent_block_ids` (filtered to placeable
    /// blocks), writes the indexed entry to
    /// `EditorConfigRes.0.selected_block_id` and sets
    /// `EditorInputRes.0.mode = Place` directly. Pin the resources
    /// are updated and no legacy command is queued.
    #[test]
    fn carve_out_select_recent_block_mutates_resources() {
        use crate::events::editor_mode::SelectRecentBlock;
        let mut world = build_event_world();

        // Seed two distinct placeable recent block ids. The
        // `BlockDefinition::get` fallback returns the default
        // block for unknown ids (which is placeable), so the
        // placeable filter is exercised even when the test uses
        // ids that resolve to known blocks.
        let first = eg_blocks::BlockId::new("core/stone");
        let second = eg_blocks::BlockId::new("core/wood");
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .recent_block_ids = vec![first.clone(), second.clone()];
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .selected_block_id = eg_blocks::BlockId::default();

        // Out-of-range index: no-op, no legacy command staged.
        world.write_message(SelectRecentBlock(10));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            eg_blocks::BlockId::default(),
            "Out-of-range index must not change selected_block_id"
        );

        // Index 0 picks the first recent block.
        world.write_message(SelectRecentBlock(0));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            first,
            "SelectRecentBlock(0) handler must mutate EditorConfigRes.selected_block_id"
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Place,
            "SelectRecentBlock handler must switch the editor mode to Place"
        );

        // Index 1 picks the second recent block.
        world.write_message(SelectRecentBlock(1));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            second
        );

        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .recent_block_ids,
            vec![second, first]
        );
    }

    /// `PickSelectedBlock` is a carve-out: the handler reads the
    /// selected object from `EditorObjectsRes` / `EditorInputRes`,
    /// writes the selected block id into `EditorConfigRes`, and enters
    /// Place mode without staging `EditorCommand::PickSelectedBlock`.
    #[test]
    fn carve_out_pick_selected_block_mutates_resources() {
        use crate::events::editor_mode::PickSelectedBlock;

        let mut world = build_event_world();
        let picked = eg_blocks::BlockId::new("core/lava");
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            block_id: picked.clone(),
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.mode = EditorMode::Select;
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }

        world.write_message(PickSelectedBlock);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            picked
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Place
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .recent_block_ids
                .first(),
            Some(&picked)
        );
    }

    /// During editor timeline playback, legacy `PickSelectedBlock`
    /// changes the effective mode by writing `last_mode = Place` while
    /// the visible mode remains `Null`. The App-world handler preserves
    /// that contract so stopping playback restores Place mode.
    #[test]
    fn carve_out_pick_selected_block_sets_playback_effective_mode() {
        use crate::events::editor_mode::PickSelectedBlock;

        let mut world = build_event_world();
        let picked = eg_blocks::BlockId::new("core/stone");
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            block_id: picked.clone(),
            ..LevelObject::default()
        }];
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.mode = EditorMode::Null;
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }
        world
            .resource_mut::<crate::state::EditorTimelineRes>()
            .0
            .playback
            .playing = true;

        world.write_message(PickSelectedBlock);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            picked
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Null
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            Some(EditorMode::Place)
        );
    }

    /// `PickBlockAt` is a carve-out: the handler reuses the existing
    /// editor screen picker against App-world resources, writes the
    /// picked block id into `EditorConfigRes`, enters Place mode, and
    /// does not stage `EditorCommand::PickBlockAt`.
    #[test]
    fn carve_out_pick_block_at_mutates_resources() {
        use crate::events::editor_mode::PickBlockAt;
        use crate::state::EditorSubsystem;
        use glam::{Vec2, Vec3};

        let mut world = build_event_world();
        let picked = eg_blocks::BlockId::new("core/grass");
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world
            .resource_mut::<crate::state::EditorObjectsRes>()
            .objects = vec![LevelObject {
            block_id: picked.clone(),
            ..LevelObject::default()
        }];
        world.resource_mut::<crate::state::EditorInputRes>().0.mode = EditorMode::Select;
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .selected_block_id = eg_blocks::BlockId::new("core/stone");
        world
            .resource_mut::<crate::state::RenderUniformsRes>()
            .color_space
            .viewport_size = [1280.0, 720.0];

        let screen = EditorSubsystem::default()
            .world_to_screen_v(Vec3::new(0.5, 0.5, 0.5), Vec2::new(1280.0, 720.0))
            .expect("block center should project to screen");

        world.write_message(PickBlockAt {
            x: screen.x as f64,
            y: screen.y as f64,
        });
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .selected_block_id,
            picked
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Place
        );
    }

    /// `Playtest` builds the App-world transition and applies the seeded
    /// gameplay reset without staging a Runtime request.
    #[test]
    fn playtest_enters_app_owned_gameplay_world() {
        use crate::events::timeline::Playtest;

        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(AppPhase::Editor);
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .editor_level_name = Some("AppPlaytest".to_string());
        world
            .resource_mut::<crate::state::EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 5.0;

        world.write_message(Playtest);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::AppPhaseRes>().current,
            AppPhase::Playing
        );
        let session = world.resource::<crate::state::SessionStateRes>();
        assert!(session.playtesting_editor);
        assert!(session.audio_stop_pending);
        assert!(!world.resource::<crate::state::GameplayResetRes>().pending);
        assert!(
            world
                .resource::<eg_game::LevelMeta>()
                .level_duration_seconds
                > 0.0
        );
        let mut query = world.query::<&eg_game::PlayerPosition>();
        assert!(query.iter(&world).next().is_some());
    }

    /// Built-in loading mutates App resources directly. Remaining export/audio
    /// side effects are Runtime requests, while import pickers use App senders.
    #[test]
    fn carve_out_session_side_effects_route_through_app_resources() {
        use crate::events::session::{
            CompleteImport, ExportLevel, LoadLevel, SetAudioBackend, TriggerAudioImport,
        };

        let mut world = build_event_world();
        let mut level = LevelMetadata::default();
        level.basic.name = "Flowerfield".to_string();
        level.objects.push(eg_types::LevelObject::default());
        world.insert_resource(crate::state::BuiltinLevelCatalogRes::from_levels([level]));
        world.resource_mut::<crate::state::EditorInputRes>().0.mode = eg_types::EditorMode::Timing;
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .app_settings
            .audio_backend = "stale".to_string();
        let source_key = eg_audio::audio::runtime_asset_source_key("Flowerfield", "audio.mp3");
        let audio_cache = world.resource::<crate::state::EditorAudioCacheRes>();
        audio_cache.insert_level_audio("Flowerfield", vec![1, 2, 3, 4]);
        audio_cache.insert_waveform(
            source_key,
            eg_audio::service::WaveformCacheEntry {
                samples: vec![0.25, 0.75],
                sample_rate: 44_100,
                window_size: eg_audio::service::WAVEFORM_WINDOW,
                complete: true,
            },
        );

        world.write_message(LoadLevel("Flowerfield".to_string()));
        world.write_message(ExportLevel);
        world.write_message(SetAudioBackend("Default".to_string()));
        world.write_message(CompleteImport);
        world.write_message(TriggerAudioImport);
        world.run_schedule(crate::state::StateSchedule);

        let session = world.resource::<crate::state::SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("Flowerfield"));
        assert!(session.audio_stop_pending);
        assert!(
            !world
                .resource::<crate::state::EditorObjectsRes>()
                .objects
                .is_empty()
        );
        let timing = &world.resource::<crate::state::EditorTimingRes>().0;
        assert_eq!(timing.waveform_samples, vec![0.25, 0.75]);
        assert!(timing.waveform_complete);
        let export = world.resource::<crate::state::LevelExportServiceRes>();
        let (filename, bytes) = export.last_request().expect("export should be launched");
        assert_eq!(filename, "Flowerfield.egz");
        let (exported, audio) =
            eg_level::read_metadata_from_egz(bytes).expect("decode exported level");
        assert_eq!(exported.basic.name, "Flowerfield");
        assert_eq!(exported.objects.len(), 1);
        assert_eq!(audio, Some(vec![1, 2, 3, 4]));
        assert_eq!(
            world
                .resource::<crate::state::SessionStateRes>()
                .app_settings
                .audio_backend,
            "Default"
        );
        assert_eq!(
            world
                .resource::<crate::state::LevelImportSenderRes>()
                .trigger_count(),
            1
        );
        assert_eq!(
            world
                .resource::<crate::state::AudioImportSenderRes>()
                .trigger_count(),
            1
        );
    }

    // --- Phase 3.15 / Navigation carve-out test ---
    //
    // `GameSetPracticeMode` is a carve-out: its handler in
    // `events::navigation` mutates
    // `SessionStateRes.practice_mode_enabled` directly instead
    // of staging a legacy `AppCommand`. The change-guarded
    // runtime sync (`Runtime::sync_app_session_state`) invokes
    // the legacy `set_practice_mode_enabled` setter (which has
    // heavy side effects: clear practice checkpoints, rebuild
    // checkpoint vertices, restart level, resume game) only on
    // real user-driven changes. The schedule-level test pins
    // the resource write + the absence of any staged legacy
    // command; the runtime tests in `eg-app` pin the full
    // message → resource → sync → legacy state pipeline.

    use crate::events::navigation::{GameResume, GameSetPracticeMode, NextLevel, PrevLevel};
    use crate::events::timeline::{
        AddTap, ClearTaps, RemoveTap, RemoveTapAt, SetPlaybackSpeed, SetSelectedTap,
        SetSelectedTapTime, SetTimelineDuration, SetTimelineTime, SetWaveformScroll,
        SetWaveformZoom, ShiftTimeline, ToggleTimelinePlayback,
    };

    /// `GameResume` is a carve-out: the handler writes the pause
    /// flag directly to `SessionStateRes.game_paused` instead of
    /// staging a legacy `AppCommand::GameResume`. Pin the resource is
    /// updated and no legacy command is queued.
    #[test]
    fn carve_out_game_resume_mutates_session_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::SessionStateRes>()
            .game_paused = true;

        world.write_message(GameResume);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            !world
                .resource::<crate::state::SessionStateRes>()
                .game_paused,
            "GameResume handler must clear SessionStateRes.game_paused"
        );
    }

    /// `GameSetPracticeMode(enabled)` is a carve-out: the
    /// handler writes the bool directly to
    /// `SessionStateRes.practice_mode_enabled` instead of
    /// staging a legacy `AppCommand::GameSetPracticeMode`. Pin
    /// the resource is updated and no legacy command is queued.
    #[test]
    fn carve_out_game_set_practice_mode_mutates_session_resource() {
        let mut world = build_event_world();
        // Start from a known state (the seeded resource has
        // `practice_mode_enabled = false` because the
        // `Default` impl is used; we explicitly reset here
        // to make the test self-contained).
        world
            .resource_mut::<SessionStateRes>()
            .practice_mode_enabled = false;
        assert!(!world.resource::<SessionStateRes>().practice_mode_enabled);

        // Enable practice mode.
        world.write_message(GameSetPracticeMode { enabled: true });
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            world.resource::<SessionStateRes>().practice_mode_enabled,
            "GameSetPracticeMode(true) handler must mutate SessionStateRes.practice_mode_enabled"
        );

        // Disable practice mode.
        world.write_message(GameSetPracticeMode { enabled: false });
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            !world.resource::<SessionStateRes>().practice_mode_enabled,
            "GameSetPracticeMode(false) handler must mutate SessionStateRes.practice_mode_enabled"
        );

        // No legacy command is staged for either write.
    }

    /// `NextLevel` and `PrevLevel` are carve-outs: in `Menu` they mutate
    /// `MenuStateRes.selected_level` directly instead of staging legacy
    /// commands.
    #[test]
    fn carve_out_menu_next_prev_level_mutates_menu_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Menu);
        world
            .resource_mut::<crate::state::MenuStateRes>()
            .0
            .selected_level = 2;

        world.write_message(NextLevel);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::MenuStateRes>()
                .0
                .selected_level,
            0,
            "NextLevel in Menu must wrap through MenuStateRes"
        );

        world.write_message(PrevLevel);
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::MenuStateRes>()
                .0
                .selected_level,
            2,
            "PrevLevel in Menu must wrap through MenuStateRes"
        );
    }

    #[test]
    fn carve_out_editor_next_prev_level_mutates_cursor_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world
            .resource_mut::<crate::state::MenuStateRes>()
            .0
            .selected_level = 1;
        {
            let mut config = world.resource_mut::<crate::state::EditorConfigRes>();
            config.0.snap_to_grid = true;
            config.0.snap_step = 0.25;
        }
        {
            let mut input = world.resource_mut::<crate::state::EditorInputRes>();
            input.0.cursor = [1.0, 0.0, 2.0];
            input.0.ctrl_held = false;
        }

        world.write_message(NextLevel);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [1.25, 0.0, 2.0],
            "NextLevel in Editor must move EditorInputRes.cursor by the snap step"
        );

        world.write_message(PrevLevel);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.0],
            "PrevLevel in Editor must move EditorInputRes.cursor back by the snap step"
        );
        assert_eq!(
            world
                .resource::<crate::state::MenuStateRes>()
                .0
                .selected_level,
            1,
            "editor navigation must leave MenuStateRes untouched"
        );
    }

    #[test]
    fn carve_out_next_prev_level_is_noop_outside_menu_and_editor() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);
        world
            .resource_mut::<crate::state::EditorInputRes>()
            .0
            .cursor = [1.0, 0.0, 2.0];
        world
            .resource_mut::<crate::state::MenuStateRes>()
            .0
            .selected_level = 1;

        world.write_message(NextLevel);
        world.write_message(PrevLevel);
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.0],
            "Playing NextLevel/PrevLevel must not move the editor cursor"
        );
        assert_eq!(
            world
                .resource::<crate::state::MenuStateRes>()
                .0
                .selected_level,
            1,
            "Playing NextLevel/PrevLevel must not move menu selection"
        );
    }

    #[test]
    fn carve_out_next_prev_level_missing_menu_resource_does_not_stage_legacy_command() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Menu);
        world.remove_resource::<crate::state::MenuStateRes>();

        world.write_message(NextLevel);
        world.write_message(PrevLevel);
        world.run_schedule(crate::state::StateSchedule);
    }

    #[test]
    fn carve_out_shift_timeline_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 4.0;
            timeline.0.clock.time_seconds = 1.0;
        }

        world.write_message(ShiftTimeline(1.5));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );

        world.write_message(ShiftTimeline(10.0));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            4.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_timeline_time_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 3.0;
            timeline.0.clock.time_seconds = 0.0;
        }

        world.write_message(SetTimelineTime(1.25));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            1.25,
            epsilon = 1e-6
        );

        world.write_message(SetTimelineTime(9.0));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            3.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_timeline_duration_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 8.0;
            timeline.0.clock.time_seconds = 6.0;
        }

        world.write_message(SetTimelineDuration(2.0));
        world.run_schedule(crate::state::StateSchedule);
        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        approx::assert_abs_diff_eq!(timeline.clock.duration_seconds, 2.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(timeline.clock.time_seconds, 2.0, epsilon = 1e-6);

        world.write_message(SetTimelineDuration(0.0));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .clock
                .duration_seconds,
            0.1,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_selected_tap_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.taps.tap_times = vec![0.25, 1.0];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]];
            timeline.0.taps.selected_index = None;
        }

        world.write_message(SetSelectedTap(Some(1)));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .taps
                .selected_index,
            Some(1)
        );

        world.write_message(SetSelectedTap(Some(99)));
        world.run_schedule(crate::state::StateSchedule);
        assert_eq!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .taps
                .selected_index,
            None,
            "out-of-range selection must clear the selected tap"
        );
    }

    #[test]
    fn carve_out_set_selected_tap_time_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.duration_seconds = 2.0;
            timeline.0.clock.time_seconds = 1.0;
            timeline.0.taps.tap_times = vec![0.25, 1.0, 1.5];
            timeline.0.taps.tap_indicator_positions =
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
            timeline.0.taps.selected_index = Some(1);
        }
        world
            .resource_mut::<crate::state::EditorInputRes>()
            .0
            .cursor = [9.0, 0.0, 9.0];

        world.write_message(SetSelectedTapTime(1.75));
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.5, 1.75]);
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 2.0], [1.0, 0.0, 1.0]],
            "SetSelectedTapTime must keep the moved tap's indicator position"
        );
        assert_eq!(timeline.taps.selected_index, Some(2));
        approx::assert_abs_diff_eq!(timeline.clock.time_seconds, 1.75, epsilon = 1e-6);
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.cursor,
            [1.0, 0.0, 1.0],
            "SetSelectedTapTime must move the editor cursor to the selected tap position"
        );

        world.write_message(SetSelectedTapTime(9.0));
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.5, 2.0]);
        assert_eq!(timeline.taps.selected_index, Some(2));
        approx::assert_abs_diff_eq!(
            timeline.clock.time_seconds,
            timeline.clock.duration_seconds,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_remove_tap_messages_mutate_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.time_seconds = 1.0;
            timeline.0.taps.tap_times = vec![0.25, 1.0, 1.5];
            timeline.0.taps.tap_indicator_positions =
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
            timeline.0.taps.selected_index = Some(2);
        }

        world.write_message(RemoveTap);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.5]);
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 2.0]]
        );
        assert_eq!(
            timeline.taps.selected_index,
            Some(1),
            "removing an earlier tap must shift the selected tap index"
        );

        world.write_message(RemoveTapAt(1.5));
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25]);
        assert_eq!(timeline.taps.tap_indicator_positions, vec![[0.0, 0.0, 0.0]]);
        assert_eq!(
            timeline.taps.selected_index, None,
            "removing the selected tap must clear selection"
        );
    }

    #[test]
    fn carve_out_toggle_timeline_playback_mutates_timeline_and_input_resources() {
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::AppPhaseRes>()
            .set(eg_types::AppPhase::Editor);
        world.resource_mut::<crate::state::EditorInputRes>().0.mode = eg_types::EditorMode::Timing;

        world.write_message(ToggleTimelinePlayback);
        world.run_schedule(crate::state::StateSchedule);

        assert!(
            world
                .resource::<crate::state::EditorTimelineRes>()
                .0
                .playback
                .playing
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            eg_types::EditorMode::Null
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            Some(eg_types::EditorMode::Timing)
        );

        world.write_message(ToggleTimelinePlayback);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert!(!timeline.playback.playing);
        assert!(timeline.playback.runtime.is_none());
        assert!(timeline.playback.pending_seek_time_seconds.is_none());
        approx::assert_abs_diff_eq!(
            timeline.playback.seek_resync_cooldown_seconds,
            0.0,
            epsilon = 1e-6
        );
        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            eg_types::EditorMode::Timing
        );
        assert_eq!(
            world
                .resource::<crate::state::EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            None
        );
    }

    #[test]
    fn carve_out_add_tap_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.clock.time_seconds = 1.0;
            timeline.0.preview.position = [3.5, 0.0, 4.5];
            timeline.0.taps.tap_times = vec![0.25, 1.5];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 2.0]];
            timeline.0.taps.selected_index = None;
        }

        world.write_message(AddTap);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.0, 1.5]);
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            vec![[0.0, 0.0, 0.0], [3.0, 0.0, 4.0], [2.0, 0.0, 2.0]],
            "AddTap must store the preview-derived tap indicator position"
        );
        assert_eq!(timeline.taps.selected_index, Some(1));

        world.write_message(AddTap);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert_eq!(
            timeline.taps.tap_times,
            vec![0.25, 1.0, 1.5],
            "AddTap must select an existing epsilon-matched tap instead of duplicating it"
        );
        assert_eq!(timeline.taps.selected_index, Some(1));
    }

    #[test]
    fn carve_out_clear_taps_mutates_editor_timeline_resource() {
        let mut world = build_event_world();
        {
            let mut timeline = world.resource_mut::<crate::state::EditorTimelineRes>();
            timeline.0.taps.tap_times = vec![0.25, 1.0];
            timeline.0.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]];
            timeline.0.taps.selected_index = Some(1);
        }

        world.write_message(ClearTaps);
        world.run_schedule(crate::state::StateSchedule);

        let timeline = &world.resource::<crate::state::EditorTimelineRes>().0;
        assert!(timeline.taps.tap_times.is_empty());
        assert!(timeline.taps.tap_indicator_positions.is_empty());
        assert_eq!(timeline.taps.selected_index, None);
    }

    #[test]
    fn carve_out_set_playback_speed_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        world.write_message(SetPlaybackSpeed(1.5));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimingRes>()
                .0
                .playback_speed,
            1.5,
            epsilon = 1e-6
        );

        world.write_message(SetPlaybackSpeed(9.0));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimingRes>()
                .0
                .playback_speed,
            2.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_waveform_zoom_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        world.write_message(SetWaveformZoom(3.5));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimingRes>()
                .0
                .waveform_zoom,
            3.5,
            epsilon = 1e-6
        );

        world.write_message(SetWaveformZoom(0.01));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimingRes>()
                .0
                .waveform_zoom,
            0.1,
            epsilon = 1e-6
        );
    }

    #[test]
    fn carve_out_set_waveform_scroll_mutates_editor_timing_resource() {
        let mut world = build_event_world();
        world.write_message(SetWaveformScroll(2.25));
        world.run_schedule(crate::state::StateSchedule);
        approx::assert_abs_diff_eq!(
            world
                .resource::<crate::state::EditorTimingRes>()
                .0
                .waveform_scroll,
            2.25,
            epsilon = 1e-6
        );
    }
}
