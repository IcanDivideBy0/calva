import package::resources::camera::Camera;

@group(0) @binding(0) var<uniform> camera: Camera;

//
// Vertex shader
//

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    let clip = vec4<f32>(tc * 2.0 - 1.0, 1.0, 1.0);

    var view_ray4 = camera.inv_proj * clip;
    var view_ray = view_ray4.xyz / view_ray4.w;

    // Use rotation only
    let inv_view = mat3x3<f32>(
        camera.inv_view[0].xyz,
        camera.inv_view[1].xyz,
        camera.inv_view[2].xyz,
    );
    let view_dir = inv_view * view_ray; // world space

    return VertexOutput(clip, view_dir);
}

//
// Fragment shader
//

@group(1) @binding(0) var t_skybox: texture_cube<f32>;
@group(1) @binding(1) var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_skybox, t_sampler, in.view_dir);
}
