struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}
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
    let world_pos = 1.1 * in.position * instance.radius + instance.position;
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
    @location(3) l_radius: f32,
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
    out.l_radius = instance.radius;
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

var<push_constant> GAMMA_INV: f32;

fn fresnel_schlick(cos_theta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

const PI: f32 = 3.14159265359;

fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let num = a2;
    let denom = (NdotH2 * (a2 - 1.0) + 1.0);

    return num / (PI * denom * denom);
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    return NdotV / (NdotV * (1.0 - k) + k);
}

fn geometry_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = geometry_schlick_ggx(NdotV, roughness);
    let ggx1 = geometry_schlick_ggx(NdotL, roughness);

    return ggx1 * ggx2;
}

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

    let N = normal;
    let V = normalize(-frag_pos_view);
    let L = normalize(in.l_position - frag_pos_view);
    let H = normalize(L + V);
    let NdotL = max(dot(N, L), 0.0);

    let dist = distance(in.l_position, frag_pos_view);
    let attenuation = 1.0 / (dist * dist);
    // let attenuation = 1.0 - smoothstep(0.0, in.l_radius, dist);
    // let attenuation = 1.0 / smoothstep(0.0, 1.0, dist / in.l_radius);
    // let attenuation = pow(1.0 - min(dist / in.l_radius, 1.0), 2.0);

    let radiance = in.l_color * attenuation;

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);

    let num = NDF * G * F;
    let denom = 4.0 * max(dot(N, V), 0.0) * NdotL + 0.0001;
    let specular = num / denom;

    let kS = F;
    let kD = (1.0 - kS) * (1.0 - metallic);

    var color = (kD * albedo / PI + specular) * radiance * NdotL;
    let alpha = 1.0 - smoothstep(in.l_radius * 0.8, in.l_radius, dist);

    // color = vec3<f32>(alpha);
    // return vec4<f32>(color, 1.0);

    color = color / (color + vec3(1.0));
    return vec4<f32>(
      pow(color, vec3<f32>(GAMMA_INV)),
      alpha
    );
}
