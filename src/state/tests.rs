/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::EditorDirtyFlags;
use super::State;
use crate::commands::AppCommand;
use crate::editor_domain::{derive_tap_indicator_positions, derive_timeline_position};
use crate::game::simulate_timeline_state_with_triggers;
use crate::platform::audio::runtime_asset_source_key;
use crate::test_utils::assert_approx_eq as approx_eq;
use crate::types::{AppPhase, EditorMode, GizmoAxis, GizmoDragKind, LevelObject, SpawnDirection};
use glam::{Vec2, Vec3};

#[test]
fn test_marquee_no_redundant_selections_before_drag_started() {
    pollster::block_on(async {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.ui.mode = EditorMode::Select;

        // Add two blocks
        state.editor.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });
        state.editor.objects.push(LevelObject {
            position: [5.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });

        // 1. Just hovering over the first block (no mouse down)
        state.editor.ui.hovered_block_index = Some(0);
        state.rebuild_editor_hover_outline_vertices();

        let count = state
            .render
            .meshes
            .editor_hover_outline
            .draw_data()
            .map(|(_, c)| c)
            .expect("hover outline mesh should exist after rebuilding vertices");
        // One block outline = 12 prisms * 36 vertices/prism = 432 vertices
        assert_eq!(
            count, 432,
            "Should only have one block outlined when just hovering"
        );

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
            .map(|(_, c)| c)
            .expect("hover outline mesh should still exist before marquee drag activates");
        assert_eq!(
            count, 432,
            "Should still only have one block outlined if marquee drag hasn't started"
        );

        // 3. Drag far enough for marquee to be active
        state.editor.ui.marquee_current_screen = Some([200.0, 200.0]);
        let (_, _, is_active) = state.editor.marquee_selection_rect_screen().unwrap();
        assert!(is_active, "Marquee SHOULD be active now");
    });
}

// ── EditorDirtyFlags contract tests ─────────────────────────────
#[test]
fn dirty_flags_default_is_clean() {
    let flags = EditorDirtyFlags::default();
    assert!(!flags.any());
}

#[test]
fn dirty_flags_from_object_sync_sets_all() {
    let flags = EditorDirtyFlags::from_object_sync();
    assert!(flags.sync_game_objects);
    assert!(flags.rebuild_block_mesh);
    assert!(flags.rebuild_selection_overlays);
    assert!(flags.rebuild_tap_indicators);
    assert!(flags.rebuild_preview_player);
    assert!(flags.any());
}

