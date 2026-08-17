/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy::{
    ecs::prelude::*,
    time::{Time, Virtual},
};

use crate::state::{FrameTimeRes, PerfRes};

/// Begin Puffin's frame before any application work runs.
///
/// Keeping this in Bevy's [`First`] schedule makes captures align with the
/// engine frame rather than with the former compatibility schedule.
pub fn begin_puffin_frame_system() {
    puffin::GlobalProfiler::lock().new_frame();
}

pub fn advance_frame_time_system(
    mut frame_time: ResMut<FrameTimeRes>,
    mut perf: ResMut<PerfRes>,
    virtual_time: Option<Res<Time<Virtual>>>,
) {
    if let Some(virtual_time) = virtual_time.filter(|time| time.delta_secs() > 0.0) {
        frame_time.advance_from_bevy_time(virtual_time.delta_secs(), virtual_time.elapsed_secs());
    } else {
        frame_time.advance();
    }
    perf.0.fps_smoothed = frame_time.fps_smoothed;
}
