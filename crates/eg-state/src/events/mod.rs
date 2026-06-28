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
//! [`PendingCommands`] resource. `State::tick_state` can still run the
//! schedule on the legacy State-owned world, but it no longer drains
//! those commands into `State::dispatch`. Input-state messages are the
//! first direct-handler carve-out: they update `EditorInputRes` instead
//! of staging legacy editor commands.
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
//! writes directly to `EditorInputRes`; pointer/camera drag events remain
//! routed through `dispatch_editor` until their interaction handlers move
//! over as a group.
//!
//! - `SetShiftHeld` / `SetCtrlHeld` / `SetAltHeld` (direct resource writes)
//! - `SetPan{Up,Down,Left,Right}Held` (direct resource writes)
//! - `MouseButton` / `PrimaryClick` / `PointerMoved` /
//!   `UpdateCursorFromScreen` / `CameraDrag` (pointer / camera drag)
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
/// command during the compatibility window. The State-owned world can
/// still stage commands for tests, but `State::tick_state` no longer
/// forwards them to legacy dispatch.
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
        world.init_resource::<crate::state::EditorInputRes>();
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
        let _ = world.run_schedule(crate::state::StateSchedule);
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
        let _ = world.run_schedule(crate::state::StateSchedule);
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
        let mut world = build_event_world();
        world.write_message(SetEditorMode(EditorMode::Tapping));
        let _ = world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.editor_commands,
            vec![EditorCommand::SetMode(EditorMode::Tapping)]
        );
    }

    #[test]
    fn schedule_drains_set_snap_to_grid_into_editor_command() {
        let mut world = build_event_world();
        world.write_message(SetSnapToGrid(false));
        let _ = world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(
            pending.editor_commands,
            vec![EditorCommand::SetSnapToGrid(false)]
        );
    }

    #[test]
    fn multiple_messages_drain_in_order() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        world.write_message(TurnRight);
        let _ = world.run_schedule(crate::state::StateSchedule);
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
        let _ = world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert!(pending.is_empty());
    }

    #[test]
    fn mixed_message_types_drain_into_correct_buckets() {
        let mut world = build_event_world();
        world.write_message(TurnRight);
        world.write_message(SetSnapToGrid(true));
        let _ = world.run_schedule(crate::state::StateSchedule);
        let pending = world.resource::<PendingCommands>();
        assert_eq!(pending.app_commands, vec![AppCommand::TurnRight]);
        assert_eq!(
            pending.editor_commands,
            vec![EditorCommand::SetSnapToGrid(true)]
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
}
