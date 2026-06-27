/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorDirtyFlags;
use super::State;
use crate::commands::AppCommand;
use crate::state::editor_command::EditorCommand;
use crate::state::{AppPhaseRes, EditorInputRes, EditorRuntimeRes, PerfRes, SessionStateRes};
use eg_audio::audio::runtime_asset_source_key;
use eg_editor_domain::{derive_tap_indicator_positions, derive_timeline_position};
use eg_game::simulate_timeline_state;
use eg_mesh::{GizmoAxis, GizmoDragKind};
use eg_types::{
    AppPhase, CAMERA_TRIGGER_BLOCK_ID, CameraMode, CameraTrigger, Direction, EasingDirection,
    EditorMode, LevelObject, PhysicalSize, TRANSFORM_TRIGGER_BLOCK_ID, TimedTrigger, TriggerTarget,
};
use glam::{Vec2, Vec3};

async fn new_editor_state() -> State {
    let mut state = State::new_test().await;
    state.enter_editor_phase("Test Level".to_string());
    state
}

fn transform_trigger_object(
    time_seconds: f32,
    duration_seconds: f32,
    position: [f32; 3],
    object_ids: Vec<&str>,
) -> LevelObject {
    LevelObject {
        position,
        trigger: Some(TimedTrigger {
            time_seconds,
            duration_seconds,
            easing: EasingDirection::Linear,
            target: TriggerTarget::Objects {
                object_ids: object_ids.into_iter().map(str::to_string).collect(),
            },
            transform: true,
            camera: None,
        }),
        ..LevelObject::new(TRANSFORM_TRIGGER_BLOCK_ID)
    }
}

fn camera_trigger_object(time_seconds: f32) -> LevelObject {
    LevelObject {
        trigger: Some(TimedTrigger {
            time_seconds,
            duration_seconds: 0.0,
            easing: EasingDirection::Linear,
            target: TriggerTarget::Camera,
            transform: false,
            camera: Some(CameraTrigger {
                mode: CameraMode::Static,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [0.0, 0.0, 0.0],
                yaw: 0.0,
                pitch: 0.0,
                ..CameraTrigger::default()
            }),
        }),
        ..LevelObject::new(CAMERA_TRIGGER_BLOCK_ID)
    }
}

#[tokio::test]
async fn test_marquee_no_redundant_selections_before_drag_started() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.input.mode = EditorMode::Select;
    state.editor.objects.push(LevelObject::default());
    state.editor.objects.push(LevelObject {
        position: [5.0, 0.0, 0.0],
        ..LevelObject::default()
    });

    // 1. Just hovering over the first block (no mouse down)
    state.editor.input.hovered_block_index = Some(0);
    state.rebuild_editor_hover_outline_vertices();

    let count = state
        .render
        .meshes
        .editor_hover_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("hover outline mesh should exist after rebuilding vertices");
    // One stencil outline hull = 1 prism * 36 vertices/prism.
    assert_eq!(
        count, 36,
        "Should only have one block outlined when just hovering"
    );
    assert!(
        state
            .render
            .meshes
            .editor_hover_stencil
            .draw_data()
            .is_some()
    );

    // 2. Mouse down - starts marquee but drag hasn't started (moved only 1 pixel)
    state.editor.input.left_mouse_down = true;
    state.editor.input.marquee_start_screen = Some([100.0, 100.0]);
    state.editor.input.marquee_current_screen = Some([101.0, 101.0]);

    // Verify marquee is not active yet
    let (_, _, is_active) = state.editor.marquee_selection_rect_screen().unwrap();
    assert!(!is_active, "Marquee should NOT be active yet");

    // Even if we were to have overlapping blocks (mathematically),
    // they shouldn't show up because drag isn't active.
    state.rebuild_editor_hover_outline_vertices();
    let count = state
        .render
        .meshes
        .editor_hover_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("hover outline mesh should still exist before marquee drag activates");
    assert_eq!(
        count, 36,
        "Should still only have one block outlined if marquee drag hasn't started"
    );

    // 3. Drag far enough for marquee to be active
    state.editor.input.marquee_current_screen = Some([200.0, 200.0]);
    let (_, _, is_active) = state.editor.marquee_selection_rect_screen().unwrap();
    assert!(is_active, "Marquee SHOULD be active now");
}

#[tokio::test]
async fn selection_outline_builds_instances_per_selected_block() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.input.mode = EditorMode::Select;
    state.editor.objects.push(LevelObject::default());
    state.editor.objects.push(LevelObject {
        position: [2.0, 0.0, 0.0],
        ..LevelObject::default()
    });

    state.editor.input.selected_block_index = Some(0);
    state.editor.input.selected_block_indices = vec![0, 1];

    state.rebuild_editor_selection_outline_vertices();

    assert_eq!(
        state.render.meshes.editor_selection_outline_instances.len(),
        2
    );
    let instances = &state.render.meshes.editor_selection_outline_instances;
    for instance in instances {
        assert_eq!(
            instance.outline_vertices.end - instance.outline_vertices.start,
            36
        );
        assert_eq!(
            instance.mask_vertices.end - instance.mask_vertices.start,
            36
        );
    }

    let total_outline = state
        .render
        .meshes
        .editor_selection_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection outline mesh should exist");
    assert_eq!(total_outline, 72);

    let total_mask = state
        .render
        .meshes
        .editor_selection_stencil
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection mask mesh should exist");
    assert_eq!(total_mask, 72);
}

#[tokio::test]
async fn selection_outline_uses_single_bounds_mesh_for_large_selections() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.input.mode = EditorMode::Select;

    for index in 0..701 {
        state.editor.objects.push(LevelObject {
            position: [index as f32, 0.0, 0.0],
            ..LevelObject::default()
        });
    }
    state.editor.input.selected_block_index = Some(0);
    state.editor.input.selected_block_indices = (0..701).collect();

    state.rebuild_editor_selection_outline_vertices();

    assert_eq!(
        state.render.meshes.editor_selection_outline_instances.len(),
        1
    );
    let instance = &state.render.meshes.editor_selection_outline_instances[0];
    assert_eq!(
        instance.outline_vertices.end - instance.outline_vertices.start,
        36
    );
    assert_eq!(
        instance.mask_vertices.end - instance.mask_vertices.start,
        36
    );

    let total_outline = state
        .render
        .meshes
        .editor_selection_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection outline mesh should exist");
    assert_eq!(total_outline, 36);

    let total_mask = state
        .render
        .meshes
        .editor_selection_stencil
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection mask mesh should exist");
    assert_eq!(total_mask, 36);
}

// ── EditorDirtyFlags contract tests ─────────────────────────────
#[tokio::test]
async fn dirty_flags_default_is_clean() {
    let flags = EditorDirtyFlags::default();
    assert!(!flags.any());
}

#[tokio::test]
async fn state_world_is_seeded_with_default_resources() {
    // Phase 2b: every newly-constructed `State` carries a `World` that has
    // been pre-populated with the default-constructible eg-state resources
    // via `register_state_resources`. Verify the canonical resources are
    // present and at their default values.
    let state = State::new_test().await;

    let world = state.world();
    assert_eq!(
        world.resource::<AppPhaseRes>().get(),
        eg_types::AppPhase::Menu,
        "AppPhaseRes should default to Menu"
    );
    assert!(
        world.get_resource::<SessionStateRes>().is_some(),
        "SessionStateRes should be registered"
    );
    assert!(
        world.get_resource::<EditorInputRes>().is_some(),
        "EditorInputRes should be registered"
    );
    assert!(
        world.get_resource::<EditorRuntimeRes>().is_some(),
        "EditorRuntimeRes should be registered"
    );
    assert!(
        world.get_resource::<PerfRes>().is_some(),
        "PerfRes should be registered"
    );
}

