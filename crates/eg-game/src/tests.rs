/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::components::{
    BlockId, CachedBlockBehavior, ColorTint, Grounded, Object, ObjectBundle, ObjectData, ObjectId,
    Player, PlayerBundle, PlayerDirection, PlayerPosition, PlayerSpeed, Position, Rotation, Size,
    SpawnDirection, SpawnPosition, Trail, VerticalVelocity,
};
use super::events::ConsumedEventsBuffer;
use super::physics::{BASE_PLAYER_SPEED, aabb_overlaps_object_xz, object_xz_contains};
use super::resources::{
    FrameDelta, GameClock, PlayerProgress, SpatialIndex, TimedTurnInput, TurnInputKind,
};
use super::simulation::{TimelineSimulationRuntime, simulate_timeline_state};
use super::test_app::TestApp;
use eg_block_geometry::hitbox_cuboids;
use eg_types::{Direction, LevelObject};

fn app_with_objects(objects: impl IntoIterator<Item = LevelObject>) -> TestApp {
    let mut app = TestApp::minimal();
    for (index, object) in objects.into_iter().enumerate() {
        app.world_mut().spawn(ObjectBundle {
            object: Object,
            id: ObjectId(format!("__obj_{index}")),
            data: ObjectData(object.clone()),
            position: Position(object.position),
            rotation: Rotation(object.rotation),
            size: Size(object.size),
            block_id: BlockId(object.block_id.clone()),
            color_tint: ColorTint(object.color_tint),
            cached_behavior: CachedBlockBehavior::from_block_id(&object.block_id),
        });
    }
    rebuild_spatial_index(&mut app);
    app
}

fn rebuild_spatial_index(app: &mut TestApp) {
    let mut spatial = app
        .world_mut()
        .remove_resource::<SpatialIndex>()
        .expect("gameplay plugin should register SpatialIndex");
    spatial.rebuild(app.world_mut());
    app.world_mut().insert_resource(spatial);
}

fn spawn_player(
    app: &mut TestApp,
    position: [f32; 3],
    direction: Direction,
    speed: f32,
    grounded: bool,
) {
    app.world_mut().spawn(PlayerBundle {
        player: Player,
        position: PlayerPosition(position),
        direction: PlayerDirection(direction),
        speed: PlayerSpeed(speed),
        vertical_velocity: VerticalVelocity::default(),
        grounded: Grounded(grounded),
        trail: Trail {
            segments: vec![vec![position]],
        },
        spawn_position: SpawnPosition(position),
        spawn_direction: SpawnDirection(direction),
    });
}

fn start_game(app: &mut TestApp) {
    app.world_mut().resource_mut::<GameClock>().started = true;
}

fn tick(app: &mut TestApp, dt: f32) {
    app.world_mut().resource_mut::<FrameDelta>().0 = dt;
    app.tick_gameplay();
}

fn support_surface_y_at(app: &mut TestApp, x: f32, z: f32, max_y: f32) -> Option<f32> {
    let entities = app
        .world()
        .resource::<SpatialIndex>()
        .grid
        .query_point(x, z)
        .to_vec();
    let world = app.world_mut();
    let mut query = world.query::<&ObjectData>();
    let mut top_surface = Some(0.0);

    for entity in entities {
        let Ok(object) = query.get(world, entity) else {
            continue;
        };
        if !CachedBlockBehavior::from_block_id(&object.0.block_id).support_surface
            || !object_xz_contains(&object.0, x, z)
        {
            continue;
        }
        for cuboid in hitbox_cuboids(&object.0) {
            let top = cuboid.max[1];
            if top <= max_y {
                top_surface = match top_surface {
                    Some(existing) if existing > top => Some(existing),
                    _ => Some(top),
                };
            }
        }
    }

    top_surface
}

fn player_snapshot(app: &mut TestApp) -> ([f32; 3], Direction, bool, f32) {
    let world = app.world_mut();
    let mut query = world.query::<(&PlayerPosition, &PlayerDirection, &Grounded, &PlayerSpeed)>();
    let (position, direction, grounded, speed) = query.single(world).expect("player should exist");
    (position.0, direction.0, grounded.0, speed.0)
}

