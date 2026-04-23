/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::block_repository::{
    resolve_block_definition, resolve_block_texture_layers, BlockRenderProfile,
};
use crate::mesh::noise::pseudo_random_noise;
use crate::mesh::obj::{append_obj_mesh, resolve_obj_mesh};
use crate::mesh::shapes::{append_prism_with_layers, PrismFaceColors, PrismTextureLayers};
use crate::mesh::transforms::rotate_vertices_around_euler;
use crate::types::{LevelObject, Vertex};

const TORCH_BLOCK_ID: &str = "core/torch";
const TORCH_LIGHT_RADIUS: f32 = 3.25;
const TORCH_GLOW_STRENGTH: f32 = 0.48;
const TORCH_FLICKER_BASE: f32 = 0.85;
const TORCH_FLICKER_AMPLITUDE: f32 = 0.15;
const TORCH_FLICKER_FREQUENCY: f32 = 9.0;
const TORCH_PHASE_OFFSET_X: f32 = 1.37;
const TORCH_PHASE_OFFSET_Z: f32 = 0.91;
const TORCH_WARMTH_RGB: [f32; 3] = [1.0, 0.78, 0.42];

pub(crate) fn build_block_vertices(objects: &[LevelObject]) -> Vec<Vertex> {
    build_block_vertices_with_phase_impl(objects.iter(), 0.0)
}

pub(crate) fn build_block_vertices_from_refs(objects: &[&LevelObject]) -> Vec<Vertex> {
    build_block_vertices_with_phase_impl(objects.iter().copied(), 0.0)
}

pub(crate) fn build_block_vertices_with_phase(
    objects: &[LevelObject],
    pulse_phase_seconds: f32,
) -> Vec<Vertex> {
    build_block_vertices_with_phase_impl(objects.iter(), pulse_phase_seconds)
}

