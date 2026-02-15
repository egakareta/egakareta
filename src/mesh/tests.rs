#[cfg(test)]
mod tests {
    use crate::mesh::blocks::build_block_vertices;
    use crate::mesh::builders::{build_editor_gizmo_vertices, build_editor_hover_outline_vertices};
    use crate::mesh::obj::parse_obj_mesh;
    use crate::types::LevelObject;

    fn bounds_xy(vertices: &[[f32; 3]]) -> (f32, f32, f32, f32) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for pos in vertices {
            min_x = min_x.min(pos[0]);
            max_x = max_x.max(pos[0]);
            min_y = min_y.min(pos[1]);
            max_y = max_y.max(pos[1]);
        }
        (min_x, max_x, min_y, max_y)
    }

    #[test]
    fn block_vertices_rotate_about_center() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 1.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        };
        let vertices = build_block_vertices(&[obj]);
        let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
        let (min_x, max_x, min_y, max_y) = bounds_xy(&positions);

        assert!((min_x - 0.5).abs() < 1e-5);
        assert!((max_x - 1.5).abs() < 1e-5);
        assert!((min_y - -0.5).abs() < 1e-5);
        assert!((max_y - 1.5).abs() < 1e-5);
    }

    #[test]
    fn gizmo_vertices_generate_with_screen_scaled_inputs() {
        let vertices = build_editor_gizmo_vertices(
            [0.0, 0.0, 0.0],
            [2.0, 2.0, 2.0],
            [3.0, 4.0, 5.0],
            0.1,
            None,
        );
        assert!(!vertices.is_empty());

        let max_x = vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_x >= 4.0);
    }

    #[test]
    fn hover_outline_vertices_are_translucent() {
        let vertices = build_editor_hover_outline_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert!(!vertices.is_empty());
        assert!(vertices.iter().any(|v| v.color[3] < 1.0));
    }

    #[test]
    fn obj_parser_supports_uvs_and_normals() {
        let obj = r#"
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
"#;

        let mesh = parse_obj_mesh(obj).expect("valid mesh");
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.texcoords.len(), 3);
        assert_eq!(mesh.normals.len(), 1);
        assert_eq!(mesh.faces.len(), 1);
        assert_eq!(mesh.faces[0][0].texcoord_index, Some(0));
        assert_eq!(mesh.faces[0][0].normal_index, Some(0));
    }
}
