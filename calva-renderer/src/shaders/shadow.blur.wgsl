//
// Vertex shader
//

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index : u32) -> [[builtin(position)]] vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    );

    return vec4<f32>(tc * 4.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

[[group(0), binding(0)]] var input: texture_depth_2d_array;

fn blur(layer: i32, position: vec4<f32>, direction: vec2<i32>) -> f32 {
    let c = vec2<i32>(floor(position.xy));

    var result = 0.0;

    result = result + textureLoad(input, c + vec2<i32>(-3) * direction, layer, 0) * ( 1.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>(-2) * direction, layer, 0) * ( 6.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>(-1) * direction, layer, 0) * (15.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 0) * direction, layer, 0) * (20.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 1) * direction, layer, 0) * (15.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 2) * direction, layer, 0) * ( 6.0 / 64.0);
    result = result + textureLoad(input, c + vec2<i32>( 3) * direction, layer, 0) * ( 1.0 / 64.0);

    return result;
}

[[stage(fragment)]]
fn fs_main_horizontal(
    [[builtin(view_index)]] view_index: i32,
    [[builtin(position)]] position: vec4<f32>,
) -> [[builtin(frag_depth)]] f32 {
    return blur(view_index, position, vec2<i32>(1, 0));
}

[[stage(fragment)]]
fn fs_main_vertical(
    [[builtin(view_index)]] view_index: i32,
    [[builtin(position)]] position: vec4<f32>,
) -> [[builtin(frag_depth)]] f32 {
    return blur(view_index, position, vec2<i32>(0, 1));
}
