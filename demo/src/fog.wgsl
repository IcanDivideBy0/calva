struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

//
// Vertex shader
//


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
    out.ndc = out.position.xy;
    out.uv = out.ndc * vec2<f32>(0.5, -0.5) + 0.5;

    return out;
}

//
// Fragment shader
//

@group(1) @binding(0) var t_sampler: sampler;
@group(1) @binding(1) var t_noise: texture_3d<f32>;

var<push_constant> time: f32;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = time / 3.0;
    let uvw = vec3<f32>(in.uv, 0.0) - vec3<f32>(t, t, t);

    let noise = textureSample(t_noise, t_sampler, uvw * 3.0).r;

    var color = vec3<f32>(noise);

    return vec4<f32>(color, 1.0);
}