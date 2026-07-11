/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorDirtyFlags;
use super::State;
use crate::state::{
    AppPhaseRes, EditorInputRes, EditorRuntimeRes, FrameTimeRes, MenuStateRes, PerfRes,
    SessionStateRes, TurnRightRequest,
};
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

fn tick_state_with_app(state: &mut State, app: &mut crate::state::test_support::TestApp) {
    state.prepare_app_world_for_state_schedule(app.world_mut());
    app.tick_state();
    state.drain_pending_async_results();
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
        .render_meshes()
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
            .render_meshes()
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
        .render_meshes()
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
        state
            .render_meshes()
            .editor_selection_outline_instances
            .len(),
        2
    );
    let instances = &state.render_meshes().editor_selection_outline_instances;
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
        .render_meshes()
        .editor_selection_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection outline mesh should exist");
    assert_eq!(total_outline, 72);

    let total_mask = state
        .render_meshes()
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
        state
            .render_meshes()
            .editor_selection_outline_instances
            .len(),
        1
    );
    let instance = &state.render_meshes().editor_selection_outline_instances[0];
    assert_eq!(
        instance.outline_vertices.end - instance.outline_vertices.start,
        36
    );
    assert_eq!(
        instance.mask_vertices.end - instance.mask_vertices.start,
        36
    );

    let total_outline = state
        .render_meshes()
        .editor_selection_outline
        .draw_data()
        .map(|draw_data| draw_data.count())
        .expect("selection outline mesh should exist");
    assert_eq!(total_outline, 36);

    let total_mask = state
        .render_meshes()
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
async fn app_world_is_seeded_with_default_resources() {
    let app = crate::state::test_support::TestApp::minimal();
    let world = app.world();
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
        world.get_resource::<FrameTimeRes>().is_some(),
        "FrameTimeRes should be registered"
    );
    assert!(
        world.get_resource::<PerfRes>().is_some(),
        "PerfRes should be registered"
    );
}

#[test]
fn test_app_tick_state_advances_frame_time_resource() {
    let mut app = crate::state::test_support::TestApp::minimal();
    {
        let mut frame_time = app.world_mut().resource_mut::<FrameTimeRes>();
        frame_time.last_frame = web_time::Instant::now() - std::time::Duration::from_millis(20);
    }

    app.tick_state();

    let frame_time = *app.world().resource::<FrameTimeRes>();
    assert!(
        frame_time.delta_seconds > 0.0,
        "StateSchedule must advance FrameTimeRes.delta_seconds"
    );
    assert!(
        frame_time.global_time_seconds > 0.0,
        "StateSchedule must advance FrameTimeRes.global_time_seconds"
    );
    assert!(
        frame_time.fps_smoothed > 0.0,
        "StateSchedule must update frame FPS smoothing"
    );
    assert_eq!(
        app.world().resource::<PerfRes>().0.fps_smoothed,
        frame_time.fps_smoothed,
        "PerfRes FPS must be driven from App-world frame timing"
    );
    assert_eq!(
        app.world()
            .resource::<crate::state::render::resources::RenderUniformsRes>()
            .color_space
            .time_seconds,
        frame_time.global_time_seconds,
        "RenderUniformsRes color-space time must be driven from App-world frame timing"
    );
}

#[test]
fn test_app_tick_state_resets_menu_editor_frame_accumulator() {
    let mut app = crate::state::test_support::TestApp::minimal();
    {
        app.world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(AppPhase::Menu);
        let mut frame_time = app.world_mut().resource_mut::<FrameTimeRes>();
        frame_time.last_frame = web_time::Instant::now() - std::time::Duration::from_millis(20);
        frame_time.accumulator = 0.2;
    }

    app.tick_state();

    assert_eq!(
        app.world().resource::<FrameTimeRes>().accumulator,
        0.0,
        "StateSchedule must own the non-playing accumulator reset before legacy update"
    );
}

