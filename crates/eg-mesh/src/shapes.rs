/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::Vertex;

#[derive(Copy, Clone)]
pub struct PrismFaceColors {
    pub top: [f32; 4],
    pub side: [f32; 4],
    pub bottom: [f32; 4],
    pub outline: [f32; 4],
}

impl PrismFaceColors {
    pub fn new_with_outline(
        top: [f32; 4],
        side: [f32; 4],
        bottom: [f32; 4],
        outline: [f32; 4],
    ) -> Self {
        Self {
            top,
            side,
            bottom,
            outline,
        }
    }

    pub fn tinted(self, tint_rgb: [f32; 3]) -> Self {
        Self {
            top: apply_color_tint(self.top, tint_rgb),
            side: apply_color_tint(self.side, tint_rgb),
            bottom: apply_color_tint(self.bottom, tint_rgb),
            outline: apply_color_tint(self.outline, tint_rgb),
        }
    }
}

fn apply_color_tint(color: [f32; 4], tint_rgb: [f32; 3]) -> [f32; 4] {
    [
        color[0] * tint_rgb[0].clamp(0.0, 1.0),
        color[1] * tint_rgb[1].clamp(0.0, 1.0),
        color[2] * tint_rgb[2].clamp(0.0, 1.0),
        color[3],
    ]
}

#[derive(Copy, Clone)]
pub struct PrismTextureLayers {
    pub top: u32,
    pub side: u32,
    pub bottom: u32,
}

impl PrismTextureLayers {
    pub fn new(top: u32, side: u32, bottom: u32) -> Self {
        Self { top, side, bottom }
    }
}

pub fn append_prism(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    color_top: [f32; 4],
    color_side: [f32; 4],
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    let mut push_face =
        |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], color: [f32; 4]| {
            vertices.push(Vertex::untextured(p0, color));
            vertices.push(Vertex::untextured(p1, color));
            vertices.push(Vertex::untextured(p2, color));
            vertices.push(Vertex::untextured(p0, color));
            vertices.push(Vertex::untextured(p2, color));
            vertices.push(Vertex::untextured(p3, color));
        };

    // Top (+Y)
    push_face(
        [x_min, y_max, z_min],
        [x_min, y_max, z_max],
        [x_max, y_max, z_max],
        [x_max, y_max, z_min],
        color_top,
    );

    // +X
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        color_side,
    );

    // -X
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        color_side,
    );

    // +Z
    push_face(
        [x_min, y_min, z_max],
        [x_max, y_min, z_max],
        [x_max, y_max, z_max],
        [x_min, y_max, z_max],
        color_side,
    );

    // -Z
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_max, z_min],
        [x_max, y_max, z_min],
        [x_max, y_min, z_min],
        color_side,
    );

    // Bottom (-Y)
    push_face(
        [x_min, y_min, z_min],
        [x_max, y_min, z_min],
        [x_max, y_min, z_max],
        [x_min, y_min, z_max],
        color_side,
    );
}

