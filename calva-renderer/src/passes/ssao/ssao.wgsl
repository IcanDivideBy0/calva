struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    frustum: array<vec4<f32>, 6>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

//
// Vertex shader
//

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

struct SsaoConfig {
    radius: f32,
    bias: f32,
    power: f32,
}

const SAMPLES_COUNT: u32 = 32u;
struct RandomData {
    samples: array<vec4<f32>, SAMPLES_COUNT>,
    noise: array<array<vec4<f32>, 4>, 4>,
}

@group(1) @binding(0) var<uniform> config: SsaoConfig;
@group(1) @binding(1) var<uniform> random_data: RandomData;
@group(1) @binding(2) var t_sampler: sampler;
@group(1) @binding(3) var t_normal: texture_2d<f32>;
@group(1) @binding(4) var t_depth: texture_depth_2d;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let t_depth_size = vec2<f32>(textureDimensions(t_depth));

    let depth_coord = vec2<i32>(in.uv * t_depth_size);
    let frag_depth = textureSample(t_depth, t_sampler, in.uv);
    let frag_position4 = camera.inv_proj * vec4<f32>(in.ndc, frag_depth, 1.0);
    let frag_position = frag_position4.xyz / frag_position4.w;

    let frag_normal = textureSample(t_normal, t_sampler, in.uv).xyz;

    let c = vec2<i32>(floor(in.position.xy));
    let random = random_data.noise[c.x & 3][c.y & 3].xyz;

    let tangent = normalize(random - frag_normal * dot(random, frag_normal));
    let bitangent = cross(frag_normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, frag_normal);

    var occlusion: f32 = 0.0;
    for (var i: u32 = 0u; i < SAMPLES_COUNT; i++) {
        // Reorient sample vector in view space ...
        var sample_pos = tbn * random_data.samples[i].xyz;

        // ... and calculate sample point.
        sample_pos = frag_position + sample_pos * config.radius;

        // Project point and calculate NDC.
        var sample_clip = camera.proj * vec4<f32>(sample_pos, 1.0);
        let sample_ndc = sample_clip.xy / sample_clip.w;

        // Create texture coordinate out of it.
        let sample_uv = sample_ndc * vec2<f32>(0.5, -0.5) + 0.5;
        let sample_coord = vec2<i32>(sample_uv * t_depth_size);

        // Get sample out of depth texture
        let depth = textureLoad(t_depth, sample_coord, 0);
        let frag_pos4 = camera.inv_proj * vec4<f32>(sample_uv, depth, 1.0);
        let frag_pos = frag_pos4.xyz / frag_pos4.w;

        let range_check = smoothstep(0.0, 1.0, config.radius / abs(frag_position.z - frag_pos.z));

        occlusion = occlusion + select(0.0, 1.0, frag_pos.z >= sample_pos.z + config.bias) * range_check;
    }

    occlusion = 1.0 - (occlusion / f32(SAMPLES_COUNT));
    return pow(occlusion, config.power);
}
