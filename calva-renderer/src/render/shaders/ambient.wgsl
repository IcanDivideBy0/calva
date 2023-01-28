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

struct AmbientConfig {
    factor: f32,
}

@group(0) @binding(0) var<uniform> config: AmbientConfig;
@group(0) @binding(1) var t_albedo: texture_2d<f32>;

var<push_constant> GAMMA: f32;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let diffuse = textureLoad(t_albedo, vec2<i32>(position.xy), 0).rgb;
    let color = pow(config.factor * diffuse, vec3<f32>(1.0 / GAMMA));

    return vec4<f32>(color, 1.0);
}
