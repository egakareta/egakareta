/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorDirtyFlags;
use super::State;
use crate::commands::AppCommand;
use crate::state::editor_command::EditorCommand;
use crate::test_utils::{block, editor_test, sized, stone};
use eg_audio::audio::runtime_asset_source_key;
use eg_editor_domain::{derive_tap_indicator_positions, derive_timeline_position};
use eg_game::simulate_timeline_state_with_triggers;
use eg_mesh::{GizmoAxis, GizmoDragKind};
use eg_types::{AppPhase, Direction, EditorMode, LevelObject, PhysicalSize};
use eg_types::{TimedTrigger, TimedTriggerAction, TimedTriggerEasing, TimedTriggerTarget};
use glam::{Vec2, Vec3};

async fn new_editor_state() -> State {
    let mut state = State::new_test().await;
    state.enter_editor_phase("Test Level".to_string());
    state
}

editor_test!(
    test_marquee_no_redundant_selections_before_drag_started,
    |state| {
        state.editor.objects.push(stone(0.0, 0.0, 0.0));
        state.editor.objects.push(stone(5.0, 0.0, 0.0));

        // 1. Just hovering over the first block (no mouse down)
        state.editor.ui.hovered_block_index = Some(0);
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
        assert!(state
            .render
            .meshes
            .editor_hover_stencil
            .draw_data()
            .is_some());

        // 2. Mouse down - starts marquee but drag hasn't started (moved only 1 pixel)
        state.editor.ui.left_mouse_down = true;
        state.editor.ui.marquee_start_screen = Some([100.0, 100.0]);
        state.editor.ui.marquee_current_screen = Some([101.0, 101.0]);

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
        state.editor.ui.marquee_current_screen = Some([200.0, 200.0]);
        let (_, _, is_active) = state.editor.marquee_selection_rect_screen().unwrap();
        assert!(is_active, "Marquee SHOULD be active now");
    }
);

