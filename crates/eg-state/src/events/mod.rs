/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy `Message` taxonomy for the eg-state `App`.
//!
//! Phase 3 fragments the monolithic `AppCommand` / `EditorCommand` enums
//! into a per-semantic Bevy `Message` type. During the Phase 7
//! compatibility window, messages are drained from the App `World`,
//! converted back to the legacy enums via [`ToLegacy`], and staged in
//! [`PendingCommands`] for `Runtime` to apply to legacy `State`.
//!
//! Bevy 0.19 uses `Message` (not `Event`) for buffered per-frame
//! payloads; observer-based `Event`s fire immediately. We use `Message`
//! here because we want pull-based consumption by the per-message
//! handler systems in the [`CommandSet`] system set, then a single
//! drain into [`PendingCommands`] at the end of the frame.
//!
//! The per-message drain systems are wired into the `StateSchedule` so
//! a single Bevy system per message type drains that message into the
//! [`PendingCommands`] resource. Runtime runs the schedule on the App
//! world and drains staged commands into `State::dispatch`. Input-state
//! messages update App-world resources directly instead of staging legacy
//! editor commands.
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

/// A legacy command, recovered from a Bevy message via [`ToLegacy`].
#[derive(Debug, Clone)]
pub enum LegacyCommand {
    /// Equivalent to an [`AppCommand`] variant.
    App(AppCommand),
    /// Equivalent to an [`EditorCommand`] variant.
    Editor(EditorCommand),
}

/// Trait mapping a Bevy message to its legacy `AppCommand` /
/// `EditorCommand` equivalent.
///
/// Implementing this trait is the only boilerplate the migration to
/// messages requires per command — the dispatch path on `State` is
/// unchanged.
pub trait ToLegacy {
    /// Convert `self` to a [`LegacyCommand`].
    fn to_legacy(self) -> LegacyCommand;
}

/// Per-frame buffer of legacy commands recovered from Bevy messages.
///
/// The Bevy schedule's per-message drain systems write into this
/// resource. Runtime takes the App-world resource and calls
/// `State::dispatch` / `State::dispatch_editor` for each buffered
/// command during the compatibility window.
#[derive(Resource, Default, Debug)]
pub struct PendingCommands {
    /// Commands to route through `State::dispatch`.
    pub app_commands: Vec<AppCommand>,
    /// Commands to route through `State::dispatch_editor`.
    pub editor_commands: Vec<EditorCommand>,
}

impl PendingCommands {
    /// True when no commands are buffered.
    pub fn is_empty(&self) -> bool {
        self.app_commands.is_empty() && self.editor_commands.is_empty()
    }

    /// Number of buffered commands (app + editor).
    pub fn len(&self) -> usize {
        self.app_commands.len() + self.editor_commands.len()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.app_commands.clear();
        self.editor_commands.clear();
    }
}

/// System set that contains the per-message drain systems.
///
/// All systems in this set read a single `MessageReader<T>` and write
/// [`PendingCommands`]. They run as part of the `StateSchedule` and
/// feed Runtime with the commands to apply to the legacy subsystems.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandSet;

/// Register every eg-state message type on the given `App` and ensure
/// the [`PendingCommands`] resource is initialized.
///
/// The schedule itself is owned by `State` (Phase 2c / 7), so this
/// helper does not add a `Schedule` — only the `Messages<T>` resources
/// and the `PendingCommands` buffer. Use [`add_command_systems`] to
/// add the per-message drain systems to a `Schedule`.
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

    // PendingCommands resource for the per-message systems to write into.
    app.init_resource::<PendingCommands>();
}

/// World-flavoured variant of [`register_command_events`] for callers
/// that work with a bare `World` (e.g. `State` constructs its `World`
/// directly without going through an `App`). Initializes the
/// `Messages<T>` resource for every event type + the
/// [`PendingCommands`] buffer; does **not** add a schedule — see
/// `register_state_command_pipeline` for the full `World` setup.
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

    world.init_resource::<PendingCommands>();
}

