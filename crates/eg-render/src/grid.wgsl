/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERCIAL.md for details.

 */
struct CameraData {
    view_proj: mat4x4<f32>,
};

struct GridData {
    center: vec2<f32>,
    half_extent: f32,
    darkening: f32,
};

struct ColorSpaceData {
    flags: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u_camera: CameraData;

@group(1) @binding(0)
var<uniform> u_grid: GridData;

@group(2) @binding(0)
var<uniform> u_color_space: ColorSpaceData;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_xz: vec2<f32>,
    @location(1) local_xz: vec2<f32>,
};

@vertex
fn vs_grid(input: VertexInput) -> VertexOutput {
    let local_xz = input.position.xz * u_grid.half_extent;
    let world_xz = u_grid.center + local_xz;
    let world_position = vec3<f32>(world_xz.x, 0.012, world_xz.y);

    var out: VertexOutput;
    out.position = u_camera.view_proj * vec4<f32>(world_position, 1.0);
    out.world_xz = world_xz;
    out.local_xz = local_xz;
    return out;
}

@fragment
fn fs_grid(input: VertexOutput) -> @location(0) vec4<f32> {
    let minor = grid_alpha(input.world_xz, 1.0, 0.012);
    let major = grid_alpha(input.world_xz, 4.0, 0.026);
    let axis = axis_alpha(input.world_xz, 0.04);

    let minor_color = vec3<f32>(0.18, 0.20, 0.24);
    let major_color = vec3<f32>(0.27, 0.31, 0.37);
    let axis_color = vec3<f32>(0.38, 0.54, 0.66);

    let line_alpha = max(max(minor * 0.34, major * 0.58), axis * 0.72);
    var color = mix(minor_color, major_color, major);
    color = mix(color, axis_color, axis);

    let edge_distance = max(abs(input.local_xz.x), abs(input.local_xz.y)) / u_grid.half_extent;
    let edge_fade = 1.0 - smoothstep(0.92, 1.0, edge_distance);
    let floor_alpha = u_grid.darkening * edge_fade;
    let line_layer_alpha = line_alpha * edge_fade;
    let alpha = line_layer_alpha + floor_alpha * (1.0 - line_layer_alpha);
    color = color * line_layer_alpha / max(alpha, 1e-5);

    if u_color_space.flags.x > 0.5 {
        color = linear_to_srgb(color);
    }

    return vec4<f32>(color, alpha);
}

fn grid_alpha(world_xz: vec2<f32>, spacing: f32, half_width: f32) -> f32 {
    let coord = world_xz / spacing;
    let grid = abs(fract(coord - vec2<f32>(0.5)) - vec2<f32>(0.5));
    let distance_to_line = min(grid.x, grid.y) * spacing;
    return stroke_alpha(distance_to_line, half_width, world_pixel_radius(world_xz));
}

fn axis_alpha(world_xz: vec2<f32>, half_width: f32) -> f32 {
    let distance_to_axis = min(abs(world_xz.x), abs(world_xz.y));
    return stroke_alpha(distance_to_axis, half_width, world_pixel_radius(world_xz));
}

fn stroke_alpha(distance_to_line: f32, half_width: f32, pixel_radius: f32) -> f32 {
    return clamp(
        (half_width + pixel_radius - distance_to_line) / (2.0 * pixel_radius),
        0.0,
        1.0
    );
}

fn world_pixel_radius(world_xz: vec2<f32>) -> f32 {
    return max(max(fwidth(world_xz.x), fwidth(world_xz.y)) * 0.5, 1e-5);
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let threshold = vec3<f32>(0.0031308);
    let lo = 12.92 * value;
    let hi = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(lo, hi, value > threshold);
}