fn object_snapshots(app: &mut TestApp) -> Vec<LevelObject> {
    let world = app.world_mut();
    let mut query = world.query::<&ObjectData>();
    query.iter(world).map(|data| data.0.clone()).collect()
}

#[test]
fn test_ground_detection_normal() {
    let mut app = app_with_objects([LevelObject::default()]);
    assert_eq!(support_surface_y_at(&mut app, 0.5, 0.5, 2.0), Some(1.0));
}

#[test]
fn test_ground_detection_includes_block_edges() {
    let mut app = app_with_objects([LevelObject::default()]);
    assert_eq!(support_surface_y_at(&mut app, 0.0, 0.5, 2.0), Some(1.0));
    assert_eq!(support_surface_y_at(&mut app, 1.0, 0.5, 2.0), Some(1.0));
    assert_eq!(support_surface_y_at(&mut app, 0.5, 0.0, 2.0), Some(1.0));
    assert_eq!(support_surface_y_at(&mut app, 0.5, 1.0, 2.0), Some(1.0));
}

#[test]
fn player_stays_grounded_when_footprint_overlaps_platform_edge() {
    let mut app = app_with_objects([LevelObject::default()]);
    spawn_player(
        &mut app,
        [1.05, 1.0, 0.5],
        Direction::Forward,
        BASE_PLAYER_SPEED,
        true,
    );
    start_game(&mut app);

    for _ in 0..12 {
        tick(&mut app, 1.0 / 120.0);
    }

    let (position, _, grounded, _) = player_snapshot(&mut app);
    assert!(grounded);
    assert!(!app.world().resource::<GameClock>().game_over);
    approx::assert_abs_diff_eq!(position[1], 1.0, epsilon = 1e-6);
}

#[test]
fn test_ground_detection_under_overhang() {
    let mut app = app_with_objects([
        LevelObject::default(),
        LevelObject {
            position: [0.0, 3.0, 0.0],
            ..LevelObject::default()
        },
    ]);
    assert_eq!(support_surface_y_at(&mut app, 0.5, 0.5, 1.5), Some(1.0));
}

#[test]
fn test_cant_turn_while_falling() {
    let mut app = app_with_objects([]);
    spawn_player(
        &mut app,
        [0.0, 1.0, 0.0],
        Direction::Forward,
        BASE_PLAYER_SPEED,
        false,
    );
    start_game(&mut app);
    app.world_mut()
        .resource_mut::<super::resources::PendingTurnInputs>()
        .taps
        .push_back(TimedTurnInput {
            at_seconds: 0.0,
            kind: TurnInputKind::TurnRight,
        });
    tick(&mut app, 0.0);

    let (_, direction, _, _) = player_snapshot(&mut app);
    assert_eq!(direction, Direction::Forward);
}

#[test]
fn test_can_turn_while_grounded() {
    let mut app = app_with_objects([]);
    spawn_player(
        &mut app,
        [0.0, 1.0, 0.0],
        Direction::Forward,
        BASE_PLAYER_SPEED,
        true,
    );
    start_game(&mut app);
    app.world_mut()
        .resource_mut::<super::resources::PendingTurnInputs>()
        .taps
        .push_back(TimedTurnInput {
            at_seconds: 0.0,
            kind: TurnInputKind::TurnRight,
        });
    tick(&mut app, 0.0);

    let (_, direction, _, _) = player_snapshot(&mut app);
    assert_eq!(direction, Direction::Right);
}

#[test]
fn rotated_object_contains_expected_points() {
    let obj = LevelObject {
        size: [2.0, 1.0, 1.0],
        rotation: [0.0, 90.0, 0.0],
        ..LevelObject::default()
    };

    assert!(object_xz_contains(&obj, 1.0, 0.5));
    assert!(!object_xz_contains(&obj, 2.1, 0.5));
}

