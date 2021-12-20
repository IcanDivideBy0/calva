struct Camera {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    inv_view: mat4x4<f32>;
    inv_proj: mat4x4<f32>;
};

[[group(0), binding(0)]] var<uniform> camera: Camera;


//
// Vertex shader
//

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] view_dir: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index : u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    ) * 2.0;

    let clip = vec4<f32>(tc * 2.0 - 1.0, 1.0, 1.0);

    var view_ray = camera.inv_proj * clip;
    var view_ray = view_ray.xyz / view_ray.w;

    // Use rotation only
    let inv_view = mat3x3<f32>(
        camera.inv_view.x.xyz,
        camera.inv_view.y.xyz,
        camera.inv_view.z.xyz,
    );
    let view_dir = inv_view * view_ray; // world space

    return VertexOutput (clip, view_dir);
}

//
// Fragment shader
//

[[group(1), binding(0)]] var t_skybox: texture_cube<f32>;
[[group(1), binding(1)]] var s_skybox: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return textureSample(t_skybox, s_skybox, in.view_dir);
}
