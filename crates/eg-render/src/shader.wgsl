/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERCIAL.md for details.

 */
struct CameraData {
    view_proj: mat4x4<f32>,
    camera_position: vec4<f32>,
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
    @location(7) animation_anchor: vec4<f32>,
    @location(8) animation_params: vec4<f32>,
    @location(9) animation_params2: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv_norm: vec2<f32>,
    @location(3) texture_layer: f32,
    @location(4) color_outline: vec4<f32>,
    @location(5) render_profile: f32,
    @location(6) world_position: vec3<f32>,
};

// Per-instance block data for the instanced rendering path. `model_row_*` hold the
// rows of a 3x4 affine local->world transform; `color_tint_and_noise` packs the
// object color tint (xyz) and a pre-computed additive noise factor (w); and
// `animation_anchor` provides the world-space idle-animation pivot and phase seed.
struct InstanceInput {
    @location(10) model_row_0: vec4<f32>,
    @location(11) model_row_1: vec4<f32>,
    @location(12) model_row_2: vec4<f32>,
    @location(13) color_tint_and_noise: vec4<f32>,
    @location(14) animation_anchor: vec4<f32>,
    @location(15) trigger_params: vec4<f32>,
};

// Apply idle spin/bob/pulse animation around a world-space anchor. Shared by the
// per-vertex and instanced vertex paths so motion stays identical.
fn animate_world_position(
    world_in: vec3<f32>,
    animation_anchor: vec4<f32>,
    animation_params: vec4<f32>,
    animation_params2: vec4<f32>,
) -> vec3<f32> {
    let animation_strength = abs(animation_params.y)
        + abs(animation_params.w)
        + abs(animation_params2.y);
    if animation_strength <= 0.000001 {
        return world_in;
    }

    let anchor = animation_anchor.xyz;
    var local = world_in - anchor;
    let phase = animation_anchor.w;

    let pulse = sin(u_color_space.flags.y * animation_params2.z + phase)
        * animation_params2.y;
    local = local * max(0.05, 1.0 + pulse);

    let spin_axis = round(animation_params.x);
    let angle = u_color_space.flags.y * animation_params.y + phase;
    let c_spin = cos(angle);
    let s_spin = sin(angle);
    if spin_axis > 0.5 && spin_axis < 1.5 {
        local = vec3<f32>(
            local.x,
            local.y * c_spin - local.z * s_spin,
            local.y * s_spin + local.z * c_spin
        );
    } else if spin_axis > 1.5 && spin_axis < 2.5 {
        local = vec3<f32>(
            local.x * c_spin - local.z * s_spin,
            local.y,
            local.x * s_spin + local.z * c_spin
        );
    } else if spin_axis > 2.5 && spin_axis < 3.5 {
        local = vec3<f32>(
            local.x * c_spin - local.y * s_spin,
            local.x * s_spin + local.y * c_spin,
            local.z
        );
    }

    var bob = vec3<f32>(0.0, 0.0, 0.0);
    let bob_axis = round(animation_params.z);
    let bob_amount = sin(u_color_space.flags.y * animation_params2.x + phase)
        * animation_params.w;
    if bob_axis > 0.5 && bob_axis < 1.5 {
        bob.x = bob_amount;
    } else if bob_axis > 1.5 && bob_axis < 2.5 {
        bob.y = bob_amount;
    } else if bob_axis > 2.5 && bob_axis < 3.5 {
        bob.z = bob_amount;
    }

    return anchor + local + bob;
}

// Apply the shared line rotation/offset and project to clip space. Returns both the
// clip-space position and the rotated world position used for fragment effects.
fn world_to_clip(world_position: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    let c = cos(u_line.rotation);
    let s = sin(u_line.rotation);
    let rotated_pos = vec3<f32>(
        world_position.x * c - world_position.z * s,
        world_position.y,
        world_position.x * s + world_position.z * c
    );
    let offset = vec3<f32>(u_line.offset.x, 0.0, u_line.offset.y);
    out.position = u_camera.view_proj * vec4<f32>(rotated_pos + offset, 1.0);
    out.world_position = rotated_pos + offset;
    return out;
}