#[tokio::test]
async fn tick_state_is_safe_to_call_with_empty_schedule() {
    let mut state = State::new_test().await;
    let mut app = crate::state::test_support::TestApp::minimal();
    tick_state_with_app(&mut state, &mut app);
    tick_state_with_app(&mut state, &mut app);
    tick_state_with_app(&mut state, &mut app);
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
    let mut app = crate::state::test_support::TestApp::minimal();

    // Initial tick: no transition.
    tick_state_with_app(&mut state, &mut app);
    assert_eq!(
        app.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Menu
    );

    // Change the phase manually (in later phases this happens via
    // a `NavigationEvent`; for Phase 2c we set it directly).
    app.world_mut()
        .resource_mut::<AppPhaseRes>()
        .set(eg_types::AppPhase::Playing);

    // Tick: should detect the change and update `previous_app_phase`.
    tick_state_with_app(&mut state, &mut app);
    assert_eq!(
        app.world().resource::<AppPhaseRes>().get(),
        eg_types::AppPhase::Playing
    );
    assert_eq!(
        app.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Playing
    );

    // Subsequent tick with no change: no-op.
    tick_state_with_app(&mut state, &mut app);
    assert_eq!(
        app.world().resource::<AppPhaseRes>().previous_app_phase,
        eg_types::AppPhase::Playing
    );
}

// ── Phase 3 — command / event fragmentation tests ───────────────
#[test]
fn test_app_tick_state_writes_command_messages_to_app_resources() {
    // Runtime owns request consumption during the compatibility window;
    // the schedule only writes App-world resources.
    use crate::events::editor_mode::SetEditorMode;
    use crate::events::navigation::TurnRight;
    use eg_types::EditorMode;

    let mut app = crate::state::test_support::TestApp::minimal();

    // Write a TurnRight message while the default test phase is Menu.
    app.world_mut().write_message(TurnRight);
    // Write a SetEditorMode(Place) message. Phase 3.13 carved
    // this out: the handler mutates `EditorInputRes.0.mode`
    // directly instead of staging a legacy
    // `EditorCommand::SetMode`. Pin the carve-out behavior: the
    // resource is updated and no editor command is staged.
    app.world_mut()
        .write_message(SetEditorMode(EditorMode::Place));

    // Tick: per-event systems fire and write App-world resources/requests.
    app.tick_state();

    assert!(
        app.world().resource::<TurnRightRequest>().pending,
        "Menu TurnRight handler must write TurnRightRequest"
    );
    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        EditorMode::Place,
        "SetEditorMode handler must mutate EditorInputRes.mode"
    );
    assert_eq!(
        app.world().resource::<AppPhaseRes>().get(),
        AppPhase::Menu,
        "Menu TurnRight request write must not start gameplay in the schedule"
    );
}

#[test]
fn test_app_tick_state_writes_input_state_directly() {
    use crate::events::input_state::{SetCtrlHeld, SetPanRightHeld, SetShiftHeld};

    let mut app = crate::state::test_support::TestApp::minimal();

    app.world_mut().write_message(SetShiftHeld(true));
    app.world_mut().write_message(SetCtrlHeld(true));
    app.world_mut().write_message(SetPanRightHeld(true));

    app.tick_state();

    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.shift_held);
    assert!(input.ctrl_held);
    assert!(input.pan_right_held);
}

#[test]
fn test_app_camera_pan_system_owns_key_motion() {
    use crate::state::editor_camera::update_editor_camera_pan_system;
    use crate::state::{AppPhaseRes, EditorCameraRes, EditorFrameRes, EditorInputRes};

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(AppPhaseRes::new(AppPhase::Editor));
    world.insert_resource(crate::state::FrameTimeRes {
        delta_seconds: 0.1,
        ..Default::default()
    });
    world.insert_resource(EditorInputRes(eg_types::EditorInputState {
        pan_right_held: true,
        ..Default::default()
    }));
    world.insert_resource(EditorCameraRes::default());
    world.insert_resource(EditorFrameRes::default());

    let mut schedule = bevy_ecs::schedule::Schedule::default();
    schedule.add_systems(update_editor_camera_pan_system);
    schedule.run(&mut world);

    let camera = &world.resource::<EditorCameraRes>().0;
    assert_ne!(camera.editor_pan, [0.0, 0.0]);
    assert!(world.resource::<EditorFrameRes>().camera_pan_applied);
}