#[tokio::test]
async fn tick_state_is_safe_to_call_with_empty_schedule() {
    // Phase 2b: `tick_state` runs the (currently empty) `StateSchedule`
    // and must not panic. Future phases add systems; this test guards
    // against regressions in the call site.
    let mut state = State::new_test().await;
    state.tick_state();
    state.tick_state();
    state.tick_state();
}

#[tokio::test]
async fn tick_state_runs_phase_transition_system() {
    // Phase 2c: `tick_state` now runs the `phase_transition_system`
    // which detects changes to `AppPhaseRes` and runs `OnEnter` /
    // `OnExit` side effects. Demonstrate the bookkeeping works:
    // - the first tick is a no-op (current == previous == Menu)
    // - after we change the resource, the next tick updates the
    //   `previous_app_phase` field.
    let mut state = State::new_test().await;

    // Initial tick: no transition.
    state.tick_state();
    assert_eq!(
        state.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Menu
    );

    // Change the phase manually (in later phases this happens via
    // a `NavigationEvent`; for Phase 2c we set it directly).
    state
        .world_mut()
        .resource_mut::<AppPhaseRes>()
        .set(eg_types::AppPhase::Playing);

    // Tick: should detect the change and update `previous_app_phase`.
    state.tick_state();
    assert_eq!(
        state.world().resource::<AppPhaseRes>().get(),
        eg_types::AppPhase::Playing
    );
    assert_eq!(
        state.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Playing
    );

    // Subsequent tick with no change: no-op.
    state.tick_state();
    assert_eq!(
        state.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Playing
    );
}

// ── Phase 3 — command / event fragmentation tests ───────────────
#[tokio::test]
async fn tick_state_drains_message_into_pending_then_dispatches() {
    // Phase 3: writing a Bevy `Message` into the world's
    // `Messages<T>` resource, then calling `State::tick_state()`,
    // must route the message through the per-event drain system into
    // `PendingCommands` and finally into the legacy dispatch path
    // (`State::dispatch` / `State::dispatch_editor`). The
    // `PhaseTransition` resource remains in the world as a
    // `TickState` side effect and is verified by the prior test.
    use crate::events::PendingCommands;
    use crate::events::editor_mode::SetEditorMode;
    use crate::events::navigation::TurnRight;
    use eg_types::EditorMode;

    let mut state = State::new_test().await;

    // Write a TurnRight message.
    state.world_mut().write_message(TurnRight);
    // Write a SetEditorMode(Place) message.
    state
        .world_mut()
        .write_message(SetEditorMode(EditorMode::Place));

    // `PendingCommands` is currently empty.
    assert!(state.world().resource::<PendingCommands>().is_empty());

    // Tick: per-event drain systems fire, stage commands in
    // `PendingCommands`, then `dispatch_pending_commands` runs the
    // legacy dispatch path and clears the buffer.
    state.tick_state();

    // After the tick, the buffer is empty (forwarded to dispatch).
    assert!(
        state.world().resource::<PendingCommands>().is_empty(),
        "PendingCommands must be drained after tick_state"
    );

    // Side effects of the dispatched commands:
    // - TurnRight in Menu phase starts the selected level (which
    //   moves the phase to Playing). The test state was constructed
    //   in Menu with a default selected_level; we don't assert the
    //   exact level here, just that the command was processed.
    // - SetEditorMode(Place) sets `editor.input.mode` to Place.
    //   We're in the Menu phase so editor state isn't initialized;
    //   set the phase to Editor first and re-tick to verify the
    //   editor command dispatches.
    state.phase = eg_types::AppPhase::Editor;
    state.enter_editor_phase("Phase3Test".to_string());
    state
        .world_mut()
        .write_message(SetEditorMode(EditorMode::Scale));
    state.tick_state();
    assert_eq!(
        state.editor.input.mode,
        EditorMode::Scale,
        "SetEditorMode message should have been dispatched to the editor"
    );
}

#[tokio::test]
async fn tick_state_handles_undo_via_message_pipeline() {
    // Verify that the editor Undo message routes through the
    // pipeline and calls `State::editor_undo`. We don't have a
    // pre-existing undo history to pop, but the call must be safe.
    use crate::events::PendingCommands;
    use crate::events::history::Undo;

    let mut state = State::new_test().await;
    state.enter_editor_phase("Phase3UndoTest".to_string());

    // Buffer is empty before the tick.
    assert!(state.world().resource::<PendingCommands>().is_empty());

    state.world_mut().write_message(Undo);
    state.tick_state();

    // After the tick, the buffer is empty (forwarded).
    assert!(state.world().resource::<PendingCommands>().is_empty());
    // The editor_undo call is safe with an empty history; no
    // specific assertion on the resulting state.
}

#[tokio::test]
async fn tick_state_handles_practice_mode_payload() {
    // Verify that a message carrying a payload (`GameSetPracticeMode`)
    // routes through the pipeline and toggles the practice-mode
    // flag on the session state.
    use crate::events::PendingCommands;
    use crate::events::navigation::GameSetPracticeMode;

    let mut state = State::new_test().await;
    state.enter_editor_phase("Phase3PracticeTest".to_string());
    state.phase = eg_types::AppPhase::Playing;
    state.session.playtesting_editor = false;

    assert!(!state.session.practice_mode_enabled);

    state
        .world_mut()
        .write_message(GameSetPracticeMode { enabled: true });
    state.tick_state();

    assert!(
        state.world().resource::<PendingCommands>().is_empty(),
        "PendingCommands should be drained after tick_state"
    );
    assert!(
        state.session.practice_mode_enabled,
        "GameSetPracticeMode(true) message should have been dispatched"
    );
}

#[tokio::test]
async fn tick_state_mirrors_frame_input_into_render_resource() {
    use crate::state::render::mirror::take_pending_mirror;
    use crate::state::render::resources::RenderFrameInputRes;

    // Drain any mirror left over from a previous test (the mirror
    // slot is a process-global resource; tests that don't go
    // through `tick_state` can leave stale closures behind).
    let _ = take_pending_mirror();

    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.input.mode = EditorMode::Place;
    state.session.editor_sky_color = [0.25, 0.5, 0.75];

    // Before tick_state runs, the resource is at its default
    // (AppPhase::Menu).
    let pre = state.world().resource::<RenderFrameInputRes>().0.phase;
    assert_eq!(pre, AppPhase::Menu);

    state.tick_state();

    // After tick_state runs, the mirror has populated the resource
    // with the current phase + editor mode + sky color.
    let frame_input = state.world().resource::<RenderFrameInputRes>().0;
    assert_eq!(frame_input.phase, AppPhase::Editor);
    assert_eq!(frame_input.editor_mode, EditorMode::Place);
    assert_eq!(frame_input.sky_color, [0.25, 0.5, 0.75]);
    assert!(!frame_input.apply_gamma_correction);
}

#[tokio::test]
async fn tick_state_registers_render_pod_resources_on_construction() {
    use crate::state::render::resources::render_pod_resources_present;
    use crate::state::render::resources::{
        RenderFrameInputRes, RenderResizeRequest, RenderUniformsRes,
    };

    let state = State::new_test().await;
    let world = state.world();

    assert!(render_pod_resources_present(world));
    assert!(world.contains_resource::<RenderUniformsRes>());
    assert!(world.contains_resource::<RenderFrameInputRes>());
    assert!(world.contains_resource::<RenderResizeRequest>());
}

