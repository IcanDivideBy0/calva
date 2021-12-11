[[block]]
struct ShadowLight {
    light_dir: vec4<f32>; // camera view space
    view_proj: mat4x4<f32>;
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

[[stage(vertex)]]
fn main(
    instance: InstanceInput,
    [[location(7)]] position: vec3<f32>,
) -> [[builtin(position)]] vec4<f32> {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    return shadow_light.view_proj * model_matrix * vec4<f32>(position, 1.0);
}
