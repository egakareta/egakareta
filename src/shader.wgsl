struct CameraData {
    view_proj: mat4x4<f32>,
};

struct LineData {
    offset: vec2<f32>,
    rotation: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> u_camera: CameraData;

@group(1) @binding(0)
var<uniform> u_line: LineData;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let c = cos(u_line.rotation);
    let s = sin(u_line.rotation);
    let rotated_pos = vec3<f32>(
        input.position.x * c - input.position.y * s,
        input.position.x * s + input.position.y * c,
        input.position.z
    );
    
    let offset = vec3<f32>(u_line.offset, 0.0);
    out.position = u_camera.view_proj * vec4<f32>(rotated_pos + offset, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
