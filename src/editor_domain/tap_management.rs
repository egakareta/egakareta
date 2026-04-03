/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::SpawnDirection;

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

pub(crate) fn clear_taps_with_indicators(
    tap_times: &mut Vec<f32>,
    tap_indicator_positions: &mut Vec<[f32; 3]>,
) {
    tap_times.clear();
    tap_indicator_positions.clear();
}

pub(crate) fn toggle_spawn_direction(direction: SpawnDirection) -> SpawnDirection {
    match direction {
        SpawnDirection::Forward => SpawnDirection::Right,
        SpawnDirection::Right => SpawnDirection::Forward,
    }
}
