[[block]]
struct Globals {
    value: f32;
};

[[group(0), binding(0)]]
var<uniform> globals: Globals;

// Vertex shader

[[block]]
struct CameraUniforms {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
};

[[group(1), binding(0)]]
var<uniform> camera: CameraUniforms;

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] index : u32) -> [[builtin(position)]] vec4<f32> {
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0)
    );

    return vec4<f32>(pos[index], 0.0, 1.0);
}

// Fragment shader

[[group(2), binding(0)]] var g_buffer_albedo: texture_2d<f32>;
[[group(2), binding(1)]] var g_buffer_position: texture_2d<f32>;
[[group(2), binding(2)]] var g_buffer_normal: texture_2d<f32>;

[[stage(fragment)]]
fn main([[builtin(position)]] coord : vec4<f32>) ->  [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(coord.xy));
    let albedo = textureLoad(g_buffer_albedo, c, 0).rgb;

    let ambient_strength = 0.1;
    return vec4<f32>(albedo * ambient_strength, 1.0);
}