fn build_block_vertices_with_phase_impl<'a, I>(objects: I, pulse_phase_seconds: f32) -> Vec<Vertex>
where
    I: Iterator<Item = &'a LevelObject> + Clone,
{
    const LIQUID_PROFILE_TAG: f32 = 1.0;
    let mut all_vertices = Vec::new();
    let torch_emitters: Vec<([f32; 3], f32)> = objects
        .clone()
        .filter_map(|obj| {
            if !is_torch_block_id(&obj.block_id) {
                return None;
            }
            let center = [
                obj.position[0] + obj.size[0] * 0.5,
                obj.position[1] + obj.size[1] * 0.5,
                obj.position[2] + obj.size[2] * 0.5,
            ];
            let phase_offset = center[0] * TORCH_PHASE_OFFSET_X + center[2] * TORCH_PHASE_OFFSET_Z;
            let flicker = TORCH_FLICKER_BASE
                + TORCH_FLICKER_AMPLITUDE
                    * (pulse_phase_seconds * TORCH_FLICKER_FREQUENCY + phase_offset).sin();
            Some((center, flicker.max(0.0)))
        })
        .collect();

    for obj in objects {
        let mut object_vertices = Vec::new();
        let vertices = &mut object_vertices;
        let object_vertex_start = vertices.len();

        let x_min = obj.position[0];
        let x_max = obj.position[0] + obj.size[0];
        let y_min = obj.position[1];
        let y_max = obj.position[1] + obj.size[1];
        let z_min = obj.position[2];
        let z_max = obj.position[2] + obj.size[2];

        let block = resolve_block_definition(&obj.block_id);
        let texture_layers = resolve_block_texture_layers(&obj.block_id);

        let mut color_top = block.render.color_top;
        let mut color_side = block.render.color_side;
        let mut color_bottom = block.render.color_bottom;
        let mut color_outline = block.render.color_outline;

        if block.render.noise.abs() > f32::EPSILON {
            let noise = pseudo_random_noise(obj.position[0], obj.position[1], obj.position[2]);
            let factor = (noise * 2.0 - 1.0) * block.render.noise;
            for i in 0..3 {
                color_top[i] = (color_top[i] + factor).clamp(0.0, 1.0);
                color_side[i] = (color_side[i] + factor).clamp(0.0, 1.0);
                color_bottom[i] = (color_bottom[i] + factor).clamp(0.0, 1.0);
            }
        }

        color_top = apply_color_tint(color_top, obj.color_tint);
        color_side = apply_color_tint(color_side, obj.color_tint);
        color_bottom = apply_color_tint(color_bottom, obj.color_tint);
        color_outline = apply_color_tint(color_outline, obj.color_tint);

        let center = [
            obj.position[0] + obj.size[0] * 0.5,
            obj.position[1] + obj.size[1] * 0.5,
            obj.position[2] + obj.size[2] * 0.5,
        ];
        let torch_light_factor = torch_emitters
            .iter()
            .map(|(torch_center, flicker)| {
                let dx = center[0] - torch_center[0];
                let dy = center[1] - torch_center[1];
                let dz = center[2] - torch_center[2];
                let distance_sq = dx * dx + dy * dy + dz * dz;
                let radius_sq = TORCH_LIGHT_RADIUS * TORCH_LIGHT_RADIUS;
                let falloff = (1.0 - distance_sq / radius_sq).max(0.0);
                falloff * falloff * *flicker
            })
            .fold(0.0_f32, f32::max);
        if block.id != TORCH_BLOCK_ID && torch_light_factor > f32::EPSILON {
            color_top = apply_torch_light(color_top, torch_light_factor * TORCH_GLOW_STRENGTH);
            color_side = apply_torch_light(color_side, torch_light_factor * TORCH_GLOW_STRENGTH);
            color_bottom =
                apply_torch_light(color_bottom, torch_light_factor * TORCH_GLOW_STRENGTH);
        }

        if let Some(mesh_path) = block.assets.mesh.as_deref() {
            if let Some(mesh) = resolve_obj_mesh(mesh_path) {
                append_obj_mesh(vertices, obj, mesh, color_top, texture_layers.side);
            }
        }

        if vertices.is_empty() && matches!(block.render.profile, BlockRenderProfile::FinishRing) {
            append_finish_ring(
                vertices,
                obj,
                color_top,
                color_outline,
                pulse_phase_seconds,
                texture_layers.side,
            );
        } else if vertices.is_empty() {
            let prism_colors = PrismFaceColors::new_with_outline(
                color_top,
                color_side,
                color_bottom,
                color_outline,
            );

            append_prism_with_layers(
                vertices,
                [x_min, y_min, z_min],
                [x_max, y_max, z_max],
                prism_colors,
                PrismTextureLayers::new(
                    texture_layers.top,
                    texture_layers.side,
                    texture_layers.bottom,
                ),
            );
        }

        if matches!(block.render.profile, BlockRenderProfile::Liquid) {
            for vertex in vertices.iter_mut().skip(object_vertex_start) {
                vertex.set_render_profile(LIQUID_PROFILE_TAG);
            }
        }

        rotate_vertices_around_euler(&mut object_vertices, center, obj.rotation_degrees);
        all_vertices.extend(object_vertices);
    }

    all_vertices
}

fn is_torch_block_id(block_id: &str) -> bool {
    block_id.trim().eq_ignore_ascii_case(TORCH_BLOCK_ID)
}

fn apply_torch_light(color: [f32; 4], strength: f32) -> [f32; 4] {
    [
        (color[0] + TORCH_WARMTH_RGB[0] * strength).clamp(0.0, 1.0),
        (color[1] + TORCH_WARMTH_RGB[1] * strength).clamp(0.0, 1.0),
        (color[2] + TORCH_WARMTH_RGB[2] * strength).clamp(0.0, 1.0),
        color[3],
    ]
}

