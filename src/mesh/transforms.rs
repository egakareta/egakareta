use crate::types::Vertex;

pub(crate) fn rotate_vertices_around_y(vertices: &mut [Vertex], center: [f32; 3], degrees: f32) {
    if degrees.abs() <= f32::EPSILON {
        return;
    }

    let radians = degrees.to_radians();
    let (sin_theta, cos_theta) = radians.sin_cos();

    for vertex in vertices.iter_mut() {
        let dx = vertex.position[0] - center[0];
        let dz = vertex.position[2] - center[2];
        vertex.position[0] = center[0] + dx * cos_theta - dz * sin_theta;
        vertex.position[2] = center[2] + dx * sin_theta + dz * cos_theta;
    }
}
