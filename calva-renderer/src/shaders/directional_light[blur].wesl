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

    return VertexOutput(
        vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0),
        vec2<f32>(tc.x, 1.0 - tc.y)
    );
}

//
// Fragment shader
//

@group(0) @binding(0) var t_sampler: sampler;
@group(0) @binding(1) var t_input: texture_depth_2d;

fn blur(position: vec4<f32>, direction: vec2<i32>) -> f32 {
    let c = vec2<i32>(position.xy);

    var result: f32 = 0.0;

    result += textureLoad(t_input, c + vec2<i32>(-3) * direction, 0) * ( 1.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>(-2) * direction, 0) * ( 6.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>(-1) * direction, 0) * (15.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>( 0) * direction, 0) * (20.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>( 1) * direction, 0) * (15.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>( 2) * direction, 0) * ( 6.0 / 64.0);
    result += textureLoad(t_input, c + vec2<i32>( 3) * direction, 0) * ( 1.0 / 64.0);

    return result;
}

@fragment
fn fs_main_horizontal(in: VertexOutput) -> @builtin(frag_depth) f32 {
    return blur(in.position, vec2<i32>(1, 0));
}

@fragment
fn fs_main_vertical(in: VertexOutput) -> @builtin(frag_depth) f32 {
    return blur(in.position, vec2<i32>(0, 1));
}
