/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::{PracticeCheckpoint, State};

impl State {
    pub(super) fn toggle_game_pause(&mut self) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        if self.session.game_paused {
            self.resume_game();
        } else {
            self.pause_game();
        }
    }

    pub(super) fn pause_game(&mut self) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = true;
        self.clear_pending_gameplay_inputs();
        self.pause_game_audio();
    }

    pub(super) fn resume_game(&mut self) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        if self.session.game_paused {
            self.session.game_paused = false;
            if !self.start_gameplay_if_needed() {
                self.resume_game_audio();
            }
        }
    }

    pub fn practice_checkpoints(&self) -> Vec<PracticeCheckpoint> {
        self.session.practice_checkpoints.clone()
    }
}
