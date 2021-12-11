
[[block]]
struct Config {
    ssao_radius: f32;
    ssao_bias: f32;
    ssao_power: f32;
    ambient_factor: f32;
    shadow_bias_factor: f32;
    shadow_bias_max: f32;
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
    light_dir: vec4<f32>; // camera view space
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
[[group(3), binding(5)]] var s_shadows: sampler_comparison;

var<private> poisson_disk: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
  vec2<f32>(-0.94201624, -0.39906216 ),
  vec2<f32>( 0.94558609, -0.76890725 ),
  vec2<f32>(-0.09418410, -0.92938870 ),
  vec2<f32>( 0.34495938,  0.29387760 )
);

fn random(seed: vec4<f32>) -> f32 {
    let d = dot(seed, vec4<f32>(12.9898, 78.233, 45.164, 94.673));
    return fract(sin(d) * 43758.5453);
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

    let N = normal;
    let V = normalize(-frag_pos_view.xyz);
    let L = -shadow_light.light_dir.xyz;
    let H = normalize(L + V);
    let NdotL = max(dot(normal, L), 0.0);

    let frag_pos_world = camera.inv_view * frag_pos_view;

    let frag_proj = shadow_light.view_proj[cascade_index] * frag_pos_world;
    let frag_proj = (frag_proj.xyz / frag_proj.w);
    let frag_proj_uv = frag_proj.xy * vec2<f32>(0.5, -0.5) + 0.5;

    let bias = 0.0;
    // https://learnopengl.com/Advanced-Lighting/Shadows/Shadow-Mapping
    // let bias = max(config.shadow_bias_factor * (1.0 - NdotL), config.shadow_bias_max);

    // http://www.opengl-tutorial.org/intermediate-tutorials/tutorial-16-shadow-mapping/#shadow-acne
    // let bias = config.shadow_bias_factor * tan(acos(NdotL));
    // let bias = clamp(bias, 0.0, config.shadow_bias_max);

    // let visibility = textureSampleCompare(t_shadows, s_shadows, frag_proj_uv, frag_proj.z) - bias;

    var visibility = 0.0;
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        // let r = random(vec4<f32>(fract(in.position.xyy), f32(i)));
        // let i = u32(r * 4.0) % 4u;

        let uv = frag_proj_uv + poisson_disk[i] / 700.0;
        let depth_cmp = textureSampleCompare(t_shadows, s_shadows, uv, i32(cascade_index), frag_proj.z) - bias;

        visibility = visibility + depth_cmp / 4.0;
    }

    return vec4<f32>(visibility * NdotL * albedo, 1.0);
}