/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(test)]
mod tests {
    use crate::mesh::blocks::{build_block_geometry, build_block_vertices};
    use crate::mesh::builders::{
        build_editor_gizmo_vertices, build_editor_hover_outline_vertices,
        build_editor_selection_outline_vertices, GizmoParams,
    };
    use crate::mesh::egmesh::resolve_egmesh;
    use crate::mesh::obj::{append_obj_mesh, parse_obj_mesh, parse_obj_mesh_with_materials};
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
            block_id: "core/dirt".to_string(),
            color_tint: [1.0, 1.0, 1.0],
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
    fn solid_block_black_tint_darkens_vertex_colors() {
        let obj = LevelObject {
            block_id: "core/solid".to_string(),
            color_tint: [0.0, 0.0, 0.0],
            ..LevelObject::default()
        };

        let vertices = build_block_vertices(&[obj]);

        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|vertex| vertex.color[0] == 0.0
            && vertex.color[1] == 0.0
            && vertex.color[2] == 0.0));
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
    fn hover_outline_vertices_are_opaque_light_blue() {
        let vertices = build_editor_hover_outline_vertices(
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            0.03,
        );
        assert!(!vertices.is_empty());
        assert!(vertices
            .iter()
            .all(|vertex| vertex.color == [0.698, 0.898, 1.0, 1.0]));
    }

    #[test]
    fn selection_outline_vertices_are_opaque_uniform_blue() {
        let vertices = build_editor_selection_outline_vertices(
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            0.03,
        );
        assert!(!vertices.is_empty());
        assert!(vertices
            .iter()
            .all(|vertex| vertex.color == [0.098, 0.6, 1.0, 1.0]));
    }

    #[test]
    fn editor_outline_vertices_use_screen_space_width_metadata() {
        let vertices = build_editor_selection_outline_vertices(
            [2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0],
            [0.0, 0.0, 0.0],
            2.0,
        );

        let min_x = vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_x - 2.0).abs() <= 1e-6);
        assert!((max_x - 7.0).abs() <= 1e-6);

        assert!(vertices.iter().all(|vertex| vertex.render_profile == 3.0));
        assert!(vertices.iter().all(|vertex| {
            let anchor_delta = [
                vertex.position[0] - vertex.color_outline[0],
                vertex.position[1] - vertex.color_outline[1],
                vertex.position[2] - vertex.color_outline[2],
            ];
            anchor_delta
                .iter()
                .all(|component| (component.abs() - 1.0).abs() <= 1e-6 || component.abs() <= 1e-6)
                && (vertex.color_outline[3] - 2.7).abs() <= 1e-6
        }));
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
    fn obj_parser_applies_mtl_materials_to_faces() {
        let obj = r#"
mtllib flag.mtl
v 0 0 0
v 1 0 0
v 0 1 0
usemtl cloth
f 1 2 3
"#;
        let mtl = r#"
newmtl cloth
Kd 0.4 0.8 0.5
d 0.5
"#;

        let mesh = parse_obj_mesh_with_materials(obj, &[("flag.mtl", mtl)]).expect("valid mesh");

        assert_eq!(mesh.materials.len(), 1);
        assert_eq!(mesh.materials[0].name, "cloth");
        assert_eq!(mesh.materials[0].color.diffuse, [0.4, 0.8, 0.5, 0.5]);
        assert_eq!(mesh.faces[0][0].material_index, Some(0));

        let mut vertices = Vec::new();
        append_obj_mesh(
            &mut vertices,
            &LevelObject::default(),
            &mesh,
            [0.5, 0.25, 1.0, 0.8],
            0,
        );

        assert!(!vertices.is_empty());
        for vertex in vertices {
            assert!((vertex.color[0] - 0.2).abs() <= 1e-6);
            assert!((vertex.color[1] - 0.2).abs() <= 1e-6);
            assert!((vertex.color[2] - 0.5).abs() <= 1e-6);
            assert!((vertex.color[3] - 0.4).abs() <= 1e-6);
        }
    }

    #[test]
    fn generated_speedportal_egmesh_builds_indexed_geometry() {
        let mesh = resolve_egmesh("speedportal.obj").expect("speedportal egmesh should resolve");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());

        let obj = LevelObject {
            block_id: "core/speedportal".to_string(),
            ..LevelObject::default()
        };
        let geometry = build_block_geometry(std::slice::from_ref(&obj));
        assert!(geometry
            .indices
            .as_ref()
            .is_some_and(|indices| !indices.is_empty()));
        assert_eq!(geometry.to_triangle_vertices().len(), mesh.indices.len());
    }

    #[test]
    fn gizmo_rotate_hover_expands_tube_more_than_dragged() {
        let params = GizmoParams {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            axis_lengths: [8.0, 8.0, 8.0],
            axis_width: 0.5,
            resize_radius: 0.15,
            resize_offsets: [0.2, 0.2, 0.2],
            show_move_handles: false,
            show_scale_handles: false,
            show_rotate_handles: true,
            hovered_part: None,
            dragged_part: None,
        };

        let hovered = build_editor_gizmo_vertices(GizmoParams {
            hovered_part: Some(GizmoPart::RotateX),
            ..params
        });
        let dragged = build_editor_gizmo_vertices(GizmoParams {
            dragged_part: Some(GizmoPart::RotateX),
            ..params
        });

        let active_red = |vertex: &Vertex| {
            (vertex.color[0] - 1.0).abs() < 0.01
                && vertex.color[1].abs() < 0.01
                && vertex.color[2].abs() < 0.01
                && (vertex.color[3] - 1.0).abs() < 0.01
        };

        let x_span = |vertices: &[Vertex]| {
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut count = 0usize;
            for vertex in vertices {
                if active_red(vertex) {
                    min_x = min_x.min(vertex.position[0]);
                    max_x = max_x.max(vertex.position[0]);
                    count += 1;
                }
            }
            assert!(count > 0, "expected active rotate-x vertices");
            max_x - min_x
        };

        let hovered_span = x_span(&hovered);
        let dragged_span = x_span(&dragged);
        assert!(
            hovered_span > dragged_span + 0.01,
            "hovered rotate ring should be thicker than dragged ring"
        );
    }

    #[test]
    fn gizmo_resize_drag_sets_active_color_state() {
        let params = GizmoParams {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            axis_lengths: [4.0, 4.0, 4.0],
            axis_width: 0.2,
            resize_radius: 0.2,
            resize_offsets: [0.3, 0.3, 0.3],
            show_move_handles: false,
            show_scale_handles: true,
            show_rotate_handles: false,
            hovered_part: None,
            dragged_part: None,
        };

        let normal = build_editor_gizmo_vertices(GizmoParams { ..params });
        let dragged = build_editor_gizmo_vertices(GizmoParams {
            dragged_part: Some(GizmoPart::ResizeX),
            ..params
        });

        let max_alpha = |vertices: &[Vertex]| {
            vertices
                .iter()
                .map(|vertex| vertex.color[3])
                .fold(f32::NEG_INFINITY, f32::max)
        };

        let normal_max_alpha = max_alpha(&normal);
        let dragged_max_alpha = max_alpha(&dragged);
        assert!(
            dragged_max_alpha > normal_max_alpha + 0.2,
            "dragging resize handle should promote alpha from normal to active"
        );
    }
}
