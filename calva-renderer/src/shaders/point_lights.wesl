import package::resources::camera::Camera;
import package::util::pbr::pbr;

@group(0) @binding(0) var<uniform> camera: Camera;

struct LightInstance {
    @location(0) position: vec3<f32>,
    @location(1) radius: f32,
    @location(2) color: vec3<f32>,
}

struct VertexInput {
    @location(3) position: vec3<f32>,
}

fn get_clip_pos(
    instance: LightInstance,
    in: VertexInput,
) -> vec4<f32> {
    let world_pos = in.position * instance.radius + instance.position;
    return camera.view_proj * vec4<f32>(world_pos, 1.0);
}

//
// Stencil pass
//

@vertex
fn vs_main_stencil(
    instance: LightInstance,
    in: VertexInput,
) -> @builtin(position) vec4<f32> {
    return get_clip_pos(instance, in);
}

//
// Lighting pass
//

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) @interpolate(linear) uv: vec2<f32>,

    @location(2) l_position: vec3<f32>,
    @location(3) l_inv_square_radius: f32,
    @location(4) l_color: vec3<f32>,
}

@vertex
fn vs_main_lighting(
    instance: LightInstance,
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    out.position = get_clip_pos(instance, in);
    out.ndc = out.position.xy / out.position.w;
    out.uv = out.ndc * vec2<f32>(0.5, -0.5) + 0.5;

    out.l_position = (camera.view * vec4<f32>(instance.position, 1.0)).xyz;
    out.l_inv_square_radius = 1.0 / (instance.radius * instance.radius);
    out.l_color = instance.color;

    return out;
}

//
// Fragment shader
//

@group(1) @binding(0) var t_sampler: sampler;
@group(1) @binding(1) var t_albedo_metallic: texture_2d<f32>;
@group(1) @binding(2) var t_normal_roughness: texture_2d<f32>;
@group(1) @binding(3) var t_depth: texture_depth_2d;

@fragment
fn fs_main_lighting(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = vec2<i32>(floor(in.position.xy));

    let albedo_metallic = textureSample(t_albedo_metallic, t_sampler, in.uv);
    let normal_roughness = textureSample(t_normal_roughness, t_sampler, in.uv);

    let albedo = albedo_metallic.rgb;
    let normal = normal_roughness.xyz;
    let metallic = albedo_metallic.a;
    let roughness = normal_roughness.a;

    let z = textureSample(t_depth, t_sampler, in.uv);
    let frag_pos_view4 = camera.inv_proj * vec4<f32>(in.ndc, z, 1.0);
    let frag_pos_view = frag_pos_view4.xyz / frag_pos_view4.w;

    let light_to_frag = in.l_position - frag_pos_view;

    let dist_square = dot(light_to_frag, light_to_frag);
    // let attenuation = 1.0 / max(dist_square, 0.0001);
    let factor = dist_square * in.l_inv_square_radius;
    let smooth_factor = saturate(1.0 - factor * factor);
    let attenuation = smooth_factor * smooth_factor / max(dist_square, 0.0001);

    return pbr(
        normal_roughness,
        albedo_metallic,
        frag_pos_view.xyz,
        light_to_frag,
        in.l_color * attenuation,
    );
}
