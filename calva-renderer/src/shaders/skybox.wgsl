[[block]]
struct Config {
    ssao_radius: f32;
    ssao_bias: f32;
    ssao_power: f32;
    ambient_factor: f32;
};

[[block]]
struct Camera {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    inv_view: mat4x4<f32>;
    inv_proj: mat4x4<f32>;
};

[[group(0), binding(0)]] var<uniform> config: Config;
[[group(1), binding(0)]] var<uniform> camera: Camera;


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
    );

    let clip = vec4<f32>(tc * 4.0 - 1.0, 1.0, 1.0);

    // Remove translation component from the view transform, so we're left
    // with the rotation alone. For such an "orthonormal" matrix, transpose
    // is the same as inverse, but cheaper
    let view_inv = transpose(mat3x3<f32>(
        camera.view.x.xyz,
        camera.view.y.xyz,
        camera.view.z.xyz,
    ));
    let view_inv = mat3x3<f32>(
        camera.inv_view.x.xyz,
        camera.inv_view.y.xyz,
        camera.inv_view.z.xyz,
    );

    let view_ray = camera.inv_proj * clip;
    let view_dir = view_inv * (view_ray.xyz / view_ray.w); // world space

    return VertexOutput (clip, view_dir);
}

//
// Fragment shader
//

[[group(2), binding(0)]] var t_skybox: texture_cube<f32>;
[[group(2), binding(1)]] var s_skybox: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(in.position));

    let uv = vec3<f32>(in.position.xy, 1.0);
    let uv = in.view_dir;

    let color = textureSample(t_skybox, s_skybox, uv).rgb;

    return vec4<f32>(color, 1.0);
}
