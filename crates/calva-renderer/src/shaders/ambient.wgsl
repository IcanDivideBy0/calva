[[block]]
struct Config {
    ssao_radius: f32;
    ssao_bias: f32;
    ssao_power: f32;
    ambient_factor: f32;
};

[[group(0), binding(0)]] var<uniform> config: Config;

// Vertex shader

var<private> positions: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0)
);

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] index : u32) -> [[builtin(position)]] vec4<f32> {
    return vec4<f32>(positions[index], 0.0, 1.0);
}

// Fragment shader

[[group(1), binding(0)]] var albedo_metallic: texture_2d<f32>;

[[group(2), binding(0)]] var ao: texture_2d<f32>;

[[stage(fragment)]]
fn main([[builtin(position)]] coord : vec4<f32>) ->  [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(coord.xy));

    let diffuse = textureLoad(albedo_metallic, c, 0).rgb;
    let ao = textureLoad(ao, c, 0).r;

    return vec4<f32>(vec3<f32>(config.ambient_factor * diffuse * ao), 1.0);
}
