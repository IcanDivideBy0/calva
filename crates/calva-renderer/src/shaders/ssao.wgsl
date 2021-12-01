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

var<private> positions: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0)
);

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] ndc: vec2<f32>;
};


[[stage(vertex)]]
fn main([[builtin(vertex_index)]] index : u32) -> VertexOutput {
    return VertexOutput (
        vec4<f32>(positions[index], 0.0, 1.0),
        positions[index]
    );
}

// Fragment shader

[[group(2), binding(0)]] var albedo_metallic: texture_2d<f32>;
[[group(2), binding(1)]] var normal_roughness: texture_2d<f32>;


let SAMPLES_COUNT: i32 = 32;

[[block]]
struct RandomData {
    samples: array<vec2<f32>, SAMPLES_COUNT>;
    noise: array<array<vec2<f32>, 4>, 4>;
};

[[group(3), binding(0)]] var<uniform> random_data: RandomData;
[[group(3), binding(1)]] var t_depth: texture_depth_2d;
[[group(3), binding(2)]] var s_depth: sampler;

[[stage(fragment)]]
fn main(in: VertexOutput) ->  [[location(0)]] f32 {
    let c = vec2<i32>(floor(in.position.xy));

    let z = textureLoad(t_depth, c, 0);
    let frag_position = camera.inv_proj * vec4<f32>(in.ndc, z, 1.0);
    let frag_position = frag_position.xyz / frag_position.w;

    let frag_normal = textureLoad(normal_roughness, c, 0).xyz;
    let random = vec3<f32>(random_data.noise[c.x%4][c.y%4], 0.0);

    let tangent = normalize(random - frag_normal * dot(random, frag_normal));
    let bitangent = cross(frag_normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, frag_normal);

    var occlusion: f32 = 0.0;
    for (var i: i32 = 0; i < SAMPLES_COUNT; i = i + 1) {
        // Reorient sample vector in view space ...
        var sample_pos = tbn * vec3<f32>(random_data.samples[i], 0.0);

        // ... and calculate sample point.
        sample_pos = frag_position + sample_pos * config.ssao_radius;

        // Project point and calculate NDC.
        var sample_clip = camera.proj * vec4<f32>(sample_pos, 1.0);
        let sample_ndc = sample_clip.xy / sample_clip.w;

        // Create texture coordinate out of it.
        let sample_uv = sample_ndc * vec2<f32>(0.5, -0.5) + 0.5;

        // Get sample out of depth texture
        let z = textureSample(t_depth, s_depth, sample_uv);
        let frag_pos = camera.inv_proj * vec4<f32>(sample_uv, z, 1.0);
        let frag_pos = frag_pos.xyz / frag_pos.w;
        let depth = frag_pos.z;

        let range_check = smoothStep(0.0, 1.0, config.ssao_radius / abs(frag_position.z - depth));

        if (depth >= sample_pos.z + config.ssao_bias) {
            occlusion = occlusion + 1.0 * range_check;
        }
    }

    occlusion = 1.0 - (occlusion / f32(SAMPLES_COUNT));
    return pow(occlusion, config.ssao_power);
}