editor_test!(
    selection_outline_builds_instances_per_selected_block,
    |state| {
        state.editor.objects.push(stone(0.0, 0.0, 0.0));
        state.editor.objects.push(stone(2.0, 0.0, 0.0));

        state.editor.ui.selected_block_index = Some(0);
        state.editor.ui.selected_block_indices = vec![0, 1];

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
);

#[tokio::test]
async fn selection_outline_uses_single_bounds_mesh_for_large_selections() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.ui.mode = EditorMode::Select;

    for index in 0..701 {
        state.editor.objects.push(LevelObject {
            position: [index as f32, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            ..LevelObject::default()
        });
    }
    state.editor.ui.selected_block_index = Some(0);
    state.editor.ui.selected_block_indices = (0..701).collect();

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
    let objects = [stone(0.0, 2.0, 0.0)];
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

fn configure_trigger_policy_parity_scene(
    state: &mut State,
    simulate_hitboxes: bool,
    timeline_time_seconds: f32,
) {
    state.editor.objects = vec![block("core/speedportal", 8.0, 0.0, 8.0)];
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = Direction::Forward;
    state.editor.timeline.taps.tap_times.clear();
    state.editor.set_triggers(vec![TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 0.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec!["object_8_0_8".to_string()],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [0.0, 0.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            simulate_hitboxes,
        },
    }]);
    state.editor.timeline.clock.time_seconds = timeline_time_seconds;
}

#[tokio::test]
async fn editor_playback_and_playtest_match_simulation_when_trigger_hitboxes_disabled() {
    let mut state = new_editor_state().await;
    let timeline_time_seconds = 0.35;
    configure_trigger_policy_parity_scene(&mut state, false, timeline_time_seconds);

    let expected = simulate_timeline_state_with_triggers(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.objects,
        &state.editor.timeline.taps.tap_times,
        &state.editor.triggers(),
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
    assert!(!state.editor_has_simulate_hitboxes_triggers());
}

#[tokio::test]
async fn editor_playback_and_playtest_match_simulation_when_trigger_hitboxes_enabled() {
    let mut state = new_editor_state().await;
    let timeline_time_seconds = 0.35;
    configure_trigger_policy_parity_scene(&mut state, true, timeline_time_seconds);

    let expected = simulate_timeline_state_with_triggers(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.objects,
        &state.editor.timeline.taps.tap_times,
        &state.editor.triggers(),
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
    assert!(state.editor_has_simulate_hitboxes_triggers());
    assert!(state.gameplay.state.speed > eg_game::BASE_PLAYER_SPEED);
}

#[tokio::test]
async fn editor_playtest_preserves_airborne_vertical_motion() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.timeline.taps.tap_times.clear();
    state.editor.spawn.position = [0.0, 2.0, 0.0];
    state.editor.timeline.clock.time_seconds = 0.25;

    let expected = simulate_timeline_state_with_triggers(
        state.editor.spawn.position,
        state.editor.spawn.direction,
        &state.editor.objects,
        &state.editor.timeline.taps.tap_times,
        &state.editor.triggers(),
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
    state.editor.ui.mode = EditorMode::Move;

    state.editor.objects = vec![stone(0.0, 0.0, 0.0), stone(4.0, 0.0, 0.0)];
    state.editor.ui.selected_block_indices = vec![0, 1];
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
    let music_source = state
        .load_level_metadata(&level_name)
        .expect("selected level metadata should exist")
        .music
        .source;

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
        runtime_asset_source_key(&level_name, &music_source),
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

    let current_level = state
        .menu
        .state
        .levels
        .first()
        .cloned()
        .expect("test requires at least one built-in level");
    let next_level = state
        .menu
        .state
        .levels
        .iter()
        .find(|name| *name != &current_level)
        .cloned()
        .expect("test requires at least two built-in levels");

    state.editor.ui.mode = EditorMode::Timing;
    state.editor.timing.waveform_samples = vec![0.25, -0.5, 0.75];
    state.editor.timing.waveform_sample_rate = 44_100;

    state.dispatch(AppCommand::Editor(EditorCommand::LoadLevel(
        next_level.clone(),
    )));

    assert!(
        state.editor.timing.waveform_samples.is_empty(),
        "switching levels in timing mode should clear stale waveform samples before loading"
    );
    assert_eq!(
        state.editor.timing.waveform_sample_rate, 0,
        "switching levels in timing mode should reset waveform sample rate before loading"
    );

    let expected_source_key =
        runtime_asset_source_key(&next_level, &state.session.editor_music_metadata.source);
    assert_eq!(
        state.audio.state.editor.waveform_loading_source,
        Some(expected_source_key),
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

    state.editor.objects = vec![stone(2.0, 1.0, 3.0)];
    state.editor.ui.selected_block_index = Some(0);
    state.editor.ui.selected_block_indices = vec![0];
    state.editor_copy_block();

    state.load_builtin_level_into_editor(&next_level);

    assert!(state.editor.runtime.interaction.clipboard.is_some());
    state.editor.ui.cursor = [8.0, 2.0, 9.0];
    let before_paste = state.editor.objects.len();
    state.editor_paste_block();

    assert_eq!(state.editor.objects.len(), before_paste + 1);
    assert_eq!(state.editor.objects[before_paste].position, [8.0, 2.0, 9.0]);
}

#[tokio::test]
async fn editor_clipboard_survives_exit_and_reentry() {
    let mut state = new_editor_state().await;

    state.editor.objects = vec![stone(1.0, 0.0, 1.0)];
    state.editor.ui.selected_block_index = Some(0);
    state.editor.ui.selected_block_indices = vec![0];
    state.editor_copy_block();

    state.back_to_menu();
    state.start_editor(0);

    assert!(state.editor.runtime.interaction.clipboard.is_some());
    state.editor.ui.cursor = [4.0, 0.0, 5.0];
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

    state.editor.ui.cursor = [5.0, 2.0, 0.0];
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
    state.editor.ui.mode = EditorMode::Select;
    state.editor.ui.right_dragging = true;
    state.editor.ui.cursor = [99.0, 99.0, 99.0];

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
    state.editor.ui.cursor = [99.0, 99.0, 99.0];

    state.dispatch(AppCommand::Editor(EditorCommand::UpdateCursorFromScreen {
        x: screen_x,
        y: screen_y,
    }));

    assert_eq!(state.editor.ui.cursor, expected_cursor);
}

#[tokio::test]
async fn spawn_hotkey_updates_spawn_from_pointer_in_select_mode() {
    let mut state = new_editor_state().await;
    state.editor.objects.clear();
    state.editor.ui.mode = EditorMode::Select;
    state.editor.ui.cursor = [99.0, 99.0, 99.0];

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
    state.editor.ui.cursor = [99.0, 99.0, 99.0];
    state.editor.ui.pointer_screen = Some([screen_x, screen_y]);

    state.dispatch(AppCommand::Editor(EditorCommand::SetSpawnHere));

    assert_eq!(state.editor.spawn.position, expected_cursor);
}

#[tokio::test]
async fn test_handle_primary_click_shift_priority() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.ui.mode = EditorMode::Select;

    // Scenario 1: Shift is held. Marquee should start even if we are over a selected block.
    state.editor.ui.shift_held = true;

    // Add a block and select it
    state
        .editor
        .objects
        .push(sized("core/stone", -5.0, -5.0, -5.0, 10.0, 10.0, 10.0));
    state.editor.ui.selected_block_indices.push(0);

    // Set camera to look at origin from above
    state.editor.camera.editor_pan = [0.0, 0.0];
    state.editor.camera.editor_target_z = 0.0;
    state.editor.camera.editor_pitch = -std::f32::consts::FRAC_PI_2; // Looking straight down
    state.editor.camera.editor_rotation = 0.0;

    // Click at the center (400, 300 in a 800x600 viewport)
    state.handle_primary_click(400.0, 300.0);

    assert!(
        state.editor.ui.marquee_start_screen.is_some(),
        "Marquee should have started when Shift is held"
    );
    assert!(
        state.editor.runtime.interaction.block_drag.is_none(),
        "Block drag should NOT have started when Shift is held"
    );

    // Scenario 2: Shift is NOT held. Block drag should start.
    state.editor.ui.marquee_start_screen = None;
    state.editor.set_left_mouse_down(false);
    state.editor.ui.shift_held = false;

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
    assert_eq!(state.editor.ui.mode, EditorMode::Null);
    assert_eq!(
        state.editor.runtime.interaction.last_mode,
        Some(EditorMode::Place)
    );

    // Stop playback
    state.toggle_editor_timeline_playback();
    assert!(!state.editor.is_playing());
    assert_eq!(state.editor.ui.mode, EditorMode::Place);
    assert!(state.editor.runtime.interaction.last_mode.is_none());
}

#[tokio::test]
async fn editor_null_mode_clears_selection() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.objects.push(stone(0.0, 0.0, 0.0));
    state.editor.ui.selected_block_index = Some(0);
    state.editor.set_mode(EditorMode::Place);
    assert_eq!(state.editor.ui.selected_block_index, Some(0));

    // Switching to Null should clear selection
    state.editor.set_mode(EditorMode::Null);
    assert!(state.editor.ui.selected_block_index.is_none());
}

#[tokio::test]
async fn test_timing_mode_persistence_during_playback() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;

    // 1. Enter Timing Mode
    state.editor.set_mode(EditorMode::Timing);
    assert_eq!(state.editor.ui.mode, EditorMode::Timing);

    // 2. Toggle playback
    state.toggle_editor_timeline_playback();

    // Mode should be Null for playback safety
    assert_eq!(state.editor.ui.mode, EditorMode::Null);
    // last_mode should be Timing
    assert_eq!(
        state.editor.runtime.interaction.last_mode,
        Some(EditorMode::Timing)
    );

    // 3. Verify ViewModel shows we are still conceptually in Timing
    let view = state.editor_ui_view_model();
    let is_timing = view.mode == EditorMode::Timing
        || (view.mode == EditorMode::Null && view.last_mode == Some(EditorMode::Timing));
    assert!(
        is_timing,
        "UI should still consider itself in Timing mode during playback"
    );

    // 4. Toggle playback off
    state.toggle_editor_timeline_playback();

    // Should return to Timing mode
    assert_eq!(state.editor.ui.mode, EditorMode::Timing);
    assert!(state.editor.runtime.interaction.last_mode.is_none());
}

#[tokio::test]
async fn timing_origin_playback_seek_resync_keeps_runtime_disabled() {
    let mut state = State::new_test().await;
    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);

    state.editor.set_triggers(vec![TimedTrigger {
        time_seconds: 0.5,
        duration_seconds: 1.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec!["object_0_0_0".to_string()],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [3.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            simulate_hitboxes: false,
        },
    }]);

    state.toggle_editor_timeline_playback();
    assert!(state.editor.timeline.playback.playing);
    assert_eq!(state.editor.ui.mode, EditorMode::Null);
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

    state.editor.objects.push(stone(0.0, 0.0, 0.0));
    state.editor.ui.selected_block_indices.push(0);
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
    state.editor.objects.push(stone(0.0, 0.0, 0.0));
    state.editor.ui.selected_block_indices.push(0);
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
    state.editor.objects.push(stone(
        move_x_handle_world.x - 0.5,
        move_x_handle_world.y - 0.5,
        move_x_handle_world.z - 0.5,
    ));
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
            state.editor.ui.hovered_block_index.is_none(),
            "expected block hover to be suppressed while gizmo handle is hovered, even though a block is behind it"
        );
}

