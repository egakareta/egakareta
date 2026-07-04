/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy_ecs::prelude::*;

use crate::state::{FrameTimeRes, PerfRes, State};

pub fn advance_frame_time_system(mut frame_time: ResMut<FrameTimeRes>, mut perf: ResMut<PerfRes>) {
    puffin::GlobalProfiler::lock().new_frame();
    frame_time.advance();
    perf.0.fps_smoothed = frame_time.fps_smoothed;
}

impl State {
    pub fn frame_time_resource(&self) -> FrameTimeRes {
        FrameTimeRes {
            last_frame: self.frame_runtime.editor.last_frame,
            delta_seconds: self.frame_runtime.editor.delta_seconds,
            accumulator: self.frame_runtime.editor.accumulator,
            global_time_seconds: self.frame_runtime.global_time_seconds,
            fps_smoothed: self.editor.perf.fps_smoothed,
        }
    }

    pub fn apply_frame_time_resource(&mut self, frame_time: &FrameTimeRes) {
        self.frame_runtime.editor.last_frame = frame_time.last_frame;
        self.frame_runtime.editor.delta_seconds = frame_time.delta_seconds;
        self.frame_runtime.editor.accumulator = frame_time.accumulator;
        self.frame_runtime.editor.scheduled_delta_available = true;
        self.frame_runtime.global_time_seconds = frame_time.global_time_seconds;
        self.editor.perf.fps_smoothed = frame_time.fps_smoothed;
    }

    pub(crate) fn frame_delta_for_legacy_update(&mut self) -> f32 {
        if self.frame_runtime.editor.scheduled_delta_available {
            self.frame_runtime.editor.scheduled_delta_available = false;
            return self.frame_runtime.editor.delta_seconds;
        }

        let mut frame_time = self.frame_time_resource();
        puffin::GlobalProfiler::lock().new_frame();
        frame_time.advance();
        self.apply_frame_time_resource(&frame_time);
        self.frame_runtime.editor.scheduled_delta_available = false;
        frame_time.delta_seconds
    }
}
