/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
const RECENT_BLOCK_LIMIT: usize = 10;

pub(crate) struct EditorConfigState {
    pub(crate) selected_block_id: String,
    pub(crate) recent_block_ids: Vec<String>,
    pub(crate) snap_to_grid: bool,
    pub(crate) snap_step: f32,
    pub(crate) snap_rotation: bool,
    pub(crate) snap_rotation_step_degrees: f32,
}

impl EditorConfigState {
    pub(crate) fn remember_recent_block_id(&mut self, block_id: String) {
        let normalized = crate::block_repository::normalize_block_id(&block_id);
        self.recent_block_ids.retain(|id| id != &normalized);
        self.recent_block_ids.insert(0, normalized);
        self.recent_block_ids.truncate(RECENT_BLOCK_LIMIT);
    }
}
