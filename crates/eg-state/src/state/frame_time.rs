/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy_ecs::prelude::*;

use crate::state::{FrameTimeRes, PerfRes};

pub fn advance_frame_time_system(mut frame_time: ResMut<FrameTimeRes>, mut perf: ResMut<PerfRes>) {
    puffin::GlobalProfiler::lock().new_frame();
    frame_time.advance();
    perf.0.fps_smoothed = frame_time.fps_smoothed;
}
