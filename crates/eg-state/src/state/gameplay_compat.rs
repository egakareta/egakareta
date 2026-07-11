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

    pub(super) fn restart_game_from_pause(&mut self) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = false;
        self.restart_level();
    }

    /// Applies the practice-mode toggle side effects while practice ownership
    /// is still synchronized back into legacy gameplay/session state.
    pub(super) fn set_practice_mode_enabled_internal(&mut self, enabled: bool) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.mark_app_gameplay_world_for_legacy_seed();
        self.session.practice_mode_enabled = enabled;
        self.session.practice_checkpoints.clear();

        if enabled {
            self.resume_game();
            return;
        }

        self.rebuild_practice_checkpoint_vertices();
        self.restart_level();
        self.resume_game();
    }

    pub(super) fn quit_game_from_pause(&mut self) {
        if self.phase != eg_types::AppPhase::Playing || self.session.playtesting_editor {
            return;
        }

        self.session.game_paused = false;
        self.back_to_menu();
    }

    pub fn apply_game_quit_to_menu(&mut self) {
        self.quit_game_from_pause();
    }

    pub fn apply_game_restart_level(&mut self) {
        self.restart_game_from_pause();
    }

    pub fn practice_checkpoints(&self) -> Vec<PracticeCheckpoint> {
        self.session.practice_checkpoints.clone()
    }

    /// Temporary compatibility setter for the App-world checkpoint list.
    /// The mesh rebuild remains here until presentation consumes
    /// `SessionStateRes.practice_checkpoints` directly.
    pub fn set_practice_checkpoints_from_app(&mut self, checkpoints: Vec<PracticeCheckpoint>) {
        self.session.practice_checkpoints = checkpoints;
        self.rebuild_practice_checkpoint_vertices();
    }
}
