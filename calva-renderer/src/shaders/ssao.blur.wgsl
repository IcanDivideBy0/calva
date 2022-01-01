//
// Vertex shader
//

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index : u32) -> [[builtin(position)]] vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    ) * 2.0;

    return vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

[[group(0), binding(0)]] var input: texture_2d<f32>;

fn blur(position: vec4<f32>, direction: vec2<i32>) -> f32 {
    let c = vec2<i32>(floor(position.xy));

    var result: f32 = 0.0;

    // result = result + textureLoad(input, c + vec2<i32>(-2) * direction, 0).r;
    // result = result + textureLoad(input, c + vec2<i32>(-1) * direction, 0).r;
    // result = result + textureLoad(input, c + vec2<i32>( 0) * direction, 0).r;
    // result = result + textureLoad(input, c + vec2<i32>( 1) * direction, 0).r;
    // result = result + textureLoad(input, c + vec2<i32>(-2) * direction, 0).r;

    // return result / 5.0;
    
    result = result + textureLoad(input, c + vec2<i32>(-3) * direction, 0).r * ( 1.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>(-2) * direction, 0).r * ( 6.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>(-1) * direction, 0).r * (15.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 0) * direction, 0).r * (20.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 1) * direction, 0).r * (15.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 2) * direction, 0).r * ( 6.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 3) * direction, 0).r * ( 1.0 / 64.0);

    return result;
}

[[stage(fragment)]]
fn fs_main_horizontal([[builtin(position)]] position: vec4<f32>) -> [[location(0)]] f32 {
    return blur(position, vec2<i32>(1, 0));
}

[[stage(fragment)]]
fn fs_main_vertical([[builtin(position)]] position: vec4<f32>) -> [[location(0)]] f32 {
    return blur(position, vec2<i32>(0, 1));
}
