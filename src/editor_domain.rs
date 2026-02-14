use crate::game::{simulate_timeline_state, TimelineSimulationRuntime, BASE_PLAYER_SPEED};
use crate::types::{
    LevelMetadata, LevelObject, MusicMetadata, SpawnDirection, SpawnMetadata, TimingPoint,
};

pub(crate) struct EditorPlaytestTransition {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn_position: [f32; 3],
    pub(crate) spawn_direction: SpawnDirection,
    pub(crate) playing_level_name: Option<String>,
    pub(crate) camera_rotation: f32,
    pub(crate) camera_pitch: f32,
}

pub(crate) struct PlayingLevelTransition {
    pub(crate) level_name: String,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn_position: [f32; 3],
    pub(crate) spawn_direction: SpawnDirection,
}

pub(crate) fn build_playing_transition_from_metadata(
    metadata: LevelMetadata,
) -> PlayingLevelTransition {
    PlayingLevelTransition {
        level_name: metadata.name,
        objects: metadata.objects,
        spawn_position: metadata.spawn.position,
        spawn_direction: metadata.spawn.direction,
    }
}

pub(crate) fn build_editor_playtest_transition(
    editor_objects: &[LevelObject],
    editor_level_name: Option<&str>,
    editor_spawn: SpawnMetadata,
    tap_times: &[f32],
    timeline_time_seconds: f32,
) -> EditorPlaytestTransition {
    let (spawn_position, spawn_direction) = derive_timeline_position(
        editor_spawn.position,
        editor_spawn.direction,
        tap_times,
        timeline_time_seconds,
        editor_objects,
    );

    EditorPlaytestTransition {
        objects: editor_objects.to_vec(),
        spawn_position,
        spawn_direction,
        playing_level_name: editor_level_name.map(|name| name.to_string()),
        camera_rotation: -45.0f32.to_radians(),
        camera_pitch: 45.0f32.to_radians(),
    }
}

pub(crate) fn playtest_return_objects(
    playtesting_editor: bool,
    editor_objects: &[LevelObject],
) -> Option<Vec<LevelObject>> {
    if playtesting_editor {
        Some(editor_objects.to_vec())
    } else {
        None
    }
}

pub(crate) struct EditorSessionInit {
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) spawn: SpawnMetadata,
    pub(crate) music: MusicMetadata,
    pub(crate) tap_times: Vec<f32>,
    pub(crate) timing_points: Vec<TimingPoint>,
    pub(crate) timeline_time_seconds: f32,
    pub(crate) timeline_duration_seconds: f32,
    pub(crate) cursor: [f32; 3],
    pub(crate) camera_pan: [f32; 2],
}

pub(crate) fn editor_session_init_from_metadata(
    metadata: Option<LevelMetadata>,
) -> EditorSessionInit {
    let (
        objects,
        spawn,
        music,
        mut tap_times,
        timing_points,
        mut timeline_time_seconds,
        timeline_duration_seconds,
        legacy_taps,
        legacy_timeline_step,
    ) = if let Some(metadata) = metadata {
        (
            metadata.objects,
            metadata.spawn,
            metadata.music,
            metadata.tap_times,
            metadata.timing_points,
            metadata.timeline_time_seconds,
            metadata.timeline_duration_seconds,
            metadata.legacy_taps,
            metadata.legacy_timeline_step,
        )
    } else {
        (
            Vec::new(),
            SpawnMetadata::default(),
            MusicMetadata::default(),
            Vec::new(),
            Vec::new(),
            0.0,
            16.0,
            Vec::new(),
            0,
        )
    };

    if tap_times.is_empty() && !legacy_taps.is_empty() {
        let seconds_per_step = 1.0 / BASE_PLAYER_SPEED.max(0.1);
        tap_times = legacy_taps
            .iter()
            .copied()
            .map(|step| step as f32 * seconds_per_step)
            .collect();
    }

    if timeline_time_seconds <= 0.0 && legacy_timeline_step > 0 {
        let seconds_per_step = 1.0 / BASE_PLAYER_SPEED.max(0.1);
        timeline_time_seconds = legacy_timeline_step as f32 * seconds_per_step;
    }

    timeline_time_seconds = timeline_time_seconds.clamp(0.0, timeline_duration_seconds.max(0.1));

    tap_times.retain(|tap| tap.is_finite() && *tap >= 0.0);
    tap_times.sort_by(f32::total_cmp);
    let cursor = cursor_from_objects(&objects);
    let camera_pan = camera_pan_from_cursor(cursor);

    EditorSessionInit {
        objects,
        spawn,
        music,
        tap_times,
        timing_points,
        timeline_time_seconds,
        timeline_duration_seconds: timeline_duration_seconds.max(0.1),
        cursor,
        camera_pan,
    }
}

