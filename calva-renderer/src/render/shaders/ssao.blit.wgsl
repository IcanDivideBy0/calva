//
// Vertex shader
//

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
    out.uv = out.position.xy * vec2<f32>(0.5, -0.5) + 0.5;

    return out;
}

//
// Fragment shader
//

@group(0) @binding(0) var t_ssao: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = 1.0 - textureSample(t_ssao, t_sampler, in.uv).r;
    // return vec4<f32>(vec3<f32>(1.0 - alpha), 1.0);
    return vec4<f32>(vec3<f32>(0.0), alpha);
}
