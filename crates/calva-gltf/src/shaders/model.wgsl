// Vertex shader

[[block]]
struct Camera {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: Camera;

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
fn main(
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

    let view_pos = camera.view * model_matrix * vec4<f32>(in.position, 1.0);

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;

    out.normal = normalize(normal_matrix * in.normal);
    out.tangent = normalize(normal_matrix * in.tangent.xyz);
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;

    out.uv = in.uv;

    return out;
}

// Fragment shader

struct FragmentOutput {
    [[location(0)]] albedo: vec4<f32>;
    [[location(1)]] position: vec4<f32>;
    [[location(2)]] normal: vec4<f32>;
};

[[group(1), binding(0)]] var t_albedo: texture_2d<f32>;
[[group(1), binding(1)]] var s_albedo: sampler;
[[group(1), binding(2)]] var t_normal: texture_2d<f32>;
[[group(1), binding(3)]] var s_normal: sampler;

fn compute_tbn(in: VertexOutput) -> mat3x3<f32> {
    let pos_dx = dpdx(in.position);
    let pos_dy = dpdy(in.position);
    let tex_dx = dpdx(vec3<f32>(in.uv, 0.0));
    let tex_dy = dpdy(vec3<f32>(in.uv, 0.0));

    let r = 1.0 / (tex_dx.x * tex_dy.y - tex_dx.y * tex_dy.x);
    let tangent = (pos_dx * tex_dy.y - pos_dy * tex_dx.y) * r;
    let bitangent = (pos_dy * tex_dx.x - pos_dx * tex_dy.x) * r;
    let normal = in.normal;

    return mat3x3<f32>(
        normalize(tangent), 
        normalize(bitangent), 
        normalize(normal),
    );
}

fn compute_normal(in: VertexOutput) -> vec3<f32> {
    // let tbn = compute_tbn(in);
    let tbn = mat3x3<f32>(in.tangent, in.bitangent, in.normal);

    // no normal map ?
    // return tbn[2];

    let n = textureSample(t_normal, s_normal, in.uv).rgb * 2.0 - 1.0;
    return normalize(tbn * n);
}

[[stage(fragment)]]
fn main(in: VertexOutput) ->  FragmentOutput {
    var out: FragmentOutput;

    out.albedo = textureSample(t_albedo, s_albedo, in.uv);
    out.position = vec4<f32>(in.position, 1.0);
    out.normal = vec4<f32>(compute_normal(in), 1.0);

    // out.albedo = out.normal;

    return out;
}