#[test]
fn dirty_flags_merge_is_union() {
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

#[test]
fn derives_position_without_taps() {
    let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
    let (position, direction) = derive_timeline_position(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &[],
        3.0 * step_time,
        &[],
    );
    assert!((position[0] - 0.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, SpawnDirection::Forward));
}

#[test]
fn derives_position_with_taps() {
    let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
    let taps = [2.0 * step_time, 4.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [0.0, 0.0, 0.0],
        SpawnDirection::Forward,
        &taps,
        5.0 * step_time,
        &[],
    );
    assert!((position[0] - 2.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, SpawnDirection::Forward));
}

#[test]
fn tap_at_zero_changes_direction() {
    let taps = [0.0];
    let (position, direction) =
        derive_timeline_position([0.0, 0.0, 0.0], SpawnDirection::Forward, &taps, 0.0, &[]);
    assert!((position[0] - 0.5).abs() < 0.1);
    assert!((position[2] - 0.5).abs() < 0.1);
    assert!(matches!(direction, SpawnDirection::Right));
}

#[test]
fn ignores_taps_after_step() {
    let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
    let taps = [5.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [1.0, 0.0, 1.0],
        SpawnDirection::Forward,
        &taps,
        2.0 * step_time,
        &[],
    );
    assert!((position[0] - 1.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, SpawnDirection::Forward));
}

#[test]
fn supports_offset_spawn_with_tap() {
    let step_time = 1.0 / crate::game::BASE_PLAYER_SPEED;
    let taps = [2.0 * step_time];
    let (position, direction) = derive_timeline_position(
        [2.0, 0.0, 2.0],
        SpawnDirection::Right,
        &taps,
        3.0 * step_time,
        &[],
    );
    assert!((position[0] - 4.5).abs() < 0.1);
    assert!((position[2] - 3.5).abs() < 0.1);
    assert!(matches!(direction, SpawnDirection::Forward));
}

#[test]
fn falls_from_elevated_platform() {
    let objects = [LevelObject {
        position: [0.0, 2.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        roundness: 0.18,
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    }];
    let (position, direction) = derive_timeline_position(
        [0.0, 3.0, 0.0],
        SpawnDirection::Forward,
        &[],
        1.0 / crate::game::BASE_PLAYER_SPEED,
        &objects,
    );
    assert!(position[1] <= 3.0);
    assert!(matches!(direction, SpawnDirection::Forward));
}

#[test]
fn test_state_phase_integrity() {
    pollster::block_on(async {
        let mut state = State::new_test().await;
        assert_eq!(state.phase, crate::types::AppPhase::Menu);

        state.start_editor(0);
        assert_eq!(state.phase, crate::types::AppPhase::Editor);

        state.toggle_editor(); // Should go back to menu from editor
        assert_eq!(state.phase, crate::types::AppPhase::Menu);
    });
}

fn configure_trigger_policy_parity_scene(
    state: &mut State,
    simulate_trigger_hitboxes: bool,
    timeline_time_seconds: f32,
) {
    state.editor.objects = vec![LevelObject {
        position: [8.0, 0.0, 8.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        roundness: 0.18,
        block_id: "core/speedportal".to_string(),
        color_tint: [1.0, 1.0, 1.0],
    }];
    state.editor.spawn.position = [0.0, 0.0, 0.0];
    state.editor.spawn.direction = SpawnDirection::Forward;
    state.editor.timeline.taps.tap_times.clear();
    state.editor.set_triggers(vec![crate::types::TimedTrigger {
        time_seconds: 0.0,
        duration_seconds: 0.0,
        easing: crate::types::TimedTriggerEasing::Linear,
        target: crate::types::TimedTriggerTarget::Object { object_id: 0 },
        action: crate::types::TimedTriggerAction::MoveTo {
            position: [0.0, 0.0, 1.0],
        },
    }]);
    state.set_editor_simulate_trigger_hitboxes(simulate_trigger_hitboxes);
    state.editor.timeline.clock.time_seconds = timeline_time_seconds;
}

#[test]
fn editor_playback_and_playtest_match_simulation_when_trigger_hitboxes_disabled() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.start_editor(0);
        let timeline_time_seconds = 0.35;
        configure_trigger_policy_parity_scene(&mut state, false, timeline_time_seconds);

        let expected = simulate_timeline_state_with_triggers(
            state.editor.spawn.position,
            state.editor.spawn.direction,
            &state.editor.objects,
            &state.editor.timeline.taps.tap_times,
            state.editor.triggers(),
            state.editor.simulate_trigger_hitboxes(),
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

        approx_eq(playback_snapshot.position[0], expected.position[0], 1e-4);
        approx_eq(playback_snapshot.position[1], expected.position[1], 1e-4);
        approx_eq(playback_snapshot.position[2], expected.position[2], 1e-4);
        approx_eq(playback_snapshot.speed, expected.speed, 1e-4);

        state.editor_playtest();
        approx_eq(state.gameplay.state.position[0], expected.position[0], 1e-4);
        approx_eq(state.gameplay.state.position[1], expected.position[1], 1e-4);
        approx_eq(state.gameplay.state.position[2], expected.position[2], 1e-4);
        approx_eq(state.gameplay.state.speed, expected.speed, 1e-4);
        assert!(!state.session.playing_trigger_hitboxes);
    });
}

#[test]
fn editor_playback_and_playtest_match_simulation_when_trigger_hitboxes_enabled() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.start_editor(0);
        let timeline_time_seconds = 0.35;
        configure_trigger_policy_parity_scene(&mut state, true, timeline_time_seconds);

        let expected = simulate_timeline_state_with_triggers(
            state.editor.spawn.position,
            state.editor.spawn.direction,
            &state.editor.objects,
            &state.editor.timeline.taps.tap_times,
            state.editor.triggers(),
            state.editor.simulate_trigger_hitboxes(),
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

        approx_eq(playback_snapshot.position[0], expected.position[0], 1e-4);
        approx_eq(playback_snapshot.position[1], expected.position[1], 1e-4);
        approx_eq(playback_snapshot.position[2], expected.position[2], 1e-4);
        approx_eq(playback_snapshot.speed, expected.speed, 1e-4);

        state.editor_playtest();
        approx_eq(state.gameplay.state.position[0], expected.position[0], 1e-4);
        approx_eq(state.gameplay.state.position[1], expected.position[1], 1e-4);
        approx_eq(state.gameplay.state.position[2], expected.position[2], 1e-4);
        approx_eq(state.gameplay.state.speed, expected.speed, 1e-4);
        assert!(state.session.playing_trigger_hitboxes);
        assert!(state.gameplay.state.speed > crate::game::BASE_PLAYER_SPEED);
    });
}