#[tokio::test]
async fn tick_state_poll_channels_emits_audio_import_result_event() {
    use crate::events::async_results::AudioImportResultEvent;
    use crate::state::async_bridge::AudioImportChannelRes;
    use bevy_ecs::message::Messages;

    let mut state = State::new_test().await;

    // Create a fresh channel whose receiver sits in the Bevy
    // world resource (and whose sender we control from the test).
    // Phase 6a's `poll_channels_system` drains this resource once
    // per frame and emits the corresponding result event. The
    // legacy `State::audio.state.editor.audio_import_channel.1`
    // is left alone (Phase 6b will share the receiver via Arc).
    let (tx, rx) = std::sync::mpsc::channel::<(String, Vec<u8>)>();
    state
        .world_mut()
        .insert_resource(AudioImportChannelRes(std::sync::Arc::new(
            std::sync::Mutex::new(rx),
        )));

    // Send an audio-import result. The poll system drains
    // receivers once per frame; tick_state runs the schedule
    // which includes the poll system.
    tx.send(("level1.ogg".to_string(), vec![1, 2, 3, 4]))
        .unwrap();

    state.tick_state();

    // The event should be in the Messages<AudioImportResultEvent>
    // buffer. We read it via the resource API.
    let messages = state.world().resource::<Messages<AudioImportResultEvent>>();
    let mut reader = messages.get_cursor();
    let events: Vec<_> = reader.read(messages).collect();
    assert_eq!(events.len(), 1, "expected one audio import event");
    assert_eq!(events[0].filename, "level1.ogg");
    assert_eq!(events[0].bytes, Some(vec![1, 2, 3, 4]));
}

