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

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] ndc: vec2<f32>;
};

[[stage(vertex)]]
fn main([[builtin(vertex_index)]] vertex_index : u32) -> VertexOutput {
    let tc = vec2<f32>(
        f32(vertex_index >> 1u),
        f32(vertex_index &  1u),
    );
    let clip = vec4<f32>(tc * 4.0 - 1.0, 0.0, 1.0);

    return VertexOutput (clip, clip.xy);
}

// Fragment shader

let SAMPLES_COUNT: u32 = 32u;

[[block]]
struct RandomData {
    samples: array<vec2<f32>, SAMPLES_COUNT>;
    noise: array<array<vec2<f32>, 4>, 4>;
};

[[group(2), binding(0)]] var<uniform> random_data: RandomData;
[[group(2), binding(1)]] var t_depth: texture_depth_multisampled_2d;
[[group(2), binding(2)]] var t_normal: texture_multisampled_2d<f32>;

[[stage(fragment)]]
fn main(
    [[builtin(sample_index)]] msaa_sample: u32,
    in: VertexOutput
) ->  [[location(0)]] f32 {
    let c = vec2<i32>(floor(in.position.xy));

    let frag_depth = textureLoad(t_depth, c, 0);
    let frag_position = camera.inv_proj * vec4<f32>(in.ndc, frag_depth, 1.0);
    let frag_position = frag_position.xyz / frag_position.w;

    let frag_normal = textureLoad(t_normal, c, 0).xyz;
    let random = vec3<f32>(random_data.noise[c.x & 3][c.y & 3], 0.0);

    let tangent = normalize(random - frag_normal * dot(random, frag_normal));
    let bitangent = cross(frag_normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, frag_normal);

    var occlusion: f32 = 0.0;
    for (var i: u32 = 0u; i < SAMPLES_COUNT; i = i + 1u) {
        // Reorient sample vector in view space ...
        var sample_pos = tbn * vec3<f32>(random_data.samples[i], 0.0);

        // ... and calculate sample point.
        sample_pos = frag_position + sample_pos * config.ssao_radius;

        // Project point and calculate NDC.
        var sample_clip = camera.proj * vec4<f32>(sample_pos, 1.0);
        let sample_ndc = sample_clip.xy / sample_clip.w;

        // Create texture coordinate out of it.
        let sample_uv = sample_ndc * vec2<f32>(0.5, -0.5) + 0.5;
        let sample_coord = vec2<i32>(sample_uv * vec2<f32>(textureDimensions(t_depth)));

        // Get sample out of depth texture
        let depth = textureLoad(t_depth, sample_coord, i32(msaa_sample));
        let frag_pos = camera.inv_proj * vec4<f32>(sample_uv, depth, 1.0);
        let frag_pos = frag_pos.xyz / frag_pos.w;

        let range_check = smoothStep(0.0, 1.0, config.ssao_radius / abs(frag_position.z - frag_pos.z));

        occlusion = occlusion + select(0.0, 1.0, frag_pos.z >= sample_pos.z + config.ssao_bias) * range_check;
    }

    occlusion = 1.0 - (occlusion / f32(SAMPLES_COUNT));
    return pow(occlusion, config.ssao_power);
}
