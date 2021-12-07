[[block]]
struct Config {
    ssao_radius: f32;
    ssao_bias: f32;
    ssao_power: f32;
    ambient_factor: f32;
};

[[group(0), binding(0)]] var<uniform> config: Config;

// Vertex shader

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] vertex_index : u32) -> [[builtin(position)]] vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    );

    return vec4<f32>(tc * 4.0 - 1.0, 1.0, 1.0);
}

// Fragment shader

[[group(1), binding(0)]] var albedo: texture_multisampled_2d<f32>;
[[group(1), binding(1)]] var ao: texture_2d<f32>;

[[stage(fragment)]]
fn main(
    [[builtin(position)]] coord : vec4<f32>,
    [[builtin(sample_index)]] msaa_sample: u32
) ->  [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(coord.xy));

    let diffuse = textureLoad(albedo, c, i32(msaa_sample)).rgb;
    let ao = textureLoad(ao, c, 0).r;

    return vec4<f32>(vec3<f32>(config.ambient_factor * diffuse * ao), 1.0);
}