#[tokio::test]
async fn tick_state_poll_channels_emits_level_import_result_event() {
    use crate::events::async_results::LevelImportResultEvent;
    use crate::state::async_bridge::LevelImportChannelRes;
    use bevy_ecs::message::Messages;

    let mut state = State::new_test().await;

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    state
        .world_mut()
        .insert_resource(LevelImportChannelRes(std::sync::Arc::new(
            std::sync::Mutex::new(rx),
        )));

    tx.send(vec![0xDE, 0xAD, 0xBE, 0xEF]).unwrap();

    state.tick_state();

    let messages = state.world().resource::<Messages<LevelImportResultEvent>>();
    let mut cursor = messages.get_cursor();
    let events: Vec<_> = cursor.read(messages).collect();
    assert_eq!(events.len(), 1, "expected one level import event");
    assert_eq!(events[0].bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[tokio::test]
async fn tick_state_poll_channels_emits_auth_service_result_event() {
    use crate::events::async_results::AuthServiceResultEvent;
    use crate::state::async_bridge::AuthChannelRes;
    use bevy_ecs::message::Messages;

    let mut state = State::new_test().await;

    let (tx, rx) = std::sync::mpsc::channel::<eg_platform::services::AuthServiceMessage>();
    state
        .world_mut()
        .insert_resource(AuthChannelRes(std::sync::Arc::new(std::sync::Mutex::new(
            rx,
        ))));

    // We use the simpler SignedOut variant that doesn't require
    // constructing an AuthSession payload.
    tx.send(eg_platform::services::AuthServiceMessage::SignedOut(Ok(())))
        .unwrap();

    state.tick_state();

    let messages = state.world().resource::<Messages<AuthServiceResultEvent>>();
    let mut cursor = messages.get_cursor();
    let events: Vec<_> = cursor.read(messages).collect();
    assert_eq!(events.len(), 1, "expected one auth event");
    assert!(matches!(
        events[0].message,
        eg_platform::services::AuthServiceMessage::SignedOut(_)
    ));
}

#[tokio::test]
async fn tick_state_does_not_insert_heavy_render_resources_yet() {
    use crate::state::render::resources::render_heavy_resources_present;
    use crate::state::render::resources::{RenderGpuRes, RenderMeshesRes, RenderSurfaceHostRes};

    let state = State::new_test().await;
    let world = state.world();

    // Phase 4a guard: the heavy resources are defined but not yet
    // inserted. The legacy `State::render` field remains the
    // authoritative source of `GpuContext` / `SceneMeshes` /
    // `SurfaceHost` until Phase 4b/4c moves ownership into the
    // world.
    assert!(!render_heavy_resources_present(world));
    assert!(!world.contains_resource::<RenderGpuRes>());
    assert!(!world.contains_resource::<RenderMeshesRes>());
    assert!(!world.contains_resource::<RenderSurfaceHostRes>());
}

#[tokio::test]
async fn tick_state_mirror_bridge_consumes_closure() {
    use crate::state::render::mirror::{install_mirror, take_pending_mirror};

    // Install a sentinel mirror; the bridge system in the schedule
    // would consume it during `tick_state` if the schedule is
    // registered. We assert that the slot is non-empty after
    // `install_mirror`, and that the schedule's bridge system
    // drains it on `tick_state`.
    install_mirror(|u, _f| {
        u.color_space.time_seconds = 42.0;
    });

    let mut state = State::new_test().await;
    state.tick_state();

    // After tick_state, the slot should be empty (the bridge system
    // consumed the closure). If the schedule is not registered,
    // the slot stays full — this assertion is the contract that
    // ties the schedule graph to the mirror bridge.
    assert!(
        take_pending_mirror().is_none(),
        "the mirror bridge system should drain the installed closure during tick_state"
    );
}

#[tokio::test]
async fn tick_state_mirrors_render_uniforms() {
    use crate::state::render::resources::RenderUniformsRes;

    let mut state = State::new_test().await;
    let expected_camera_position = state.menu.state.preview_camera_position;
    let expected_viewport = [
        state.render.gpu.config.width.max(1) as f32,
        state.render.gpu.config.height.max(1) as f32,
    ];

    state.tick_state();

    let uniforms = state.world().resource::<RenderUniformsRes>();
    assert_eq!(
        uniforms.camera.camera_position,
        [
            expected_camera_position[0],
            expected_camera_position[1],
            expected_camera_position[2],
            0.0
        ]
    );
    assert_eq!(uniforms.color_space.viewport_size, expected_viewport);
}

#[tokio::test]
async fn dirty_flags_from_object_sync_sets_all() {
    let flags = EditorDirtyFlags::from_object_sync();
    assert!(flags.sync_game_objects);
    assert!(flags.rebuild_block_mesh);
    assert!(flags.rebuild_selection_overlays);
    assert!(flags.rebuild_tap_indicators);
    assert!(flags.rebuild_preview_player);
    assert!(flags.any());
}

#[tokio::test]
async fn dirty_flags_merge_is_union() {
    let mut a = EditorDirtyFlags {
        rebuild_block_mesh: true,
        ..EditorDirtyFlags::default()
    };
    let b = EditorDirtyFlags {
        rebuild_tap_indicators: true,
        ..EditorDirtyFlags::default()
    };
    a.merge(b);
    assert!(a.rebuild_block_mesh);
    assert!(a.rebuild_tap_indicators);
    assert!(!a.sync_game_objects);
    assert!(a.any());
}

// ── Timeline position tests (pre-existing) ─────────────────────

#[tokio::test]
async fn derives_position_without_taps() {
    let step_time = 1.0 / eg_game::BASE_PLAYER_SPEED;
    let (position, direction) = derive_timeline_position(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &[],
        3.0 * step_time,
        &[],
    );
    assert!((position[0] - 0.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Forward));
}

#[tokio::test]
async fn derives_position_with_taps() {
    let step_time = 1.0 / eg_game::BASE_PLAYER_SPEED;
    let taps = [2.0 * step_time, 4.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &taps,
        5.0 * step_time,
        &[],
    );
    assert!((position[0] - 2.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Forward));
}

#[tokio::test]
async fn tap_at_zero_changes_direction() {
    let taps = [0.0];
    let (position, direction) =
        derive_timeline_position([0.0, 0.0, 0.0], Direction::Forward, &taps, 0.0, &[]);
    assert!((position[0] - 0.5).abs() < 0.1);
    assert!((position[2] - 0.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Right));
}

#[tokio::test]
async fn ignores_taps_after_step() {
    let step_time = 1.0 / eg_game::BASE_PLAYER_SPEED;
    let taps = [5.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [1.0, 0.0, 1.0],
        Direction::Forward,
        &taps,
        2.0 * step_time,
        &[],
    );
    assert!((position[0] - 1.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Forward));
}

#[tokio::test]
async fn supports_offset_spawn_with_tap() {
    let step_time = 1.0 / eg_game::BASE_PLAYER_SPEED;
    let taps = [2.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [2.0, 0.0, 2.0],
        Direction::Right,
        &taps,
        3.0 * step_time,
        &[],
    );
    assert!((position[0] - 4.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Forward));
}

#[tokio::test]
async fn falls_from_elevated_platform() {
    let objects = [LevelObject {
        position: [0.0, 2.0, 0.0],
        ..LevelObject::default()
    }];
    let (position, direction) = derive_timeline_position(
        [0.0, 3.0, 0.0],
        Direction::Forward,
        &[],
        1.0 / eg_game::BASE_PLAYER_SPEED,
        &objects,
    );
    assert!(position[1] <= 3.0);
    assert!(matches!(direction, Direction::Forward));
}

#[tokio::test]
async fn test_state_phase_integrity() {
    let mut state = State::new_test().await;
    assert_eq!(state.phase, eg_types::AppPhase::Menu);

    state.enter_editor_phase("Test Level".to_string());
    assert_eq!(state.phase, eg_types::AppPhase::Editor);

    state.toggle_editor(); // Should go back to menu from editor
    assert_eq!(state.phase, eg_types::AppPhase::Menu);
}

#[tokio::test]
async fn focused_init_path_toggle_editor_uses_selected_menu_level() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Menu;
    state.menu.state.levels = vec!["FocusedInitTest".to_string()];

    state.menu.state.selected_level = 0;
    let expected_level_name = state.menu.state.levels[0].clone();

    state.toggle_editor();

    assert_eq!(state.phase, AppPhase::Editor);
    assert_eq!(state.session.editor_level_name, Some(expected_level_name));
    assert!(state.editor.objects.is_empty());
}

fn configure_trigger_policy_parity_scene(state: &mut State, timeline_time_seconds: f32) {
    state.editor.objects = vec![LevelObject {
        id: "target".to_string(),
        position: [8.0, 0.0, 8.0],
        ..LevelObject::new("core/speedportal")
    }];
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;
    state.editor.timeline.taps.tap_times.clear();
    state.editor.objects.push(transform_trigger_object(
        0.0,
        0.0,
        [0.0, 0.0, 1.0],
        vec!["target"],
    ));
    state.editor.timeline.clock.time_seconds = timeline_time_seconds;
}

#[tokio::test]
async fn editor_playback_and_playtest_match_simulation() {
    let mut state = new_editor_state().await;
    let timeline_time_seconds = 0.35;
    configure_trigger_policy_parity_scene(&mut state, timeline_time_seconds);

    let expected = simulate_timeline_state(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.objects,
        &state.editor.timeline.taps.tap_times,
        timeline_time_seconds,
    );

    state.toggle_editor_timeline_playback();
    let playback_snapshot = state
        .editor
        .timeline
        .playback
        .runtime
        .as_ref()
        .expect("playback runtime should be initialized")
        .snapshot();

    approx::assert_abs_diff_eq!(
        playback_snapshot.position[0],
        expected.position[0],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(
        playback_snapshot.position[1],
        expected.position[1],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(
        playback_snapshot.position[2],
        expected.position[2],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(playback_snapshot.speed, expected.speed, epsilon = 1e-4);

    state.editor_playtest();
    approx::assert_abs_diff_eq!(
        state.gameplay.state.position[0],
        expected.position[0],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(
        state.gameplay.state.position[1],
        expected.position[1],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(
        state.gameplay.state.position[2],
        expected.position[2],
        epsilon = 1e-4
    );
    approx::assert_abs_diff_eq!(state.gameplay.state.speed, expected.speed, epsilon = 1e-4);
}

#[tokio::test]
async fn editor_playtest_preserves_airborne_vertical_motion() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.timeline.taps.tap_times.clear();
    state.editor.spawn.position = [0.0, 2.0, 0.0];
    state.editor.timeline.clock.time_seconds = 0.25;

    let expected = simulate_timeline_state(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.objects,
        &state.editor.timeline.taps.tap_times,
        state.editor.timeline.clock.time_seconds,
    );

    assert!(!expected.is_grounded);
    assert!(expected.vertical_velocity < 0.0);

    state.editor_playtest();

    approx::assert_abs_diff_eq!(
        state.gameplay.state.vertical_velocity,
        expected.vertical_velocity,
        epsilon = 1e-4,
    );
    assert_eq!(state.gameplay.state.is_grounded, expected.is_grounded);
}

#[tokio::test]
async fn editor_scrub_draws_ghost_trail_without_preview_head_mesh() {
    let mut state = new_editor_state().await;
    state.editor.set_mode(EditorMode::Place);
    state.set_editor_timeline_time_seconds(0.65);

    state.update();

    let trail_count = state
        .render
        .meshes
        .trail
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("trail mesh should exist after editor scrub update");
    assert!(
        trail_count > 0,
        "editor scrub should draw ghost trail vertices when timeline time is non-zero"
    );
    assert!(
        state
            .render
            .meshes
            .editor_preview_player
            .draw_data()
            .is_none(),
        "preview head mesh should remain empty"
    );
}

#[tokio::test]
async fn editor_playback_draws_ghost_trail_without_preview_head_mesh() {
    let mut state = new_editor_state().await;
    state.editor.set_mode(EditorMode::Place);
    state.set_editor_timeline_time_seconds(0.65);
    state.toggle_editor_timeline_playback();

    state.update();

    let trail_count = state
        .render
        .meshes
        .trail
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("trail mesh should exist during editor playback update");
    assert!(
        trail_count > 0,
        "editor playback should draw ghost trail vertices"
    );
    assert!(
        state
            .render
            .meshes
            .editor_preview_player
            .draw_data()
            .is_none(),
        "preview head mesh should remain empty during playback"
    );
}

#[tokio::test]
async fn test_state_input_routing() {
    let mut state = State::new_test().await;

    // Menu clicks no longer start level directly; the play button handles that.
    state.handle_primary_click(0.0, 0.0);
    assert_eq!(state.phase, eg_types::AppPhase::Menu);
}

#[tokio::test]
async fn multi_selection_clicking_rendered_gizmo_starts_gizmo_drag_not_block_drag() {
    let mut state = new_editor_state().await;
    state.editor.input.mode = EditorMode::Move;

    state.editor.objects = vec![
        LevelObject::default(),
        LevelObject {
            position: [2.0, 0.0, 0.0],
            ..LevelObject::default()
        },
    ];
    state.editor.input.selected_block_indices = vec![0, 1];
    state.sync_primary_selection_from_indices();

    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );
    let (bounds_position, bounds_size) = state
        .selected_group_bounds()
        .expect("expected group bounds for multi-selection");
    let center = Vec3::new(
        bounds_position[0] + bounds_size[0] * 0.5,
        bounds_position[1] + bounds_size[1] * 0.5,
        bounds_position[2] + bounds_size[2] * 0.5,
    );
    let axis_lengths = state
        .editor
        .gizmo_axis_lengths_world(center, 80.0, viewport);
    let move_x_handle_world = Vec3::new(center.x + axis_lengths[0], center.y, center.z);
    let move_x_handle_screen = state
        .editor
        .world_to_screen_v(move_x_handle_world, viewport)
        .expect("expected projected gizmo handle");

    state.handle_primary_click(move_x_handle_screen.x as f64, move_x_handle_screen.y as f64);

    assert!(
        state.editor.runtime.interaction.gizmo_drag.is_some(),
        "clicking rendered multi-select gizmo handle should start gizmo drag"
    );
    assert!(
        state.editor.runtime.interaction.block_drag.is_none(),
        "gizmo click must not fall through to block drag"
    );
}

#[tokio::test]
async fn timeline_seek_uses_interpolated_snapshot_cache_and_supports_backward_seek() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.objects.clear();
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;
    state.editor.timeline.taps.tap_times.clear();
    state.editor.timeline.clock.duration_seconds = 8.0;
    state.editor.invalidate_samples();

    let step = state.editor.timeline.snapshot_cache_step_seconds;

    state.set_editor_timeline_time_seconds(0.0);
    let start_position = state.editor_timeline_preview().0;

    state.set_editor_timeline_time_seconds(step);
    let end_position = state.editor_timeline_preview().0;

    state.set_editor_timeline_time_seconds(step * 0.5);
    let half_position = state.editor_timeline_preview().0;

    assert!(
        !state.editor.timeline.snapshot_cache.is_empty(),
        "timeline seek should build snapshot cache"
    );
    assert_eq!(
        state.editor.timeline.snapshot_cache_revision, state.editor.timeline.simulation_revision,
        "snapshot cache revision should match current simulation revision"
    );

    let expected_half_y = (start_position[1] + end_position[1]) * 0.5;
    assert!(
        (half_position[1] - expected_half_y).abs() < 0.02,
        "half-step seek should interpolate between adjacent cached samples"
    );

    state.set_editor_timeline_time_seconds(0.0);
    let rewound_position = state.editor_timeline_preview().0;
    assert!(
        (rewound_position[1] - start_position[1]).abs() < 0.02,
        "backward seek should resolve from snapshot cache"
    );
}

#[tokio::test]
async fn editor_playtest_stores_precomputed_audio_start_seconds() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.timeline.taps.tap_times.clear();
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;

    let target_time = 0.85;
    state.editor.timeline.clock.time_seconds = target_time;
    let expected_elapsed = state.editor_timeline_elapsed_seconds(target_time);

    state.editor_playtest();

    assert_eq!(state.phase, AppPhase::Playing);
    let stored = state
        .session
        .playtest_audio_start_seconds
        .expect("playtest should store precomputed audio start");
    assert!(
        (stored - expected_elapsed).abs() < 0.02,
        "stored playtest audio start seconds should match precomputed timeline elapsed"
    );
    assert!(
        (state.gameplay.state.elapsed_seconds - expected_elapsed).abs() < 0.02,
        "playtest gameplay elapsed_seconds should be initialized to precomputed timeline elapsed"
    );
}

#[tokio::test]
async fn editor_playtest_nonzero_timeline_handoff_stays_synced_after_first_input() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.timeline.taps.tap_times = vec![0.35, 0.7, 1.1];
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;

    let target_time = 1.25;
    state.editor.timeline.clock.time_seconds = target_time;
    let expected_elapsed = state.editor_timeline_elapsed_seconds(target_time);
    assert!(
        expected_elapsed > 0.5,
        "regression setup should use a non-trivial timeline offset"
    );

    state.editor_playtest();

    let stored = state
        .session
        .playtest_audio_start_seconds
        .expect("playtest should store audio start seconds for non-zero timeline handoff");
    assert!(
        (stored - expected_elapsed).abs() < 0.02,
        "playtest should store the non-zero timeline elapsed seconds"
    );
    assert!(
        (state.gameplay.state.elapsed_seconds - expected_elapsed).abs() < 0.02,
        "playtest should initialize gameplay elapsed_seconds from the same timeline elapsed"
    );

    state.turn_right();
    assert!(
        state.gameplay.state.started,
        "first input should start gameplay"
    );
    assert!(
        (state.gameplay.state.elapsed_seconds - expected_elapsed).abs() < 0.02,
        "first input should not reset elapsed time and trigger catch-up movement"
    );
}

#[tokio::test]
async fn start_editor_warms_audio_before_playtest() {
    let mut state = State::new_test().await;
    let level_index = 0;
    let level_name = state.menu.state.levels[level_index].clone();

    state.start_editor(level_index);

    let editor_warmup = state
        .audio
        .state
        .runtime_preload
        .last_warmup_request
        .clone()
        .expect("entering the editor should warm up runtime audio before playtest");
    let (source_key, warmup_seconds) = editor_warmup.clone();
    assert_eq!(
        source_key,
        runtime_asset_source_key(&level_name, "audio.mp3"),
        "warmup should target the active editor source"
    );
    assert!(
        warmup_seconds.abs() < f32::EPSILON,
        "editor entry warmup should decode PCM at the beginning of the track"
    );

    state.editor.timeline.clock.time_seconds = 0.85;
    state.editor_playtest();

    assert_eq!(
        state.audio.state.runtime_preload.last_warmup_request,
        Some(editor_warmup),
        "playtest should reuse the editor-entry PCM warmup instead of warming on transition"
    );
    assert!(
        !state.gameplay.state.started,
        "gameplay should still wait for first input after entering playtest"
    );
}

#[tokio::test]
async fn editor_load_level_in_timing_mode_reloads_waveform_for_new_level() {
    let mut state = new_editor_state().await;
    let next_level = "Synthetic Timing Level".to_string();

    state.editor.input.mode = EditorMode::Timing;
    state.editor.timing.waveform_samples = vec![0.25, -0.5, 0.75];
    state.editor.timing.waveform_sample_rate = 44_100;

    state.set_editor_level_name(next_level.clone());
    state.load_waveform_for_current_audio();

    assert!(
        state.editor.timing.waveform_samples.is_empty(),
        "switching levels in timing mode should clear stale waveform samples before loading"
    );
    assert_eq!(
        state.editor.timing.waveform_sample_rate, 0,
        "switching levels in timing mode should reset waveform sample rate before loading"
    );

    let expected_source_key = runtime_asset_source_key(&next_level, "audio.mp3");
    assert_eq!(
        state.audio.state.editor.cache.loading_source(),
        Some(expected_source_key.as_str()),
        "switching levels in timing mode should start loading waveform for the new level"
    );
}

#[tokio::test]
async fn editor_clipboard_survives_builtin_level_switch() {
    let mut state = new_editor_state().await;
    let next_level = state
        .menu
        .state
        .levels
        .first()
        .cloned()
        .expect("test requires at least one built-in level");

    state.editor.objects = vec![LevelObject {
        position: [2.0, 1.0, 3.0],
        ..LevelObject::default()
    }];
    state.editor.input.selected_block_index = Some(0);
    state.editor.input.selected_block_indices = vec![0];
    state.editor_copy_block();

    state.load_builtin_level_into_editor(&next_level);

    assert!(state.editor.runtime.interaction.clipboard.is_some());
    state.editor.input.cursor = [8.0, 2.0, 9.0];
    let before_paste = state.editor.objects.len();
    state.editor_paste_block();

    assert_eq!(state.editor.objects.len(), before_paste + 1);
    assert_eq!(state.editor.objects[before_paste].position, [8.0, 2.0, 9.0]);
}

#[tokio::test]
async fn editor_clipboard_survives_exit_and_reentry() {
    let mut state = new_editor_state().await;

    state.editor.objects = vec![LevelObject {
        position: [1.0, 0.0, 1.0],
        ..LevelObject::default()
    }];
    state.editor.input.selected_block_index = Some(0);
    state.editor.input.selected_block_indices = vec![0];
    state.editor_copy_block();

    state.back_to_menu();
    state.start_editor(0);

    assert!(state.editor.runtime.interaction.clipboard.is_some());
    state.editor.input.cursor = [4.0, 0.0, 5.0];
    let before_paste = state.editor.objects.len();
    state.editor_paste_block();

    assert_eq!(state.editor.objects.len(), before_paste + 1);
    assert_eq!(state.editor.objects[before_paste].position, [4.0, 0.0, 5.0]);
}

#[tokio::test]
async fn setting_timeline_duration_does_not_delete_taps() {
    let mut state = new_editor_state().await;

    // Add some taps beyond 0 duration
    state.editor.timeline.taps.tap_times = vec![1.0, 2.0, 3.0];
    state.editor.timeline.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0]; 3];

    // Set duration to 0
    state.set_editor_timeline_duration_seconds(0.0);

    // Taps should still be there
    assert_eq!(state.editor.timeline.taps.tap_times, vec![1.0, 2.0, 3.0]);
    assert_eq!(state.editor.timeline.taps.tap_indicator_positions.len(), 3);
}

