//
// Vertex shader
//

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] vertex_index : u32) -> [[builtin(position)]] vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    );

    return vec4<f32>(tc * 4.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

[[group(0), binding(0)]] var input: texture_2d<f32>;

var<private> blur_size: i32 = 4;

fn blur(position: vec4<f32>, direction: vec2<i32>) ->  f32 {
    let c = vec2<i32>(floor(position.xy));

    let r = textureLoad(input, c, 0).r;

    var result: f32 = 0.0;
    result = result + textureLoad(input, c + vec2<i32>(-2) * direction, 0).r;
    result = result + textureLoad(input, c + vec2<i32>(-1) * direction, 0).r;
    result = result + textureLoad(input, c + vec2<i32>( 0) * direction, 0).r;
    result = result + textureLoad(input, c + vec2<i32>( 1) * direction, 0).r;

    return result / f32(blur_size);
}

[[stage(fragment)]]
fn main_horizontal([[builtin(position)]] position: vec4<f32>) ->  [[location(0)]] f32 {
    return blur(position, vec2<i32>(1, 0));
}

[[stage(fragment)]]
fn main_vertical([[builtin(position)]] position: vec4<f32>) ->  [[location(0)]] f32 {
    return blur(position, vec2<i32>(0, 1));
}
