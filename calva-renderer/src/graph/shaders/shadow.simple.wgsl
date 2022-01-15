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

struct VertexInput {
    [[location(5)]] position: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
    [[builtin(view_index)]] view_index: i32,
    instance: MeshInstance,
    in: VertexInput,
) -> [[builtin(position)]] vec4<f32> {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let light_view_proj = shadow_light.view_proj[view_index];
    return light_view_proj * model_matrix * vec4<f32>(in.position, 1.0);
}
