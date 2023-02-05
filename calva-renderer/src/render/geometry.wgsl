struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var textures: binding_array<texture_2d<f32>>;
@group(1) @binding(1) var textures_sampler: sampler;

struct Material {
    albedo: u32,
    normal: u32,
    metallic_roughness: u32,
}
@group(2) @binding(0) var<storage, read> materials: array<Material>;

@group(3) @binding(0) var<storage, read> skinning_joints: array<u32>;
@group(3) @binding(1) var<storage, read> skinning_weights: array<vec4<f32>>;

// TODO: should it be a texture_storage_2d_array?
@group(4) @binding(0) var animations: binding_array<texture_2d_array<f32>>;
@group(4) @binding(1) var animations_sampler: sampler;


struct MeshInstance {
    @location(0) model_matrix_0: vec4<f32>,
    @location(1) model_matrix_1: vec4<f32>,
    @location(2) model_matrix_2: vec4<f32>,
    @location(3) model_matrix_3: vec4<f32>,
    @location(4) normal_quat: vec4<f32>,
    @location(5) material: u32,

    @location(6) skin_offset: i32,
    @location(7) animation_id: u32,
    @location(8) animation_time: f32,
}

struct VertexInput {
    @location(10) position: vec3<f32>,
    @location(11) normal: vec3<f32>,
    @location(12) tangent: vec4<f32>,
    @location(13) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) @interpolate(flat) material: u32,
}

fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

fn get_joint_matrix(animation_id: u32, time: f32, joint_index: u32) -> mat4x4<f32> {
    let texture = animations[animation_id];
    let dim = textureDimensions(texture);

    let pixel_size = 1.0 / vec2<f32>(f32(dim.x), f32(dim.y));

    let ANIMATIONS_SAMPLES_PER_SEC = 15.0;
    let frame = time * ANIMATIONS_SAMPLES_PER_SEC;
    let uv = (vec2<f32>(f32(joint_index), frame) + 0.5) * pixel_size;

    return mat4x4<f32>(
        textureSampleLevel(texture, animations_sampler, uv, 0, 0.0),
        textureSampleLevel(texture, animations_sampler, uv, 1, 0.0),
        textureSampleLevel(texture, animations_sampler, uv, 2, 0.0),
        textureSampleLevel(texture, animations_sampler, uv, 3, 0.0),
    );
}

fn get_skinning_matrix(animation_id: u32, time: f32, skin_index: u32) -> mat4x4<f32> {
    if animation_id == 0u {
        return mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0),
        );
    }

    let packed_joints = skinning_joints[skin_index];
    let weights = skinning_weights[skin_index];

    let joints = vec4<u32>(
        (packed_joints >> 0u) & 0xFFu,
        (packed_joints >> 8u) & 0xFFu,
        (packed_joints >> 16u) & 0xFFu,
        (packed_joints >> 24u) & 0xFFu,
    );

    let m1 = get_joint_matrix(animation_id, time, joints.x) * weights.x;
    let m2 = get_joint_matrix(animation_id, time, joints.y) * weights.y;
    let m3 = get_joint_matrix(animation_id, time, joints.z) * weights.z;
    let m4 = get_joint_matrix(animation_id, time, joints.w) * weights.w;

    return mat4x4<f32>(
        m1[0] + m2[0] + m3[0] + m4[0],
        m1[1] + m2[1] + m3[1] + m4[1],
        m1[2] + m2[2] + m3[2] + m4[2],
        m1[3] + m2[3] + m3[3] + m4[3],
    );
}

@vertex
fn vs_main(
    instance: MeshInstance,
    in: VertexInput,
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    var normal_matrix = mat4_to_mat3(camera.view);

    let skin_index = u32(i32(vertex_index) + instance.skin_offset);
    if skin_index > 0u {
        let skinning_matrix = get_skinning_matrix(
            instance.animation_id,
            instance.animation_time,
            skin_index
        );

        model_matrix *= skinning_matrix;
        normal_matrix *= mat4_to_mat3(skinning_matrix);
    }

    let world_pos = model_matrix * vec4<f32>(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;

    out.normal = normal_matrix * rotate(instance.normal_quat, in.normal);
    out.tangent = normal_matrix * rotate(instance.normal_quat, in.tangent.xyz);
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;

    out.uv = in.uv;
    out.material = instance.material;

    return out;
}


//===========================================================================//
// Fragment
//===========================================================================//


struct FragmentOutput {
    @location(0) albedo_metallic: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
}

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

fn normal_map(in: VertexOutput, material: Material) -> vec3<f32> {
    let texture = textures[material.normal];
    return textureSample(texture, textures_sampler, in.uv).rgb;
}

fn get_normal(in: VertexOutput, material: Material) -> vec3<f32> {
    if material.normal == 0u { // no normal mapping
        return normalize(get_vert_normal(in));
    }

    let tbn = get_tbn(in);
    let n = normal_map(in, material) * 2.0 - 1.0;
    return normalize(tbn * n);
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let material = materials[in.material];

    let albedo = textureSample(textures[material.albedo], textures_sampler, in.uv);
    let metallic_roughness = textureSample(textures[material.metallic_roughness], textures_sampler, in.uv).bg;

    if albedo.a < 0.5 { discard; }

    return FragmentOutput(
        vec4<f32>(albedo.rgb, metallic_roughness.x),
        vec4<f32>(get_normal(in, material), metallic_roughness.y),
    );
}
