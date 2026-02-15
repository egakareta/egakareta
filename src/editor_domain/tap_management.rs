use crate::types::SpawnDirection;

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
