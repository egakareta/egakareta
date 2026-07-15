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
            global_time_seconds: self.frame_runtime.global_time_seconds,
            fps_smoothed: 0.0,
        }
    }

    #[cfg(test)]
    pub fn apply_frame_time_resource(&mut self, frame_time: &FrameTimeRes) {
        self.frame_runtime.editor.last_frame = frame_time.last_frame;
        self.frame_runtime.editor.delta_seconds = frame_time.delta_seconds;
        self.frame_runtime.global_time_seconds = frame_time.global_time_seconds;
    }
}
