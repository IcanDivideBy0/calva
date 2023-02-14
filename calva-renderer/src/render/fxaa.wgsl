//
// Vertex shader
//

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    return vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

@group(0) @binding(0) var t_output: texture_2d<f32>;
var<push_constant> GAMMA: f32;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let color = textureLoad(t_output, vec2<i32>(position.xy), 0).rgb;

    // TODO: FXAA

    return vec4<f32>(
      pow(color, vec3<f32>(1.0 / GAMMA)),
      1.0
    );
}