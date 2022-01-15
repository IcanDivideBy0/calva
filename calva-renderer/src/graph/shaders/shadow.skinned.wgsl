let CASCADES: u32 = 3u;
struct ShadowLight {
    color: vec4<f32>;
    direction: vec4<f32>; // camera view space
    view_proj: array<mat4x4<f32>, CASCADES>;
    splits: array<f32, CASCADES>;
};

[[group(0), binding(0)]]
var<uniform> shadow_light: ShadowLight;

//
// Vertex shader
//

struct MeshInstance {
    [[location(0)]] model_matrix_0: vec4<f32>;
    [[location(1)]] model_matrix_1: vec4<f32>;
    [[location(2)]] model_matrix_2: vec4<f32>;
    [[location(3)]] model_matrix_3: vec4<f32>;
    [[location(4)]] normal_quat: vec4<f32>;
};

struct SkinAnimationInstance {
    [[location(5)]] frame: u32;
};

struct VertexInput {
    [[location(6)]] position: vec3<f32>;
    [[location(7)]] joints: u32;
    [[location(8)]] weights: vec4<f32>;
};

[[group(1), binding(0)]] var animation: texture_2d_array<f32>;

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

[[stage(vertex)]]
fn vs_main(
    [[builtin(view_index)]] view_index: i32,
    instance: MeshInstance,
    skin_animation_instance: SkinAnimationInstance,
    in: VertexInput,
) -> [[builtin(position)]] vec4<f32> {
    let skinning_matrix = get_skinning_matrix(skin_animation_instance.frame, in);

    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    ) * skinning_matrix;

    let light_view_proj = shadow_light.view_proj[view_index];
    return light_view_proj * model_matrix * vec4<f32>(in.position, 1.0);
}
