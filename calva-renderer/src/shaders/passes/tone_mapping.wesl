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

struct ToneMappingConfig {
    exposure: f32,
    gamma: f32,
}

@group(0) @binding(0) var<uniform> config: ToneMappingConfig;
@group(1) @binding(0) var t_hdr: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let hdr = textureLoad(t_hdr, vec2<i32>(position.xy), 0);

    // https://sotrh.github.io/learn-wgpu/intermediate/tutorial13-hdr/#tonemapping
    let sdr = aces_tone_map(hdr.rgb);
    return vec4<f32>(sdr, 1.0);

    // https://docs.blender.org/manual/en/3.4/render/color_management.html?highlight=exposure
    let color = hdr.rgb * exp2(config.exposure);

    // Gamma correction
    return vec4<f32>(
        pow(color, vec3<f32>(1.0 / config.gamma)),
        1.0
    );

}

// Maps HDR values to linear values
// Based on http://www.oscars.org/science-technology/sci-tech-projects/aces
fn aces_tone_map(hdr: vec3<f32>) -> vec3<f32> {
    let m1 = mat3x3(
        0.59719, 0.07600, 0.02840,
        0.35458, 0.90834, 0.13383,
        0.04823, 0.01566, 0.83777,
    );
    let m2 = mat3x3(
        1.60475, -0.10208, -0.00327,
        -0.53108,  1.10813, -0.07276,
        -0.07367, -0.00605,  1.07602,
    );
    let v = m1 * hdr;
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate(m2 * (a / b));
}