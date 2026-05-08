/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) struct EditorPerfState {
    pub(crate) overlay_enabled: bool,
    pub(crate) fps_smoothed: f32,
}

impl EditorPerfState {
    pub(crate) fn new() -> Self {
        Self {
            overlay_enabled: false,
            fps_smoothed: 0.0,
        }
    }
}