#[test]
fn test_app_tick_state_writes_pointer_state_directly() {
    use crate::events::pointer::{CameraDrag, MouseButton, PointerMoved, PrimaryClick};
    use crate::state::{AppPhaseRes, EditorCameraRes};

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut().resource_mut::<AppPhaseRes>().current = AppPhase::Editor;

    app.world_mut()
        .write_message(PointerMoved { x: 10.0, y: 20.0 });
    app.world_mut().write_message(MouseButton {
        button: 2,
        pressed: true,
    });
    app.world_mut()
        .write_message(CameraDrag { dx: 5.0, dy: -3.0 });
    app.world_mut()
        .write_message(PrimaryClick { x: 30.0, y: 40.0 });

    app.tick_state();

    let input = &app.world().resource::<EditorInputRes>().0;
    assert_eq!(input.pointer_screen, Some([30.0, 40.0]));
    assert!(input.left_mouse_down);
    assert!(input.right_dragging);

    let camera = &app.world().resource::<EditorCameraRes>().0;
    assert!(camera.editor_yaw < eg_types::DEFAULT_CAMERA_YAW);
    assert!(camera.editor_pitch < eg_types::DEFAULT_CAMERA_PITCH);
}

#[test]
fn test_app_tick_state_applies_undo_to_app_resources() {
    // Verify that the editor Undo message is handled in App-world resources
    // instead of relying on legacy dispatch.
    use crate::events::history::Undo;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .resource_mut::<AppPhaseRes>()
        .set(AppPhase::Editor);

    let undo_object = LevelObject {
        id: "undo".to_string(),
        position: [1.0, 0.0, 2.0],
        ..LevelObject::default()
    };
    app.world_mut()
        .resource_mut::<crate::state::EditorHistoryRes>()
        .0
        .undo
        .push(crate::state::EditorHistorySnapshot {
            objects: vec![undo_object.clone()],
            selected_block_index: Some(0),
            selected_block_indices: vec![0],
            cursor: [1.0, 0.0, 2.0],
            selected_block_id: eg_blocks::BlockId::new("core/stone"),
            selected_block_rotation_degrees: [0.0, 0.0, 0.0],
            recent_block_ids: vec![eg_blocks::BlockId::new("core/stone")],
            spawn: eg_types::SpawnMetadata::default(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 1.0,
            tap_times: Vec::new(),
            tap_indicator_positions: Vec::new(),
            selected_tap_index: None,
            timing_points: Vec::new(),
            selected_trigger_index: None,
        });

    app.world_mut().write_message(Undo);
    app.tick_state();

    assert_eq!(
        app.world()
            .resource::<crate::state::EditorObjectsRes>()
            .objects,
        vec![undo_object]
    );
}

#[test]
fn test_app_tick_state_practice_mode_mutates_session_resource() {
    // Phase 3.15: `GameSetPracticeMode` is a carve-out — the
    // handler writes `SessionStateRes.practice_mode_enabled`
    // directly instead of routing through legacy `State`. Pin the
    // resource update here; the change-guarded runtime sync (which
    // would invoke the legacy
    // `set_practice_mode_enabled` setter with the heavy side
    // effects — clear checkpoints, restart level, resume game) is
    // exercised in `eg-app` runtime tests because it requires a
    // real `Runtime`.
    use crate::events::navigation::GameSetPracticeMode;

    let mut app = crate::state::test_support::TestApp::minimal();
    assert!(
        !app.world()
            .resource::<SessionStateRes>()
            .practice_mode_enabled
    );

    app.world_mut()
        .write_message(GameSetPracticeMode { enabled: true });
    app.tick_state();

    assert!(
        app.world()
            .resource::<SessionStateRes>()
            .practice_mode_enabled,
        "GameSetPracticeMode(true) handler must mutate SessionStateRes.practice_mode_enabled"
    );
}

#[test]
fn test_app_tick_state_builds_frame_input_from_world() {
    use crate::state::render::resources::RenderFrameInputRes;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut().resource_mut::<AppPhaseRes>().current = AppPhase::Editor;
    app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Place;
    app.world_mut()
        .resource_mut::<SessionStateRes>()
        .editor_sky_color = [0.25, 0.5, 0.75];

    let pre = app.world().resource::<RenderFrameInputRes>().0.phase;
    assert_eq!(pre, AppPhase::Menu);

    app.tick_state();

    let frame_input = app.world().resource::<RenderFrameInputRes>().0;
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

    let app = crate::state::test_support::TestApp::minimal();
    let world = app.world();

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
    use eg_audio::audio::runtime_asset_source_key;

    let mut state = State::new_test().await;
    state.set_editor_level_name("ScheduleAudioImport".to_string());
    let mut app = crate::state::test_support::TestApp::minimal();

    // Create a fresh channel whose receiver sits in the Bevy
    // world resource (and whose sender we control from the test).
    // Phase 6a's `poll_channels_system` drains this resource once
    // per frame and emits the corresponding result event. The
    // legacy `State::audio.state.editor.audio_import_channel.1`
    // is left alone (Phase 6b will share the receiver via Arc).
    let (tx, rx) = std::sync::mpsc::channel::<(String, Vec<u8>)>();
    app.world_mut()
        .insert_resource(AudioImportChannelRes(std::sync::Arc::new(
            std::sync::Mutex::new(rx),
        )));

    // Send an audio-import result. The poll system drains
    // receivers once per frame; tick_state runs the schedule
    // which includes the poll system.
    tx.send(("level1.ogg".to_string(), vec![1, 2, 3, 4]))
        .unwrap();

    tick_state_with_app(&mut state, &mut app);

    // The event should be in the Messages<AudioImportResultEvent>
    // buffer. We read it via the resource API.
    let messages = app.world().resource::<Messages<AudioImportResultEvent>>();
    let mut reader = messages.get_cursor();
    let events: Vec<_> = reader.read(messages).collect();
    assert_eq!(events.len(), 1, "expected one audio import event");
    assert_eq!(events[0].filename, "level1.ogg");
    assert_eq!(events[0].bytes, Some(vec![1, 2, 3, 4]));
    assert_eq!(
        state
            .audio
            .state
            .editor
            .cache
            .bytes_for_source(&runtime_asset_source_key(
                "ScheduleAudioImport",
                "audio.mp3"
            )),
        Some(&vec![1, 2, 3, 4]),
        "StateSchedule async-result handler should apply audio imports before State::update"
    );
}

#[tokio::test]
async fn tick_state_poll_channels_emits_level_import_result_event() {
    use crate::events::async_results::LevelImportResultEvent;
    use crate::state::async_bridge::LevelImportChannelRes;
    use bevy_ecs::message::Messages;

    let mut state = State::new_test().await;
    let mut app = crate::state::test_support::TestApp::minimal();

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    app.world_mut()
        .insert_resource(LevelImportChannelRes(std::sync::Arc::new(
            std::sync::Mutex::new(rx),
        )));

    tx.send(vec![0xDE, 0xAD, 0xBE, 0xEF]).unwrap();

    tick_state_with_app(&mut state, &mut app);

    let messages = app.world().resource::<Messages<LevelImportResultEvent>>();
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
    let mut app = crate::state::test_support::TestApp::minimal();

    let (tx, rx) = std::sync::mpsc::channel::<eg_platform::services::AuthServiceMessage>();
    app.world_mut()
        .insert_resource(AuthChannelRes(std::sync::Arc::new(std::sync::Mutex::new(
            rx,
        ))));

    // We use the simpler SignedOut variant that doesn't require
    // constructing an AuthSession payload.
    tx.send(eg_platform::services::AuthServiceMessage::SignedOut(Ok(())))
        .unwrap();

    tick_state_with_app(&mut state, &mut app);

    let messages = app.world().resource::<Messages<AuthServiceResultEvent>>();
    let mut cursor = messages.get_cursor();
    let events: Vec<_> = cursor.read(messages).collect();
    assert_eq!(events.len(), 1, "expected one auth event");
    assert!(matches!(
        events[0].message,
        eg_platform::services::AuthServiceMessage::SignedOut(_)
    ));
}

/// Phase 4b/4c-final: the heavy render resources are now inserted
/// with their authoritative values during `State` construction.
/// Bevy systems can query them directly.
#[tokio::test]
async fn tick_state_inserts_heavy_render_resources_phase4c() {
    let state = State::new_test().await;

    assert!(state.surface_host().is_none());

    // Test construction seeds the real scene mesh storage: static
    // slots start empty, while the trail keeps its streaming buffer.
    let meshes = state.render_meshes();
    assert!(matches!(meshes.floor, eg_render::MeshSlot::Empty));
    assert!(matches!(
        meshes.trail,
        eg_render::MeshSlot::Streaming { .. }
    ));
    assert!(matches!(meshes.blocks, eg_render::MeshSlot::Empty));
}

#[test]
fn test_app_render_extract_builds_menu_and_editor_uniforms() {
    use crate::state::EditorCameraRes;
    use crate::state::render::resources::{RenderConfigRes, RenderUniformsRes};
    use approx::assert_abs_diff_eq;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut().insert_resource(RenderConfigRes {
        width: 800,
        height: 600,
        apply_gamma_correction: false,
    });
    app.world_mut()
        .insert_resource(MenuStateRes::new(eg_types::MenuState {
            selected_level: 0,
            levels: Vec::new(),
            preview_level_index: None,
            preview_sky_color: eg_types::LevelMetadata::default().sky_color,
            preview_camera_position: eg_types::DEFAULT_MENU_PREVIEW_CAMERA_POSITION,
            preview_camera_target: eg_types::DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
        }));
    let menu_eye = app
        .world()
        .resource::<MenuStateRes>()
        .0
        .preview_camera_position;
    app.tick_state();
    assert_eq!(
        app.world()
            .resource::<RenderUniformsRes>()
            .camera
            .camera_position,
        [menu_eye[0], menu_eye[1], menu_eye[2], 0.0]
    );

    app.world_mut()
        .insert_resource(AppPhaseRes::new(AppPhase::Editor));
    {
        let mut camera = app.world_mut().resource_mut::<EditorCameraRes>();
        camera.0.editor_pan = [4.0, 8.0];
        camera.0.editor_target_z = 6.0;
    }
    let target = app
        .world()
        .resource::<EditorCameraRes>()
        .0
        .editor_camera_target();
    let expected_eye = target
        + app
            .world()
            .resource::<EditorCameraRes>()
            .0
            .editor_camera_offset();
    app.tick_state();
    let camera_position = app
        .world()
        .resource::<RenderUniformsRes>()
        .camera
        .camera_position;
    assert_abs_diff_eq!(camera_position[0], expected_eye.x, epsilon = 1e-6);
    assert_abs_diff_eq!(camera_position[1], expected_eye.y, epsilon = 1e-6);
    assert_abs_diff_eq!(camera_position[2], expected_eye.z, epsilon = 1e-6);
    assert_eq!(camera_position[3], 0.0);
}

#[test]
fn test_app_render_extract_builds_playing_camera_and_line_uniform() {
    use crate::state::render::resources::{RenderConfigRes, RenderUniformsRes};
    use approx::assert_abs_diff_eq;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .insert_resource(AppPhaseRes::new(AppPhase::Playing));
    app.world_mut().insert_resource(RenderConfigRes {
        width: 1280,
        height: 720,
        apply_gamma_correction: false,
    });
    let position = [12.0, 3.0, 7.0];
    app.world_mut().spawn((
        eg_game::PlayerPosition(position),
        eg_game::PlayerDirection(Direction::Right),
    ));
    {
        let mut clock = app.world_mut().resource_mut::<eg_game::GameClock>();
        clock.elapsed_seconds = 0.5;
        clock.started = false;
    }
    let expected_eye = crate::state::evaluate_camera_track_for_objects(
        &app.world().resource::<crate::state::EditorCameraRes>().0,
        &[],
        0.5,
        glam::Vec3::from_array(position),
        false,
    )
    .eye;

    app.tick_state();

    let uniforms = app.world().resource::<RenderUniformsRes>();
    assert_abs_diff_eq!(
        uniforms.camera.camera_position[0],
        expected_eye.x,
        epsilon = 1e-5
    );
    assert_abs_diff_eq!(uniforms.line.offset[0], 12.0, epsilon = 1e-6);
    assert_abs_diff_eq!(uniforms.line.offset[1], 7.0, epsilon = 1e-6);
    assert_abs_diff_eq!(
        uniforms.line.rotation,
        -std::f32::consts::FRAC_PI_2,
        epsilon = 1e-6
    );
}

#[test]
fn test_app_render_extract_uses_app_time_viewport_and_gamma() {
    use crate::state::render::resources::{RenderConfigRes, RenderUniformsRes};
    use approx::assert_abs_diff_eq;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut().insert_resource(RenderConfigRes {
        width: 640,
        height: 360,
        apply_gamma_correction: true,
    });
    {
        let mut frame_time = app.world_mut().resource_mut::<FrameTimeRes>();
        frame_time.global_time_seconds = 12.5;
        frame_time.last_frame = web_time::Instant::now();
    }

    app.tick_state();

    let frame_time = app.world().resource::<FrameTimeRes>().global_time_seconds;
    let color_space = app.world().resource::<RenderUniformsRes>().color_space;
    assert_abs_diff_eq!(color_space.time_seconds, frame_time, epsilon = 1e-6);
    assert_eq!(color_space.apply_gamma_correction, 1.0);
    assert_eq!(color_space.viewport_size, [640.0, 360.0]);
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
        .render_meshes()
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
            .render_meshes()
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
        .render_meshes()
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
            .render_meshes()
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
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
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
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
    );
    let screen_x = viewport.x as f64 * 0.5;
    let screen_y = viewport.y as f64 * 0.5;
    let expected_cursor = state
        .editor
        .pick_from_screen(screen_x, screen_y, viewport)
        .expect("expected context pick")
        .cursor;
    state.editor.input.cursor = [99.0, 99.0, 99.0];

    state.force_editor_cursor_from_screen(screen_x, screen_y);

    assert_eq!(state.editor.input.cursor, expected_cursor);
}

#[tokio::test]
async fn spawn_hotkey_updates_spawn_from_pointer_in_select_mode() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.input.mode = EditorMode::Select;
    state.editor.input.cursor = [99.0, 99.0, 99.0];

    let viewport = Vec2::new(
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
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

    state.force_editor_cursor_from_pointer();
    state.editor_set_spawn_here();

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
    let view = state.ui_state_snapshot();
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

    state.editor_shift_timeline_time(0.1);

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
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
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
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
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
        state.render_gpu().config.width,
        state.render_gpu().config.height,
    );
    state.resize_surface(PhysicalSize::new(0, initial_size.height));
    assert_eq!(state.render_gpu().config.width, initial_size.width);
    assert_eq!(state.render_gpu().config.height, initial_size.height);

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
        state.render_gpu().config.width as f32,
        state.render_gpu().config.height as f32,
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
