/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERCIAL.md for details.

 */
struct CameraData {
    view_proj: mat4x4<f32>,
};

struct LineData {
    offset: vec2<f32>,
    rotation: f32,
    _pad: f32,
};

struct ColorSpaceData {
    flags: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u_camera: CameraData;

@group(1) @binding(0)
var<uniform> u_line: LineData;

@group(2) @binding(0)
var<uniform> u_color_space: ColorSpaceData;

@group(3) @binding(0)
var u_block_textures: texture_2d_array<f32>;

@group(3) @binding(1)
var u_block_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_norm: vec2<f32>,
    @location(4) texture_layer: f32,
    @location(5) color_outline: vec4<f32>,
    @location(6) render_profile: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv_norm: vec2<f32>,
    @location(3) texture_layer: f32,
    @location(4) color_outline: vec4<f32>,
    @location(5) render_profile: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var world_pos = input.position;
    let finish_ring_profile = step(1.5, input.render_profile) * (1.0 - step(2.5, input.render_profile));
    if finish_ring_profile > 0.5 {
        let center_xz = input.color_outline.xy;
        let phase_offset = input.color_outline.z;
        let pulse = 1.0 + 0.14 * sin(u_color_space.flags.y * 5.0 + phase_offset);
        let pulsed_xz = center_xz + (world_pos.xz - center_xz) * pulse;
        world_pos = vec3<f32>(pulsed_xz.x, world_pos.y, pulsed_xz.y);
    }

    let c = cos(u_line.rotation);
    let s = sin(u_line.rotation);
    let rotated_pos = vec3<f32>(
        world_pos.x * c - world_pos.z * s,
        world_pos.y,
        world_pos.x * s + world_pos.z * c
    );

    let offset = vec3<f32>(u_line.offset.x, 0.0, u_line.offset.y);
    var clip_position = u_camera.view_proj * vec4<f32>(rotated_pos + offset, 1.0);

    let editor_outline_profile = step(2.5, input.render_profile)
        * (1.0 - step(3.5, input.render_profile));
    if editor_outline_profile > 0.5 {
        let anchor_world = input.color_outline.xyz;
        let anchor_rotated = vec3<f32>(
            anchor_world.x * c - anchor_world.z * s,
            anchor_world.y,
            anchor_world.x * s + anchor_world.z * c
        );
        let anchor_clip = u_camera.view_proj * vec4<f32>(anchor_rotated + offset, 1.0);
        let viewport = max(u_color_space.flags.zw, vec2<f32>(1.0, 1.0));
        let clip_w = clip_position.w;
        let anchor_w = anchor_clip.w;
        if abs(clip_w) > 1e-6 && abs(anchor_w) > 1e-6 {
            let direction_pixels = ((clip_position.xy / clip_w)
                - (anchor_clip.xy / anchor_w)) * viewport * 0.5;
            let direction_length = length(direction_pixels);
            if direction_length > 0.0001 {
                let pixel_offset = direction_pixels / direction_length * input.color_outline.w;
                let clip_offset = (pixel_offset
                    * vec2<f32>(2.0 / viewport.x, 2.0 / viewport.y)) * clip_w;
                clip_position = vec4<f32>(
                    clip_position.x + clip_offset.x,
                    clip_position.y + clip_offset.y,
                    clip_position.z,
                    clip_position.w
                );
            }
        }
    }

    out.position = clip_position;
    out.color = input.color;
    out.uv = input.uv;
    out.uv_norm = input.uv_norm;
    out.texture_layer = input.texture_layer;
    out.color_outline = input.color_outline;
    out.render_profile = input.render_profile;
    return out;
}

@fragment
fn fs_mask() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let has_texture = input.texture_layer >= 0.0;
    let texture_layer = max(i32(round(input.texture_layer)), 0);
    let liquid_profile = step(0.5, input.render_profile) * (1.0 - step(1.5, input.render_profile));
    let wave_a = sin((input.uv.x * 8.0 + input.uv.y * 11.0) + u_color_space.flags.y * 2.8);
    let wave_b = cos((input.uv.x * 13.0 - input.uv.y * 7.0) - u_color_space.flags.y * 3.6);
    let liquid_uv_offset = vec2<f32>(wave_a, wave_b) * (0.017 * liquid_profile);
    let sampled_texture = textureSample(
        u_block_textures,
        u_block_sampler,
        input.uv + liquid_uv_offset,
        texture_layer
    );
    let texture_sample = select(vec4<f32>(1.0, 1.0, 1.0, 1.0), sampled_texture, has_texture);
    var color = (input.color * texture_sample).rgb;

    let molten_glow = (wave_a * 0.5 + 0.5) * 0.6 + (wave_b * 0.5 + 0.5) * 0.4;
    let glow_rgb = vec3<f32>(1.0, 0.42, 0.08) * molten_glow * 0.38;
    color = mix(color, color * 0.78 + glow_rgb, liquid_profile * 0.68);

    let face_size = input.uv_norm;
    if face_size.x > 0.0 && face_size.y > 0.0 {
        let base_thickness = 0.05;
        let liquid_thickness = 0.05;
        let thickness = min(
            mix(base_thickness, liquid_thickness, liquid_profile),
            min(face_size.x, face_size.y) * 0.45
        );
        let edge_x = step(input.uv.x, thickness) + step(face_size.x - thickness, input.uv.x);
        let edge_y = step(input.uv.y, thickness) + step(face_size.y - thickness, input.uv.y);
        let is_edge = clamp(edge_x + edge_y, 0.0, 1.0);
        let outline_alpha = clamp(input.color_outline.a * mix(1.0, 3.0, liquid_profile), 0.0, 1.0);

        color = mix(color, input.color_outline.rgb, is_edge * outline_alpha);
    }

    if u_color_space.flags.x > 0.5 {
        color = linear_to_srgb(color);
    }

    return vec4<f32>(color, input.color.a * texture_sample.a);
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let threshold = vec3<f32>(0.0031308);
    let lo = 12.92 * value;
    let hi = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(lo, hi, value > threshold);
}
