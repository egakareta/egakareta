use crate::game::BASE_PLAYER_SPEED;
use crate::types::{LevelMetadata, LevelObject, MusicMetadata, SpawnMetadata, TimingPoint};

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
