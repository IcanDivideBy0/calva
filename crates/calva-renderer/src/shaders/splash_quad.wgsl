// Vertex shader

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

[[group(0), binding(0)]] var gBufferAlbedo: texture_2d<f32>;
[[group(0), binding(1)]] var gBufferPosition: texture_2d<f32>;
[[group(0), binding(2)]] var gBufferNormal: texture_2d<f32>;

[[stage(fragment)]]
fn main([[builtin(position)]] coord : vec4<f32>) ->  [[location(0)]] vec4<f32> {
    var c = vec2<i32>(floor(coord.xy));
    var albedo = textureLoad(gBufferAlbedo, c, 0).rgb;
    return vec4<f32>(albedo, 1.0);
}
