/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::TimedTrigger;
use eg_types::{
    default_sky_color, LevelCreatorMetadata, LevelMetadata, LevelObject,
    LevelPreviewCameraMetadata, MusicMetadata, SpawnMetadata, TimingPoint,
};

pub struct EditorSessionInit {
    pub objects: Vec<LevelObject>,
    pub spawn: SpawnMetadata,
    pub music: MusicMetadata,
    pub creator_metadata: LevelCreatorMetadata,
    pub sky_color: [f32; 3],
    pub tap_times: Vec<f32>,
    pub timing_points: Vec<TimingPoint>,
    pub timeline_time_seconds: f32,
    pub timeline_duration_seconds: f32,
    pub triggers: Vec<TimedTrigger>,
    pub menu_preview_camera: Option<LevelPreviewCameraMetadata>,
    pub cursor: [f32; 3],
    pub camera_pan: [f32; 2],
}

pub fn editor_session_init_from_metadata(metadata: Option<LevelMetadata>) -> EditorSessionInit {
    let (
        objects,
        spawn,
        music,
        creator_metadata,
        sky_color,
        mut tap_times,
        timing_points,
        mut timeline_time_seconds,
        timeline_duration_seconds,
        mut triggers,
        menu_preview_camera,
    ) = if let Some(metadata) = metadata {
        let triggers = metadata.resolved_triggers();
        let creator_metadata = metadata.creator_metadata();
        (
            metadata.objects,
            metadata.spawn,
            metadata.music,
            creator_metadata,
            metadata.sky_color,
            metadata.tap_times,
            metadata.timing_points,
            metadata.timeline_time_seconds,
            metadata.timeline_duration_seconds,
            triggers,
            metadata.menu_preview_camera,
        )
    } else {
        (
            Vec::new(),
            SpawnMetadata::default(),
            MusicMetadata::default(),
            LevelCreatorMetadata::default(),
            default_sky_color(),
            Vec::new(),
            Vec::new(),
            0.0,
            16.0,
            Vec::new(),
            None,
        )
    };

    timeline_time_seconds = timeline_time_seconds.clamp(0.0, timeline_duration_seconds.max(0.1));

    tap_times.retain(|tap| tap.is_finite() && *tap >= 0.0);
    tap_times.sort_by(f32::total_cmp);
    triggers.retain(|trigger| trigger.time_seconds.is_finite());
    triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));
    let cursor = cursor_from_objects(&objects);
    let camera_pan = camera_pan_from_cursor(cursor);

    EditorSessionInit {
        objects,
        spawn,
        music,
        creator_metadata,
        sky_color,
        tap_times,
        timing_points,
        timeline_time_seconds,
        timeline_duration_seconds: timeline_duration_seconds.max(0.1),
        triggers,
        menu_preview_camera,
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
    [cursor[0] + 0.5, cursor[2] + 0.5]
}
