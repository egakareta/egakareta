/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy::{ecs::prelude::With, state::prelude::State};
use eg_editor_domain::derive_timeline_position;
use eg_types::{AppPhase, Direction, EditorMode, LevelObject};

use crate::state::{
    EditorInputRes, EditorRuntimeRes, FrameTimeRes, MenuStateRes, PerfRes, SessionStateRes,
};
#[test]
fn app_world_is_seeded_with_default_resources() {
    let app = crate::state::test_support::TestApp::minimal();
    let world = app.world();
    assert_eq!(
        *world.resource::<State<eg_types::AppPhase>>().get(),
        eg_types::AppPhase::Menu,
        "State<AppPhase> should default to Menu"
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
fn test_app_update_advances_frame_time_resource() {
    let mut app = crate::state::test_support::TestApp::minimal();
    {
        let mut frame_time = app.world_mut().resource_mut::<FrameTimeRes>();
        frame_time.last_frame = web_time::Instant::now() - std::time::Duration::from_millis(20);
    }

    app.tick();

    let frame_time = *app.world().resource::<FrameTimeRes>();
    assert!(
        frame_time.delta_seconds > 0.0,
        "Update must advance FrameTimeRes.delta_seconds"
    );
    assert!(
        frame_time.global_time_seconds > 0.0,
        "Update must advance FrameTimeRes.global_time_seconds"
    );
    assert!(
        frame_time.fps_smoothed > 0.0,
        "Update must update frame FPS smoothing"
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
fn update_is_safe_to_call_with_empty_schedule() {
    let mut app = crate::state::test_support::TestApp::minimal();
    app.tick();
    app.tick();
    app.tick();
}

#[test]
fn update_runs_phase_transition_system() {
    let mut app = crate::state::test_support::TestApp::minimal();

    app.tick();
    assert_eq!(
        *app.world().resource::<State<eg_types::AppPhase>>().get(),
        eg_types::AppPhase::Menu
    );

    // Change the phase manually in this direct App-world fixture.
    app.world_mut()
        .insert_resource(State::new(eg_types::AppPhase::Playing));

    app.tick();
    assert_eq!(
        *app.world().resource::<State<eg_types::AppPhase>>().get(),
        eg_types::AppPhase::Playing
    );

    app.tick();
    assert_eq!(
        *app.world().resource::<State<eg_types::AppPhase>>().get(),
        eg_types::AppPhase::Playing
    );
}

// ── Phase 3 — command / event fragmentation tests ───────────────
#[test]
fn test_app_update_writes_command_messages_to_app_resources() {
    // SetEditorMode is handled through App-world resources.
    use eg_types::EditorMode;

    use crate::events::editor_mode::SetEditorMode;

    let mut app = crate::state::test_support::TestApp::minimal();

    // Write a SetEditorMode(Place) message. Phase 3.13 carved
    // this out: the handler mutates `EditorInputRes.0.mode`
    // directly. Pin the carve-out behavior: the resource is updated.
    app.world_mut()
        .write_message(SetEditorMode(EditorMode::Place));

    // Tick: per-event systems fire and mutate App-world resources.
    app.tick();

    assert_eq!(
        app.world().resource::<EditorInputRes>().0.mode,
        EditorMode::Place,
        "SetEditorMode handler must mutate EditorInputRes.mode"
    );
    assert_eq!(
        *app.world().resource::<State<eg_types::AppPhase>>().get(),
        AppPhase::Menu
    );
}

#[test]
fn test_app_update_writes_input_state_directly() {
    use crate::events::input_state::{SetCtrlHeld, SetPanRightHeld, SetShiftHeld};

    let mut app = crate::state::test_support::TestApp::minimal();

    app.world_mut().write_message(SetShiftHeld(true));
    app.world_mut().write_message(SetCtrlHeld(true));
    app.world_mut().write_message(SetPanRightHeld(true));

    app.tick();

    let input = &app.world().resource::<EditorInputRes>().0;
    assert!(input.shift_held);
    assert!(input.ctrl_held);
    assert!(input.pan_right_held);
}

#[test]
fn test_app_camera_pan_system_owns_key_motion() {
    use crate::state::{
        EditorCameraRes, EditorInputRes, editor_camera::update_editor_camera_pan_system,
    };

    let mut world = bevy::ecs::world::World::new();
    world.insert_resource(State::new(AppPhase::Editor));
    world.insert_resource(crate::state::FrameTimeRes {
        delta_seconds: 0.1,
        ..Default::default()
    });
    world.insert_resource(EditorInputRes(eg_types::EditorInputState {
        pan_right_held: true,
        ..Default::default()
    }));
    world.insert_resource(EditorCameraRes::default());

    let mut schedule = bevy::ecs::schedule::Schedule::default();
    schedule.add_systems(update_editor_camera_pan_system);
    schedule.run(&mut world);

    let camera = &world.resource::<EditorCameraRes>().0;
    assert_ne!(camera.editor_pan, [0.0, 0.0]);
}

#[test]
fn test_app_update_writes_pointer_state_directly() {
    use crate::{
        events::pointer::{CameraDrag, MouseButton, PointerMoved, PrimaryClick},
        state::EditorCameraRes,
    };

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));

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

    app.tick();

    let input = &app.world().resource::<EditorInputRes>().0;
    assert_eq!(input.pointer_screen, Some([30.0, 40.0]));
    assert!(input.left_mouse_down);
    assert!(input.right_dragging);

    let camera = &app.world().resource::<EditorCameraRes>().0;
    assert!(camera.editor_yaw < eg_types::DEFAULT_CAMERA_YAW);
    assert!(camera.editor_pitch < eg_types::DEFAULT_CAMERA_PITCH);
}

#[test]
fn test_app_update_applies_undo_to_app_resources() {
    // Verify that the editor Undo message is handled in App-world resources
    // instead of relying on legacy dispatch.
    use crate::events::history::Undo;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));

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
    app.tick();

    let object = {
        let mut query = app
            .world_mut()
            .query_filtered::<&eg_game::ObjectData, With<crate::state::EditorAuthoredObject>>();
        query
            .single(app.world())
            .expect("restored authored object")
            .0
            .clone()
    };
    assert_eq!(object, undo_object);
}

