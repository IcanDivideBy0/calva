
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

fn fresnel_schlick(cos_theta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn pbr(
    normal_roughness: vec4<f32>,
    albedo_metallic: vec4<f32>,
    view: vec3<f32>,
    light: vec3<f32>,
    radiance: vec3<f32>,
) -> vec4<f32> {
    let roughness = normal_roughness.a;
    let albedo = albedo_metallic.rgb;
    let metallic = albedo_metallic.a;

    let N = normal_roughness.xyz;
    let V = normalize(-view);
    let L = normalize(light);
    let H = normalize(L + V);
    let NdotL = max(dot(N, L), 0.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);

    let num = NDF * G * F;
    let denom = 4.0 * max(dot(N, V), 0.0) * NdotL + 0.0001;
    let specular = num / denom;

    let kS = F;
    let kD = (1.0 - kS) * (1.0 - metallic);

    let color = (kD * albedo / PI + specular) * radiance * NdotL;

    return vec4<f32>(color, 1.0);
}