#[test]
fn rotated_overlap_uses_oriented_bounds() {
    let obj = LevelObject {
        size: [2.0, 1.0, 1.0],
        rotation: [0.0, 45.0, 0.0],
        ..LevelObject::default()
    };

    assert!(aabb_overlaps_object_xz(0.9, 1.1, 0.3, 0.5, &obj));
    assert!(!aabb_overlaps_object_xz(3.0, 3.4, 3.0, 3.4, &obj));
}

#[test]
fn wooden_fence_collision_uses_json_hitboxes() {
    let obj = LevelObject::new("core/wooden_fence");

    assert!(object_xz_contains(&obj, 0.5, 0.5));
    assert!(!object_xz_contains(&obj, 0.5, 0.2));
    assert!(aabb_overlaps_object_xz(0.45, 0.55, 0.45, 0.55, &obj));
    assert!(!aabb_overlaps_object_xz(0.45, 0.55, 0.1, 0.2, &obj));
}

#[test]
fn wooden_fence_support_height_uses_hitbox_top() {
    let mut app = app_with_objects([LevelObject::new("core/wooden_fence")]);
    assert_eq!(support_surface_y_at(&mut app, 0.5, 0.5, 2.0), Some(1.0));
    assert_eq!(support_surface_y_at(&mut app, 0.5, 0.2, 2.0), Some(0.0));
}

#[test]
fn missing_hitboxes_fall_back_to_geometry_elements() {
    let mut app = app_with_objects([LevelObject::new("core/wooden_fence")]);
    let objects = object_snapshots(&mut app);
    assert!(object_xz_contains(&objects[0], 0.5, 0.5));
    assert!(!object_xz_contains(&objects[0], 0.5, 0.2));
}

#[test]
fn rotated_ground_detection_works() {
    let mut app = app_with_objects([LevelObject {
        size: [2.0, 2.0, 1.0],
        rotation: [0.0, 90.0, 0.0],
        ..LevelObject::default()
    }]);
    assert_eq!(support_surface_y_at(&mut app, 1.0, 0.5, 3.0), Some(2.0));
    assert_eq!(support_surface_y_at(&mut app, 2.2, 0.5, 3.0), Some(0.0));
}

#[test]
fn speed_portal_overlap_removes_portal_and_boosts_speed() {
    let mut app = app_with_objects([LevelObject {
        rotation: [0.0, 30.0, 0.0],
        ..LevelObject::new("core/speedportal")
    }]);
    spawn_player(&mut app, [0.5, 0.0, 0.2], Direction::Forward, 1.0, true);
    start_game(&mut app);
    tick(&mut app, 0.0);

    let (_, _, _, speed) = player_snapshot(&mut app);
    approx::assert_abs_diff_eq!(speed, 1.5, epsilon = 1e-6);
    assert!(object_snapshots(&mut app).is_empty());
    assert!(!app.world().resource::<GameClock>().game_over);
}

#[test]
fn gem_overlap_collects_and_tracks_progress() {
    let mut app = app_with_objects([LevelObject::new("core/gem")]);
    spawn_player(
        &mut app,
        [0.5, 0.0, 0.5],
        Direction::Forward,
        BASE_PLAYER_SPEED,
        true,
    );
    {
        let world = app.world_mut();
        world.resource_mut::<GameClock>().started = true;
        world.resource_mut::<PlayerProgress>().gems_max = 1;
    }
    tick(&mut app, 0.0);

    let progress = *app.world().resource::<PlayerProgress>();
    assert_eq!(progress.gems_collected, 1);
    assert_eq!(progress.gems_max, 1);
    let events = &app.world().resource::<ConsumedEventsBuffer>().events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].object.block_id, "core/gem");
    assert!(object_snapshots(&mut app).is_empty());
    assert!(!app.world().resource::<GameClock>().game_over);
}

