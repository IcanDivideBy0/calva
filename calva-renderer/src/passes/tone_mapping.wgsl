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
    exposure: f32,
    gamma: f32,
}
@group(0) @binding(0) var<uniform> config: Config;

@group(1) @binding(0) var t_hdr: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let hdr = textureLoad(t_hdr, vec2<i32>(position.xy), 0).rgb;

    // https://docs.blender.org/manual/en/3.4/render/color_management.html?highlight=exposure
    let color = hdr * exp2(config.exposure);

    // Gamma correction
    return vec4<f32>(
        pow(color, vec3<f32>(1.0 / config.gamma)),
        1.0
    );
}
