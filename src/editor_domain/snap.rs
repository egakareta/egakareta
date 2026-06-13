/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) fn snap_component_to_step(component: f32, step: f32) -> f32 {
    (component / step).round() * step
}

pub(crate) fn snap_cell_to_step(position: [f32; 3], step: f32) -> [f32; 3] {
    [
        snap_component_to_step(position[0], step),
        snap_component_to_step(position[1].max(0.0), step),
        snap_component_to_step(position[2], step),
    ]
}
