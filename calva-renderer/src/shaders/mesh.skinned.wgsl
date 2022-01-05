struct Camera {
    view: mat4x4<f32>;
    proj: mat4x4<f32>;
    view_proj: mat4x4<f32>;
    inv_view: mat4x4<f32>;
    inv_proj: mat4x4<f32>;
};

[[group(0), binding(0)]] var<uniform> camera: Camera;

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

    [[location(7)]] animation_frame: u32;
};

struct VertexInput {
    [[location( 8)]] position: vec3<f32>;
    [[location( 9)]] normal: vec3<f32>;
    [[location(10)]] tangent: vec4<f32>;
    [[location(11)]] uv: vec2<f32>;
    [[location(12)]] joints: u32;
    [[location(13)]] weights: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
    [[location(2)]] tangent: vec3<f32>;
    [[location(3)]] bitangent: vec3<f32>;
    [[location(4)]] uv: vec2<f32>;
};

[[group(2), binding(0)]] var animation: texture_2d_array<f32>;

fn get_joint_matrix(frame: u32, joint_index: u32) -> mat4x4<f32> {
    let c = vec2<i32>(
        i32(joint_index),
        i32(frame),
    );

    return mat4x4<f32>(
        textureLoad(animation, c, 0, 0),
        textureLoad(animation, c, 1, 0),
        textureLoad(animation, c, 2, 0),
        textureLoad(animation, c, 3, 0),
    );
}

fn get_skinning_matrix(frame: u32, in: VertexInput) -> mat4x4<f32> {
    let joints = vec4<u32>(
        in.joints >>  0u & 0xFFu,
        in.joints >>  8u & 0xFFu,
        in.joints >> 16u & 0xFFu,
        in.joints >> 24u & 0xFFu,
    );

    let m1 = get_joint_matrix(frame, joints.x) * in.weights.x;
    let m2 = get_joint_matrix(frame, joints.y) * in.weights.y;
    let m3 = get_joint_matrix(frame, joints.z) * in.weights.z;
    let m4 = get_joint_matrix(frame, joints.w) * in.weights.w;

    return mat4x4<f32>(
        m1[0] + m2[0] + m3[0] + m4[0],
        m1[1] + m2[1] + m3[1] + m4[1],
        m1[2] + m2[2] + m3[2] + m4[2],
        m1[3] + m2[3] + m3[3] + m4[3],
    );
}

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

[[stage(vertex)]]
fn vs_main(
    instance: InstanceInput,
    in: VertexInput,
) -> VertexOutput {
    let skinning_matrix = get_skinning_matrix(instance.animation_frame, in);

    let model_matrix =  mat4x4<f32>(
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

    let world_pos = model_matrix * skinning_matrix * vec4<f32>(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;

    let view_normal_matrix =
        mat4_to_mat3(camera.view) *
        normal_matrix *
        mat4_to_mat3(skinning_matrix);

    out.normal = normalize(view_normal_matrix * in.normal);
    out.tangent = normalize(view_normal_matrix * in.tangent.xyz);
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;

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

[[group(1), binding(0)]] var t_albedo: texture_2d<f32>;
[[group(1), binding(1)]] var t_normal: texture_2d<f32>;
[[group(1), binding(2)]] var t_metallic_roughness: texture_2d<f32>;
[[group(1), binding(3)]] var t_sampler: sampler;

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
    let n = textureSample(t_normal, t_sampler, in.uv).rgb * 2.0 - 1.0;
    return normalize(tbn * n);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let albedo = textureSample(t_albedo, t_sampler, in.uv);

    let metallic_roughness = textureSample(t_metallic_roughness, t_sampler, in.uv).bg;
    let metallic = metallic_roughness.x;
    let roughness = metallic_roughness.y;

    if (albedo.a < 0.5) { discard; }

    return FragmentOutput (
        vec4<f32>(albedo.rgb, metallic),
        vec4<f32>(get_normal(in), roughness),
    );
}