pub fn append_prism_with_layers(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    colors: PrismFaceColors,
    texture_layers: PrismTextureLayers,
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    // Stretch the texture once across each face (UVs 0..1) regardless of the
    // block's dimensions, instead of tiling it per world unit. `uv_norm` tracks
    // the UV extent (1.0) so the shader's edge/outline detection maps to a
    // consistent fraction of the face on every block.
    let mut push_face = |p0: [f32; 3], // Bottom-Left
                         p1: [f32; 3], // Top-Left
                         p2: [f32; 3], // Top-Right
                         p3: [f32; 3], // Bottom-Right
                         color: [f32; 4],
                         texture_layer: u32| {
        let face_size = [1.0, 1.0];
        vertices.push(Vertex::textured_with_outline(
            p0,
            color,
            [0.0, 1.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p1,
            color,
            [0.0, 0.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p2,
            color,
            [1.0, 0.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p0,
            color,
            [0.0, 1.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p2,
            color,
            [1.0, 0.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p3,
            color,
            [1.0, 1.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
    };

    // Top (+Y)
    push_face(
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        colors.top,
        texture_layers.top,
    );

    // +X
    push_face(
        [x_max, y_min, z_max],
        [x_max, y_max, z_max],
        [x_max, y_max, z_min],
        [x_max, y_min, z_min],
        colors.side,
        texture_layers.side,
    );

    // -X
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_max, z_min],
        [x_min, y_max, z_max],
        [x_min, y_min, z_max],
        colors.side,
        texture_layers.side,
    );

    // +Z
    push_face(
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        colors.side,
        texture_layers.side,
    );

    // -Z
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_min, y_max, z_min],
        [x_min, y_min, z_min],
        colors.side,
        texture_layers.side,
    );

    // Bottom (-Y)
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_min, z_max],
        [x_max, y_min, z_max],
        [x_max, y_min, z_min],
        colors.bottom,
        texture_layers.bottom,
    );
}

pub fn append_quad(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    color: [f32; 4],
) {
    vertices.push(Vertex::untextured(p0, color));
    vertices.push(Vertex::untextured(p1, color));
    vertices.push(Vertex::untextured(p2, color));
    vertices.push(Vertex::untextured(p0, color));
    vertices.push(Vertex::untextured(p2, color));
    vertices.push(Vertex::untextured(p3, color));
}

#[cfg(test)]
mod tests {
    use super::{PrismFaceColors, PrismTextureLayers, append_prism_with_layers};
    use crate::Vertex;

    #[test]
    fn test_prism_uv_orientations() {
        let mut vertices = Vec::new();
        let min = [0.0, 0.0, 0.0];
        let max = [1.0, 1.0, 1.0];
        let color = [1.0, 1.0, 1.0, 1.0];

        // Using distinct layer IDs to ensure we're checking the right faces if needed,
        // though here we mostly care about the UVs associated with specific positions.
        append_prism_with_layers(
            &mut vertices,
            min,
            max,
            PrismFaceColors::new_with_outline(color, color, color, [0.0, 0.0, 0.0, 0.0]),
            PrismTextureLayers::new(1, 2, 3),
        );

        // Each prism has 6 faces * 6 vertices = 36 vertices
        assert_eq!(vertices.len(), 36);

        // Helper to check if a vertex at a specific position has the expected UV
        let check_uv = |pos: [f32; 3], expected_uv: [f32; 2], face_name: &str| {
            let matches: Vec<&Vertex> = vertices
                .iter()
                .filter(|v| {
                    (v.position[0] - pos[0]).abs() < 1e-5
                        && (v.position[1] - pos[1]).abs() < 1e-5
                        && (v.position[2] - pos[2]).abs() < 1e-5
                })
                .collect();

            assert!(
                !matches.is_empty(),
                "No vertex found at {:?} for {}",
                pos,
                face_name
            );

            let has_correct_uv = matches.iter().any(|v| {
                (v.uv[0] - expected_uv[0]).abs() < 1e-5 && (v.uv[1] - expected_uv[1]).abs() < 1e-5
            });

            assert!(
                has_correct_uv,
                "Vertex at {:?} for {} has incorrect UV. Expected {:?}, found {:?}",
                pos, face_name, expected_uv, matches[0].uv
            );
        };

        // Side faces (+Z, -Z, +X, -X) should have V=0 at top (Y=1) and V=1 at bottom (Y=0)
        // +Z face (front)
        check_uv([1.0, 1.0, 1.0], [0.0, 0.0], "+Z Top-Left");
        check_uv([0.0, 1.0, 1.0], [1.0, 0.0], "+Z Top-Right");
        check_uv([1.0, 0.0, 1.0], [0.0, 1.0], "+Z Bottom-Left");
        check_uv([0.0, 0.0, 1.0], [1.0, 1.0], "+Z Bottom-Right");

        // -Z face (back)
        check_uv([0.0, 1.0, 0.0], [0.0, 0.0], "-Z Top-Left");
        check_uv([1.0, 1.0, 0.0], [1.0, 0.0], "-Z Top-Right");
        check_uv([0.0, 0.0, 0.0], [0.0, 1.0], "-Z Bottom-Left");
        check_uv([1.0, 0.0, 0.0], [1.0, 1.0], "-Z Bottom-Right");

        // +X face (right)
        check_uv([1.0, 1.0, 1.0], [0.0, 0.0], "+X Top-Left");
        check_uv([1.0, 1.0, 0.0], [1.0, 0.0], "+X Top-Right");
        check_uv([1.0, 0.0, 1.0], [0.0, 1.0], "+X Bottom-Left");
        check_uv([1.0, 0.0, 0.0], [1.0, 1.0], "+X Bottom-Right");

        // -X face (left)
        check_uv([0.0, 1.0, 0.0], [0.0, 0.0], "-X Top-Left");
        check_uv([0.0, 1.0, 1.0], [1.0, 0.0], "-X Top-Right");
        check_uv([0.0, 0.0, 0.0], [0.0, 1.0], "-X Bottom-Left");
        check_uv([0.0, 0.0, 1.0], [1.0, 1.0], "-X Bottom-Right");

        // Top face (+Y)
        // [x_min, y_max, z_max] -> [0, 1, 1] is Bottom-Left in push_face order
        // DX=1, DZ=1 so U_MAX=1, V_MAX=1
        check_uv([0.0, 1.0, 1.0], [0.0, 1.0], "Top Bottom-Left");
        check_uv([0.0, 1.0, 0.0], [0.0, 0.0], "Top Top-Left");
        check_uv([1.0, 1.0, 0.0], [1.0, 0.0], "Top Top-Right");
        check_uv([1.0, 1.0, 1.0], [1.0, 1.0], "Top Bottom-Right");

        // Test stretched resize: textures stretch to fill each face (UVs 0..1)
        // regardless of the prism's dimensions, instead of tiling per world unit.
        let mut stretched_vertices = Vec::new();
        append_prism_with_layers(
            &mut stretched_vertices,
            [0.0, 0.0, 0.0],
            [2.0, 1.0, 3.0], // Resized: DX=2, DY=1, DZ=3
            PrismFaceColors::new_with_outline(color, color, color, [0.0, 0.0, 0.0, 0.0]),
            PrismTextureLayers::new(1, 2, 3),
        );

        let check_stretched_uv =
            |vertices: &[Vertex], pos: [f32; 3], expected_uv: [f32; 2], face_name: &str| {
                let matches: Vec<&Vertex> = vertices
                    .iter()
                    .filter(|v| {
                        (v.position[0] - pos[0]).abs() < 1e-5
                            && (v.position[1] - pos[1]).abs() < 1e-5
                            && (v.position[2] - pos[2]).abs() < 1e-5
                    })
                    .collect();

                assert!(
                    !matches.is_empty(),
                    "No vertex found at {:?} for {}",
                    pos,
                    face_name
                );

                let has_correct_uv = matches.iter().any(|v| {
                    (v.uv[0] - expected_uv[0]).abs() < 1e-5
                        && (v.uv[1] - expected_uv[1]).abs() < 1e-5
                });

                assert!(
                    has_correct_uv,
                    "Vertex at {:?} for {} has incorrect UV. Expected {:?}, found {:?}",
                    pos, face_name, expected_uv, matches[0].uv
                );
            };

        // Top face of stretched block: DX=2, DZ=3, but UVs stay 0..1.
        check_stretched_uv(
            &stretched_vertices,
            [2.0, 1.0, 0.0],
            [1.0, 0.0],
            "Stretched Top Top-Right",
        );
        check_stretched_uv(
            &stretched_vertices,
            [2.0, 1.0, 3.0],
            [1.0, 1.0],
            "Stretched Top Bottom-Right",
        );

        // Side face (+Z): DX=2, DY=1, but UVs stay 0..1.
        // We're looking from +Z to -Z.
        // Left is -X (x=0), Right is +X (x=2).
        check_stretched_uv(
            &stretched_vertices,
            [0.0, 1.0, 3.0],
            [0.0, 0.0], // x=0 is Left, so U=0
            "Stretched +Z Top-Left",
        );
        check_stretched_uv(
            &stretched_vertices,
            [2.0, 1.0, 3.0],
            [1.0, 0.0], // x=2 is Right, so U=1 (stretched, not 2)
            "Stretched +Z Top-Right",
        );
    }

    #[test]
    fn prism_face_colors_apply_tint_to_rgb_only() {
        let colors = PrismFaceColors::new_with_outline(
            [0.8, 0.6, 0.4, 0.2],
            [0.7, 0.5, 0.3, 0.4],
            [0.6, 0.4, 0.2, 0.6],
            [0.5, 0.3, 0.1, 0.8],
        )
        .tinted([0.5, 0.25, 2.0]);

        assert_eq!(colors.top, [0.4, 0.15, 0.4, 0.2]);
        assert_eq!(colors.side, [0.35, 0.125, 0.3, 0.4]);
        assert_eq!(colors.bottom, [0.3, 0.1, 0.2, 0.6]);
        assert_eq!(colors.outline, [0.25, 0.075, 0.1, 0.8]);
    }

    #[test]
    fn test_prism_outline_metadata() {
        let mut vertices = Vec::new();
        let min = [0.0, 0.0, 0.0];
        let max = [2.5, 1.0, 3.5];
        let color = [1.0, 1.0, 1.0, 1.0];
        let outline_color = [0.1, 0.2, 0.3, 0.4];

        append_prism_with_layers(
            &mut vertices,
            min,
            max,
            PrismFaceColors::new_with_outline(color, color, color, outline_color),
            PrismTextureLayers::new(1, 2, 3),
        );

        // A single prism has 6 faces * 6 vertices = 36 vertices.
        assert_eq!(vertices.len(), 36);

        // Check metadata on all vertices
        for v in &vertices {
            assert_eq!(v.color_outline, outline_color);
        }

        // Specifically check uv_norm for each vertex group (6 vertices per face).
        // Textures stretch once across each face, so the UV extent is always 1.0
        // on both axes regardless of the prism's dimensions.
        // Order: Top (+Y), +X, -X, +Z, -Z, Bottom (-Y)
        for v in &vertices {
            assert_eq!(v.uv_norm, [1.0, 1.0]);
        }
    }

    #[test]
    fn test_prism_winding_order() {
        use glam::Vec3;

        let mut vertices = Vec::new();
        let min = [0.0, 0.0, 0.0];
        let max = [2.0, 2.0, 2.0];
        let color = [1.0, 1.0, 1.0, 1.0];

        append_prism_with_layers(
            &mut vertices,
            min,
            max,
            PrismFaceColors::new_with_outline(color, color, color, [0.0, 0.0, 0.0, 0.0]),
            PrismTextureLayers::new(1, 2, 3),
        );

        assert_eq!(vertices.len(), 36);

        let expected_outward_normals = [
            Vec3::new(0.0, 1.0, 0.0),  // Top (+Y)
            Vec3::new(1.0, 0.0, 0.0),  // Right (+X)
            Vec3::new(-1.0, 0.0, 0.0), // Left (-X)
            Vec3::new(0.0, 0.0, 1.0),  // Front (+Z)
            Vec3::new(0.0, 0.0, -1.0), // Back (-Z)
            Vec3::new(0.0, -1.0, 0.0), // Bottom (-Y)
        ];

        for (face_index, expected_outward_normal) in expected_outward_normals.iter().enumerate() {
            let start_idx = face_index * 6;

            // First triangle of the face (indices 0, 1, 2)
            let p0 = Vec3::from(vertices[start_idx].position);
            let p1 = Vec3::from(vertices[start_idx + 1].position);
            let p2 = Vec3::from(vertices[start_idx + 2].position);

            let u = p1 - p0;
            let v = p2 - p0;
            let normal = u.cross(v).normalize();

            // In a Right-Handed system, if vertices are given in CW order when viewed from outside,
            // the computed geometric normal (u x v) will point exactly opposite to the outward normal.
            // i.e., normal dot expected_outward_normal should be -1.0
            let dot = normal.dot(*expected_outward_normal);
            assert!(
                dot < -0.99,
                "Face {} is not clockwise. Expected inward normal (opposite of outward), but got dot product {}",
                face_index,
                dot
            );
        }
    }
}