/// Add every per-message drain system to a `Schedule`.
///
/// `State::tick_state` runs the schedule, so the systems execute once
/// per frame in the order they're added (Bevy schedules are not
/// parallel-by-default for unconstrained systems, so this is
/// deterministic). All systems target the [`CommandSet`].
pub fn add_command_systems(schedule: &mut Schedule) {
    // Bevy's `IntoScheduleConfigs` only goes up to 16 systems in a flat
    // tuple, so we add each domain's systems in its own `add_systems`
    // call. The systems remain ordered by their call site (per-domain
    // grouping is also a useful documentation aid).
    schedule
        // Navigation
        .add_systems((
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
        ))
        // Auth
        .add_systems((
            auth::handle_auth_submit_sign_in,
            auth::handle_auth_sign_out,
            auth::handle_auth_open_signup,
            auth::handle_copy_latest_perf_frame,
            auth::handle_resize_surface,
        ))
        // Editor mode
        .add_systems((
            editor_mode::handle_set_editor_mode,
            editor_mode::handle_set_block_id,
            editor_mode::handle_select_recent_block,
            editor_mode::handle_pick_selected_block,
            editor_mode::handle_pick_block_at,
        ))
        // Snap
        .add_systems((
            snap::handle_set_snap_to_grid,
            snap::handle_set_snap_step,
            snap::handle_set_snap_rotation,
            snap::handle_set_snap_rotation_step,
        ))
        // Block ops
        .add_systems((
            block::handle_remove_block,
            block::handle_duplicate_block,
            block::handle_copy_block,
            block::handle_paste_block,
            block::handle_update_selected_block,
        ))
        // Selection / transform
        .add_systems((
            selection::handle_snap_selection_to_grid,
            selection::handle_focus_camera_target,
            selection::handle_add_transform_trigger,
        ))
        // Timeline
        .add_systems((
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
        ))
        // Timing
        .add_systems((
            timing::handle_add_timing_point,
            timing::handle_remove_timing_point,
            timing::handle_set_timing_point_time,
            timing::handle_set_timing_point_bpm,
            timing::handle_set_timing_point_time_signature,
            timing::handle_set_timing_selected,
        ))
        // BPM
        .add_systems((bpm::handle_bpm_tap, bpm::handle_bpm_tap_reset))
        // Spawn
        .add_systems((
            spawn::handle_set_spawn_here,
            spawn::handle_rotate_spawn_direction,
            spawn::handle_rotate_placement_preview,
        ))
        // History
        .add_systems((
            history::handle_undo,
            history::handle_redo,
            history::handle_history_snapshot,
        ))
        // Camera
        .add_systems((
            camera::handle_adjust_zoom,
            camera::handle_set_camera_orientation,
            camera::handle_add_camera_trigger,
            camera::handle_add_camera_follow_trigger,
            camera::handle_set_trigger_selected,
        ))
        // Misc
        .add_systems((
            misc::handle_toggle_hitbox_visualization,
            misc::handle_toggle_perf_overlay,
            misc::handle_toggle_place_window,
            misc::handle_escape,
        ))
        // Session
        .add_systems((
            session::handle_load_level,
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
        ))
        .add_systems((
            session::handle_complete_import,
            session::handle_update_music,
            session::handle_update_creator_metadata,
            session::handle_update_sky_color,
            session::handle_trigger_audio_import,
            session::handle_capture_menu_preview_camera,
            session::handle_use_auto_menu_preview_camera,
        ))
        // Input state (modifier / pan held). These write directly to
        // EditorInputRes instead of staging legacy commands.
        .add_systems((
            input_state::handle_set_shift_held,
            input_state::handle_set_ctrl_held,
            input_state::handle_set_alt_held,
            input_state::handle_set_pan_up_held,
            input_state::handle_set_pan_down_held,
            input_state::handle_set_pan_left_held,
            input_state::handle_set_pan_right_held,
        ))
        // Pointer / drag.
        .add_systems((
            pointer::handle_mouse_button,
            pointer::handle_primary_click,
            pointer::handle_pointer_moved,
            pointer::handle_update_cursor_from_screen,
            pointer::handle_camera_drag,
        ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::auth::ResizeSurface;
    use crate::events::editor_mode::SetEditorMode;
    use crate::events::navigation::TurnRight;
    use crate::events::snap::SetSnapToGrid;
    use bevy_ecs::message::Messages;
    use eg_types::EditorMode;

    #[test]
    fn pending_commands_default_is_empty() {
        let pending = PendingCommands::default();
        assert!(pending.is_empty());
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn pending_commands_clear_resets_both() {
        let mut pending = PendingCommands {
            app_commands: vec![AppCommand::TurnRight],
            editor_commands: vec![EditorCommand::SetSnapToGrid(false)],
        };
        assert!(!pending.is_empty());
        pending.clear();
        assert!(pending.is_empty());
    }

    #[test]
    fn to_legacy_turn_right_app_command() {
        let legacy = TurnRight.to_legacy();
        assert!(matches!(legacy, LegacyCommand::App(AppCommand::TurnRight)));
    }

    #[test]
    fn to_legacy_resize_surface_carries_dimensions() {
        let legacy = ResizeSurface {
            width: 1280,
            height: 720,
        }
        .to_legacy();
        assert!(matches!(
            legacy,
            LegacyCommand::App(AppCommand::ResizeSurface {
                width: 1280,
                height: 720
            })
        ));
    }

    #[test]
    fn to_legacy_set_editor_mode_routes_to_editor() {
        let legacy = SetEditorMode(EditorMode::Place).to_legacy();
        assert!(matches!(
            legacy,
            LegacyCommand::Editor(EditorCommand::SetMode(EditorMode::Place))
        ));
    }

    #[test]
    fn to_legacy_snap_to_grid_carries_bool() {
        let legacy = SetSnapToGrid(true).to_legacy();
        assert!(matches!(
            legacy,
            LegacyCommand::Editor(EditorCommand::SetSnapToGrid(true))
        ));
    }

    /// Build a `World` that has the message resources + the per-message
    /// drain schedule.
    fn build_event_world() -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        // All per-message drain systems run in the same schedule, so the
        // helper must register every resource any of them reads. The
        // carve-out handlers (input_state, pointer) reach for
        // `EditorInputRes`, `EditorCameraRes`, and `AppPhaseRes`; the
        // `snap` carve-out reaches for `EditorConfigRes`; the `bpm`
        // carve-out reaches for `EditorTimingRes`; the `session`
        // carve-out (SetShowMetadata, ToggleSettings, ...) reaches for
        // `SessionStateRes`; the `timing` carve-out reaches for
        // `EditorTimingRes`; the `spawn` carve-out reaches for
        // `EditorInputRes` (cursor) + `EditorSpawnRes` +
        // `EditorConfigRes`; the `camera` carve-out reaches for
        // `EditorCameraRes` + `EditorTriggerRes`. The rest of the
        // schedule only reads message buffers + the `PendingCommands`
        // accumulator.
        world.init_resource::<crate::state::EditorInputRes>();
        world.init_resource::<crate::state::EditorCameraRes>();
        world.init_resource::<crate::state::EditorConfigRes>();
        world.init_resource::<crate::state::EditorSpawnRes>();
        world.init_resource::<crate::state::EditorTriggerRes>();
        world.init_resource::<crate::state::EditorTimingRes>();
        world.init_resource::<crate::state::SessionStateRes>();
        world.init_resource::<crate::state::PerfRes>();
        world.init_resource::<crate::state::AppPhaseRes>();
        register_command_events_world(&mut world);
        let mut schedule = bevy_ecs::schedule::Schedule::new(crate::state::StateSchedule);
        add_command_systems(&mut schedule);
        world.add_schedule(schedule);
        world
    }

    #[test]
    fn schedule_drains_turn_right_message_into_pending() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(pending.app_commands, vec![AppCommand::TurnRight]);
    }

    #[test]
    fn schedule_drains_resize_surface_with_payload() {
        let mut world = build_event_world();
        world.write_message(ResizeSurface {
            width: 1920,
            height: 1080,
        });
        world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.app_commands,
            vec![AppCommand::ResizeSurface {
                width: 1920,
                height: 1080
            }]
        );
    }

    #[test]
    fn schedule_drains_set_editor_mode_into_editor_command() {
        // `SetEditorMode` is a Phase 3.13 carve-out: the handler
        // mutates `EditorInputRes.0.mode` directly. Pin the
        // resource update and confirm no legacy command is
        // staged (the `PendingCommands` buffer stays empty for
        // this message). The legacy `ToLegacy` impl is preserved
        // so the existing `editor_dispatch` tests can still drive
        // `State::dispatch_editor(EditorCommand::SetMode(...))`
        // directly.
        let mut world = build_event_world();
        world.write_message(SetEditorMode(EditorMode::Tapping));
        world.run_schedule(crate::state::StateSchedule);

        assert_eq!(
            world.resource::<crate::state::EditorInputRes>().0.mode,
            EditorMode::Tapping,
            "SetEditorMode handler must mutate EditorInputRes.mode"
        );
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetEditorMode must not stage a legacy editor command"
        );
    }

    #[test]
    fn schedule_drains_set_snap_to_grid_into_editor_command() {
        // `SetSnapToGrid` is a carve-out: the handler mutates
        // `EditorConfigRes` directly. Assert the resource is updated
        // and no legacy command is staged.
        let mut world = build_event_world();
        world
            .resource_mut::<crate::state::EditorConfigRes>()
            .0
            .snap_to_grid = true;
        world.write_message(SetSnapToGrid(false));
        world.run_schedule(crate::state::StateSchedule);
        assert!(
            !world
                .resource::<crate::state::EditorConfigRes>()
                .0
                .snap_to_grid
        );
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetSnapToGrid must not stage a legacy editor command"
        );
    }

    #[test]
    fn multiple_messages_drain_in_order() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.app_commands,
            vec![
                AppCommand::TurnRight,
                AppCommand::TurnRight,
                AppCommand::TurnRight
            ]
        );
    }

    #[test]
    fn empty_message_buffer_is_noop() {
        let mut world = build_event_world();
        world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert!(pending.is_empty());
    }

    #[test]
    fn mixed_message_types_drain_into_correct_buckets() {
        // `TurnRight` still stages a legacy `AppCommand`; `SetSnapToGrid`
        // is a carve-out and writes to `EditorConfigRes` directly. Pin
        // both behaviours in one test.
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(SetSnapToGrid(true));
        world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(pending.app_commands, vec![AppCommand::TurnRight]);
        assert!(
            pending.editor_commands.is_empty(),
            "SetSnapToGrid carve-out must not stage a legacy editor command"
        );
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
    // directly, bypassing `PendingCommands`. Pin the contract: the
    // message reaches the schedule, the handler mutates the resource,
    // and the buffer is empty afterwards (no legacy command staged).

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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetShiftHeld must not stage a legacy editor command"
        );
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetBlockId must not stage a legacy editor command"
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "BpmTap must not stage a legacy editor command"
        );
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "BpmTapReset must not stage a legacy editor command"
        );
    }

    // --- Session carve-out pattern ---
    //
    // `SetShowMetadata`, `SetShowSettings`, `ToggleSettings`,
    // `SetSettingsSection`, and `SetKeybindCapture` are carve-outs:
    // their handlers mutate `SessionStateRes` directly. Pin the
    // resource is updated and no legacy command is queued.

    use crate::events::session::{
        SetKeybindCapture, SetSettingsSection, SetShowMetadata, SetShowSettings, ToggleSettings,
    };
    use crate::state::SessionStateRes;
    use eg_types::SettingsSection;

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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetShowMetadata must not stage a legacy editor command"
        );
    }

    #[test]
    fn carve_out_set_show_settings_mutates_session_resource() {
        let mut world = build_event_world();
        world.resource_mut::<SessionStateRes>().editor_show_settings = false;

        world.write_message(SetShowSettings(true));
        world.run_schedule(crate::state::StateSchedule);

        assert!(world.resource::<SessionStateRes>().editor_show_settings);
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "AddTimingPoint must not stage a legacy editor command"
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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

        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetSpawnHere must not stage a legacy editor command"
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

        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "RotateSpawnDirection must not stage a legacy editor command"
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "RotatePlacementPreview must not stage a legacy editor command"
        );
    }

    // --- Camera + TriggerSelected carve-out pattern ---
    //
    // 3 of the 5 `events::camera::*` messages are carve-outs:
    // their handlers mutate `EditorCameraRes.0` (AdjustZoom,
    // SetCameraOrientation) or `EditorTriggerRes.0.selected_index`
    // (SetTriggerSelected) directly. The other 2
    // (AddCameraTrigger, AddCameraFollowTrigger) keep the legacy
    // PendingCommands path because they push to `editor.objects`.

    use crate::events::camera::{
        AddCameraTrigger, AdjustZoom, SetCameraOrientation, SetTriggerSelected,
    };
    use crate::state::EditorCameraRes;
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "AdjustZoom must not stage a legacy editor command"
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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
        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
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

        let pending = world.resource::<PendingCommands>();
        assert!(pending.editor_commands.is_empty());
    }

    /// `AddCameraTrigger` is **not** a carve-out: the handler
    /// stages a legacy `EditorCommand::AddCameraTrigger` because
    /// the legacy setter pushes to `editor.objects` (a complex
    /// store with history + sync side effects). Pin the legacy
    /// command is staged.
    #[test]
    fn add_camera_trigger_still_uses_pending_commands() {
        let mut world = build_event_world();

        world.write_message(AddCameraTrigger);
        world.run_schedule(crate::state::StateSchedule);

        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.editor_commands,
            vec![crate::state::editor_command::EditorCommand::AddCameraTrigger],
            "AddCameraTrigger must still stage a legacy editor command"
        );
    }

    // --- Phase 3.12 / Misc carve-out tests ---
    //
    // The `misc` event handlers for `ToggleHitboxVisualization`,
    // `TogglePerfOverlay`, and `TogglePlaceWindow` write directly to
    // App-world resources (`EditorInputRes`, `PerfRes`, and
    // `SessionStateRes`) instead of staging a legacy
    // `EditorCommand`. The `Escape` handler keeps the legacy
    // `PendingCommands` path because the legacy
    // `handle_editor_escape` is a multi-step context-sensitive
    // state machine.

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

        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "ToggleHitboxVisualization must not stage a legacy editor command"
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

        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "TogglePerfOverlay must not stage a legacy editor command"
        );
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

        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "TogglePlaceWindow must not stage a legacy editor command"
        );
    }

    /// `Escape` is **not** a carve-out: the handler stages a
    /// legacy `EditorCommand::Escape` because the legacy
    /// `handle_editor_escape` is a multi-step context-sensitive
    /// state machine (stop playback → reset timeline → back to
    /// menu). Pin the legacy command is staged.
    #[test]
    fn escape_still_uses_pending_commands() {
        let mut world = build_event_world();

        world.write_message(Escape);
        world.run_schedule(crate::state::StateSchedule);

        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.editor_commands,
            vec![crate::state::editor_command::EditorCommand::Escape],
            "Escape must still stage a legacy editor command"
        );
    }

    // --- Phase 3.13 / Editor-mode carve-out test ---
    //
    // The `SetEditorMode` handler in `events::editor_mode` writes
    // `EditorInputRes.0.mode` directly. The legacy
    // `State.editor.input.mode` is synced from the resource by
    // `Runtime::sync_app_editor_input_state` (which the misc
    // carve-out also extends). The legacy `set_editor_mode` has
    // heavy side effects (rebuilds vertices, marks dirty flags);
    // the change guard in the runtime keeps those to one
    // notification per real user-driven change.

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

        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SetEditorMode must not stage a legacy editor command"
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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SelectRecentBlock must not stage a legacy editor command"
        );
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

        // No legacy command is staged across the whole test.
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.editor_commands.is_empty(),
            "SelectRecentBlock must not stage a legacy editor command"
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

    use crate::events::navigation::GameSetPracticeMode;

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
        let pending = world.resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "GameSetPracticeMode must not stage any legacy command (carve-out)"
        );
    }
}
