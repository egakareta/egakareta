use super::EditorDirtyFlags;
use super::State;
use crate::commands::AppCommand;
use crate::editor_domain::{derive_tap_indicator_positions, derive_timeline_position};
use crate::types::{AppPhase, EditorMode, LevelObject, SpawnDirection};
use glam::{Vec2, Vec3};

#[test]
fn test_marquee_no_redundant_selections_before_drag_started() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
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
            .unwrap_or(0);
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
            .unwrap_or(0);
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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
        assert_eq!(state.phase, crate::types::AppPhase::Splash);

        state.start_editor(0);
        assert_eq!(state.phase, crate::types::AppPhase::Editor);

        state.toggle_editor(); // Should go back to menu from editor
        assert_eq!(state.phase, crate::types::AppPhase::Menu);
    });
}

#[test]
fn test_state_input_routing() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

        // First click exits splash to menu.
        state.handle_primary_click(0.0, 0.0);
        assert_eq!(state.phase, crate::types::AppPhase::Menu);

        // Second click in menu starts level.
        state.handle_primary_click(0.0, 0.0);
        assert_eq!(state.phase, crate::types::AppPhase::Playing);
    });
}

#[test]
fn multi_selection_clicking_rendered_gizmo_starts_gizmo_drag_not_block_drag() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
        let stored = state.session.playtest_audio_start_seconds;
        assert!(
            stored.is_some(),
            "playtest should store precomputed audio start"
        );
        assert!(
            (stored.unwrap_or_default() - expected_elapsed).abs() < 0.02,
            "stored playtest audio start seconds should match precomputed timeline elapsed"
        );
    });
}

#[test]
fn setting_timeline_duration_does_not_delete_taps() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };

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
    assert!(!EditorMode::Timing.can_select());
    assert!(!EditorMode::Null.can_select());
}

#[test]
fn editor_playback_mode_switching() {
    pollster::block_on(async {
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
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
        let mut state = match State::new_test().await {
            Some(s) => s,
            None => return,
        };
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
