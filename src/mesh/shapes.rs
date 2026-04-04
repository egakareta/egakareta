/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::types::Vertex;

#[derive(Copy, Clone)]
pub(crate) struct PrismFaceColors {
    pub(crate) top: [f32; 4],
    pub(crate) side: [f32; 4],
    pub(crate) bottom: [f32; 4],
    pub(crate) outline: [f32; 4],
}

impl PrismFaceColors {
    pub(crate) fn new_with_outline(
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

    pub(crate) fn uniform_with_outline(color: [f32; 4], outline: [f32; 4]) -> Self {
        Self::new_with_outline(color, color, color, outline)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct PrismTextureLayers {
    pub(crate) top: u32,
    pub(crate) side: u32,
    pub(crate) bottom: u32,
}

impl PrismTextureLayers {
    pub(crate) fn new(top: u32, side: u32, bottom: u32) -> Self {
        Self { top, side, bottom }
    }
}

pub(crate) fn append_prism(
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

pub(crate) fn append_prism_with_layers(
    vertices: &mut Vec<Vertex>,
    min: [f32; 3],
    max: [f32; 3],
    colors: PrismFaceColors,
    texture_layers: PrismTextureLayers,
) {
    let [x_min, y_min, z_min] = min;
    let [x_max, y_max, z_max] = max;

    let mut push_face = |p0: [f32; 3], // Bottom-Left
                         p1: [f32; 3], // Top-Left
                         p2: [f32; 3], // Top-Right
                         p3: [f32; 3], // Bottom-Right
                         color: [f32; 4],
                         texture_layer: u32,
                         u_max: f32,
                         v_max: f32| {
        let face_size = [u_max, v_max];
        vertices.push(Vertex::textured_with_outline(
            p0,
            color,
            [0.0, v_max],
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
            [u_max, 0.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p0,
            color,
            [0.0, v_max],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p2,
            color,
            [u_max, 0.0],
            face_size,
            texture_layer,
            colors.outline,
        ));
        vertices.push(Vertex::textured_with_outline(
            p3,
            color,
            [u_max, v_max],
            face_size,
            texture_layer,
            colors.outline,
        ));
    };

    let dx = (x_max - x_min).abs();
    let dy = (y_max - y_min).abs();
    let dz = (z_max - z_min).abs();

    // Top (+Y)
    push_face(
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        colors.top,
        texture_layers.top,
        dx,
        dz,
    );

    // +X
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        colors.side,
        texture_layers.side,
        dz,
        dy,
    );

    // -X
    push_face(
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_min, y_max, z_min],
        [x_min, y_min, z_min],
        colors.side,
        texture_layers.side,
        dz,
        dy,
    );

    // +Z
    push_face(
        [x_min, y_min, z_max],
        [x_min, y_max, z_max],
        [x_max, y_max, z_max],
        [x_max, y_min, z_max],
        colors.side,
        texture_layers.side,
        dx,
        dy,
    );

    // -Z
    push_face(
        [x_max, y_min, z_min],
        [x_max, y_max, z_min],
        [x_min, y_max, z_min],
        [x_min, y_min, z_min],
        colors.side,
        texture_layers.side,
        dx,
        dy,
    );

    // Bottom (-Y)
    push_face(
        [x_min, y_min, z_min],
        [x_min, y_min, z_max],
        [x_max, y_min, z_max],
        [x_max, y_min, z_min],
        colors.bottom,
        texture_layers.bottom,
        dx,
        dz,
    );
}

pub(crate) fn append_quad(
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
    use super::{append_prism_with_layers, PrismFaceColors, PrismTextureLayers};
    use crate::types::Vertex;

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
            PrismFaceColors::uniform_with_outline(color, [0.0, 0.0, 0.0, 0.0]),
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
        check_uv([0.0, 1.0, 1.0], [0.0, 0.0], "+Z Top-Left");
        check_uv([1.0, 1.0, 1.0], [1.0, 0.0], "+Z Top-Right");
        check_uv([0.0, 0.0, 1.0], [0.0, 1.0], "+Z Bottom-Left");
        check_uv([1.0, 0.0, 1.0], [1.0, 1.0], "+Z Bottom-Right");

        // -Z face (back)
        check_uv([1.0, 1.0, 0.0], [0.0, 0.0], "-Z Top-Left");
        check_uv([0.0, 1.0, 0.0], [1.0, 0.0], "-Z Top-Right");
        check_uv([1.0, 0.0, 0.0], [0.0, 1.0], "-Z Bottom-Left");
        check_uv([0.0, 0.0, 0.0], [1.0, 1.0], "-Z Bottom-Right");

        // +X face (right)
        check_uv([1.0, 1.0, 0.0], [0.0, 0.0], "+X Top-Left");
        check_uv([1.0, 1.0, 1.0], [1.0, 0.0], "+X Top-Right");
        check_uv([1.0, 0.0, 0.0], [0.0, 1.0], "+X Bottom-Left");
        check_uv([1.0, 0.0, 1.0], [1.0, 1.0], "+X Bottom-Right");

        // -X face (left)
        check_uv([0.0, 1.0, 1.0], [0.0, 0.0], "-X Top-Left");
        check_uv([0.0, 1.0, 0.0], [1.0, 0.0], "-X Top-Right");
        check_uv([0.0, 0.0, 1.0], [0.0, 1.0], "-X Bottom-Left");
        check_uv([0.0, 0.0, 0.0], [1.0, 1.0], "-X Bottom-Right");

        // Top face (+Y)
        // [x_min, y_max, z_max] -> [0, 1, 1] is Bottom-Left in push_face order
        // DX=1, DZ=1 so U_MAX=1, V_MAX=1
        check_uv([0.0, 1.0, 1.0], [0.0, 1.0], "Top Bottom-Left");
        check_uv([0.0, 1.0, 0.0], [0.0, 0.0], "Top Top-Left");
        check_uv([1.0, 1.0, 0.0], [1.0, 0.0], "Top Top-Right");
        check_uv([1.0, 1.0, 1.0], [1.0, 1.0], "Top Bottom-Right");

        // Test tiled resize
        let mut tiled_vertices = Vec::new();
        append_prism_with_layers(
            &mut tiled_vertices,
            [0.0, 0.0, 0.0],
            [2.0, 1.0, 3.0], // Resized: DX=2, DY=1, DZ=3
            PrismFaceColors::uniform_with_outline(color, [0.0, 0.0, 0.0, 0.0]),
            PrismTextureLayers::new(1, 2, 3),
        );

        let check_tiled_uv =
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

        // Top face of tiled block: DX=2, DZ=3
        check_tiled_uv(
            &tiled_vertices,
            [2.0, 1.0, 0.0],
            [2.0, 0.0],
            "Tiled Top Top-Right",
        );
        check_tiled_uv(
            &tiled_vertices,
            [2.0, 1.0, 3.0],
            [2.0, 3.0],
            "Tiled Top Bottom-Right",
        );

        // Side face (+Z): DX=2, DY=1
        check_tiled_uv(
            &tiled_vertices,
            [2.0, 1.0, 3.0],
            [2.0, 0.0],
            "Tiled +Z Top-Right",
        );
        check_tiled_uv(
            &tiled_vertices,
            [2.0, 0.0, 3.0],
            [2.0, 1.0],
            "Tiled +Z Bottom-Right",
        );
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

        // Specifically check uv_norm for each vertex group (6 vertices per face)
        // Order: Top (+Y), +X, -X, +Z, -Z, Bottom (-Y)

        // Top (+Y) face: Size should be [2.5, 3.5] (DX, DZ)
        for i in 0..6 {
            assert_eq!(vertices[i].uv_norm, [2.5, 3.5]);
        }

        // +X face: Size should be [3.5, 1.0] (DZ, DY)
        for i in 6..12 {
            assert_eq!(vertices[i].uv_norm, [3.5, 1.0]);
        }

        // -X face: Size should be [3.5, 1.0] (DZ, DY)
        for i in 12..18 {
            assert_eq!(vertices[i].uv_norm, [3.5, 1.0]);
        }

        // +Z face: Size should be [2.5, 1.0] (DX, DY)
        for i in 18..24 {
            assert_eq!(vertices[i].uv_norm, [2.5, 1.0]);
        }

        // -Z face: Size should be [2.5, 1.0] (DX, DY)
        for i in 24..30 {
            assert_eq!(vertices[i].uv_norm, [2.5, 1.0]);
        }

        // Bottom (-Y) face: Size should be [2.5, 3.5] (DX, DZ)
        for i in 30..36 {
            assert_eq!(vertices[i].uv_norm, [2.5, 3.5]);
        }
    }
}
