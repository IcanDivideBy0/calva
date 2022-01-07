//
// Vertex shader
//

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index : u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    ) * 2.0;

    return VertexOutput(
        vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0),
        vec2<f32>(tc.x, 1.0 - tc.y)
    );
}

//
// Fragment shader
//

[[group(0), binding(0)]] var t_input: texture_2d<f32>;
[[group(0), binding(1)]] var t_sampler: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return textureSample(t_input, t_sampler, in.uv);
}
