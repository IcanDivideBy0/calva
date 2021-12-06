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
    inv_proj: mat4x4<f32>;
};

[[group(0), binding(0)]] var<uniform> config: Config;
[[group(1), binding(0)]] var<uniform> camera: Camera;

// Vertex shader

struct InstanceInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] radius: f32;
    [[location(2)]] color: vec3<f32>;
};

struct VertexInput {
    [[location(3)]] position: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] ndc: vec2<f32>;

    [[location(1)]] l_position: vec3<f32>;
    [[location(2)]] l_radius: f32;
    [[location(3)]] l_color: vec3<f32>;
};

[[stage(vertex)]]
fn main(
    instance: InstanceInput,
    in: VertexInput,
) -> VertexOutput {
    let world_pos = in.position * instance.radius + instance.position;
    let clip_pos = camera.proj * camera.view * vec4<f32>(world_pos, 1.0);

    return VertexOutput (
        clip_pos,
        clip_pos.xy / clip_pos.w,

        (camera.view * vec4<f32>(instance.position, 1.0)).xyz,
        instance.radius,
        instance.color,
    );
}

// Fragment shader

[[group(2), binding(0)]] var t_albedo_metallic: texture_multisampled_2d<f32>;
[[group(2), binding(1)]] var t_normal_roughness: texture_multisampled_2d<f32>;
[[group(2), binding(2)]] var t_depth: texture_depth_multisampled_2d;
[[group(2), binding(3)]] var t_ao: texture_2d<f32>;

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
fn main(
    [[builtin(sample_index)]] msaa_sample: u32,
    in: VertexOutput,
) -> [[location(0)]] vec4<f32> {
    let c = vec2<i32>(floor(in.clip_position.xy));

    let ao = textureLoad(t_ao, c, 0).r;
    let albedo_metallic = textureLoad(t_albedo_metallic, c, i32(msaa_sample));
    let normal_roughness = textureLoad(t_normal_roughness, c, i32(msaa_sample));

    let albedo = albedo_metallic.rgb * ao;
    let normal = normal_roughness.xyz;
    let metallic = albedo_metallic.a;
    let roughness = normal_roughness.a;

    let z = textureLoad(t_depth, c, i32(msaa_sample));
    let frag_pos = camera.inv_proj * vec4<f32>(in.ndc, z, 1.0);
    let frag_pos = frag_pos.xyz / frag_pos.w;

    let N = normal;
    let V = normalize(-frag_pos);
    let L = normalize(in.l_position - frag_pos);
    let H = normalize(L + V);

    let dist = distance(in.l_position, frag_pos);
    // let attenuation = 1.0 - smoothStep(0.0, in.radius, dist);
    let attenuation = pow(1.0 - min(dist / in.l_radius, 1.0), 2.0);

    let radiance = in.l_color * attenuation;

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);
    let F  = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let NDF = distribution_ggx(N, H, roughness);
    let G   = geometry_smith(N, V, L, roughness); 

    let num      = NDF * G * F;
    let denom    = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0)  + 0.0001;
    let specular = num / denom;

    let kS = F;
    let kD = (1.0 - kS) * (1.0 - metallic);

    let NdotL = max(dot(N, L), 0.0);
    var color = (kD * albedo / PI + specular) * radiance * NdotL;

    color = color / (color + 1.0);
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