#[tokio::test]
async fn timing_mode_timeline_scrub_skips_simulation_and_mesh_dirty() {
    let mut state = State::new_test().await;

    state.phase = AppPhase::Editor;
    state.editor.set_mode(EditorMode::Timing);

    // Add an object transform trigger to ensure that in Compose it WOULD mark dirty
    state.editor.set_triggers(vec![TimedTrigger {
        time_seconds: 1.0,
        duration_seconds: 1.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec!["object_0_0_0".to_string()],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [10.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            simulate_hitboxes: false,
        },
    }]);

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
    state.editor.set_triggers(vec![TimedTrigger {
        time_seconds: 1.0,
        duration_seconds: 1.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Objects {
            object_ids: vec!["object_0_0_0".to_string()],
        },
        action: TimedTriggerAction::TransformObjects {
            position: [10.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            simulate_hitboxes: false,
        },
    }]);

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

    state.editor.set_triggers(vec![TimedTrigger {
        time_seconds: 1.0,
        duration_seconds: 0.0,
        easing: TimedTriggerEasing::Linear,
        target: TimedTriggerTarget::Camera,
        action: TimedTriggerAction::CameraPose {
            transition_interval_seconds: 1.0,
            use_full_segment_transition: false,
            target_position: [0.0, 0.0, 0.0],
            rotation: 0.0,
            pitch: 0.0,
        },
    }]);

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
    state.editor.ui.cursor = [0.0, 2.0, 0.0];
    state.next_level();
    assert_eq!(state.editor.ui.cursor, [1.0, 2.0, 0.0]);
    state.prev_level();
    assert_eq!(state.editor.ui.cursor, [0.0, 2.0, 0.0]);

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
    state.editor.ui.cursor = [2.0, 0.0, 2.0];
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
    state.editor.ui.marquee_start_screen = Some([32.0, 64.0]);
    state.editor.ui.marquee_current_screen = Some([48.0, 80.0]);
    state.handle_mouse_button(0, false);
    assert!(state.editor.ui.marquee_start_screen.is_none());
    assert!(state.editor.ui.marquee_current_screen.is_none());

    state.handle_mouse_button(2, true);
    assert!(state.editor.ui.right_dragging);
    state.handle_mouse_button(2, false);
    assert!(!state.editor.ui.right_dragging);

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
    assert_eq!(state.editor.ui.pointer_screen, Some([220.0, 180.0]));
    assert!(state.editor.ui.left_mouse_down);
    assert!(state.editor.ui.marquee_start_screen.is_none());

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
        state.editor.ui.pointer_screen,
        Some([future_tap_screen.x as f64, future_tap_screen.y as f64])
    );
    assert!(state.editor_tap_times().is_empty());

    state.editor.set_left_mouse_down(false);
    state.handle_primary_click(future_tap_screen.x as f64, future_tap_screen.y as f64);
    assert_eq!(state.editor_tap_times().len(), 1);
    assert_eq!(state.editor.timeline.taps.selected_index, Some(0));
    assert!(state.editor.ui.marquee_start_screen.is_none());

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