fn cursor_from_objects(objects: &[LevelObject]) -> [f32; 3] {
    if let Some(first) = objects.first() {
        [
            first.position[0].round(),
            first.position[1].round(),
            first.position[2].round(),
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn camera_pan_from_cursor(cursor: [f32; 3]) -> [f32; 2] {
    [cursor[0] + 0.5, cursor[1] + 0.5]
}

#[allow(dead_code)]
pub(crate) fn move_cursor_xy(cursor: &mut [f32; 3], dx: i32, dy: i32, bounds: i32) {
    cursor[0] = (cursor[0] + dx as f32).clamp(-bounds as f32, bounds as f32);
    cursor[1] = (cursor[1] + dy as f32).clamp(-bounds as f32, bounds as f32);
}

pub(crate) fn create_block_at_cursor(cursor: [f32; 3], block_id: &str) -> LevelObject {
    LevelObject {
        position: cursor,
        size: [1.0, 1.0, 1.0],
        rotation_degrees: 0.0,
        roundness: 0.18,
        block_id: block_id.to_string(),
    }
}

pub(crate) fn remove_topmost_block_at_cursor(
    objects: &mut Vec<LevelObject>,
    cursor: [f32; 3],
) -> bool {
    let mut top_index: Option<usize> = None;
    let mut top_height = f32::NEG_INFINITY;

    for (index, obj) in objects.iter().enumerate() {
        let occupies_x =
            cursor[0] + 0.5 >= obj.position[0] && cursor[0] + 0.5 <= obj.position[0] + obj.size[0];
        let occupies_y =
            cursor[1] + 0.5 >= obj.position[1] && cursor[1] + 0.5 <= obj.position[1] + obj.size[1];
        if occupies_x && occupies_y {
            let top = obj.position[2] + obj.size[2];
            if top > top_height {
                top_height = top;
                top_index = Some(index);
            }
        }
    }

    if let Some(index) = top_index {
        objects.remove(index);
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub(crate) fn add_tap_time(tap_times: &mut Vec<f32>, time_seconds: f32) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    let clamped_time = time_seconds.max(0.0);
    if !tap_times
        .iter()
        .any(|existing| (existing - clamped_time).abs() <= TAP_EPSILON_SECONDS)
    {
        tap_times.push(clamped_time);
        tap_times.sort_by(f32::total_cmp);
    }
}

pub(crate) fn add_tap_with_indicator(
    tap_times: &mut Vec<f32>,
    tap_indicator_positions: &mut Vec<[f32; 3]>,
    time_seconds: f32,
    indicator_position: [f32; 3],
) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    let clamped_time = time_seconds.max(0.0);
    if tap_times
        .iter()
        .any(|existing| (existing - clamped_time).abs() <= TAP_EPSILON_SECONDS)
    {
        return;
    }

    let insert_index = tap_times.partition_point(|existing| *existing < clamped_time);
    tap_times.insert(insert_index, clamped_time);
    tap_indicator_positions.insert(insert_index, indicator_position);
}

#[allow(dead_code)]
pub(crate) fn remove_tap_time(tap_times: &mut Vec<f32>, time_seconds: f32) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    tap_times.retain(|tap| (tap - time_seconds).abs() > TAP_EPSILON_SECONDS);
}

pub(crate) fn remove_tap_with_indicator(
    tap_times: &mut Vec<f32>,
    tap_indicator_positions: &mut Vec<[f32; 3]>,
    time_seconds: f32,
) {
    const TAP_EPSILON_SECONDS: f32 = 0.01;
    if let Some(index) = tap_times
        .iter()
        .position(|tap| (*tap - time_seconds).abs() <= TAP_EPSILON_SECONDS)
    {
        tap_times.remove(index);
        if index < tap_indicator_positions.len() {
            tap_indicator_positions.remove(index);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn clear_tap_times(tap_times: &mut Vec<f32>) {
    tap_times.clear();
}

pub(crate) fn clear_taps_with_indicators(
    tap_times: &mut Vec<f32>,
    tap_indicator_positions: &mut Vec<[f32; 3]>,
) {
    tap_times.clear();
    tap_indicator_positions.clear();
}

pub(crate) fn retain_taps_up_to_duration_with_indicators(
    tap_times: &mut Vec<f32>,
    tap_indicator_positions: &mut Vec<[f32; 3]>,
    duration_seconds: f32,
) {
    let mut retained_times = Vec::with_capacity(tap_times.len());
    let mut retained_positions = Vec::with_capacity(tap_indicator_positions.len());
    for (index, tap) in tap_times.iter().copied().enumerate() {
        if tap <= duration_seconds {
            retained_times.push(tap);
            if let Some(position) = tap_indicator_positions.get(index).copied() {
                retained_positions.push(position);
            }
        }
    }

    *tap_times = retained_times;
    *tap_indicator_positions = retained_positions;
}

pub(crate) fn toggle_spawn_direction(direction: SpawnDirection) -> SpawnDirection {
    match direction {
        SpawnDirection::Forward => SpawnDirection::Right,
        SpawnDirection::Right => SpawnDirection::Forward,
    }
}

pub(crate) fn derive_timeline_position(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> ([f32; 3], SpawnDirection) {
    let state = derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects);
    (state.position, state.direction)
}

pub(crate) fn derive_timeline_elapsed_seconds(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> f32 {
    derive_timeline_state(spawn, direction, tap_times, timeline_time_seconds, objects)
        .elapsed_seconds
}

pub(crate) fn derive_tap_indicator_positions(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    objects: &[LevelObject],
) -> Vec<[f32; 3]> {
    let mut sorted_taps: Vec<f32> = tap_times
        .iter()
        .copied()
        .filter(|tap| tap.is_finite() && *tap >= 0.0)
        .collect();
    sorted_taps.sort_by(f32::total_cmp);

    let mut runtime = TimelineSimulationRuntime::new(spawn, direction, objects, &sorted_taps);
    let mut positions = Vec::with_capacity(sorted_taps.len());
    for tap_time in sorted_taps {
        runtime.advance_to(tap_time);
        let snapshot = runtime.snapshot();
        positions.push([
            (snapshot.position[0] - 0.5).round(),
            (snapshot.position[1] - 0.5).round(),
            snapshot.position[2].round(),
        ]);
    }

    positions.sort_unstable_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then(a[1].total_cmp(&b[1]))
            .then(a[2].total_cmp(&b[2]))
    });
    positions.dedup_by(|a, b| {
        (a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001 && (a[2] - b[2]).abs() < 0.001
    });
    positions
}

#[allow(dead_code)]
pub(crate) fn derive_timeline_time_for_world_target(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    duration_seconds: f32,
    objects: &[LevelObject],
    target: [f32; 3],
) -> f32 {
    let duration = duration_seconds.max(0.0);
    if duration <= 0.0 {
        return 0.0;
    }

    let last_tap_time = tap_times
        .iter()
        .copied()
        .filter(|tap| tap.is_finite() && *tap >= 0.0)
        .fold(0.0_f32, f32::max)
        .min(duration);

    fn distance_sq(position: [f32; 3], target: [f32; 3]) -> f32 {
        let dx = position[0] - target[0];
        let dy = position[1] - target[1];
        let dz = position[2] - target[2];
        dx * dx + dy * dy + dz * dz
    }

    let sample_best_time = |start_time: f32, end_time: f32, samples: usize| -> (f32, f32) {
        let mut runtime = TimelineSimulationRuntime::new(spawn, direction, objects, tap_times);
        runtime.advance_to(start_time);

        let mut best_time = start_time;
        let mut best_distance_sq = f32::INFINITY;
        let step = if samples == 0 {
            0.0
        } else {
            (end_time - start_time) / samples as f32
        };

        for index in 0..=samples {
            let t = (start_time + step * index as f32).clamp(start_time, end_time);
            runtime.advance_to(t);
            let snapshot = runtime.snapshot();
            let current_distance_sq = distance_sq(snapshot.position, target);
            if current_distance_sq < best_distance_sq {
                best_distance_sq = current_distance_sq;
                best_time = t;
                if best_distance_sq <= 1e-6 {
                    break;
                }
            }
        }

        (best_time, best_distance_sq)
    };

    let solve_in_range = |range_start: f32, range_end: f32| -> (f32, f32) {
        let coarse_samples =
            (((range_end - range_start).max(0.0) * 30.0).clamp(120.0, 900.0)) as usize;
        let (mut refined_time, mut best_distance_sq) =
            sample_best_time(range_start, range_end, coarse_samples);

        for (window_seconds, refinement_samples) in [(1.2_f32, 180_usize), (0.25_f32, 120_usize)] {
            if best_distance_sq <= 1e-6 {
                break;
            }

            let window = window_seconds.min(duration.max(0.01));
            let start_time = (refined_time - window).max(range_start);
            let end_time = (refined_time + window).min(range_end);

            let (local_best_time, local_best_distance_sq) =
                sample_best_time(start_time, end_time, refinement_samples);
            refined_time = local_best_time;
            best_distance_sq = local_best_distance_sq;
        }

        (refined_time.clamp(range_start, range_end), best_distance_sq)
    };

    let (local_time, local_distance_sq) = solve_in_range(last_tap_time, duration);
    const ACCEPT_LOCAL_DISTANCE_SQ: f32 = 1.5 * 1.5;
    if local_distance_sq <= ACCEPT_LOCAL_DISTANCE_SQ || last_tap_time <= 1e-6 {
        return local_time;
    }

    let (full_time, _) = solve_in_range(0.0, duration);
    full_time
}

#[allow(dead_code)]
pub(crate) fn derive_timeline_time_for_world_target_near_time(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    duration_seconds: f32,
    objects: &[LevelObject],
    target: [f32; 3],
    search: TimelineNearSearch,
) -> f32 {
    let duration = duration_seconds.max(0.0);
    if duration <= 0.0 {
        return 0.0;
    }

    let range_start = (search.seed_time - search.window_seconds.max(0.01)).clamp(0.0, duration);
    let range_end = (search.seed_time + search.window_seconds.max(0.01)).clamp(0.0, duration);
    if range_end <= range_start {
        return range_start;
    }

    fn distance_sq(position: [f32; 3], target: [f32; 3]) -> f32 {
        let dx = position[0] - target[0];
        let dy = position[1] - target[1];
        let dz = position[2] - target[2];
        dx * dx + dy * dy + dz * dz
    }

    let sample_best_time = |start_time: f32, end_time: f32, samples: usize| -> (f32, f32) {
        let mut runtime = TimelineSimulationRuntime::new(spawn, direction, objects, tap_times);
        runtime.advance_to(start_time);

        let mut best_time = start_time;
        let mut best_distance_sq = f32::INFINITY;
        let step = if samples == 0 {
            0.0
        } else {
            (end_time - start_time) / samples as f32
        };

        for index in 0..=samples {
            let t = (start_time + step * index as f32).clamp(start_time, end_time);
            runtime.advance_to(t);
            let snapshot = runtime.snapshot();
            let current_distance_sq = distance_sq(snapshot.position, target);
            if current_distance_sq < best_distance_sq {
                best_distance_sq = current_distance_sq;
                best_time = t;
                if best_distance_sq <= 1e-6 {
                    break;
                }
            }
        }

        (best_time, best_distance_sq)
    };

    let coarse_samples = (((range_end - range_start) * 90.0).clamp(90.0, 360.0)) as usize;
    let (mut refined_time, mut best_distance_sq) =
        sample_best_time(range_start, range_end, coarse_samples);

    for (window_seconds, refinement_samples) in [(0.35_f32, 120_usize), (0.12_f32, 80_usize)] {
        if best_distance_sq <= 1e-6 {
            break;
        }

        let start_time = (refined_time - window_seconds).max(range_start);
        let end_time = (refined_time + window_seconds).min(range_end);
        let (local_best_time, local_best_distance_sq) =
            sample_best_time(start_time, end_time, refinement_samples);
        refined_time = local_best_time;
        best_distance_sq = local_best_distance_sq;
    }

    refined_time.clamp(range_start, range_end)
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct TimelineNearSearch {
    pub(crate) seed_time: f32,
    pub(crate) window_seconds: f32,
}

pub(crate) struct TimelineState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
    pub(crate) elapsed_seconds: f32,
}

pub(crate) fn derive_timeline_state(
    spawn: [f32; 3],
    direction: SpawnDirection,
    tap_times: &[f32],
    timeline_time_seconds: f32,
    objects: &[LevelObject],
) -> TimelineState {
    let simulated =
        simulate_timeline_state(spawn, direction, objects, tap_times, timeline_time_seconds);

    TimelineState {
        position: simulated.position,
        direction: simulated.direction,
        elapsed_seconds: simulated.elapsed_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        add_tap_time, add_tap_with_indicator, build_editor_playtest_transition,
        build_playing_transition_from_metadata, clear_tap_times, clear_taps_with_indicators,
        create_block_at_cursor, derive_tap_indicator_positions, derive_timeline_position,
        derive_timeline_time_for_world_target, derive_timeline_time_for_world_target_near_time,
        editor_session_init_from_metadata, move_cursor_xy, playtest_return_objects,
        remove_tap_time, remove_tap_with_indicator, remove_topmost_block_at_cursor,
        retain_taps_up_to_duration_with_indicators, TimelineNearSearch,
    };
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

        retain_taps_up_to_duration_with_indicators(&mut taps, &mut indicators, 0.15);
        assert_eq!(taps, vec![0.1]);
        assert_eq!(indicators, vec![[1.0, 0.0, 0.0]]);

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
            },
            LevelObject {
                position: [0.0, 0.0, 1.0],
                size: [1.0, 1.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/grass".to_string(),
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

        let transition = build_editor_playtest_transition(
            &objects,
            Some("Demo"),
            SpawnMetadata::default(),
            &[],
            1.0 / crate::game::BASE_PLAYER_SPEED,
        );

        assert!(transition.objects.is_empty());
        assert!((transition.spawn_position[1] - 1.5).abs() < 0.1);
        assert!(matches!(
            transition.spawn_direction,
            crate::types::SpawnDirection::Forward
        ));
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "benchmark"]
    fn benchmark_toggle_tap_cell_average_budget() {
        let mut taps = Vec::new();
        let mut indicators = Vec::new();

        let iterations = 4000usize;
        let started_at = std::time::Instant::now();
        for i in 0..iterations {
            let tap_time = i as f32 * 0.01;
            let cell = [i as f32, 0.0, 0.0];
            add_tap_with_indicator(&mut taps, &mut indicators, tap_time, cell);
            remove_tap_with_indicator(&mut taps, &mut indicators, tap_time);
        }
        let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let average_ms = elapsed_ms / iterations as f64;

        assert!(
            average_ms < 0.10,
            "tap toggle average too slow: {average_ms:.4}ms/op (total {elapsed_ms:.2}ms)"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "benchmark"]
    fn benchmark_tap_indicator_rebuild_dense_taps_budget() {
        let tap_count = 600usize;
        let taps: Vec<f32> = (0..tap_count).map(|i| i as f32 * 0.03).collect();

        let started_at = std::time::Instant::now();
        let positions = derive_tap_indicator_positions(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &taps,
            &[],
        );
        let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;

        assert!(!positions.is_empty());
        assert!(
            elapsed_ms < 180.0,
            "tap indicator rebuild too slow: {elapsed_ms:.2}ms for {tap_count} taps"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "benchmark"]
    fn benchmark_near_time_solver_dense_taps_budget() {
        let taps: Vec<f32> = (0..240).map(|i| i as f32 * 0.05).collect();

        let started_at = std::time::Instant::now();
        let solved_time = derive_timeline_time_for_world_target_near_time(
            [0.0, 0.0, 0.0],
            crate::types::SpawnDirection::Forward,
            &taps,
            30.0,
            &[],
            [4.5, 10.5, 0.0],
            TimelineNearSearch {
                seed_time: 10.0,
                window_seconds: 1.25,
            },
        );
        let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;

        assert!(solved_time.is_finite());
        assert!(
            elapsed_ms < 120.0,
            "near-time solver too slow: {elapsed_ms:.2}ms (result {solved_time:.3}s)"
        );
    }
}
