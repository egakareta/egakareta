/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::BlockId;

const RECENT_BLOCK_LIMIT: usize = 10;

#[derive(Default)]
pub struct EditorConfigState {
    pub selected_block_id: BlockId,
    pub recent_block_ids: Vec<BlockId>,
    pub snap_to_grid: bool,
    pub snap_step: f32,
    pub snap_rotation: bool,
    pub snap_rotation_step_degrees: f32,
    pub selected_block_rotation_degrees: [f32; 3],
}

impl EditorConfigState {
    pub fn remember_recent_block_id(&mut self, block_id: BlockId) {
        self.recent_block_ids.retain(|id| id != &block_id);
        self.recent_block_ids.insert(0, block_id);
        self.recent_block_ids.truncate(RECENT_BLOCK_LIMIT);
    }
}