#[tokio::test]
async fn setting_spawn_recomputes_tap_indicator_positions() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;
    state.editor.timeline.taps.tap_times = vec![1.0 / eg_game::BASE_PLAYER_SPEED];
    state.editor.timeline.taps.tap_indicator_positions = derive_tap_indicator_positions(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.timeline.taps.tap_times,
        &state.editor.objects,
    );

    state.editor.input.cursor = [5.0, 2.0, 0.0];
    state.editor_set_spawn_here();

    let expected = derive_tap_indicator_positions(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.timeline.taps.tap_times,
        &state.editor.objects,
    );

    assert_eq!(state.editor.spawn.position, [5.0, 2.0, 0.0]);
    assert_eq!(state.editor.timeline.taps.tap_indicator_positions, expected);
}

#[tokio::test]
async fn context_cursor_update_forces_cursor_in_select_mode_while_right_dragging() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.input.mode = EditorMode::Select;
    state.editor.input.right_dragging = true;
    state.editor.input.cursor = [99.0, 99.0, 99.0];

    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );
    let screen_x = viewport.x as f64 * 0.5;
    let screen_y = viewport.y as f64 * 0.5;
    let expected_cursor = state
        .editor
        .pick_from_screen(screen_x, screen_y, viewport)
        .expect("expected context pick")
        .cursor;
    state.editor.input.cursor = [99.0, 99.0, 99.0];

    state.dispatch(AppCommand::Editor(EditorCommand::UpdateCursorFromScreen {
        x: screen_x,
        y: screen_y,
    }));

    assert_eq!(state.editor.input.cursor, expected_cursor);
}

