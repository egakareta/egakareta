/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::physics::{aabb_overlaps_object_xz, object_xz_contains, BASE_PLAYER_SPEED};
use super::simulation::{
    simulate_timeline_state, simulate_timeline_state_with_triggers, TimelineSimulationRuntime,
};
use super::state::GameState;
use crate::test_utils::assert_approx_eq as approx_eq;
use crate::triggers::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};
use crate::types::{LevelObject, SpawnDirection};

#[test]
fn test_ground_detection_normal() {
    let mut game = GameState::new();
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();

    // Player at x=0.5, z=0.5 (center of block), check top Y height.
    let height = game.top_surface_y_at(0.5, 0.5, 2.0);
    assert_eq!(height, Some(1.0));
}

#[test]
fn test_ground_detection_includes_block_edges() {
    let mut game = GameState::new();
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();

    assert_eq!(game.top_surface_y_at(0.0, 0.5, 2.0), Some(1.0));
    assert_eq!(game.top_surface_y_at(1.0, 0.5, 2.0), Some(1.0));
    assert_eq!(game.top_surface_y_at(0.5, 0.0, 2.0), Some(1.0));
    assert_eq!(game.top_surface_y_at(0.5, 1.0, 2.0), Some(1.0));
}

#[test]
fn player_stays_grounded_when_footprint_overlaps_platform_edge() {
    let mut game = GameState::new();
    game.started = true;
    game.position = [1.05, 1.0, 0.5];
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();

    for _ in 0..12 {
        game.update(1.0 / 120.0);
    }

    assert!(game.is_grounded);
    assert!(!game.game_over);
    approx_eq(game.position[1], 1.0, 1e-6);
}

#[test]
fn test_ground_detection_under_overhang() {
    let mut game = GameState::new();
    // Ground block
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    // Overhang block at height 3
    game.objects.push(LevelObject {
        position: [0.0, 3.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();

    // Player is walking on the ground block (y=1).
    // We check top surface with max_y slightly above player head.
    // It should ignore the block at z=3.
    let height = game.top_surface_y_at(0.5, 0.5, 1.5);
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
        rotation_degrees: [0.0, 90.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    };

    assert!(object_xz_contains(&obj, 1.0, 0.5));
    assert!(!object_xz_contains(&obj, 2.1, 0.5));
}

#[test]
fn rotated_overlap_uses_oriented_bounds() {
    let obj = LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [2.0, 1.0, 1.0],
        rotation_degrees: [0.0, 45.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    };

    assert!(aabb_overlaps_object_xz(0.9, 1.1, 0.3, 0.5, &obj));
    assert!(!aabb_overlaps_object_xz(3.0, 3.4, 3.0, 3.4, &obj));
}

#[test]
fn rotated_ground_detection_works() {
    let mut game = GameState::new();
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [2.0, 2.0, 1.0],
        rotation_degrees: [0.0, 90.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();

    let inside = game.top_surface_y_at(1.0, 0.5, 3.0);
    let outside = game.top_surface_y_at(2.2, 0.5, 3.0);
    assert_eq!(inside, Some(2.0));
    assert_eq!(outside, Some(0.0));
}

#[test]
fn speed_portal_overlap_removes_portal_and_boosts_speed() {
    let mut game = GameState::new();
    game.started = true;
    game.position = [0.5, 0.0, 0.2];
    game.speed = 1.0;
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 30.0, 0.0],
        block_id: "core/speedportal".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();
    game.update(0.0);

    approx_eq(game.speed, 1.5, 1e-6);
    assert!(game.objects.is_empty());
    assert!(!game.game_over);
}

#[test]
fn gem_overlap_collects_and_tracks_progress() {
    let mut game = GameState::new();
    game.started = true;
    game.position = [0.5, 0.0, 0.5];
    game.objects.push(LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [0.72, 0.86, 0.72],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/gem".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    });
    game.rebuild_behavior_cache();
    game.initialize_level_progress_from_objects();

    game.update(0.0);

    let progress = game.level_progress();
    assert_eq!(progress.gems_collected, 1);
    assert_eq!(progress.gems_max, 1);
    let events = game.take_consumed_object_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].object.block_id, "core/gem");
    assert!(game.take_consumed_object_events().is_empty());
    assert!(game.objects.is_empty());
    assert!(!game.game_over);
}

#[test]
fn level_completes_at_configured_duration() {
    let mut game = GameState::new();
    game.started = true;
    game.set_level_duration_seconds(0.05);

    game.update(0.04);
    assert!(!game.level_complete);

    game.update(0.02);

    assert!(game.level_complete);
    assert!(!game.started);
    assert!(!game.game_over);
    assert!(game.level_progress().completed);
    approx_eq(game.level_progress().progress_percent, 100.0, 1e-6);
    approx_eq(game.elapsed_seconds, 0.05, 1e-6);
}

#[test]
fn timeline_negative_time_clamps_to_zero() {
    let snapshot =
        simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[], -2.0);

    approx_eq(snapshot.position[0], 0.5, 1e-6);
    approx_eq(snapshot.position[1], 0.0, 1e-6);
    approx_eq(snapshot.position[2], 0.5, 1e-6);
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
    approx_eq(snapshot.position[2], 0.5, 0.05);
    approx_eq(snapshot.elapsed_seconds, dt, 1e-6);
}

#[test]
fn timeline_tap_at_nonzero_time_turns_at_exact_boundary() {
    let tap_time = 1.0 / BASE_PLAYER_SPEED;
    let snapshot = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &[],
        &[tap_time],
        tap_time,
    );

    assert!(matches!(snapshot.direction, SpawnDirection::Right));
    approx_eq(snapshot.position[0], 0.5, 1e-5);
    approx_eq(snapshot.position[2], 1.5, 1e-5);
    approx_eq(snapshot.elapsed_seconds, tap_time, 1e-6);

    let next_tick = tap_time + 1.0 / 240.0;
    let after_turn = simulate_timeline_state(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &[],
        &[tap_time],
        next_tick,
    );

    approx_eq(
        after_turn.position[0],
        0.5 + BASE_PLAYER_SPEED / 240.0,
        1e-5,
    );
    approx_eq(after_turn.position[2], 1.5, 1e-5);
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

#[test]
fn timeline_trigger_hitbox_mode_does_not_resurrect_consumed_portals() {
    let objects = vec![LevelObject {
        position: [0.0, 0.0, 1.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/speedportal".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    }];

    let triggers = vec![TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 0.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec![0],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [0.0, 0.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
        },
    }];

    let mut runtime = TimelineSimulationRuntime::new_with_triggers(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &objects,
        &[],
        &triggers,
        true,
    );
    runtime.advance_to(0.6);

    let snapshot = runtime.snapshot();
    approx_eq(snapshot.speed, BASE_PLAYER_SPEED * 1.5, 1e-4);
}

#[test]
fn timeline_state_with_disabled_trigger_hitboxes_matches_plain_simulation() {
    let objects = vec![LevelObject {
        position: [0.0, 0.0, 2.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    }];

    let triggers = vec![TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 1.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec![0],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [5.0, 0.0, 2.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
        },
    }];

    let plain =
        simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &objects, &[], 0.5);

    let trigger_disabled = simulate_timeline_state_with_triggers(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &objects,
        &[],
        &triggers,
        false,
        0.5,
    );

    approx_eq(plain.position[0], trigger_disabled.position[0], 1e-6);
    approx_eq(plain.position[1], trigger_disabled.position[1], 1e-6);
    approx_eq(plain.position[2], trigger_disabled.position[2], 1e-6);
    approx_eq(plain.speed, trigger_disabled.speed, 1e-6);
}
