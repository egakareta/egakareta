/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
#[cfg(test)]
mod tests {
    use crate::mesh::blocks::build_block_vertices;
    use crate::mesh::builders::{
        build_editor_gizmo_vertices, build_editor_hover_outline_vertices, GizmoParams,
    };
    use crate::mesh::obj::parse_obj_mesh;
    use crate::types::{GizmoPart, LevelObject, Vertex};

    fn bounds_xz(vertices: &[[f32; 3]]) -> (f32, f32, f32, f32) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for pos in vertices {
            min_x = min_x.min(pos[0]);
            max_x = max_x.max(pos[0]);
            min_z = min_z.min(pos[2]);
            max_z = max_z.max(pos[2]);
        }
        (min_x, max_x, min_z, max_z)
    }

    #[test]
    fn block_vertices_rotate_about_center() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 1.0],
            rotation_degrees: [0.0, 90.0, 0.0],
            roundness: 0.18,
            block_id: "core/dirt".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            name: String::new(),
            group_path: Vec::new(),
        };
        let vertices = build_block_vertices(&[obj]);
        let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
        let (min_x, max_x, min_z, max_z) = bounds_xz(&positions);

        assert!((min_x - 0.5).abs() < 1e-5);
        assert!((max_x - 1.5).abs() < 1e-5);
        assert!((min_z - -0.5).abs() < 1e-5);
        assert!((max_z - 1.5).abs() < 1e-5);
    }

    #[test]
    fn gizmo_hover_tip_growth_preserves_shaft_length() {
        let position = [0.0, 0.0, 0.0];
        let size = [1.0, 1.0, 1.0];
        let axis_lengths = [10.0, 10.0, 10.0];
        let axis_width = 1.0;
        let resize_radius = 1.0;
        let resize_offsets = [1.0, 1.0, 1.0];

        // 1. Get vertices without hover
        let vertices_normal = build_editor_gizmo_vertices(GizmoParams {
            position,
            size,
            rotation_degrees: [0.0, 0.0, 0.0],
            axis_lengths,
            axis_width,
            resize_radius,
            resize_offsets,
            show_move_handles: true,
            show_scale_handles: false,
            show_rotate_handles: false,
            hovered_part: None,
            dragged_part: None,
        });

        // 2. Get vertices with MoveX hovered
        let vertices_hovered = build_editor_gizmo_vertices(GizmoParams {
            position,
            size,
            rotation_degrees: [0.0, 0.0, 0.0],
            axis_lengths,
            axis_width,
            resize_radius,
            resize_offsets,
            show_move_handles: true,
            show_scale_handles: false,
            show_rotate_handles: false,
            hovered_part: Some(GizmoPart::MoveX),
            dragged_part: None,
        });

        // Function to find the max X of the shaft (prism vertices)
        let find_shaft_end_x = |verts: &[Vertex], is_hovered: bool| -> f32 {
            let target_color = if is_hovered {
                [1.0, 0.0, 0.0, 1.0]
            } else {
                [0.804, 0.0, 0.0, 0.6]
            };
            let target_color_dark = [
                target_color[0] * 0.8,
                target_color[1] * 0.8,
                target_color[2] * 0.8,
                target_color[3] * 0.8,
            ];

            let shaft_verts: Vec<_> = verts
                .iter()
                .filter(|v| {
                    let matches_color = (v.color[0] - target_color[0]).abs() < 0.01
                        && (v.color[1] - target_color[1]).abs() < 0.01;
                    let matches_dark = (v.color[0] - target_color_dark[0]).abs() < 0.01
                        && (v.color[1] - target_color_dark[1]).abs() < 0.01;

                    (matches_color || matches_dark) && v.position[0] > 0.5
                })
                .collect();

            // The shaft (prism) has 36 vertices. Some might be from the cone (tip),
            // but the shaft is generated before the cone in the builder.
            // The shaft's far end (p_max_x) is what we want.
            assert!(
                shaft_verts.len() >= 36,
                "expected at least 36 shaft vertices for gizmo shaft, got {}",
                shaft_verts.len()
            );
            shaft_verts[0..36]
                .iter()
                .map(|v| v.position[0])
                .fold(f32::NEG_INFINITY, f32::max)
        };

        let end_x_normal = find_shaft_end_x(&vertices_normal, false);
        let end_x_hovered = find_shaft_end_x(&vertices_hovered, true);

        // Shaft end should be identical because we fixed shaft length and start offset
        assert!(
            (end_x_normal - end_x_hovered).abs() < 1e-5,
            "Shaft end X should be identical. Normal: {}, Hovered: {}",
            end_x_normal,
            end_x_hovered
        );

        // Function to find the absolute max X (the tip of the cone)
        let find_tip_end_x = |verts: &[Vertex]| -> f32 {
            // Since we only have one X arrow and it's the furthest thing in +X,
            // we can just look for the max X.
            // We filter for high R component to avoid any white/black helper vertices if they exist.
            verts
                .iter()
                .filter(|v| v.color[0] > 0.5 && v.position[0] > 0.5)
                .map(|v| v.position[0])
                .fold(f32::NEG_INFINITY, f32::max)
        };

        let tip_x_normal = find_tip_end_x(&vertices_normal);
        let tip_x_hovered = find_tip_end_x(&vertices_hovered);

        // Tip should be further out because tip_length increased
        assert!(
            tip_x_hovered > tip_x_normal,
            "Tip should be longer when hovered. Normal: {}, Hovered: {}",
            tip_x_normal,
            tip_x_hovered
        );
    }

    #[test]
    fn gizmo_vertices_generate_with_screen_scaled_inputs() {
        let vertices = build_editor_gizmo_vertices(GizmoParams {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 2.0, 2.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            axis_lengths: [3.0, 4.0, 5.0],
            axis_width: 0.1,
            resize_radius: 0.2,
            resize_offsets: [0.3, 0.3, 0.3],
            show_move_handles: true,
            show_scale_handles: true,
            show_rotate_handles: false,
            hovered_part: None,
            dragged_part: None,
        });
        assert!(!vertices.is_empty());

        let max_x = vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_x >= 4.0);
    }

    #[test]
    fn hover_outline_vertices_are_translucent() {
        let vertices = build_editor_hover_outline_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0.03);
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

    #[test]
    fn finish_ring_generates_vertices() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 0.3],
            rotation_degrees: [0.0, 0.0, 0.0],
            roundness: 0.0,
            block_id: "core/finish".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            name: String::new(),
            group_path: Vec::new(),
        };

        let vertices = build_block_vertices(&[obj]);
        assert!(vertices.len() >= 168); // 28 segments * (outer + inner + face) triangles
    }
}