#[tokio::test]
async fn spawn_hotkey_updates_spawn_from_pointer_in_select_mode() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.input.mode = EditorMode::Select;
    state.editor.input.cursor = [99.0, 99.0, 99.0];

    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );
    let screen_x = viewport.x as f64 * 0.5;
    let screen_y = viewport.y as f64 * 0.5;
    let expected_cursor = state
        .editor
        .pick_from_screen(screen_x, screen_y, viewport)
        .expect("expected pointer pick")
        .cursor;
    state.editor.input.cursor = [99.0, 99.0, 99.0];
    state.editor.input.pointer_screen = Some([screen_x, screen_y]);

    state.dispatch(AppCommand::Editor(EditorCommand::SetSpawnHere));

    assert_eq!(state.editor.spawn.position, expected_cursor);
}

#[tokio::test]
async fn test_handle_primary_click_shift_priority() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.input.mode = EditorMode::Select;

    // Scenario 1: Shift is held. Marquee should start even if we are over a selected block.
    state.editor.input.shift_held = true;

    // Add a block and select it
    state.editor.objects.push(LevelObject {
        position: [-5.0, -5.0, -5.0],
        size: [10.0, 10.0, 10.0],
        ..LevelObject::default()
    });
    state.editor.input.selected_block_indices.push(0);

    // Set camera to look at origin from above
    state.editor.camera.editor_pan = [0.0, 0.0];
    state.editor.camera.editor_target_z = 0.0;
    state.editor.camera.editor_pitch = -std::f32::consts::FRAC_PI_2; // Looking straight down
    state.editor.camera.editor_yaw = 0.0;

    // Click at the center (400, 300 in a 800x600 viewport)
    state.handle_primary_click(400.0, 300.0);

    assert!(
        state.editor.input.marquee_start_screen.is_some(),
        "Marquee should have started when Shift is held"
    );
    assert!(
        state.editor.runtime.interaction.block_drag.is_none(),
        "Block drag should NOT have started when Shift is held"
    );

    // Scenario 2: Shift is NOT held. Block drag should start.
    state.editor.input.marquee_start_screen = None;
    state.editor.set_left_mouse_down(false);
    state.editor.input.shift_held = false;

    state.handle_primary_click(400.0, 300.0);

    assert!(
        state.editor.runtime.interaction.block_drag.is_some(),
        "Block drag should have started when Shift is NOT held"
    );
}

#[tokio::test]
async fn editor_mode_selection_constraints() {
    assert!(EditorMode::Select.can_select());
    assert!(EditorMode::Move.can_select());
    assert!(EditorMode::Scale.can_select());
    assert!(EditorMode::Place.can_select());
    assert!(!EditorMode::Tapping.can_select());
    assert!(!EditorMode::Timing.can_select());
    assert!(!EditorMode::Null.can_select());
}

#[tokio::test]
async fn editor_playback_mode_switching() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Place);

    // Start playback
    state.toggle_editor_timeline_playback();
    assert!(state.editor.is_playing());
    assert_eq!(state.editor.input.mode, EditorMode::Null);
    assert_eq!(
        state.editor.runtime.interaction.last_mode,
        Some(EditorMode::Place)
    );

    // Stop playback
    state.toggle_editor_timeline_playback();
    assert!(!state.editor.is_playing());
    assert_eq!(state.editor.input.mode, EditorMode::Place);
    assert!(state.editor.runtime.interaction.last_mode.is_none());
}

#[tokio::test]
async fn editor_null_mode_clears_selection() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.objects.push(LevelObject::default());
    state.editor.input.selected_block_index = Some(0);
    state.editor.set_mode(EditorMode::Place);
    assert_eq!(state.editor.input.selected_block_index, Some(0));

    // Switching to Null should clear selection
    state.editor.set_mode(EditorMode::Null);
    assert!(state.editor.input.selected_block_index.is_none());
}

#[tokio::test]
async fn test_timing_mode_persistence_during_playback() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;

    // 1. Enter Timing Mode
    state.editor.set_mode(EditorMode::Timing);
    assert_eq!(state.editor.input.mode, EditorMode::Timing);

    // 2. Toggle playback
    state.toggle_editor_timeline_playback();

    // Mode should be Null for playback safety
    assert_eq!(state.editor.input.mode, EditorMode::Null);
    // last_mode should be Timing
    assert_eq!(
        state.editor.runtime.interaction.last_mode,
        Some(EditorMode::Timing)
    );

    // 3. Verify ViewModel shows we are still conceptually in Timing
    let view = state.ui_view_model();
    let is_timing = view.mode == EditorMode::Timing
        || (view.mode == EditorMode::Null && view.last_mode == Some(EditorMode::Timing));
    assert!(
        is_timing,
        "UI should still consider itself in Timing mode during playback"
    );

    // 4. Toggle playback off
    state.toggle_editor_timeline_playback();

    // Should return to Timing mode
    assert_eq!(state.editor.input.mode, EditorMode::Timing);
    assert!(state.editor.runtime.interaction.last_mode.is_none());
}

