pub(crate) mod cursor;
pub(crate) mod session;
pub(crate) mod tap_management;
pub(crate) mod timeline;
pub(crate) mod transitions;

pub(crate) use cursor::*;
pub(crate) use session::*;
pub(crate) use tap_management::*;
pub(crate) use timeline::*;
pub(crate) use transitions::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LevelMetadata, LevelObject, MusicMetadata, SpawnMetadata};

    #[test]
    fn keeps_tap_times_unique_and_sorted() {
        let mut taps = vec![0.4, 0.1];
        add_tap_time(&mut taps, 0.3);
        add_tap_time(&mut taps, 0.1);
        assert_eq!(taps, vec![0.1, 0.3, 0.4]);
    }

    #[test]
    fn can_remove_and_clear_tap_times() {
        let mut taps = vec![0.1, 0.2, 0.3];
        remove_tap_time(&mut taps, 0.2);
        assert_eq!(taps, vec![0.1, 0.3]);
        clear_tap_times(&mut taps);
        assert!(taps.is_empty());
    }

    #[test]
    fn keeps_tap_indicators_in_sync_with_tap_time_edits() {
        let mut taps = Vec::new();
        let mut indicators = Vec::new();

        add_tap_with_indicator(&mut taps, &mut indicators, 0.3, [3.0, 0.0, 0.0]);
        add_tap_with_indicator(&mut taps, &mut indicators, 0.1, [1.0, 0.0, 0.0]);
        add_tap_with_indicator(&mut taps, &mut indicators, 0.2, [2.0, 0.0, 0.0]);

        assert_eq!(taps, vec![0.1, 0.2, 0.3]);
        assert_eq!(
            indicators,
            vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]
        );

        remove_tap_with_indicator(&mut taps, &mut indicators, 0.2);
        assert_eq!(taps, vec![0.1, 0.3]);
        assert_eq!(indicators, vec![[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);

        clear_taps_with_indicators(&mut taps, &mut indicators);
        assert!(taps.is_empty());
        assert!(indicators.is_empty());
    }

    #[test]
    fn moves_cursor_within_bounds() {
        let mut cursor = [0.0, 0.0, 0.0];
        move_cursor_xy(&mut cursor, 8, -10, 3);
        assert_eq!(cursor, [3.0, -3.0, 0.0]);
    }

    #[test]
    fn creates_block_at_cursor() {
        let block = create_block_at_cursor([1.0, 2.0, 3.0], "core/grass");
        assert_eq!(block.position, [1.0, 2.0, 3.0]);
        assert_eq!(block.size, [1.0, 1.0, 1.0]);
        assert_eq!(block.block_id, "core/grass");
    }

    #[test]
    fn removes_topmost_block_at_cursor_cell() {
        let mut objects = vec![
            LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
            LevelObject {
                position: [0.0, 0.0, 1.0],
                size: [1.0, 1.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/grass".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            },
        ];

        let removed = remove_topmost_block_at_cursor(&mut objects, [0.0, 0.0, 0.0]);
        assert!(removed);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn initializes_editor_session_from_metadata() {
        let metadata = LevelMetadata {
            format_version: 1,
            name: "Test".to_string(),
            music: MusicMetadata {
                source: "audio.mp3".to_string(),
                title: None,
                author: None,
                extra: serde_json::Map::new(),
            },
            spawn: SpawnMetadata {
                position: [2.0, 3.0, 1.0],
                direction: crate::types::SpawnDirection::Right,
            },
            tap_times: vec![0.8, 0.2],
            timing_points: Vec::new(),
            timeline_time_seconds: 0.5,
            timeline_duration_seconds: 16.0,
            legacy_taps: Vec::new(),
            legacy_timeline_step: 0,
            objects: vec![LevelObject {
                position: [4.0, 6.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            }],
            extra: serde_json::Map::new(),
        };

        let init = editor_session_init_from_metadata(Some(metadata));
        assert_eq!(init.cursor, [4.0, 6.0, 0.0]);
        assert_eq!(init.camera_pan, [4.5, 6.5]);
        assert_eq!(init.tap_times, vec![0.2, 0.8]);
        assert!((init.timeline_time_seconds - 0.5).abs() <= 1e-6);
    }

    #[test]
    fn initializes_editor_session_defaults_without_metadata() {
        let init = editor_session_init_from_metadata(None);
        assert_eq!(init.cursor, [0.0, 0.0, 0.0]);
        assert_eq!(init.camera_pan, [0.5, 0.5]);
        assert_eq!(init.timeline_time_seconds, 0.0);
        assert!(init.tap_times.is_empty());
        assert!(init.objects.is_empty());
    }

    #[test]
    fn builds_editor_playtest_transition() {
        let objects = Vec::new();
        let timeline_time_seconds = 1.0 / crate::game::BASE_PLAYER_SPEED;

        let transition = build_editor_playtest_transition(
            &objects,
            Some("Demo"),
            SpawnMetadata::default(),
            &[],
            timeline_time_seconds,
        );

        assert!(transition.objects.is_empty());
        assert!((transition.spawn_position[1] - 1.5).abs() < 0.1);
        assert!(matches!(
            transition.spawn_direction,
            crate::types::SpawnDirection::Forward
        ));
        assert!((transition.playtest_audio_start_seconds - timeline_time_seconds).abs() < 0.05);
        assert_eq!(transition.playing_level_name.as_deref(), Some("Demo"));
    }

    #[test]
    fn returns_objects_only_when_playtesting() {
        let objects = vec![LevelObject {
            position: [1.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
            color_tint: [1.0, 1.0, 1.0],
        }];

        assert!(playtest_return_objects(true, &objects).is_some());
        assert!(playtest_return_objects(false, &objects).is_none());
    }

    #[test]
    fn builds_playing_transition_from_metadata() {
        let metadata = LevelMetadata {
            format_version: 1,
            name: "Starter".to_string(),
            music: MusicMetadata {
                source: "audio.mp3".to_string(),
                title: None,
                author: None,
                extra: serde_json::Map::new(),
            },
            spawn: SpawnMetadata {
                position: [3.0, 4.0, 1.0],
                direction: crate::types::SpawnDirection::Right,
            },
            tap_times: vec![],
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            legacy_taps: Vec::new(),
            legacy_timeline_step: 0,
            objects: vec![LevelObject {
                position: [1.0, 2.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/standard".to_string(),
                color_tint: [1.0, 1.0, 1.0],
            }],
            extra: serde_json::Map::new(),
        };

        let transition = build_playing_transition_from_metadata(metadata);
        assert_eq!(transition.level_name, "Starter");
        assert_eq!(transition.spawn_position, [3.0, 4.0, 1.0]);
        assert!(matches!(
            transition.spawn_direction,
            crate::types::SpawnDirection::Right
        ));
        assert_eq!(transition.objects.len(), 1);
    }

    #[test]
    fn derives_timeline_time_for_forward_target_cell() {
        let target = [0.5, 4.5, 0.0];
        let time = derive_timeline_time_for_world_target(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[],
            8.0,
            &[],
            target,
        );

        assert!((time - 0.5).abs() < 0.08, "unexpected time: {time}");
    }

    #[test]
    fn derives_timeline_time_for_turned_target_cell() {
        let target = [2.5, 0.5, 0.0];
        let time = derive_timeline_time_for_world_target(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[0.0],
            4.0,
            &[],
            target,
        );

        assert!((time - 0.25).abs() < 0.08, "unexpected time: {time}");
    }

    #[test]
    fn derives_timeline_time_clamps_zero_duration() {
        let time = derive_timeline_time_for_world_target(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[],
            0.0,
            &[],
            [100.0, 100.0, 0.0],
        );

        assert_eq!(time, 0.0);
    }

    #[test]
    fn derives_timeline_time_prefers_last_tap_segment_when_target_is_near_it() {
        let taps = [0.4, 0.8, 1.2];
        let target = [10.0, 0.5, 0.0];
        let time = derive_timeline_time_for_world_target(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &taps,
            4.0,
            &[],
            target,
        );

        assert!(
            time >= 1.2,
            "expected time in/after last-tap segment, got {time}"
        );
    }

    #[test]
    fn derives_timeline_time_falls_back_to_earlier_segment_when_needed() {
        let taps = [1.0, 2.0, 3.0];
        let target = [0.5, 1.5, 0.0];
        let time = derive_timeline_time_for_world_target(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &taps,
            6.0,
            &[],
            target,
        );

        assert!(time < 1.0, "expected fallback to early segment, got {time}");
    }

    #[test]
    fn derives_timeline_time_near_seed_matches_expected_local_target() {
        let target = [0.5, 4.5, 0.0];
        let time = derive_timeline_time_for_world_target_near_time(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[],
            8.0,
            &[],
            target,
            TimelineNearSearch {
                seed_time: 0.55,
                window_seconds: 1.0,
            },
        );

        assert!(
            (time - 0.5).abs() < 0.08,
            "unexpected near-search time: {time}"
        );
    }

    #[test]
    fn near_solver_stays_within_requested_window() {
        let seed = 2.2;
        let window = 0.35;
        let target = [0.5, 8.5, 0.0];
        let time = derive_timeline_time_for_world_target_near_time(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[],
            8.0,
            &[],
            target,
            TimelineNearSearch {
                seed_time: seed,
                window_seconds: window,
            },
        );

        assert!(time >= seed - window && time <= seed + window);
    }

    #[test]
    fn near_solver_tracks_turned_segment_target() {
        let target = [2.5, 0.5, 0.0];
        let time = derive_timeline_time_for_world_target_near_time(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &[0.0],
            4.0,
            &[],
            target,
            TimelineNearSearch {
                seed_time: 0.35,
                window_seconds: 0.7,
            },
        );

        assert!(
            (time - 0.25).abs() < 0.12,
            "unexpected near-search time: {time}"
        );
    }

    #[test]
    fn derives_tap_indicator_positions_with_single_simulation_path() {
        let taps = [0.4, 0.1, 0.4, 0.7];
        let positions = derive_tap_indicator_positions(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &taps,
            &[],
        );

        assert!(!positions.is_empty());
        let mut unique_check = positions.clone();
        unique_check.sort_by(|a, b| {
            a[0].total_cmp(&b[0])
                .then(a[1].total_cmp(&b[1]))
                .then(a[2].total_cmp(&b[2]))
        });
        unique_check.dedup_by(|a, b| {
            (a[0] - b[0]).abs() < 0.001
                && (a[1] - b[1]).abs() < 0.001
                && (a[2] - b[2]).abs() < 0.001
        });
        assert_eq!(positions.len(), unique_check.len());
    }

    #[test]
    fn tap_indicator_positions_match_exact_timeline_per_tap() {
        let taps = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625];
        let spawn = [0.0, 0.0, 0.0];
        let direction = crate::types::SpawnDirection::Forward;

        let derived = derive_tap_indicator_positions(spawn, direction, &taps, &[]);

        let mut expected = Vec::new();
        for tap in taps {
            let (position, _) = derive_timeline_position(spawn, direction, &taps, tap, &[]);
            expected.push([
                (position[0] - 0.5).round(),
                (position[1] - 0.5).round(),
                position[2].round(),
            ]);
        }
        expected.sort_by(|a, b| {
            a[0].total_cmp(&b[0])
                .then(a[1].total_cmp(&b[1]))
                .then(a[2].total_cmp(&b[2]))
        });
        expected.dedup_by(|a, b| {
            (a[0] - b[0]).abs() < 0.001
                && (a[1] - b[1]).abs() < 0.001
                && (a[2] - b[2]).abs() < 0.001
        });

        assert_eq!(derived, expected);
    }
}
