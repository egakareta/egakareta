/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::types::Vertex;

pub(crate) fn build_grid_vertices() -> Vec<Vertex> {
    let color = [1.0, 1.0, 1.0, 1.0];
    vec![
        Vertex::untextured([-1.0, 0.0, -1.0], color),
        Vertex::untextured([1.0, 0.0, -1.0], color),
        Vertex::untextured([1.0, 0.0, 1.0], color),
        Vertex::untextured([-1.0, 0.0, -1.0], color),
        Vertex::untextured([1.0, 0.0, 1.0], color),
        Vertex::untextured([-1.0, 0.0, 1.0], color),
    ]
}
