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
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv_norm: vec2<f32>,
    @location(3) texture_layer: f32,
    @location(4) color_outline: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let c = cos(u_line.rotation);
    let s = sin(u_line.rotation);
    let rotated_pos = vec3<f32>(
        input.position.x * c - input.position.z * s,
        input.position.y,
        input.position.x * s + input.position.z * c
    );
    
    let offset = vec3<f32>(u_line.offset.x, 0.0, u_line.offset.y);
    out.position = u_camera.view_proj * vec4<f32>(rotated_pos + offset, 1.0);
    out.color = input.color;
    out.uv = input.uv;
    out.uv_norm = input.uv_norm;
    out.texture_layer = input.texture_layer;
    out.color_outline = input.color_outline;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let has_texture = input.texture_layer >= 0.0;
    let texture_layer = max(i32(round(input.texture_layer)), 0);
    let sampled_texture = textureSample(u_block_textures, u_block_sampler, input.uv, texture_layer);
    let texture_sample = select(vec4<f32>(1.0, 1.0, 1.0, 1.0), sampled_texture, has_texture);
    var color = (input.color * texture_sample).rgb;

    let face_size = input.uv_norm;
    if (face_size.x > 0.0 && face_size.y > 0.0) {
        let thickness = 0.05;
        let edge_x = step(input.uv.x, thickness) + step(face_size.x - thickness, input.uv.x);
        let edge_y = step(input.uv.y, thickness) + step(face_size.y - thickness, input.uv.y);
        let is_edge = clamp(edge_x + edge_y, 0.0, 1.0);

        color = mix(color, input.color_outline.rgb, is_edge * input.color_outline.a);
    }

    if (u_color_space.flags.x > 0.5) {
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
