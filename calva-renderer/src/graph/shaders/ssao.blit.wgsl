//
// Vertex shader
//

@stage(vertex)
fn vs_main(@builtin(vertex_index) vertex_index : u32) -> @builtin(position) vec4<f32> {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    ) * 2.0;
    return vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
}

//
// Fragment shader
//

@group(0) @binding(0) var t_ssao: texture_2d<f32>;

@stage(fragment)
fn fs_main(
    @builtin(position) position: vec4<f32>
) -> @location(0) vec4<f32> {
    let c = vec2<i32>(floor(position.xy));
    let alpha = 1.0 - textureLoad(t_ssao, c, 0).r;

    return vec4<f32>(vec3<f32>(0.0), alpha);
}
