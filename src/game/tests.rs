use super::physics::{aabb_overlaps_object_xy, object_xy_contains, BASE_PLAYER_SPEED};
use super::simulation::{simulate_timeline_state, TimelineSimulationRuntime};
use super::state::GameState;
use crate::types::{LevelObject, SpawnDirection};

fn approx_eq(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "expected {a} ~= {b}");
}

#[test]
fn test_ground_detection_normal() {
    let mut game = GameState::new();
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });

    // Player at 0.5, 0.5 (center of block), check ground at 0.5, 0.5
    // Max Z should be > 1.0 to detect the block top
    let height = game.top_surface_height_at(0.5, 0.5, 2.0);
    assert_eq!(height, Some(1.0));
}

#[test]
fn test_ground_detection_under_overhang() {
    let mut game = GameState::new();
    // Ground block
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });
    // Overhang block at height 3
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 3.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });

    // Player is walking on the ground block (z=1).
    // We check ground height with max_z slightly above player head (e.g. 1.0 + SNAP)
    // It should ignore the block at z=3.
    let height = game.top_surface_height_at(0.5, 0.5, 1.5);
    assert_eq!(height, Some(1.0));
}

#[test]
fn test_cant_turn_while_falling() {
    let mut game = GameState::new();
    game.started = true;
    game.is_grounded = false;
    let initial_direction = game.direction;
    game.turn_right();
    assert_eq!(game.direction, initial_direction);
}

#[test]
fn test_can_turn_while_grounded() {
    let mut game = GameState::new();
    game.started = true;
    game.is_grounded = true;
    let initial_direction = game.direction;
    game.turn_right();
    assert_ne!(game.direction, initial_direction);
}

#[test]
fn rotated_object_contains_expected_points() {
    let obj = LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [2.0, 1.0, 1.0],
        rotation_degrees: 90.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    };

    assert!(object_xy_contains(&obj, 1.0, 0.5));
    assert!(!object_xy_contains(&obj, 2.1, 0.5));
}

#[test]
fn rotated_overlap_uses_oriented_bounds() {
    let obj = LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [2.0, 1.0, 1.0],
        rotation_degrees: 45.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    };

    assert!(aabb_overlaps_object_xy(0.9, 1.1, 0.3, 0.5, &obj));
    assert!(!aabb_overlaps_object_xy(3.0, 3.4, 3.0, 3.4, &obj));
}

#[test]
fn rotated_ground_detection_works() {
    let mut game = GameState::new();
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [2.0, 1.0, 2.0],
        rotation_degrees: 90.0,
        roundness: 0.18,
        block_id: "core/standard".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });

    let inside = game.top_surface_height_at(1.0, 0.5, 3.0);
    let outside = game.top_surface_height_at(2.2, 0.5, 3.0);
    assert_eq!(inside, Some(2.0));
    assert_eq!(outside, Some(0.0));
}

#[test]
fn speed_portal_overlap_removes_portal_and_boosts_speed() {
    let mut game = GameState::new();
    game.started = true;
    game.position = [0.5, 0.2, 0.0];
    game.speed = 1.0;
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 30.0,
        roundness: 0.18,
        block_id: "core/speedportal".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });

    game.update(0.0);

    approx_eq(game.speed, 1.5, 1e-6);
    assert!(game.objects.is_empty());
    assert!(!game.game_over);
}

#[test]
fn finish_block_overlap_completes_level_after_sink() {
    let mut game = GameState::new();
    game.started = true;
    game.position = [0.5, 0.5, 0.0];
    game.objects.push(LevelObject {
        position: [0.0, 0.0, -0.1],
        size: [1.0, 1.0, 0.2],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: "core/finish".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    });

    game.update(0.0);
    assert!(!game.level_complete);

    for _ in 0..180 {
        game.update(1.0 / 120.0);
        if game.level_complete {
            break;
        }
    }

    assert!(game.level_complete);
    assert!(!game.game_over);
}

#[test]
fn timeline_negative_time_clamps_to_zero() {
    let snapshot =
        simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[], -2.0);

    approx_eq(snapshot.position[0], 0.5, 1e-6);
    approx_eq(snapshot.position[1], 0.5, 1e-6);
    approx_eq(snapshot.position[2], 0.0, 1e-6);
    assert!(matches!(snapshot.direction, SpawnDirection::Forward));
    approx_eq(snapshot.elapsed_seconds, 0.0, 1e-6);
}

#[test]
fn timeline_tap_at_zero_turns_before_movement() {
    let dt = 1.0 / BASE_PLAYER_SPEED;
    let snapshot =
        simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[0.0], dt);

    assert!(matches!(snapshot.direction, SpawnDirection::Right));
    approx_eq(snapshot.position[0], 1.5, 0.05);
    approx_eq(snapshot.position[1], 0.5, 0.05);
    approx_eq(snapshot.elapsed_seconds, dt, 1e-6);
}

#[test]
fn timeline_incremental_runtime_matches_direct_simulation() {
    let mut runtime = TimelineSimulationRuntime::new(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &[],
        &[0.375, 0.125, 0.25],
    );

    for target in [0.05_f32, 0.2, 0.31, 0.6] {
        runtime.advance_to(target);
    }

    let incremental = runtime.snapshot();
    let direct = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &[],
        &[0.375, 0.125, 0.25],
        0.6,
    );

    approx_eq(incremental.position[0], direct.position[0], 0.02);
    approx_eq(incremental.position[1], direct.position[1], 0.02);
    approx_eq(incremental.position[2], direct.position[2], 0.02);
    assert!(matches!(
        (incremental.direction, direct.direction),
        (SpawnDirection::Forward, SpawnDirection::Forward)
            | (SpawnDirection::Right, SpawnDirection::Right)
    ));
    approx_eq(incremental.elapsed_seconds, direct.elapsed_seconds, 1e-6);
}
