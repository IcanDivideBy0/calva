import package::util::pbr::pbr;
import package::resources::{
    camera::Camera,
    directional_light::DirectionalLight,
};

//
// Vertex shader
//

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> directional_light: DirectionalLight;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index & 1u),
    ) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(tc * 2.0 - 1.0, 0.0, 1.0);
    out.ndc = out.position.xy;
    out.uv = out.ndc * vec2<f32>(0.5, -0.5) + 0.5;

    return out;
}

//
// Fragment shader
//

@group(2) @binding(0) var t_albedo_metallic: texture_2d<f32>;
@group(2) @binding(1) var t_normal_roughness: texture_2d<f32>;
@group(2) @binding(2) var t_depth: texture_depth_2d;

@group(2) @binding(3) var t_shadows: texture_depth_2d;
@group(2) @binding(4) var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo_metallic = textureSample(t_albedo_metallic, t_sampler, in.uv);
    let normal_roughness = textureSample(t_normal_roughness, t_sampler, in.uv);

    let albedo = albedo_metallic.rgb;
    let normal = normal_roughness.xyz;
    let metallic = albedo_metallic.a;
    let roughness = normal_roughness.a;

    let z = textureSample(t_depth, t_sampler, in.uv);
    var frag_pos_view = camera.inv_proj * vec4<f32>(in.ndc, z, 1.0);
    frag_pos_view = frag_pos_view / frag_pos_view.w;

    let frag_pos_world = camera.inv_view * frag_pos_view;

    let frag_proj4 = directional_light.view_proj * frag_pos_world;
    let frag_proj = frag_proj4.xyz / frag_proj4.w;
    let frag_proj_uv = frag_proj.xy * vec2<f32>(0.5, -0.5) + 0.5;

    let light_depth = textureSample(t_shadows, t_sampler, frag_proj_uv);

    // Exponential shadow mapping
    let ratio = 60.0;
    let visibility = clamp(exp(ratio * 10.0 * (light_depth - frag_proj.z)), 0.0, 1.0);

    return pbr(
        normal_roughness,
        albedo_metallic,
        frag_pos_view.xyz,
        -directional_light.direction_view.xyz,
        directional_light.color.rgb * visibility,
    );
}