#[test]
fn test_app_update_practice_mode_mutates_session_resource() {
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
    app.tick();

    assert!(
        app.world()
            .resource::<SessionStateRes>()
            .practice_mode_enabled,
        "GameSetPracticeMode(true) handler must mutate SessionStateRes.practice_mode_enabled"
    );
}

#[test]
fn test_app_update_builds_frame_input_from_world() {
    use crate::state::render::resources::RenderFrameInputRes;

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
    app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Place;
    app.world_mut()
        .resource_mut::<SessionStateRes>()
        .editor_sky_color = [0.25, 0.5, 0.75];

    let pre = app.world().resource::<RenderFrameInputRes>().0.phase;
    assert_eq!(pre, AppPhase::Menu);

    app.tick();

    let frame_input = app.world().resource::<RenderFrameInputRes>().0;
    assert_eq!(frame_input.phase, AppPhase::Editor);
    assert_eq!(frame_input.editor_mode, EditorMode::Place);
    assert_eq!(frame_input.sky_color, [0.25, 0.5, 0.75]);
    assert!(!frame_input.apply_gamma_correction);
}

#[test]
fn update_registers_render_pod_resources_on_construction() {
    use crate::state::render::resources::{
        RenderFrameInputRes, RenderResizeRequest, RenderUniformsRes, render_pod_resources_present,
    };

    let app = crate::state::test_support::TestApp::minimal();
    let world = app.world();

    assert!(render_pod_resources_present(world));
    assert!(world.contains_resource::<RenderUniformsRes>());
    assert!(world.contains_resource::<RenderFrameInputRes>());
    assert!(world.contains_resource::<RenderResizeRequest>());
}

#[test]
fn test_app_render_extract_builds_menu_and_editor_uniforms() {
    use approx::assert_abs_diff_eq;

    use crate::state::{
        EditorCameraRes,
        render::resources::{RenderConfigRes, RenderUniformsRes},
    };

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
    app.tick();
    assert_eq!(
        app.world()
            .resource::<RenderUniformsRes>()
            .camera
            .camera_position,
        [menu_eye[0], menu_eye[1], menu_eye[2], 0.0]
    );

    app.world_mut()
        .insert_resource(State::new(AppPhase::Editor));
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
    app.tick();
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
    use approx::assert_abs_diff_eq;

    use crate::state::render::resources::{
        RenderCameraPoseRes, RenderConfigRes, RenderUniformsRes,
    };

    let mut app = crate::state::test_support::TestApp::minimal();
    app.world_mut()
        .insert_resource(State::new(AppPhase::Playing));
    app.world_mut().insert_resource(RenderConfigRes {
        width: 1280,
        height: 720,
        apply_gamma_correction: false,
    });
    let position = [12.0, 3.0, 7.0];
    let player_entity = {
        let world = app.world_mut();
        world
            .query_filtered::<bevy::ecs::entity::Entity, bevy::ecs::prelude::With<eg_game::PlayerPosition>>()
            .single(world)
            .expect("TestApp should seed one player entity")
    };
    app.world_mut().entity_mut(player_entity).insert((
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

    app.tick();

    let uniforms = app.world().resource::<RenderUniformsRes>();
    assert_abs_diff_eq!(
        uniforms.camera.camera_position[0],
        expected_eye.x,
        epsilon = 1e-5
    );
    let pose = app.world().resource::<RenderCameraPoseRes>();
    assert_eq!(pose.eye, expected_eye.to_array());
    assert_eq!(pose.target, position);
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
    use approx::assert_abs_diff_eq;

    use crate::state::render::resources::{RenderConfigRes, RenderUniformsRes};

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

    app.tick();

    let frame_time = app.world().resource::<FrameTimeRes>().global_time_seconds;
    let color_space = app.world().resource::<RenderUniformsRes>().color_space;
    assert_abs_diff_eq!(color_space.time_seconds, frame_time, epsilon = 1e-6);
    assert_eq!(color_space.apply_gamma_correction, 1.0);
    assert_eq!(color_space.viewport_size, [640.0, 360.0]);
}

// ── Timeline position tests (pre-existing) ─────────────────────

#[test]
fn derives_position_without_taps() {
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

#[test]
fn derives_position_with_taps() {
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

#[test]
fn tap_at_zero_changes_direction() {
    let taps = [0.0];
    let (position, direction) =
        derive_timeline_position([0.0, 0.0, 0.0], Direction::Forward, &taps, 0.0, &[]);
    assert!((position[0] - 0.5).abs() < 0.1);
    assert!((position[2] - 0.5).abs() < 0.1);
    assert!(matches!(direction, Direction::Right));
}

#[test]
fn ignores_taps_after_step() {
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

#[test]
fn supports_offset_spawn_with_tap() {
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

#[test]
fn falls_from_elevated_platform() {
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

#[test]
fn editor_mode_selection_constraints() {
    assert!(EditorMode::Select.can_select());
    assert!(EditorMode::Move.can_select());
    assert!(EditorMode::Scale.can_select());
    assert!(EditorMode::Place.can_select());
    assert!(!EditorMode::Tapping.can_select());
    assert!(!EditorMode::Timing.can_select());
    assert!(!EditorMode::Null.can_select());
}
