struct DirectionalLight {
    color: vec4<f32>,
    direction_world: vec4<f32>,
    direction_view: vec4<f32>,
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> light: DirectionalLight;

@group(1) @binding(0) var<storage, read> skinning_joints: array<u32>;
@group(1) @binding(1) var<storage, read> skinning_weights: array<vec4<f32>>;

// TODO: should it be a texture_storage_2d_array?
@group(2) @binding(0) var animations: binding_array<texture_2d_array<f32>>;
@group(2) @binding(1) var animations_sampler: sampler;

struct MeshInstance {
    @location(0) model_matrix_0: vec4<f32>,
    @location(1) model_matrix_1: vec4<f32>,
    @location(2) model_matrix_2: vec4<f32>,
    @location(3) model_matrix_3: vec4<f32>,

    @location(4) material: u32,

    @location(5) skin_offset: i32,
    @location(6) animation_id: u32,
    @location(7) animation_time: f32,
}

struct VertexInput {
    @location(10) position: vec3<f32>,
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
) -> @builtin(position) vec4<f32> {
    var model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let skin_index = u32(i32(vertex_index) + instance.skin_offset);
    if skin_index > 0u {
        let skinning_matrix = get_skinning_matrix(
            instance.animation_id,
            instance.animation_time,
            skin_index
        );

        model_matrix *= skinning_matrix;
    }

    return light.view_proj * model_matrix * vec4<f32>(in.position, 1.0);
}
