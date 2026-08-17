/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERCIAL.md for details.

 */
#import bevy_pbr::{
    mesh_functions,
}

@group(3) @binding(0)
var block_textures: texture_2d_array<f32>;

@group(3) @binding(1)
var block_sampler: sampler;

struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_norm: vec2<f32>,
    @location(4) texture_layer: f32,
    @location(5) color_outline: vec4<f32>,
    @location(6) render_profile: f32,
    @location(7) animation_anchor: vec4<f32>,
    @location(8) animation_params: vec4<f32>,
    @location(9) animation_params2: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) texture_layer: f32,
    @location(3) color_outline: vec4<f32>,
    @location(4) render_profile: f32,
};

fn animate_position(input: VertexInput) -> vec3<f32> {
    let anchor = input.animation_anchor.xyz;
    let phase = input.animation_anchor.w;
    let bob_amplitude = input.animation_params.w;
    let bob_frequency = input.animation_params2.x;
    let bob_axis = round(input.animation_params.z);
    let bob = sin(phase) * bob_amplitude;
    var result = input.position;
    if bob_axis > 1.5 && bob_axis < 2.5 {
        result.y += bob;
    }
    // Preserve an explicit dependency on the anchor for future spin/pulse
    // parity without changing unanimated block positions today.
    return result + (anchor - anchor);
}

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = mesh_functions::mesh_position_local_to_clip(
        mesh_functions::get_world_from_local(input.instance_index),
        vec4<f32>(animate_position(input), 1.0),
    );
    out.color = input.color;
    out.uv = input.uv;
    out.texture_layer = input.texture_layer;
    out.color_outline = input.color_outline;
    out.render_profile = input.render_profile;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let texture_layer = max(0, i32(round(input.texture_layer)));
    let texture_sample = textureSample(
        block_textures,
        block_sampler,
        input.uv,
        texture_layer,
    );
    let base_color = select(
        input.color,
        input.color * texture_sample,
        input.texture_layer >= 0.0,
    );
    let outline_profile = step(2.5, input.render_profile) * (1.0 - step(3.5, input.render_profile));
    let color = mix(base_color, input.color, outline_profile);
    if color.a < 0.004 {
        discard;
    }
    return color;
}
