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

impl super::State {
    pub(crate) fn toggle_perf_overlay(&mut self) {
        self.editor.perf.overlay_enabled = !self.editor.perf.overlay_enabled;
        puffin::set_scopes_on(self.editor.perf.overlay_enabled);
    }

    pub(crate) fn perf_overlay_enabled(&self) -> bool {
        self.editor.perf.overlay_enabled
    }

    pub(crate) fn perf_fps(&self) -> f32 {
        self.editor.perf.fps_smoothed
    }

    pub(crate) fn perf_graphics_backend(&self) -> String {
        format!("{:?}", self.render.gpu.adapter_info.backend)
    }

    pub(crate) fn perf_audio_backend(&self) -> String {
        self.audio.state.runtime.backend_name()
    }
}
