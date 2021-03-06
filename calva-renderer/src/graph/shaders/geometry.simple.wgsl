struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

//
// Vertex shader
//

struct MeshInstance {
    @location(0) model_matrix_0: vec4<f32>,
    @location(1) model_matrix_1: vec4<f32>,
    @location(2) model_matrix_2: vec4<f32>,
    @location(3) model_matrix_3: vec4<f32>,
    @location(4) normal_quat: vec4<f32>,
}

struct VertexInput {
    @location(5) position: vec3<f32>,
    @location(6) normal: vec3<f32>,
    @location(7) tangent: vec4<f32>,
    @location(8) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
}

fn rotate(quat: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(quat.xyz, cross(quat.xyz, v) + quat.w * v);
}

@vertex
fn vs_main(
    instance: MeshInstance,
    in: VertexInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let world_pos = model_matrix * vec4<f32>(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;

    let view3 = mat3x3<f32>(
        camera.view[0].xyz,
        camera.view[1].xyz,
        camera.view[2].xyz,
    );
    out.normal = view3 * rotate(instance.normal_quat, in.normal);
    out.tangent = view3 * rotate(instance.normal_quat, in.tangent.xyz);
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;

    out.uv = in.uv;

    return out;
}

//
// Fragment shader
//

struct FragmentOutput {
    @location(0) albedo_metallic: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
}

@group(1) @binding(0) var t_albedo: texture_2d<f32>;
@group(1) @binding(1) var t_normal: texture_2d<f32>;
@group(1) @binding(2) var t_metallic_roughness: texture_2d<f32>;
@group(1) @binding(3) var t_sampler: sampler;

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

    return mat3x3<f32>(
        normalize(in.tangent),
        normalize(in.bitangent),
        normalize(in.normal)
    );
}

fn get_normal(in: VertexOutput) -> vec3<f32> {
    // no normal mapping
    // return normalize(get_vert_normal(in));

    let tbn = get_tbn(in);
    let n = textureSample(t_normal, t_sampler, in.uv).rgb * 2.0 - 1.0;
    return normalize(tbn * n);
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let albedo = textureSample(t_albedo, t_sampler, in.uv);

    let metallic_roughness = textureSample(t_metallic_roughness, t_sampler, in.uv).bg;
    let metallic = metallic_roughness.x;
    let roughness = metallic_roughness.y;

    if (albedo.a < 0.5) { discard; }

    return FragmentOutput(
        vec4<f32>(albedo.rgb, metallic),
        vec4<f32>(get_normal(in), roughness),
    );
}
