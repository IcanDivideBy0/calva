// Vertex shader

var<private> pos: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0)
);

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] index : u32) -> [[builtin(position)]] vec4<f32> {
    return vec4<f32>(pos[index], 0.0, 1.0);
}

// Fragment shader

[[group(0), binding(0)]] var g_buffer_albedo: texture_2d<f32>;
[[group(0), binding(1)]] var g_buffer_position: texture_2d<f32>;
[[group(0), binding(2)]] var g_buffer_normal: texture_2d<f32>;

[[block]]
struct Ambient {
    factor: f32;
};

[[group(1), binding(0)]] var<uniform> ambient: Ambient;

// let ambient_factor: f32 = 0.1;

[[stage(fragment)]]
fn main([[builtin(position)]] coord : vec4<f32>) ->  [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(coord.xy));
    let albedo = textureLoad(g_buffer_albedo, c, 0).rgb;

    return vec4<f32>(albedo * ambient.factor, 1.0);
}
