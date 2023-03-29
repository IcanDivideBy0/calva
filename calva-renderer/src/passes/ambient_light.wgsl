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

struct Config {
    color: vec3<f32>,
    strength: f32,
}
@group(0) @binding(0) var<uniform> config: Config;

@group(1) @binding(0) var t_albedo_metallic: texture_2d<f32>;
@group(1) @binding(1) var t_emissive: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    var ambient = textureLoad(t_albedo_metallic, vec2<i32>(position.xy), 0).rgb;
    var emissive = textureLoad(t_emissive, vec2<i32>(position.xy), 0).rgb;

    return vec4<f32>(config.color * config.strength * ambient + emissive, 1.0);
}
