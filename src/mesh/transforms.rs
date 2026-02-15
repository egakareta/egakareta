use crate::types::Vertex;

pub(crate) fn rotate_vertices_around_z(vertices: &mut [Vertex], center: [f32; 3], degrees: f32) {
    if degrees.abs() <= f32::EPSILON {
        return;
    }

    let radians = degrees.to_radians();
    let (sin_theta, cos_theta) = radians.sin_cos();

    for vertex in vertices.iter_mut() {
        let dx = vertex.position[0] - center[0];
        let dy = vertex.position[1] - center[1];
        vertex.position[0] = center[0] + dx * cos_theta - dy * sin_theta;
        vertex.position[1] = center[1] + dx * sin_theta + dy * cos_theta;
    }
}
