/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{EditorDirtyFlags, State};
use eg_types::AppPhase;

impl State {
    fn handle_editor_escape(&mut self) {
        if self.phase == AppPhase::Playing && !self.session.playtesting_editor {
            self.toggle_game_pause();
            return;
        }

        if !self.is_editor() {
            self.back_to_menu();
            return;
        }

        if self.clear_editor_selection_for_escape() {
            return;
        }

        if self.editor.timeline.playback.playing {
            self.toggle_editor_timeline_playback();
        } else if self.editor.timeline.clock.time_seconds > 0.001 {
            self.set_editor_timeline_time_seconds(0.0);
        } else {
            self.back_to_menu();
        }
    }

    fn clear_editor_selection_for_escape(&mut self) -> bool {
        let had_block_selection = self.has_block_selection();
        let had_tap_selection = self.editor.selected_tap().is_some();

        if had_block_selection {
            self.editor.clear_block_selection();
            self.editor
                .mark_dirty(EditorDirtyFlags::selection_overlay_changed());
        }

        if had_tap_selection {
            self.clear_editor_tap_selection();
        }

        had_block_selection || had_tap_selection
    }

    pub(super) fn clear_editor_tap_selection(&mut self) -> bool {
        let had_tap_selection = self.editor.selected_tap().is_some();
        if had_tap_selection {
            self.editor.set_selected_tap_index(None);
            self.rebuild_tap_indicator_vertices();
        }
        had_tap_selection
    }

    pub fn apply_editor_escape(&mut self) {
        self.handle_editor_escape();
    }
}