#[tokio::test]
async fn timing_origin_playback_seek_resync_keeps_runtime_disabled() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);

    state.editor.objects.push(transform_trigger_object(
        0.5,
        1.0,
        [3.0, 0.0, 0.0],
        vec!["object_0_0_0"],
    ));

    state.toggle_editor_timeline_playback();
    assert!(state.editor.timeline.playback.playing);
    assert_eq!(state.editor.input.mode, EditorMode::Null);
    assert_eq!(
        state.editor.runtime.interaction.last_mode,
        Some(EditorMode::Timing)
    );

    state.editor.runtime.dirty = EditorDirtyFlags::default();
    state.set_editor_timeline_time_seconds(1.0);

    assert!(
        !state.editor.runtime.dirty.rebuild_block_mesh,
        "timing-origin playback seek should not mark block mesh dirty"
    );

    state.resync_editor_timeline_playback_audio();
    assert!(
        state.editor.timeline.playback.runtime.is_none(),
        "timing-origin playback seek resync should not build simulation runtime"
    );
}

#[tokio::test]
async fn playback_seek_queues_debounced_audio_resync() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);

    state.toggle_editor_timeline_playback();
    assert!(state.editor.timeline.playback.playing);

    state.set_editor_timeline_time_seconds(0.75);

    assert_eq!(
        state.editor.timeline.playback.pending_seek_time_seconds,
        Some(0.75)
    );
    assert!(state.editor.timeline.playback.seek_resync_cooldown_seconds > 0.0);
}

#[tokio::test]
async fn playback_shift_timeline_command_queues_debounced_audio_resync() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);
    state.editor.timeline.clock.time_seconds = 0.5;

    state.toggle_editor_timeline_playback();
    assert!(state.editor.timeline.playback.playing);

    state.dispatch(AppCommand::Editor(EditorCommand::ShiftTimeline(0.1)));

    assert!((state.editor.timeline.clock.time_seconds - 0.6).abs() < 1e-5);
    assert_eq!(
        state.editor.timeline.playback.pending_seek_time_seconds,
        Some(0.6)
    );
    assert!(state.editor.timeline.playback.seek_resync_cooldown_seconds > 0.0);
}

#[tokio::test]
async fn test_gizmo_move_shaft_is_pickable() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Move);

    state.editor.objects.push(LevelObject::default());
    state.editor.input.selected_block_indices.push(0);
    state.sync_editor_objects();

    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );

    let (bounds_position, bounds_size) = state
        .selected_group_bounds()
        .expect("expected group bounds");
    let center = Vec3::new(
        bounds_position[0] + bounds_size[0] * 0.5,
        bounds_position[1] + bounds_size[1] * 0.5,
        bounds_position[2] + bounds_size[2] * 0.5,
    );
    let axis_lengths = state
        .editor
        .gizmo_axis_lengths_world(center, 100.0, viewport);

    // Test the midpoint of the X+ arrow shaft (not the tip)
    let shaft_mid_world = center + Vec3::X * axis_lengths[0] * 0.5;
    let shaft_mid_screen = state
        .editor
        .world_to_screen_v(shaft_mid_world, viewport)
        .expect("expected projected shaft midpoint");

    let picked = state.editor.pick_gizmo_handle(
        shaft_mid_screen.x as f64,
        shaft_mid_screen.y as f64,
        viewport,
    );

    assert_eq!(
        picked,
        Some((GizmoDragKind::Move, GizmoAxis::X)),
        "midpoint of move arrow shaft should be pickable"
    );

    // Test a point 75% along the X+ shaft (far enough from center to avoid Y axis ambiguity)
    let shaft_quarter_world = center + Vec3::X * axis_lengths[0] * 0.75;
    let shaft_quarter_screen = state
        .editor
        .world_to_screen_v(shaft_quarter_world, viewport)
        .expect("expected projected shaft quarter point");

    let picked_quarter = state.editor.pick_gizmo_handle(
        shaft_quarter_screen.x as f64,
        shaft_quarter_screen.y as f64,
        viewport,
    );

    assert_eq!(
        picked_quarter,
        Some((GizmoDragKind::Move, GizmoAxis::X)),
        "quarter point of move arrow shaft should be pickable"
    );
}

#[tokio::test]
async fn test_gizmo_hover_priority_suppresses_block_outline() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Move);

    // 1. Add a primary block and select it to show the gizmo
    state.editor.objects.push(LevelObject::default());
    state.editor.input.selected_block_indices.push(0);
    state.sync_editor_objects();

    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );

    // 2. Find screen position of a gizmo handle (e.g. Move X)
    let (bounds_position, bounds_size) = state
        .selected_group_bounds()
        .expect("expected group bounds");
    let center = Vec3::new(
        bounds_position[0] + bounds_size[0] * 0.5,
        bounds_position[1] + bounds_size[1] * 0.5,
        bounds_position[2] + bounds_size[2] * 0.5,
    );
    let axis_lengths = state
        .editor
        .gizmo_axis_lengths_world(center, 100.0, viewport);
    let move_x_handle_world = Vec3::new(center.x + axis_lengths[0], center.y, center.z);
    let move_x_handle_screen = state
        .editor
        .world_to_screen_v(move_x_handle_world, viewport)
        .expect("expected projected gizmo handle");

    // 3. Add a second block exactly where the gizmo handle is projected
    // This ensures that a raycast from the mouse position *would* hit a block.
    state.editor.objects.push(LevelObject {
        position: [
            move_x_handle_world.x - 0.5,
            move_x_handle_world.y - 0.5,
            move_x_handle_world.z - 0.5,
        ],
        ..LevelObject::default()
    });
    state.sync_editor_objects();

    // 4. Verify that picking at the gizmo handle's screen coordinates hits the second block.
    // This confirms the test scenario: block underneath the gizmo.
    let pick = state
        .editor
        .pick_from_screen(
            move_x_handle_screen.x as f64,
            move_x_handle_screen.y as f64,
            viewport,
        )
        .expect("expected pick to hit the second block behind the gizmo handle");
    assert_eq!(pick.hit_block_index, Some(1));

    // 5. Move pointer to the gizmo handle.
    state.handle_pointer_moved(move_x_handle_screen.x as f64, move_x_handle_screen.y as f64);

    // 6. Assertions: Gizmo should be hovered, and block hover should be SUPPRESSED.
    assert!(
        state.editor.runtime.interaction.hovered_gizmo.is_some(),
        "expected gizmo handle to be hovered"
    );
    assert!(
        state.editor.input.hovered_block_index.is_none(),
        "expected block hover to be suppressed while gizmo handle is hovered, even though a block is behind it"
    );
}

#[tokio::test]
async fn timing_mode_timeline_scrub_skips_simulation_and_mesh_dirty() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);

    // Add an object transform trigger to ensure that in Compose it WOULD mark dirty
    state.editor.objects.push(transform_trigger_object(
        1.0,
        1.0,
        [10.0, 0.0, 0.0],
        vec!["object_0_0_0"],
    ));

    // Clear any initial dirty flags
    state.editor.runtime.dirty = EditorDirtyFlags::default();
    state.editor.timeline.snapshot_cache.clear();
    state.editor.timeline.snapshot_cache_revision = 0;

    // Scrub
    state.set_editor_timeline_time_seconds(0.5);

    // Verify:
    // 1. rebuild_block_mesh is NOT dirty
    assert!(!state.editor.runtime.dirty.rebuild_block_mesh);
    // 2. Snapshot cache is still empty (simulation was not run)
    assert!(state.editor.timeline.snapshot_cache.is_empty());
}