@vertex
fn vs_instanced(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Transform template-local geometry into world space via the instance affine.
    let local = vec4<f32>(input.position, 1.0);
    let world_local = vec3<f32>(
        dot(instance.model_row_0, local),
        dot(instance.model_row_1, local),
        dot(instance.model_row_2, local)
    );

    let camera_trigger_profile = step(5.5, input.render_profile)
        * (1.0 - step(6.5, input.render_profile));
    let camera_trigger_ring = step(0.5, input.color_outline.w);
    let ring_scale = max(0.33, 1.0 - instance.trigger_params.x * 0.67);
    let local_center = vec4<f32>(0.5, 0.5, 0.5, 1.0);
    let world_center = vec3<f32>(
        dot(instance.model_row_0, local_center),
        dot(instance.model_row_1, local_center),
        dot(instance.model_row_2, local_center)
    );
    let adjusted_world_local = select(
        world_local,
        world_center + (world_local - world_center) * ring_scale,
        camera_trigger_profile * camera_trigger_ring > 0.5
    );

    let animated = animate_world_position(
        adjusted_world_local,
        instance.animation_anchor,
        input.animation_params,
        input.animation_params2
    );

    var out = world_to_clip(animated);

    // Reproduce the CPU color pipeline: additive noise (uniform across faces) then
    // multiplicative object tint. Noise is not applied to the outline color.
    let tint = clamp(instance.color_tint_and_noise.xyz, vec3<f32>(0.0), vec3<f32>(1.0));
    let noise_factor = instance.color_tint_and_noise.w;
    let base_rgb = clamp(input.color.rgb + noise_factor, vec3<f32>(0.0), vec3<f32>(1.0));
    out.color = vec4<f32>(base_rgb * tint, input.color.a);
    out.uv = input.uv;
    out.uv_norm = input.uv_norm;
    out.texture_layer = input.texture_layer;
    let outline_anchor_local = vec4<f32>(input.color_outline.xyz, 1.0);
    let outline_anchor_world = vec3<f32>(
        dot(instance.model_row_0, outline_anchor_local),
        dot(instance.model_row_1, outline_anchor_local),
        dot(instance.model_row_2, outline_anchor_local)
    );
    let outline_rgb = mix(
        input.color_outline.rgb * tint,
        outline_anchor_world,
        camera_trigger_profile
    );
    out.color_outline = vec4<f32>(outline_rgb, input.color_outline.a);
    out.render_profile = input.render_profile;
    return out;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var world_position = input.position;
    let animation_strength = abs(input.animation_params.y)
        + abs(input.animation_params.w)
        + abs(input.animation_params2.y);
    if animation_strength > 0.000001 {
        let anchor = input.animation_anchor.xyz;
        var local = input.position - anchor;
        let phase = input.animation_anchor.w;

        let pulse = sin(u_color_space.flags.y * input.animation_params2.z + phase)
            * input.animation_params2.y;
        local = local * max(0.05, 1.0 + pulse);

        let spin_axis = round(input.animation_params.x);
        let angle = u_color_space.flags.y * input.animation_params.y + phase;
        let c_spin = cos(angle);
        let s_spin = sin(angle);
        if spin_axis > 0.5 && spin_axis < 1.5 {
            local = vec3<f32>(
                local.x,
                local.y * c_spin - local.z * s_spin,
                local.y * s_spin + local.z * c_spin
            );
        } else if spin_axis > 1.5 && spin_axis < 2.5 {
            local = vec3<f32>(
                local.x * c_spin - local.z * s_spin,
                local.y,
                local.x * s_spin + local.z * c_spin
            );
        } else if spin_axis > 2.5 && spin_axis < 3.5 {
            local = vec3<f32>(
                local.x * c_spin - local.y * s_spin,
                local.x * s_spin + local.y * c_spin,
                local.z
            );
        }

        var bob = vec3<f32>(0.0, 0.0, 0.0);
        let bob_axis = round(input.animation_params.z);
        let bob_amount = sin(u_color_space.flags.y * input.animation_params2.x + phase)
            * input.animation_params.w;
        if bob_axis > 0.5 && bob_axis < 1.5 {
            bob.x = bob_amount;
        } else if bob_axis > 1.5 && bob_axis < 2.5 {
            bob.y = bob_amount;
        } else if bob_axis > 2.5 && bob_axis < 3.5 {
            bob.z = bob_amount;
        }

        world_position = anchor + local + bob;
    }

    let c = cos(u_line.rotation);
    let s = sin(u_line.rotation);
    let rotated_pos = vec3<f32>(
        world_position.x * c - world_position.z * s,
        world_position.y,
        world_position.x * s + world_position.z * c
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
    out.world_position = rotated_pos + offset;
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
    let gem_profile = step(3.5, input.render_profile) * (1.0 - step(4.5, input.render_profile));
    let camera_trigger_profile = step(5.5, input.render_profile) * (1.0 - step(6.5, input.render_profile));
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

    let gem_spark = pow(max(wave_a * 0.5 + wave_b * 0.5 + 0.5, 0.0), 3.0);
    let gem_prism = vec3<f32>(0.10, 0.95, 1.0) * (1.0 - input.uv.y)
        + vec3<f32>(1.0, 0.28, 0.95) * input.uv.y;
    color = mix(color, color * 0.72 + gem_prism * (0.62 + gem_spark * 0.55), gem_profile);

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

    let camera_trigger_distance = distance(u_camera.camera_position.xyz, input.color_outline.xyz);
    let camera_trigger_fade = smoothstep(0.75, 3.0, camera_trigger_distance);
    let profile_fade = mix(1.0, camera_trigger_fade, camera_trigger_profile);
    let final_color = vec4<f32>(color, input.color.a * texture_sample.a * profile_fade);
    if final_color.a < 0.004 {
        discard;
    }
    return final_color;
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let threshold = vec3<f32>(0.0031308);
    let lo = 12.92 * value;
    let hi = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(lo, hi, value > threshold);
}
