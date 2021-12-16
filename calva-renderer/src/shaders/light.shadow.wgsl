
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

let CASCADES: u32 = 4u;
[[block]]
struct ShadowLight {
    color: vec4<f32>;
    direction: vec4<f32>; // camera view space
    view_proj: array<mat4x4<f32>, CASCADES>;
    splits: array<f32, CASCADES>;
};

[[group(0), binding(0)]] var<uniform> config: Config;
[[group(1), binding(0)]] var<uniform> camera: Camera;
[[group(2), binding(0)]] var<uniform> shadow_light: ShadowLight;

//
// Vertex shader
//

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] ndc: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index : u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    );
    let clip = vec4<f32>(tc * 4.0 - 1.0, 0.0, 1.0);

    return VertexOutput (clip, clip.xy);
}

//
// Fragment shader
//

[[group(3), binding(0)]] var t_albedo_metallic: texture_multisampled_2d<f32>;
[[group(3), binding(1)]] var t_normal_roughness: texture_multisampled_2d<f32>;
[[group(3), binding(2)]] var t_depth: texture_depth_multisampled_2d;
[[group(3), binding(3)]] var t_ao: texture_2d<f32>;

[[group(3), binding(4)]] var t_shadows: texture_depth_2d_array;
[[group(3), binding(5)]] var s_shadows: sampler;

fn fresnel_schlick(cos_theta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

let PI: f32 = 3.14159265359;

fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a      = roughness * roughness;
    let a2     = a * a;
    let NdotH  = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let num   = a2;
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
    let ggx2  = geometry_schlick_ggx(NdotV, roughness);
    let ggx1  = geometry_schlick_ggx(NdotL, roughness);

    return ggx1 * ggx2;
}

[[stage(fragment)]]
fn fs_main(
    [[builtin(sample_index)]] msaa_sample: u32,
    in: VertexOutput,
) -> [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(in.position.xy));

    let ao = textureLoad(t_ao, c, 0).r;
    let albedo_metallic = textureLoad(t_albedo_metallic, c, i32(msaa_sample));
    let normal_roughness = textureLoad(t_normal_roughness, c, i32(msaa_sample));

    let albedo = albedo_metallic.rgb * ao;
    let normal = normal_roughness.xyz;
    let metallic = albedo_metallic.a;
    let roughness = normal_roughness.a;

    let z = textureLoad(t_depth, c, i32(msaa_sample));
    var frag_pos_view = camera.inv_proj * vec4<f32>(in.ndc, z, 1.0);
    frag_pos_view = frag_pos_view / frag_pos_view.w;

    var cascade_index = 0u;
    for (var i: u32 = 0u; i < CASCADES; i = i + 1u) {
        if (z > shadow_light.splits[i]) {
            cascade_index = i;
        }
    }

    let frag_pos_world = camera.inv_view * frag_pos_view;

    let frag_proj = shadow_light.view_proj[cascade_index] * frag_pos_world;
    let frag_proj = frag_proj.xyz / frag_proj.w;
    let frag_proj_uv = frag_proj.xy * vec2<f32>(0.5, -0.5) + 0.5;

    let light_depth = textureSample(t_shadows, s_shadows, frag_proj_uv, i32(cascade_index));

    // Exponential shadow mapping
    let ratio = 60.0; // TODO: compute a different ratio for each cascade
    let visibility = clamp(exp(ratio * 10.0 * (light_depth - frag_proj.z)), 0.0, 1.0 );

    let N = normal;
    let V = normalize(-frag_pos_view.xyz);
    let L = normalize(-shadow_light.direction.xyz);
    let H = normalize(L + V);
    let NdotL = max(dot(normal, L), 0.0);

    let radiance = shadow_light.color.rgb * shadow_light.color.a * visibility;

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);
    let F  = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let NDF = distribution_ggx(N, H, roughness);
    let G   = geometry_smith(N, V, L, roughness); 

    let num      = NDF * G * F;
    let denom    = 4.0 * max(dot(N, V), 0.0) * NdotL + 0.0001;
    let specular = num / denom;

    let kS = F;
    let kD = (1.0 - kS) * (1.0 - metallic);

    var color = (kD * albedo / PI + specular) * radiance * NdotL;

    color = color / (color + 1.0);
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
