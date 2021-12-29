let CASCADES: u32 = 4u;
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
    [[location(7)]] position: vec3<f32>;
    [[location(8)]] joints: u32;
    [[location(9)]] weights: vec4<f32>;
};

struct JointMatrices {
    matrices: array<mat4x4<f32>, 100>;
};

[[group(1), binding(0)]] var<uniform> joint_matrices: JointMatrices;

fn get_joint_matrix(joint_index: u32) -> mat4x4<f32> {
    return joint_matrices.matrices[joint_index];
}

fn get_skinning_matrix(in: VertexInput) -> mat4x4<f32> {
    let joints_x: u32 = in.joints >>  0u & 0xFFu;
    let joints_y: u32 = in.joints >>  8u & 0xFFu;
    let joints_z: u32 = in.joints >> 16u & 0xFFu;
    let joints_w: u32 = in.joints >> 24u & 0xFFu;

    let m1 = get_joint_matrix(joints_x) * in.weights.x;
    let m2 = get_joint_matrix(joints_y) * in.weights.y;
    let m3 = get_joint_matrix(joints_z) * in.weights.z;
    let m4 = get_joint_matrix(joints_w) * in.weights.w;

    // TODO: fixme, weights are wrong ?
    if (true) { return get_joint_matrix(joints_x); }

    return mat4x4<f32>(
        m1.x + m2.x + m3.x + m4.x,
        m1.y + m2.y + m3.y + m4.y,
        m1.z + m2.z + m3.z + m4.z,
        m1.w + m2.w + m3.w + m4.w,
    );
}

[[stage(vertex)]]
fn vs_main(
    [[builtin(view_index)]] view_index: i32,
    instance: InstanceInput,
    in: VertexInput,
) -> [[builtin(position)]] vec4<f32> {
    let skinning_matrix = get_skinning_matrix(in);

    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    ) * skinning_matrix;

    let light_view_proj = shadow_light.view_proj[view_index];
    return light_view_proj * model_matrix * vec4<f32>(in.position, 1.0);
}
