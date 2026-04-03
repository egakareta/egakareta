/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::Vertex;
use glam::{EulerRot, Mat3, Vec3};

pub(crate) fn rotate_vertices_around_euler(
    vertices: &mut [Vertex],
    center: [f32; 3],
    degrees: [f32; 3],
) {
    if degrees
        .iter()
        .all(|component| component.abs() <= f32::EPSILON)
    {
        return;
    }

    let rotation = Mat3::from_euler(
        EulerRot::XYZ,
        degrees[0].to_radians(),
        degrees[1].to_radians(),
        degrees[2].to_radians(),
    );
    let center = Vec3::from(center);

    for vertex in vertices.iter_mut() {
        let local = Vec3::from(vertex.position) - center;
        let world = center + rotation * local;
        vertex.position = world.to_array();
    }
}