#[tokio::test]
async fn compose_mode_timeline_scrub_runs_simulation_and_mesh_dirty() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Place);

    // Add an object transform trigger
    state.editor.objects.push(transform_trigger_object(
        1.0,
        1.0,
        [10.0, 0.0, 0.0],
        vec!["object_0_0_0"],
    ));

    // Clear any initial dirty flags
    state.editor.runtime.dirty = EditorDirtyFlags::default();

    // Scrub
    state.set_editor_timeline_time_seconds(0.5);

    // Verify:
    // 1. rebuild_block_mesh IS dirty because of triggers
    assert!(state.editor.runtime.dirty.rebuild_block_mesh);
    // 2. Snapshot cache is NOT empty (simulation was run for preview)
    assert!(!state.editor.timeline.snapshot_cache.is_empty());
}

#[tokio::test]
async fn camera_trigger_timeline_scrub_marks_block_mesh_dirty() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Place);

    state.editor.objects.push(camera_trigger_object(1.0));

    state.editor.runtime.dirty = EditorDirtyFlags::default();
    state.set_editor_timeline_time_seconds(0.5);

    assert!(state.editor.runtime.dirty.rebuild_block_mesh);
}

#[tokio::test]
async fn state_navigation_resize_and_phase_queries_cover_core_branches() {
    let mut state = State::new_test().await;

    assert!(state.is_menu());
    assert!(!state.is_editor());

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = state.window();
    }

    let initial_size = PhysicalSize::new(
        state.render.gpu.config.width,
        state.render.gpu.config.height,
    );
    state.resize_surface(PhysicalSize::new(0, initial_size.height));
    assert_eq!(state.render.gpu.config.width, initial_size.width);
    assert_eq!(state.render.gpu.config.height, initial_size.height);

    let level_count = state.menu.state.levels.len();
    state.menu.state.selected_level = level_count - 1;
    state.next_level();
    assert_eq!(state.menu.state.selected_level, 0);

    state.prev_level();
    assert_eq!(state.menu.state.selected_level, level_count - 1);

    state.menu.state.selected_level = 1;
    state.prev_level();
    assert_eq!(state.menu.state.selected_level, 0);

    state.enter_editor_phase("Navigation Branches".to_string());
    state.editor.input.cursor = [0.0, 2.0, 0.0];
    state.next_level();
    assert_eq!(state.editor.input.cursor, [1.0, 2.0, 0.0]);
    state.prev_level();
    assert_eq!(state.editor.input.cursor, [0.0, 2.0, 0.0]);

    assert!(state.is_editor());
    assert!(!state.is_menu());
}

#[tokio::test]
async fn turn_right_covers_menu_playing_editor_and_game_over_paths() {
    let mut state = State::new_test().await;
    let first_level = "TurnRightBranchTest".to_string();

    state.menu.state.levels = vec![first_level.clone()];
    state.menu.state.selected_level = 0;

    state.turn_right();
    assert_eq!(state.phase, AppPhase::Playing);

    state.phase = AppPhase::Playing;
    state.session.playtesting_editor = false;
    state.session.playing_level_name = Some(first_level);
    state.gameplay.state.started = false;
    state.gameplay.state.game_over = false;
    state.turn_right();
    assert!(state.gameplay.state.started);

    state.gameplay.state.started = true;
    state.gameplay.state.game_over = true;
    state.turn_right();
    assert!(!state.gameplay.state.started);

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Place);
    state.editor.input.cursor = [2.0, 0.0, 2.0];
    let before = state.editor.objects.len();
    state.turn_right();
    assert_eq!(state.editor.objects.len(), before + 1);

    state.phase = AppPhase::GameOver;
    state.turn_right();
    assert_eq!(state.phase, AppPhase::Menu);
}

#[tokio::test]
async fn toggle_editor_and_mouse_paths_cover_playtest_and_release_guards() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Menu;
    state.menu.state.levels = vec!["Missing test level".to_string()];
    state.toggle_editor();
    assert_eq!(state.phase, AppPhase::Editor);
    assert_eq!(
        state.session.editor_level_name,
        Some("Missing test level".to_string())
    );

    state.phase = AppPhase::Playing;
    state.session.playtesting_editor = true;
    state.toggle_editor();
    assert_eq!(state.phase, AppPhase::Editor);

    state.phase = AppPhase::Playing;
    state.session.playtesting_editor = false;
    state.toggle_editor();
    assert_eq!(state.phase, AppPhase::Playing);

    state.phase = AppPhase::Editor;
    state.editor.input.marquee_start_screen = Some([32.0, 64.0]);
    state.editor.input.marquee_current_screen = Some([48.0, 80.0]);
    state.handle_mouse_button(0, false);
    assert!(state.editor.input.marquee_start_screen.is_none());
    assert!(state.editor.input.marquee_current_screen.is_none());

    state.handle_mouse_button(2, true);
    assert!(state.editor.input.right_dragging);
    state.handle_mouse_button(2, false);
    assert!(!state.editor.input.right_dragging);

    state.phase = AppPhase::Menu;
    state.handle_mouse_button(0, true);
    assert_eq!(state.phase, AppPhase::Menu);
}

#[tokio::test]
async fn handle_primary_click_covers_tapping_trigger_and_timing_mode_paths() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;

    state.editor.set_mode(EditorMode::Timing);
    state.handle_primary_click(220.0, 180.0);
    assert_eq!(state.editor.input.pointer_screen, Some([220.0, 180.0]));
    assert!(state.editor.input.left_mouse_down);
    assert!(state.editor.input.marquee_start_screen.is_none());

    state.editor.set_left_mouse_down(false);
    state.editor.set_mode(EditorMode::Tapping);
    state.editor.timeline.clock.duration_seconds = 4.0;
    state.set_editor_timeline_time_seconds(0.0);
    let viewport = Vec2::new(
        state.render.gpu.config.width as f32,
        state.render.gpu.config.height as f32,
    );
    let future_tap_screen = state
        .editor
        .world_to_screen_v(Vec3::new(0.5, 0.0, 4.5), viewport)
        .expect("future tap point should project to the screen");
    assert!(state.editor_tap_times().is_empty());
    state.handle_primary_click(future_tap_screen.x as f64, future_tap_screen.y as f64);
    assert_eq!(
        state.editor.input.pointer_screen,
        Some([future_tap_screen.x as f64, future_tap_screen.y as f64])
    );
    assert!(state.editor_tap_times().is_empty());

    state.editor.set_left_mouse_down(false);
    state.handle_primary_click(future_tap_screen.x as f64, future_tap_screen.y as f64);
    assert_eq!(state.editor_tap_times().len(), 1);
    assert_eq!(state.editor.timeline.taps.selected_index, Some(0));
    assert!(state.editor.input.marquee_start_screen.is_none());

    state.editor.set_left_mouse_down(false);
    let selected_tap_position = state.editor.timeline.taps.tap_indicator_positions[0];
    state
        .editor
        .world_to_screen_v(
            Vec3::new(
                selected_tap_position[0] + 0.5,
                selected_tap_position[1] + 0.1,
                selected_tap_position[2] + 0.5,
            ),
            viewport,
        )
        .expect("selected tap should project to the screen");
}