#[test]
fn level_completes_at_configured_duration() {
    let mut app = TestApp::minimal();
    {
        let mut clock = app.world_mut().resource_mut::<GameClock>();
        clock.started = true;
        clock.level_duration_seconds = 0.05;
    }

    tick(&mut app, 0.04);
    assert!(!app.world().resource::<GameClock>().level_complete);

    tick(&mut app, 0.02);

    let clock = app.world().resource::<GameClock>();
    assert!(clock.level_complete);
    assert!(!clock.started);
    assert!(!clock.game_over);
    assert!(app.world().resource::<PlayerProgress>().completed);
    approx::assert_abs_diff_eq!(
        app.world().resource::<PlayerProgress>().progress_percent,
        100.0,
        epsilon = 1e-6
    );
    approx::assert_abs_diff_eq!(clock.elapsed_seconds, 0.05, epsilon = 1e-6);
}

#[test]
fn timeline_negative_time_clamps_to_zero() {
    let snapshot = simulate_timeline_state([0.0, 0.0, 0.0], Direction::Forward, &[], &[], -2.0);

    approx::assert_abs_diff_eq!(snapshot.position[0], 0.5, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(snapshot.position[1], 0.0, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(snapshot.position[2], 0.5, epsilon = 1e-6);
    assert!(matches!(snapshot.direction, Direction::Forward));
    approx::assert_abs_diff_eq!(snapshot.elapsed_seconds, 0.0, epsilon = 1e-6);
}

#[test]
fn timeline_tap_at_zero_turns_before_movement() {
    let dt = 1.0 / BASE_PLAYER_SPEED;
    let snapshot = simulate_timeline_state([0.0, 0.0, 0.0], Direction::Forward, &[], &[0.0], dt);

    assert!(matches!(snapshot.direction, Direction::Right));
    approx::assert_abs_diff_eq!(snapshot.position[0], 1.5, epsilon = 0.05);
    approx::assert_abs_diff_eq!(snapshot.position[2], 0.5, epsilon = 0.05);
    approx::assert_abs_diff_eq!(snapshot.elapsed_seconds, dt, epsilon = 1e-6);
}

#[test]
fn timeline_tap_at_nonzero_time_turns_at_exact_boundary() {
    let tap_time = 1.0 / BASE_PLAYER_SPEED;
    let snapshot = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &[],
        &[tap_time],
        tap_time,
    );

    assert!(matches!(snapshot.direction, Direction::Right));
    approx::assert_abs_diff_eq!(snapshot.position[0], 0.5, epsilon = 1e-5);
    approx::assert_abs_diff_eq!(snapshot.position[2], 1.5, epsilon = 1e-5);
    approx::assert_abs_diff_eq!(snapshot.elapsed_seconds, tap_time, epsilon = 1e-6);

    let next_tick = tap_time + 1.0 / 240.0;
    let after_turn = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &[],
        &[tap_time],
        next_tick,
    );

    approx::assert_abs_diff_eq!(
        after_turn.position[0],
        0.5 + BASE_PLAYER_SPEED / 240.0,
        epsilon = 1e-5,
    );
    approx::assert_abs_diff_eq!(after_turn.position[2], 1.5, epsilon = 1e-5);
}

#[test]
fn timeline_incremental_runtime_matches_direct_simulation() {
    let mut runtime = TimelineSimulationRuntime::new(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &[],
        &[0.375, 0.125, 0.25],
    );

    for target in [0.05_f32, 0.2, 0.31, 0.6] {
        runtime.advance_to(target);
    }

    let incremental = runtime.snapshot();
    let direct = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        Direction::Forward,
        &[],
        &[0.375, 0.125, 0.25],
        0.6,
    );

    approx::assert_abs_diff_eq!(incremental.position[0], direct.position[0], epsilon = 0.02);
    approx::assert_abs_diff_eq!(incremental.position[1], direct.position[1], epsilon = 0.02);
    approx::assert_abs_diff_eq!(incremental.position[2], direct.position[2], epsilon = 0.02);
    assert!(matches!(
        (incremental.direction, direct.direction),
        (Direction::Forward, Direction::Forward) | (Direction::Right, Direction::Right)
    ));
    approx::assert_abs_diff_eq!(
        incremental.elapsed_seconds,
        direct.elapsed_seconds,
        epsilon = 1e-6
    );
}
