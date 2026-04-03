/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERICAL.md for details.

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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
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
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var color = input.color.rgb;

    if (u_color_space.flags.x > 0.5) {
        color = linear_to_srgb(color);
    }

    return vec4<f32>(color, input.color.a);
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let threshold = vec3<f32>(0.0031308);
    let lo = 12.92 * value;
    let hi = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(lo, hi, value > threshold);
}