fn apply_color_tint(color: [f32; 4], tint_rgb: [f32; 3]) -> [f32; 4] {
    let (tint_hue, tint_sat, _) = rgb_to_hsv(tint_rgb[0], tint_rgb[1], tint_rgb[2]);
    if tint_sat <= 1e-4 {
        return color;
    }

    let (.., source_sat, source_val) = rgb_to_hsv(color[0], color[1], color[2]);
    let (r, g, b) = hsv_to_rgb(tint_hue, source_sat, source_val);
    [r, g, b, color[3]]
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if (max - g).abs() <= f32::EPSILON {
        (((b - r) / delta) + 2.0) / 6.0
    } else {
        (((r - g) / delta) + 4.0) / 6.0
    };
    let sat = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, sat, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h_prime = h.rem_euclid(1.0) * 6.0;
    let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h_prime {
        h if (0.0..1.0).contains(&h) => (c, x, 0.0),
        h if (1.0..2.0).contains(&h) => (x, c, 0.0),
        h if (2.0..3.0).contains(&h) => (0.0, c, x),
        h if (3.0..4.0).contains(&h) => (0.0, x, c),
        h if (4.0..5.0).contains(&h) => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
}

fn append_finish_ring(
    vertices: &mut Vec<Vertex>,
    obj: &LevelObject,
    color_outer: [f32; 4],
    color_inner: [f32; 4],
    pulse_phase_seconds: f32,
    texture_layer: u32,
) {
    const SEGMENTS: usize = 28;
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
        obj.position[2] + obj.size[2] * 0.5,
    ];

    let phase_offset = (obj.position[0] * 0.37 + obj.position[2] * 0.21) * std::f32::consts::PI;
    let pulse = 1.0 + 0.14 * (pulse_phase_seconds * 5.0 + phase_offset).sin();

    let base_radius = (obj.size[0].min(obj.size[2]) * 0.5 * 0.85).max(0.15);
    let outer_radius = base_radius * pulse;
    let inner_radius = (outer_radius * 0.56).max(0.08);
    let half_thickness = (obj.size[1] * 0.16).clamp(0.03, 0.14);
    let y_top = center[1] + half_thickness;
    let y_bottom = center[1] - half_thickness;

    let mut funnel_color = color_inner;
    funnel_color[3] = (funnel_color[3] * 0.72).clamp(0.0, 1.0);
    let sink_point = [
        center[0],
        obj.position[1] - obj.size[1] * 0.9 - 0.25,
        center[2],
    ];

    for index in 0..SEGMENTS {
        let t0 = index as f32 / SEGMENTS as f32;
        let t1 = (index + 1) as f32 / SEGMENTS as f32;
        let a0 = t0 * std::f32::consts::TAU;
        let a1 = t1 * std::f32::consts::TAU;

        let (cos0, sin0) = (a0.cos(), a0.sin());
        let (cos1, sin1) = (a1.cos(), a1.sin());

        let outer_top_0 = [
            center[0] + cos0 * outer_radius,
            y_top,
            center[2] + sin0 * outer_radius,
        ];
        let outer_top_1 = [
            center[0] + cos1 * outer_radius,
            y_top,
            center[2] + sin1 * outer_radius,
        ];
        let inner_top_0 = [
            center[0] + cos0 * inner_radius,
            y_top,
            center[2] + sin0 * inner_radius,
        ];
        let inner_top_1 = [
            center[0] + cos1 * inner_radius,
            y_top,
            center[2] + sin1 * inner_radius,
        ];

        let outer_bottom_0 = [
            center[0] + cos0 * outer_radius,
            y_bottom,
            center[2] + sin0 * outer_radius,
        ];
        let outer_bottom_1 = [
            center[0] + cos1 * outer_radius,
            y_bottom,
            center[2] + sin1 * outer_radius,
        ];
        let inner_bottom_0 = [
            center[0] + cos0 * inner_radius,
            y_bottom,
            center[2] + sin0 * inner_radius,
        ];
        let inner_bottom_1 = [
            center[0] + cos1 * inner_radius,
            y_bottom,
            center[2] + sin1 * inner_radius,
        ];

        push_triangle(
            vertices,
            outer_top_0,
            outer_top_1,
            inner_top_1,
            color_outer,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_top_0,
            inner_top_1,
            inner_top_0,
            color_outer,
            texture_layer,
        );

        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_1,
            outer_bottom_1,
            color_outer,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            inner_bottom_0,
            inner_bottom_1,
            color_outer,
            texture_layer,
        );

        push_triangle(
            vertices,
            outer_bottom_0,
            outer_bottom_1,
            outer_top_1,
            color_inner,
            texture_layer,
        );
        push_triangle(
            vertices,
            outer_bottom_0,
            outer_top_1,
            outer_top_0,
            color_inner,
            texture_layer,
        );

        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_1,
            inner_bottom_1,
            color_inner,
            texture_layer,
        );
        push_triangle(
            vertices,
            inner_bottom_0,
            inner_top_0,
            inner_top_1,
            color_inner,
            texture_layer,
        );

        if index % 2 == 0 {
            push_triangle(
                vertices,
                inner_bottom_0,
                inner_bottom_1,
                sink_point,
                funnel_color,
                texture_layer,
            );
        }
    }
}

fn push_triangle(
    vertices: &mut Vec<Vertex>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 4],
    texture_layer: u32,
) {
    vertices.push(Vertex::textured(p0, color, [0.0, 0.0], texture_layer));
    vertices.push(Vertex::textured(p1, color, [1.0, 0.0], texture_layer));
    vertices.push(Vertex::textured(p2, color, [0.5, 1.0], texture_layer));
}
