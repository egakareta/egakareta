/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
pub(crate) struct EditorConfigState {
    pub(crate) selected_block_id: String,
    pub(crate) snap_to_grid: bool,
    pub(crate) snap_step: f32,
    pub(crate) snap_rotation: bool,
    pub(crate) snap_rotation_step_degrees: f32,
}
