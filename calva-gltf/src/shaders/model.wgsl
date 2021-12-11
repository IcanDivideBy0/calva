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

[[group(0), binding(0)]] var<uniform> config: Config;
[[group(1), binding(0)]] var<uniform> camera: Camera;

//
// Vertex shader
//

struct InstanceInput {
    [[location(0)]] model_matrix_0: vec4<f32>;
    [[location(1)]] model_matrix_1: vec4<f32>;
    [[location(2)]] model_matrix_2: vec4<f32>;
    [[location(3)]] model_matrix_3: vec4<f32>;

    [[location(4)]] normal_matrix_0: vec3<f32>;
    [[location(5)]] normal_matrix_1: vec3<f32>;
    [[location(6)]] normal_matrix_2: vec3<f32>;
};

struct VertexInput {
    [[location(7)]]  position: vec3<f32>;
    [[location(8)]]  normal: vec3<f32>;
    [[location(9)]]  tangent: vec4<f32>;
    [[location(10)]] uv: vec2<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
    [[location(2)]] tangent: vec3<f32>;
    [[location(3)]] bitangent: vec3<f32>;
    [[location(4)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main(
    instance: InstanceInput,
    in: VertexInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    let world_pos = model_matrix * vec4<f32>(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;

    out.normal = normalize(normal_matrix * in.normal);
    out.tangent = normalize(normal_matrix * in.tangent.xyz);
    // glTF tangents export shenanigans: tangent.w must be flipped?
    // https://github.com/KhronosGroup/glTF-Sample-Models/issues/174
    // https://github.com/KhronosGroup/glTF/issues/2056
    out.bitangent = cross(out.normal, out.tangent) * -in.tangent.w;

    out.uv = in.uv;

    return out;
}

//
// Fragment shader
//

struct FragmentOutput {
    [[location(0)]] albedo_metallic: vec4<f32>;
    [[location(1)]] normal_roughness: vec4<f32>;
};

[[group(2), binding(0)]] var t_albedo: texture_2d<f32>;
[[group(2), binding(1)]] var s_albedo: sampler;
[[group(2), binding(2)]] var t_normal: texture_2d<f32>;
[[group(2), binding(3)]] var s_normal: sampler;
[[group(2), binding(4)]] var t_metallic_roughness: texture_2d<f32>;
[[group(2), binding(5)]] var s_metallic_roughness: sampler;

fn get_vert_normal(in: VertexOutput) -> vec3<f32> {
    // no normals
    // return cross(dpdx(in.position), dpdy(in.position));
    return in.normal;
}

fn compute_tbn(in: VertexOutput) -> mat3x3<f32> {
    let pos_dx = dpdx(in.position);
    let pos_dy = dpdy(in.position);
    let tex_dx = dpdx(in.uv);
    let tex_dy = dpdy(in.uv);

    let scale = sign(tex_dx.x * tex_dy.y - tex_dx.y * tex_dy.x);
    let tangent = (pos_dx * tex_dy.y - pos_dy * tex_dx.y) * scale;
    let bitangent = (pos_dy * tex_dx.x - pos_dx * tex_dy.x) * scale;
    let normal = get_vert_normal(in);

    return mat3x3<f32>(
        normalize(tangent), 
        normalize(bitangent), 
        normalize(normal),
    );
}

fn get_tbn(in: VertexOutput) -> mat3x3<f32> {
    // no tangents
    // return compute_tbn(in);

    return mat3x3<f32>(in.tangent, in.bitangent, in.normal);
}

fn get_normal(in: VertexOutput) -> vec3<f32> {
    // no normal mapping
    // return get_vert_normal(in);

    let tbn = get_tbn(in);
    let n = textureSample(t_normal, s_normal, in.uv).rgb * 2.0 - 1.0;
    return normalize(tbn * n);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) ->  FragmentOutput {
    let albedo = textureSample(t_albedo, s_albedo, in.uv);

    let metallic_roughness = textureSample(t_metallic_roughness, s_metallic_roughness, in.uv).bg;
    let metallic = metallic_roughness.x;
    let roughness = metallic_roughness.y;

    if (albedo.a < 0.5) { discard; }

    return FragmentOutput (
        vec4<f32>(albedo.rgb, metallic),
        vec4<f32>(get_normal(in), roughness),
    );
}