#[test]
fn editor_scrub_draws_ghost_trail_without_preview_head_mesh() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.start_editor(0);
        state.editor.set_mode(EditorMode::Place);
        state.set_editor_timeline_time_seconds(0.65);

        state.update();

        let trail_count = state
            .render
            .meshes
            .trail
            .draw_data()
            .map(|(_, count)| count)
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
    });
}

#[test]
fn editor_playback_draws_ghost_trail_without_preview_head_mesh() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.start_editor(0);
        state.editor.set_mode(EditorMode::Place);
        state.set_editor_timeline_time_seconds(0.65);
        state.toggle_editor_timeline_playback();

        state.update();

        let trail_count = state
            .render
            .meshes
            .trail
            .draw_data()
            .map(|(_, count)| count)
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
    });
}

#[test]
fn test_state_input_routing() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        // First click in menu starts level.
        state.handle_primary_click(0.0, 0.0);
        assert_eq!(state.phase, crate::types::AppPhase::Playing);
    });
}

#[test]
fn multi_selection_clicking_rendered_gizmo_starts_gizmo_drag_not_block_drag() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.start_editor(0);
        state.editor.ui.mode = EditorMode::Move;

        state.editor.objects = vec![
            LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                position: [4.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                roundness: 0.18,
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
        ];
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
    });
}

#[test]
fn timeline_seek_uses_interpolated_snapshot_cache_and_supports_backward_seek() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Editor;
        state.editor.objects.clear();
        state.editor.spawn.position = [0.0, 0.0, 0.0];
        state.editor.spawn.direction = SpawnDirection::Forward;
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
            state.editor.timeline.snapshot_cache_revision,
            state.editor.timeline.simulation_revision,
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
    });
}

#[test]
fn editor_playtest_stores_precomputed_audio_start_seconds() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.dispatch(AppCommand::ToggleEditor);
        state.editor.objects.clear();
        state.editor.timeline.taps.tap_times.clear();
        state.editor.spawn.position = [0.0, 0.0, 0.0];
        state.editor.spawn.direction = SpawnDirection::Forward;

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
    });
}

#[test]
fn editor_playtest_nonzero_timeline_handoff_stays_synced_after_first_input() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.dispatch(AppCommand::ToggleEditor);
        state.editor.objects.clear();
        state.editor.timeline.taps.tap_times = vec![0.35, 0.7, 1.1];
        state.editor.spawn.position = [0.0, 0.0, 0.0];
        state.editor.spawn.direction = SpawnDirection::Forward;

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
    });
}

#[test]
fn editor_playtest_warms_audio_before_first_input() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.dispatch(AppCommand::ToggleEditor);

        let level_name = state
            .session
            .editor_level_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let music_source = state.session.editor_music_metadata.source.clone();

        let target_time = 0.85;
        state.editor.timeline.clock.time_seconds = target_time;
        let expected_elapsed = state.editor_timeline_elapsed_seconds(target_time);

        state.editor_playtest();

        let warmup = state
            .audio
            .state
            .runtime_preload
            .last_warmup_request
            .clone()
            .expect("playtest should warm up runtime audio before first input");
        let (source_key, warmup_seconds) = warmup;
        assert_eq!(
            source_key,
            runtime_asset_source_key(&level_name, &music_source),
            "warmup should target the active playtest source"
        );
        assert!(
            (warmup_seconds - expected_elapsed).abs() < 0.02,
            "warmup should use precomputed timeline elapsed seconds"
        );
        assert!(
            !state.gameplay.state.started,
            "warmup must happen before first input starts gameplay"
        );
    });
}

