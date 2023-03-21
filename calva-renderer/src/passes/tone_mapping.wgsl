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

@group(0) @binding(0) var t_hdr: texture_2d<f32>;

struct Config {
    exposure: f32,
    gamma: f32,
}

var<push_constant> CONFIG: Config;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let hdr_color = textureLoad(t_hdr, vec2<i32>(position.xy), 0).rgb;

    // Reinhard tone mapping
    // let mapped = hdr_color / (hdr_color + 1.0);

    // Exposure tone mapping
    let mapped = vec3(1.0) - exp(-hdr_color * CONFIG.exposure);

    // Gamma correction
    return vec4<f32>(
        pow(mapped, vec3<f32>(1.0 / CONFIG.gamma)),
        1.0
    );
}