#[test]
fn editor_load_level_in_timing_mode_reloads_waveform_for_new_level() {
    pollster::block_on(async {
        let mut state = State::new_test().await;
        state.start_editor(0);

        let current_level = state
            .session
            .editor_level_name
            .clone()
            .expect("editor level should be set after starting editor");
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

        state.dispatch(AppCommand::EditorLoadLevel(next_level.clone()));

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
    });
}

#[test]
fn setting_timeline_duration_does_not_delete_taps() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.dispatch(AppCommand::ToggleEditor);

        // Add some taps beyond 0 duration
        state.editor.timeline.taps.tap_times = vec![1.0, 2.0, 3.0];
        state.editor.timeline.taps.tap_indicator_positions = vec![[0.0, 0.0, 0.0]; 3];

        // Set duration to 0
        state.set_editor_timeline_duration_seconds(0.0);

        // Taps should still be there
        assert_eq!(state.editor.timeline.taps.tap_times, vec![1.0, 2.0, 3.0]);
        assert_eq!(state.editor.timeline.taps.tap_indicator_positions.len(), 3);
    });
}

#[test]
fn setting_spawn_recomputes_tap_indicator_positions() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Menu;
        state.dispatch(AppCommand::ToggleEditor);
        state.editor.objects.clear();
        state.editor.spawn.position = [0.0, 0.0, 0.0];
        state.editor.spawn.direction = SpawnDirection::Forward;
        state.editor.timeline.taps.tap_times = vec![1.0 / crate::game::BASE_PLAYER_SPEED];
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
    });
}

#[test]
fn test_handle_primary_click_shift_priority() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Editor;
        state.editor.ui.mode = EditorMode::Select;

        // Scenario 1: Shift is held. Marquee should start even if we are over a selected block.
        state.editor.ui.shift_held = true;

        // Add a block and select it
        state.editor.objects.push(LevelObject {
            position: [-5.0, -5.0, -5.0],
            size: [10.0, 10.0, 10.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });
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
    });
}

#[test]
fn editor_mode_selection_constraints() {
    assert!(EditorMode::Select.can_select());
    assert!(EditorMode::Move.can_select());
    assert!(EditorMode::Scale.can_select());
    assert!(EditorMode::Place.can_select());
    assert!(!EditorMode::Trigger.can_select());
    assert!(!EditorMode::Timing.can_select());
    assert!(!EditorMode::Null.can_select());
}

#[test]
fn editor_playback_mode_switching() {
    pollster::block_on(async {
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
    });
}

#[test]
fn editor_null_mode_clears_selection() {
    pollster::block_on(async {
        let mut state = State::new_test().await;
        state.phase = AppPhase::Editor;
        state.editor.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });
        state.editor.ui.selected_block_index = Some(0);
        state.editor.set_mode(EditorMode::Place);
        assert_eq!(state.editor.ui.selected_block_index, Some(0));

        // Switching to Null should clear selection
        state.editor.set_mode(EditorMode::Null);
        assert!(state.editor.ui.selected_block_index.is_none());
    });
}

#[test]
fn test_timing_mode_persistence_during_playback() {
    pollster::block_on(async {
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
    });
}
#[test]
fn test_gizmo_move_shaft_is_pickable() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Editor;
        state.editor.set_mode(EditorMode::Move);

        state.editor.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });
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
    });
}

#[test]
fn test_gizmo_hover_priority_suppresses_block_outline() {
    pollster::block_on(async {
        let mut state = State::new_test().await;

        state.phase = AppPhase::Editor;
        state.editor.set_mode(EditorMode::Move);

        // 1. Add a primary block and select it to show the gizmo
        state.editor.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        });
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
        state.editor.objects.push(LevelObject {
            position: [
                move_x_handle_world.x - 0.5,
                move_x_handle_world.y - 0.5,
                move_x_handle_world.z - 0.5,
            ],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.18,
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
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
            state.editor.ui.hovered_block_index.is_none(),
            "expected block hover to be suppressed while gizmo handle is hovered, even though a block is behind it"
        );
    });
}
